//! Saved IDs, operators, and outcomes select work; only freshly discovered mutations execute.
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, ensure};
use serde::Serialize;

use crate::{
    cancellation::Cancellation,
    identity::MutationIdentity,
    mutation::{Mutation, Operator},
    report::{Outcome, RunStatus},
    report_reader::{LoadedReport, SavedMutation},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RerunMode {
    All,
    Missed,
}

#[derive(Clone, Debug, Serialize)]
pub struct RerunFilter {
    pub selection: RerunMode,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub source_ids: Vec<usize>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub operators: Vec<Operator>,
}

impl RerunFilter {
    pub fn new(
        selection: RerunMode,
        mut source_ids: Vec<usize>,
        mut operators: Vec<Operator>,
    ) -> Self {
        source_ids.sort_unstable();
        source_ids.dedup();
        operators.sort_unstable();
        operators.dedup();
        Self {
            selection,
            source_ids,
            operators,
        }
    }

    pub fn description(&self) -> String {
        let mut text = match self.selection {
            RerunMode::All => "all saved mutations",
            RerunMode::Missed => "completed missed mutations",
        }
        .to_owned();
        if !self.source_ids.is_empty() {
            text.push_str(&format!(
                "; source IDs: {}",
                self.source_ids
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        if !self.operators.is_empty() {
            text.push_str(&format!(
                "; operators: {}",
                self.operators
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        text
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct RerunRequest {
    pub source_run: PathBuf,
    #[serde(flatten)]
    pub filter: RerunFilter,
}

#[derive(Clone, Debug, Serialize)]
pub struct RerunProvenance {
    pub source_run: PathBuf,
    pub source_checkpoint: u64,
    pub source_status: RunStatus,
    #[serde(flatten)]
    pub filter: RerunFilter,
    pub mutations: Vec<MutationMapping>,
}

#[derive(Clone, Debug, Serialize)]
pub struct MutationMapping {
    pub id: usize,
    pub source_id: usize,
}

pub struct RerunSelection {
    language: crate::cli::Lang,
    rust_scope: Option<crate::rust::ExecutionScope>,
    vitest_scope: Option<crate::vitest::ExecutionScope>,
    node_scope: Option<crate::node::ExecutionScope>,
    provenance: RerunProvenance,
    targets: Vec<SavedMutation>,
}

impl RerunSelection {
    pub fn load(request: &RerunRequest) -> Result<Self> {
        Self::read(&request.source_run, &request.filter).with_context(|| {
            format!(
                "loading saved mutations from {}",
                request.source_run.display()
            )
        })
    }

    fn read(directory: &Path, filter: &RerunFilter) -> Result<Self> {
        let report = LoadedReport::load(directory)?;
        report.run.validate_rerun_scope()?;
        ensure!(
            report.complete,
            "saved run has an incomplete mutation inventory"
        );
        let missed: HashSet<_> = report
            .executions
            .iter()
            .filter(|execution| execution.outcome == Some(Outcome::Missed))
            .map(|execution| execution.id)
            .collect();
        // Validate against the whole inventory before intersecting filters.
        for id in &filter.source_ids {
            ensure!(
                *id > 0 && *id <= report.mutations.len(),
                "unknown source mutation ID {id}"
            );
        }
        Ok(Self {
            language: report.run.language(),
            rust_scope: report.run.rust_scope,
            vitest_scope: report.run.vitest_scope,
            node_scope: report.run.node_scope,
            provenance: RerunProvenance {
                source_run: report.directory,
                source_checkpoint: report.run.checkpoint,
                source_status: report.run.status,
                filter: filter.clone(),
                mutations: Vec::new(),
            },
            targets: report
                .mutations
                .into_iter()
                .filter(|m| filter.selection == RerunMode::All || missed.contains(&m.id))
                .filter(|m| filter.source_ids.is_empty() || filter.source_ids.contains(&m.id))
                .filter(|m| filter.operators.is_empty() || filter.operators.contains(&m.operator))
                .collect(),
        })
    }

    pub fn validate_language(&self, language: crate::cli::Lang) -> Result<()> {
        ensure!(
            self.language == language,
            "saved run language {:?} does not match current project language {:?}; start a fresh scan",
            self.language,
            language
        );
        Ok(())
    }

    pub fn validate_node_scope(&self, current: &crate::node::ExecutionScope) -> Result<()> {
        self.node_scope
            .as_ref()
            .context("saved run lacks Node execution scope")?
            .validate_rerun(current)
    }

    pub fn validate_vitest_scope(&self, current: &crate::vitest::ExecutionScope) -> Result<()> {
        self.vitest_scope
            .as_ref()
            .context("saved run lacks Vitest execution scope")?
            .validate_rerun(current)
    }

    pub fn validate_rust_scope(&self, current: &crate::rust::ExecutionScope) -> Result<()> {
        self.rust_scope
            .as_ref()
            .context("saved run lacks Rust execution scope")?
            .validate_rerun(current)
    }

    pub fn select(
        mut self,
        mutations: Vec<Mutation>,
        cancellation: &Cancellation,
    ) -> Result<(Vec<Mutation>, RerunProvenance)> {
        let mut by_file: HashMap<&Path, Vec<(usize, &Mutation)>> = HashMap::new();
        for (index, mutation) in mutations.iter().enumerate() {
            cancellation.check()?;
            by_file
                .entry(&mutation.file.path)
                .or_default()
                .push((index, mutation));
        }
        let mut selected = BTreeMap::new();
        for target in &self.targets {
            cancellation.check()?;
            let candidates = by_file.get(target.file.as_path()).with_context(|| {
                format!(
                    "saved mutation {} in {} is missing from current discovery (file removed, excluded, or inactive); no mutations were run",
                    target.id, target.file.display()
                )
            })?;
            ensure!(
                candidates
                    .iter()
                    .all(|(_, m)| m.file.fingerprint == target.source_fingerprint),
                "saved mutation {} in {} is stale: source fingerprint changed; run a new full scan; no mutations were run",
                target.id,
                target.file.display()
            );
            let identity = MutationIdentity::from(target);
            let mut matches = candidates.iter().filter(|(_, m)| identity.matches(m));
            let (index, _) = matches.next().with_context(|| {
                format!(
                    "saved mutation {} in {} no longer matches a discovered mutation; no mutations were run",
                    target.id, target.file.display()
                )
            })?;
            ensure!(
                matches.next().is_none(),
                "ambiguous match for saved mutation {}; no mutations were run",
                target.id
            );
            ensure!(
                selected.insert(*index, target.id).is_none(),
                "ambiguous saved selection: multiple source IDs match the same mutation in {}; no mutations were run",
                target.file.display()
            );
        }
        // Keep current discovery order, independent of the source report's order
        // or worker completion order. Assign new run-local IDs after selection.
        self.provenance.mutations = selected
            .values()
            .enumerate()
            .map(|(index, source_id)| MutationMapping {
                id: index + 1,
                source_id: *source_id,
            })
            .collect();
        let mutations = mutations
            .into_iter()
            .enumerate()
            .filter_map(|(index, mutation)| selected.contains_key(&index).then_some(mutation))
            .collect();
        Ok((mutations, self.provenance))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        mutation::{self, OPERATOR_CATALOG_VERSION, Scope},
        saved_report::SCHEMA_VERSION,
    };
    use serde::de::DeserializeOwned;
    use std::fs;
    fn read_json<T: DeserializeOwned>(directory: &Path, name: &str) -> Result<T> {
        let bytes = fs::read(directory.join(name)).with_context(|| format!("reading {name}"))?;
        serde_json::from_slice(&bytes).with_context(|| format!("parsing {name}"))
    }

    use serde_json::{Value, json};

    struct Fixture {
        directory: tempfile::TempDir,
        mutations: Vec<Mutation>,
    }

    impl Fixture {
        fn new() -> Result<Self> {
            let directory = tempfile::tempdir()?;
            fs::write(
                directory.path().join("a.go"),
                "package p\nvar a = true\nvar b = false\n",
            )?;
            let mutations = mutation::discover(
                directory.path(),
                vec!["a.go".into()],
                &Scope::new(&[], &[])?,
            )?;
            let fixture = Self {
                directory,
                mutations,
            };
            fixture.write("run.json", json!({
                "schema_version": SCHEMA_VERSION, "operator_catalog_version": OPERATOR_CATALOG_VERSION,
                "checkpoint": 10, "status": "completed", "finished_at_unix_seconds": 100,
                "planned_mutations": 2,
            }))?;
            fixture.write("mutants.json", json!({
                "schema_version": SCHEMA_VERSION, "operator_catalog_version": OPERATOR_CATALOG_VERSION,
                "complete": true,
                "mutations": fixture.mutations.iter().enumerate().map(|(i, m)| json!({
                    "id": i + 1, "file": m.file.path, "operator": m.operator,
                    "source_fingerprint": m.file.fingerprint, "byte_start": m.range.start,
                    "byte_end": m.range.end, "line": m.line, "column": m.column,
                    "end_line": m.end_line, "end_column": m.end_column,
                    "original": m.original(), "replacement": m.replacement,
                })).collect::<Vec<_>>()
            }))?;
            fixture.write(
                "outcomes.json",
                json!({
                    "schema_version": SCHEMA_VERSION, "checkpoint": 10, "status": "completed",
                    "mutations": [
                        {"id": 1, "state": "completed", "outcome": "killed"},
                        {"id": 2, "state": "completed", "outcome": "missed"}
                    ]
                }),
            )?;
            Ok(fixture)
        }

        fn write(&self, name: &str, value: Value) -> Result<()> {
            Ok(fs::write(
                self.directory.path().join(name),
                serde_json::to_vec(&value)?,
            )?)
        }

        fn edit(&self, name: &str, pointer: &str, value: Value) -> Result<()> {
            let mut document: Value = read_json(self.directory.path(), name)?;
            *document
                .pointer_mut(pointer)
                .context("missing fixture JSON field")? = value;
            self.write(name, document)
        }

        fn select(&self, current: Vec<Mutation>) -> Result<(Vec<Mutation>, RerunProvenance)> {
            RerunSelection::load(&self.request(RerunMode::Missed, vec![], vec![]))?
                .select(current, &Cancellation::default())
        }

        fn request(
            &self,
            mode: RerunMode,
            ids: Vec<usize>,
            operators: Vec<Operator>,
        ) -> RerunRequest {
            RerunRequest {
                source_run: self.directory.path().to_owned(),
                filter: RerunFilter::new(mode, ids, operators),
            }
        }
    }

    #[test]
    fn saved_language_must_match_the_current_project() -> Result<()> {
        let fixture = Fixture::new()?;
        let selection = RerunSelection::load(&fixture.request(RerunMode::All, vec![], vec![]))?;
        // Legacy Go reports without an explicit language still work for Go.
        selection.validate_language(crate::cli::Lang::Go)?;
        assert!(
            selection
                .validate_language(crate::cli::Lang::Rust)
                .unwrap_err()
                .to_string()
                .contains("does not match")
        );
        Ok(())
    }

    #[test]
    fn selects_only_missed_with_new_ids_and_accepts_finalized_partial_runs() -> Result<()> {
        let fixture = Fixture::new()?;
        for status in ["completed", "failed", "interrupted"] {
            fixture.edit("run.json", "/status", json!(status))?;
            fixture.edit("outcomes.json", "/status", json!(status))?;
            if status != "completed" {
                fixture.edit("outcomes.json", "/mutations/0/state", json!("interrupted"))?;
                fixture.edit("outcomes.json", "/mutations/0/outcome", Value::Null)?;
            }
            let (selected, provenance) = fixture.select(fixture.mutations.clone())?;
            assert_eq!(selected.len(), 1);
            assert_eq!(selected[0].range, fixture.mutations[1].range);
            assert_eq!(provenance.source_checkpoint, 10);
            assert_eq!(provenance.mutations[0].id, 1);
            assert_eq!(provenance.mutations[0].source_id, 2);
        }
        Ok(())
    }

    #[test]
    fn rejects_incomplete_inconsistent_and_unsupported_reports() -> Result<()> {
        for (file, pointer, value, expected) in [
            (
                "run.json",
                "/schema_version",
                json!(99),
                "unsupported schema",
            ),
            (
                "mutants.json",
                "/schema_version",
                json!(99),
                "unsupported schema",
            ),
            (
                "outcomes.json",
                "/schema_version",
                json!(99),
                "unsupported schema",
            ),
            (
                "mutants.json",
                "/operator_catalog_version",
                json!(99),
                "incompatible operator",
            ),
            ("run.json", "/status", json!("running"), "not finalized"),
            (
                "run.json",
                "/finished_at_unix_seconds",
                Value::Null,
                "not finalized",
            ),
            (
                "outcomes.json",
                "/checkpoint",
                json!(9),
                "inconsistent checkpoints",
            ),
            (
                "outcomes.json",
                "/status",
                json!("failed"),
                "inconsistent checkpoints",
            ),
            (
                "mutants.json",
                "/complete",
                json!(false),
                "incomplete mutation inventory",
            ),
            (
                "run.json",
                "/planned_mutations",
                json!(3),
                "inconsistent mutation counts",
            ),
            (
                "mutants.json",
                "/mutations/1/id",
                json!(1),
                "duplicate inventory",
            ),
            (
                "outcomes.json",
                "/mutations/1/id",
                json!(1),
                "duplicate outcome",
            ),
            (
                "outcomes.json",
                "/mutations/1/id",
                json!(3),
                "unknown outcome",
            ),
            (
                "outcomes.json",
                "/mutations/1/state",
                json!("pending"),
                "inconsistent execution",
            ),
            (
                "outcomes.json",
                "/mutations/1/outcome",
                Value::Null,
                "inconsistent execution",
            ),
            (
                "mutants.json",
                "/mutations/1/file",
                json!("../a.go"),
                "invalid module-relative",
            ),
            (
                "mutants.json",
                "/mutations/1/file",
                json!("/a.go"),
                "invalid module-relative",
            ),
            (
                "mutants.json",
                "/mutations/1/byte_end",
                json!(0),
                "invalid source coordinates",
            ),
        ] {
            let fixture = Fixture::new()?;
            fixture.edit(file, pointer, value)?;
            let error = RerunSelection::load(&fixture.request(RerunMode::Missed, vec![], vec![]))
                .err()
                .context("invalid report accepted")?;
            assert!(
                format!("{error:#}").contains(expected),
                "{file} {pointer}: {error:#}"
            );
        }
        Ok(())
    }

    #[test]
    fn explicit_ids_can_select_every_outcome_and_unfinished_work_from_finalized_runs() -> Result<()>
    {
        let fixture = Fixture::new()?;
        for (state, outcome) in [
            ("completed", json!("killed")),
            ("completed", json!("missed")),
            ("completed", json!("timeout")),
            ("completed", json!("build_failed")),
            ("completed", json!("error")),
            ("pending", Value::Null),
            ("interrupted", Value::Null),
        ] {
            fixture.edit("run.json", "/status", json!("interrupted"))?;
            fixture.edit("outcomes.json", "/status", json!("interrupted"))?;
            fixture.edit("outcomes.json", "/mutations/0/state", json!(state))?;
            fixture.edit("outcomes.json", "/mutations/0/outcome", outcome.clone())?;
            for mode in [RerunMode::All, RerunMode::Missed] {
                let request = fixture.request(mode, vec![1, 1], vec![Operator::BooleanFlip]);
                let (selected, provenance) = RerunSelection::load(&request)?
                    .select(fixture.mutations.clone(), &Cancellation::default())?;
                let expected = usize::from(mode == RerunMode::All || outcome == json!("missed"));
                assert_eq!(selected.len(), expected, "{state} {outcome} {mode:?}");
                assert_eq!(provenance.filter.source_ids, [1]);
                if expected == 1 {
                    assert_eq!(selected[0].range, fixture.mutations[0].range);
                    assert_eq!(provenance.mutations[0].source_id, 1);
                }
            }
        }
        Ok(())
    }

    #[test]
    fn unknown_ids_are_rejected_even_if_other_filters_would_select_nothing() -> Result<()> {
        let fixture = Fixture::new()?;
        for id in [0, 3, usize::MAX] {
            let request = fixture.request(RerunMode::Missed, vec![id], vec![Operator::LogicalSwap]);
            let error = RerunSelection::load(&request)
                .err()
                .context("unknown ID accepted")?;
            assert!(format!("{error:#}").contains(&format!("unknown source mutation ID {id}")));
        }
        Ok(())
    }

    #[test]
    fn rejects_stale_missing_altered_and_ambiguous_targets() -> Result<()> {
        let fixture = Fixture::new()?;
        fs::write(
            fixture.directory.path().join("a.go"),
            "// moved\npackage p\nvar a = true\nvar b = false\n",
        )?;
        let current = mutation::discover(
            fixture.directory.path(),
            vec!["a.go".into()],
            &Scope::new(&[], &[])?,
        )?;
        assert!(
            fixture
                .select(current)
                .unwrap_err()
                .to_string()
                .contains("stale")
        );
        assert!(
            fixture
                .select(vec![])
                .unwrap_err()
                .to_string()
                .contains("missing")
        );
        let mut ambiguous = fixture.mutations.clone();
        ambiguous.push(fixture.mutations[1].clone());
        assert!(
            fixture
                .select(ambiguous)
                .unwrap_err()
                .to_string()
                .contains("ambiguous")
        );
        for (field, value) in [
            ("replacement", json!("arbitrary code")),
            ("operator", json!("logical_swap")),
            ("column", json!(999)),
        ] {
            let fixture = Fixture::new()?;
            fixture.edit("mutants.json", &format!("/mutations/1/{field}"), value)?;
            assert!(
                fixture
                    .select(fixture.mutations.clone())
                    .unwrap_err()
                    .to_string()
                    .contains("no longer matches")
            );
        }
        // Different saved IDs cannot silently collapse into the same new ID.
        let fixture = Fixture::new()?;
        let mut inventory: Value = read_json(fixture.directory.path(), "mutants.json")?;
        inventory["mutations"][0] = inventory["mutations"][1].clone();
        inventory["mutations"][0]["id"] = json!(1);
        fixture.write("mutants.json", inventory)?;
        fixture.edit("outcomes.json", "/mutations/0/outcome", json!("missed"))?;
        assert!(
            fixture
                .select(fixture.mutations.clone())
                .unwrap_err()
                .to_string()
                .contains("ambiguous saved selection")
        );
        Ok(())
    }
}
