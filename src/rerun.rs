//! Saved outcomes select work; only freshly discovered mutations can be executed.
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fs,
    path::{Component, Path, PathBuf},
};

use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

use crate::{
    cancellation::Cancellation,
    mutation::{Mutation, OPERATOR_CATALOG_VERSION, Operator},
    report::{ExecutionState, Outcome, RunStatus},
    saved_report::SCHEMA_VERSION,
};

// Read only the fields needed for selection. Older reports need not contain
// effective configuration or presentation settings, and artifacts are never read.
#[derive(Deserialize)]
struct SavedRun {
    schema_version: u32,
    operator_catalog_version: u32,
    checkpoint: u64,
    status: RunStatus,
    finished_at_unix_seconds: Option<u64>,
    planned_mutations: Option<usize>,
}

#[derive(Deserialize)]
struct SavedInventory {
    schema_version: u32,
    operator_catalog_version: u32,
    complete: bool,
    mutations: Vec<SavedMutation>,
}

#[derive(Deserialize)]
struct SavedMutation {
    id: usize,
    file: PathBuf,
    operator: Operator,
    source_fingerprint: String,
    byte_start: usize,
    byte_end: usize,
    line: usize,
    column: usize,
    end_line: usize,
    end_column: usize,
    original: String,
    replacement: String,
}

impl SavedMutation {
    fn matches(&self, current: &Mutation) -> bool {
        self.operator == current.operator
            && self.byte_start == current.range.start
            && self.byte_end == current.range.end
            && self.original == current.original()
            && self.replacement == current.replacement
            && self.line == current.line
            && self.column == current.column
            && self.end_line == current.end_line
            && self.end_column == current.end_column
    }
}

#[derive(Deserialize)]
struct SavedOutcomes {
    schema_version: u32,
    checkpoint: u64,
    status: RunStatus,
    mutations: Vec<SavedExecution>,
}

#[derive(Deserialize)]
struct SavedExecution {
    id: usize,
    state: ExecutionState,
    outcome: Option<Outcome>,
}

#[derive(Clone, Debug, Serialize)]
pub struct RerunProvenance {
    pub source_run: PathBuf,
    pub source_checkpoint: u64,
    pub source_status: RunStatus,
    pub selection: &'static str,
    pub mutations: Vec<MutationMapping>,
}

#[derive(Clone, Debug, Serialize)]
pub struct MutationMapping {
    pub id: usize,
    pub source_id: usize,
}

pub struct RerunSelection {
    provenance: RerunProvenance,
    targets: Vec<SavedMutation>,
}

impl RerunSelection {
    pub fn load(directory: &Path) -> Result<Self> {
        Self::read(directory)
            .with_context(|| format!("loading missed mutations from {}", directory.display()))
    }

    fn read(directory: &Path) -> Result<Self> {
        let run: SavedRun = read_json(directory, "run.json")?;
        let inventory: SavedInventory = read_json(directory, "mutants.json")?;
        let outcomes: SavedOutcomes = read_json(directory, "outcomes.json")?;
        for (name, version) in [
            ("run.json", run.schema_version),
            ("mutants.json", inventory.schema_version),
            ("outcomes.json", outcomes.schema_version),
        ] {
            ensure!(
                version == SCHEMA_VERSION,
                "unsupported schema version {version} in {name}"
            );
        }
        ensure!(
            run.operator_catalog_version == OPERATOR_CATALOG_VERSION
                && inventory.operator_catalog_version == OPERATOR_CATALOG_VERSION,
            "saved run uses an incompatible operator catalog"
        );
        ensure!(
            run.status != RunStatus::Running && run.finished_at_unix_seconds.is_some(),
            "saved run is not finalized; wait for it to finish or use a finalized run"
        );
        ensure!(
            run.checkpoint == outcomes.checkpoint && run.status == outcomes.status,
            "saved run has inconsistent checkpoints or statuses"
        );
        ensure!(
            inventory.complete,
            "saved run has an incomplete mutation inventory"
        );
        let count = inventory.mutations.len();
        ensure!(
            run.planned_mutations == Some(count) && outcomes.mutations.len() == count,
            "saved run has inconsistent mutation counts"
        );

        let mut inventory_by_id = BTreeMap::new();
        for mutation in inventory.mutations {
            let id = mutation.id;
            ensure!(id > 0 && id <= count, "invalid inventory mutation ID {id}");
            ensure!(
                !mutation.file.as_os_str().is_empty()
                    && mutation
                        .file
                        .components()
                        .all(|part| matches!(part, Component::Normal(_))),
                "mutation {id} has an invalid module-relative source path"
            );
            ensure!(
                mutation.byte_start < mutation.byte_end
                    && mutation.byte_end - mutation.byte_start == mutation.original.len()
                    && mutation.line > 0
                    && mutation.column > 0
                    && mutation.end_line >= mutation.line
                    && mutation.end_column > 0,
                "mutation {id} has invalid source coordinates"
            );
            ensure!(
                inventory_by_id.insert(id, mutation).is_none(),
                "duplicate inventory mutation ID {id}"
            );
        }
        let mut missed = HashSet::new();
        let mut seen = HashSet::new();
        for execution in outcomes.mutations {
            let id = execution.id;
            ensure!(
                inventory_by_id.contains_key(&id),
                "unknown outcome mutation ID {id}"
            );
            ensure!(seen.insert(id), "duplicate outcome mutation ID {id}");
            ensure!(
                matches!(
                    (execution.state, execution.outcome),
                    (ExecutionState::Completed, Some(_))
                        | (ExecutionState::Pending | ExecutionState::Interrupted, None)
                ),
                "mutation {id} has an inconsistent execution state/outcome"
            );
            ensure!(
                run.status != RunStatus::Completed || execution.state == ExecutionState::Completed,
                "completed saved run contains unfinished mutation {id}"
            );
            if execution.outcome == Some(Outcome::Missed) {
                missed.insert(id);
            }
        }
        Ok(Self {
            provenance: RerunProvenance {
                source_run: directory.to_owned(),
                source_checkpoint: run.checkpoint,
                source_status: run.status,
                selection: "missed",
                mutations: Vec::new(),
            },
            targets: inventory_by_id
                .into_values()
                .filter(|m| missed.contains(&m.id))
                .collect(),
        })
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
                    "missed mutation {} in {} is missing from current discovery (file removed, excluded, or inactive); no mutations were run",
                    target.id, target.file.display()
                )
            })?;
            ensure!(
                candidates
                    .iter()
                    .all(|(_, m)| m.file.fingerprint == target.source_fingerprint),
                "missed mutation {} in {} is stale: source fingerprint changed; run a new full scan; no mutations were run",
                target.id,
                target.file.display()
            );
            let mut matches = candidates.iter().filter(|(_, m)| target.matches(m));
            let (index, _) = matches.next().with_context(|| {
                format!(
                    "missed mutation {} in {} no longer matches a discovered mutation; no mutations were run",
                    target.id, target.file.display()
                )
            })?;
            ensure!(
                matches.next().is_none(),
                "ambiguous match for missed mutation {}; no mutations were run",
                target.id
            );
            ensure!(
                selected.insert(*index, target.id).is_none(),
                "ambiguous saved selection: multiple missed IDs match the same mutation in {}; no mutations were run",
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

fn read_json<T: DeserializeOwned>(directory: &Path, name: &str) -> Result<T> {
    let bytes = fs::read(directory.join(name)).with_context(|| format!("reading {name}"))?;
    serde_json::from_slice(&bytes).with_context(|| format!("parsing {name}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mutation::{self, Scope};
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
            RerunSelection::load(self.directory.path())?.select(current, &Cancellation::default())
        }
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
            let error = RerunSelection::load(fixture.directory.path())
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
