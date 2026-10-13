//! Read finalized reports independently of language tools, the current checkout, and presentation.
use std::{
    collections::BTreeMap,
    fs::File,
    io::Read,
    path::{Component, Path, PathBuf},
};

use anyhow::{Context, Result, ensure};
use serde::{Deserialize, de::DeserializeOwned};

use crate::{
    dismissals::{Dismissal, DismissalSnapshot},
    mutation::{OPERATOR_CATALOG_VERSION, Operator},
    report::{ExecutionState, Outcome, RunStatus},
    saved_report::{LogRetention, SCHEMA_VERSION},
    score::{MinimumScore, MutationScore, ScorePolicy, ScoreScope},
};

#[derive(Debug, Deserialize)]
pub struct SavedRun {
    schema_version: u32,
    operator_catalog_version: u32,
    pub checkpoint: u64,
    pub status: RunStatus,
    finished_at_unix_seconds: Option<u64>,
    planned_mutations: Option<usize>,
    pub module_root: Option<PathBuf>,
    pub phase: Option<String>,
    pub error: Option<String>,
    pub rust_scope: Option<crate::rust::ExecutionScope>,
    pub score: Option<MutationScore>,
    pub score_policy: Option<ScorePolicy>,
    effective_config: Option<ScoreConfig>,
}

impl SavedRun {
    pub fn validate_rerun_scope(&self) -> Result<()> {
        match self.language() {
            crate::cli::Lang::Rust => self
                .rust_scope
                .as_ref()
                .context(
                    "saved Rust run lacks execution scope; start a fresh scan before rerunning",
                )?
                .validate_rerun_record(),
            crate::cli::Lang::Go => {
                ensure!(
                    self.rust_scope.is_none(),
                    "saved run has conflicting language and Rust scope"
                );
                Ok(())
            }
            crate::cli::Lang::Auto => anyhow::bail!("saved run has no resolved language"),
        }
    }

    pub fn language(&self) -> crate::cli::Lang {
        self.effective_config
            .as_ref()
            .and_then(|config| config.lang)
            .unwrap_or(crate::cli::Lang::Go)
    }
}

/// Only the saved configuration needed to verify score scope and policy.
#[derive(Debug, Deserialize)]
struct ScoreConfig {
    lang: Option<crate::cli::Lang>,
    #[serde(default)]
    include: Vec<String>,
    #[serde(default)]
    exclude: Vec<String>,
    #[serde(default)]
    operators: Vec<Operator>,
    rerun: Option<serde_json::Value>,
    min_score: Option<MinimumScore>,
}

#[derive(Deserialize)]
struct SavedInventory {
    schema_version: u32,
    operator_catalog_version: u32,
    complete: bool,
    mutations: Vec<SavedMutation>,
    dismissals: Option<DismissalSnapshot>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct SavedMutation {
    pub id: usize,
    pub file: PathBuf,
    pub operator: Operator,
    pub source_fingerprint: String,
    pub byte_start: usize,
    pub byte_end: usize,
    pub line: usize,
    pub column: usize,
    pub end_line: usize,
    pub end_column: usize,
    pub original: String,
    pub replacement: String,
    pub diff: Option<PathBuf>,
}

#[derive(Deserialize)]
struct SavedOutcomes {
    schema_version: u32,
    checkpoint: u64,
    status: RunStatus,
    mutations: Vec<SavedExecution>,
    baseline: Option<SavedCommand>,
    discovery: Option<SavedCommand>,
}

#[derive(Debug, Deserialize)]
pub struct SavedCommand {
    pub outcome: String,
    pub logs: SavedLogs,
}

#[derive(Clone, Debug, Deserialize)]
pub struct SavedLogs {
    pub text: Option<PathBuf>,
    pub stdout: Option<PathBuf>,
    pub stderr: Option<PathBuf>,
    pub retention: Option<LogRetention>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct SavedExecution {
    pub id: usize,
    pub state: ExecutionState,
    pub outcome: Option<Outcome>,
    pub duration_seconds: Option<f64>,
    pub error: Option<String>,
    pub logs: Option<SavedLogs>,
}

#[derive(Debug)]
pub struct LoadedReport {
    pub directory: PathBuf,
    pub run: SavedRun,
    pub complete: bool,
    // Both vectors are validated, sorted by ID, and have matching lengths.
    pub mutations: Vec<SavedMutation>,
    pub executions: Vec<SavedExecution>,
    pub baseline: Option<SavedCommand>,
    pub discovery: Option<SavedCommand>,
    pub dismissals: Option<DismissalSnapshot>,
}

impl LoadedReport {
    pub fn load(directory: &Path) -> Result<Self> {
        Self::read(directory)
            .with_context(|| format!("reading saved report {}", directory.display()))
    }

    fn read(directory: &Path) -> Result<Self> {
        let directory = directory
            .canonicalize()
            .context("opening report directory")?;
        ensure!(directory.is_dir(), "saved report is not a directory");
        let run: SavedRun = read_json(&directory, "run.json")?;
        let mut inventory: SavedInventory = read_json(&directory, "mutants.json")?;
        let outcomes: SavedOutcomes = read_json(&directory, "outcomes.json")?;
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
        let count = inventory.mutations.len();
        if inventory.complete {
            ensure!(
                run.planned_mutations == Some(count) && outcomes.mutations.len() == count,
                "saved run has inconsistent mutation counts"
            );
        } else {
            ensure!(
                count == 0
                    && outcomes.mutations.is_empty()
                    && run.planned_mutations.is_none()
                    && run.status != RunStatus::Completed,
                "saved run has an incomplete mutation inventory with inconsistent counts"
            );
        }
        let mut mutations = BTreeMap::new();
        for mutation in inventory.mutations {
            let id = mutation.id;
            ensure!(id > 0 && id <= count, "invalid inventory mutation ID {id}");
            ensure!(
                is_relative_file(&mutation.file),
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
                mutations.insert(id, mutation).is_none(),
                "duplicate inventory mutation ID {id}"
            );
        }
        let mut executions = BTreeMap::new();
        for command in [&outcomes.baseline, &outcomes.discovery]
            .into_iter()
            .flatten()
        {
            if let Some(retention) = &command.logs.retention {
                retention
                    .validate()
                    .context("validating command log retention")?;
            }
        }
        if let Some(dismissals) = &mut inventory.dismissals {
            ensure!(
                inventory.complete,
                "incomplete inventory cannot contain a dismissal review"
            );
            dismissals
                .validate(&mutations)
                .context("validating saved dismissal review")?;
        }
        for execution in outcomes.mutations {
            if let Some(retention) = execution
                .logs
                .as_ref()
                .and_then(|logs| logs.retention.as_ref())
            {
                retention
                    .validate()
                    .context("validating mutation log retention")?;
            }
            let id = execution.id;
            ensure!(
                mutations.contains_key(&id),
                "unknown outcome mutation ID {id}"
            );
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
            ensure!(
                executions.insert(id, execution).is_none(),
                "duplicate outcome mutation ID {id}"
            );
        }
        if let Some(score) = &run.score {
            let config = run
                .effective_config
                .as_ref()
                .context("saved score has no configuration")?;
            let expected = MutationScore::calculate(
                ScoreScope::selected(
                    config.rerun.is_some(),
                    !config.include.is_empty()
                        || !config.exclude.is_empty()
                        || !config.operators.is_empty(),
                ),
                run.status,
                inventory.complete,
                executions.values().map(|execution| {
                    (
                        execution.outcome,
                        inventory
                            .dismissals
                            .as_ref()
                            .is_some_and(|d| d.dismissal(execution.id).is_some()),
                    )
                }),
            );
            ensure!(
                expected.matches_saved(score),
                "saved mutation score disagrees with execution outcomes or scope"
            );
            ensure!(
                run.score_policy.as_ref()
                    == Some(&ScorePolicy::evaluate(config.min_score, &expected)),
                "saved minimum-score policy disagrees with score or configuration"
            );
        } else {
            ensure!(
                run.score_policy.is_none(),
                "saved minimum-score policy has no score"
            );
        }
        Ok(Self {
            directory,
            run,
            complete: inventory.complete,
            mutations: mutations.into_values().collect(),
            executions: executions.into_values().collect(),
            baseline: outcomes.baseline,
            discovery: outcomes.discovery,
            dismissals: inventory.dismissals,
        })
    }

    pub fn operator_summary(&self) -> String {
        crate::operator_summary::summarize(
            self.complete,
            self.mutations
                .iter()
                .zip(&self.executions)
                .map(|(mutation, execution)| {
                    (
                        mutation.operator,
                        execution.state,
                        execution.outcome,
                        execution.duration_seconds,
                    )
                }),
        )
    }

    pub fn dismissal(&self, id: usize) -> Option<&Dismissal> {
        self.dismissals.as_ref().and_then(|d| d.dismissal(id))
    }

    pub fn artifact(&self, path: &Path) -> Result<String> {
        const LIMIT: u64 = 256 * 1024;
        let bytes = read_relative(&self.directory, path, LIMIT + 1)?;
        let truncated = bytes.len() > LIMIT as usize;
        let mut text = clean_text(&String::from_utf8_lossy(
            &bytes[..bytes.len().min(LIMIT as usize)],
        ));
        if truncated {
            text.push_str("\n\n[Preview truncated at 256 KiB; the full saved artifact remains in the report.]");
        }
        Ok(text)
    }
}

fn is_relative_file(path: &Path) -> bool {
    !path.as_os_str().is_empty()
        && path
            .components()
            .all(|part| matches!(part, Component::Normal(_)))
}

fn read_relative(directory: &Path, path: &Path, limit: u64) -> Result<Vec<u8>> {
    ensure!(
        is_relative_file(path),
        "artifact path must stay inside the report"
    );
    let resolved = directory
        .join(path)
        .canonicalize()
        .with_context(|| format!("opening {}", path.display()))?;
    ensure!(
        resolved.starts_with(directory),
        "artifact path escapes the report directory"
    );
    ensure!(resolved.is_file(), "artifact is not a regular file");
    let mut bytes = Vec::new();
    File::open(&resolved)?.take(limit).read_to_end(&mut bytes)?;
    Ok(bytes)
}

pub(crate) fn read_json<T: DeserializeOwned>(directory: &Path, name: &str) -> Result<T> {
    const LIMIT: u64 = 64 * 1024 * 1024;
    let bytes = read_relative(directory, Path::new(name), LIMIT + 1)?;
    ensure!(
        bytes.len() <= LIMIT as usize,
        "{name} exceeds the 64 MiB report metadata limit"
    );
    serde_json::from_slice(&bytes).with_context(|| format!("parsing {name}"))
}

/// Never send controls embedded in paths, diffs, or captured output to the terminal.
pub fn clean_text(text: &str) -> String {
    let mut clean = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '\n' => clean.push('\n'),
            '\t' => clean.push_str("    "),
            '\r' => {}
            c if c.is_control() => clean.push('�'),
            c => clean.push(c),
        }
    }
    clean
}

#[cfg(test)]
pub(crate) fn fixture() -> tempfile::TempDir {
    use serde_json::json;
    use std::fs;
    let temp = tempfile::tempdir().unwrap();
    fs::create_dir(temp.path().join("diff")).unwrap();
    fs::create_dir(temp.path().join("logs")).unwrap();
    let run = json!({
        "schema_version": SCHEMA_VERSION, "operator_catalog_version": OPERATOR_CATALOG_VERSION,
        "checkpoint": 10, "status": "interrupted", "finished_at_unix_seconds": 100,
        "planned_mutations": 4, "module_root": "/missing/a project's checkout", "phase": "mutations"
    });
    let mut mutations = Vec::new();
    let mut executions = Vec::new();
    for (index, (file, operator, state, outcome)) in [
        ("a.go", "boolean_flip", "completed", Some("missed")),
        (
            "目录/b.go",
            "arithmetic_replace",
            "completed",
            Some("killed"),
        ),
        ("c.go", "boolean_flip", "interrupted", None),
        ("d.go", "boolean_flip", "pending", None),
    ]
    .into_iter()
    .enumerate()
    {
        let id = index + 1;
        let diff = format!("diff/{id:06}.diff");
        let log = format!("logs/{id:06}.log");
        mutations.push(json!({
            "id": id, "file": file, "operator": operator, "source_fingerprint": "sha256:fixture",
            "byte_start": 0, "byte_end": 4, "line": 1, "column": 1,
            "end_line": 1, "end_column": 5, "original": "true", "replacement": "false", "diff": diff
        }));
        executions.push(json!({
            "id": id, "state": state, "outcome": outcome, "duration_seconds": 0.25,
            "logs": {"text": log}
        }));
        fs::write(
            temp.path().join(diff),
            "--- a/file.go\n+++ b/file.go\n@@ -1 +1 @@\n-true\n+false\n",
        )
        .unwrap();
        fs::write(
            temp.path().join(log),
            format!("diagnostic {id}\n\x1b[2Jdata must stay text\n"),
        )
        .unwrap();
    }
    let inventory = json!({
        "schema_version": SCHEMA_VERSION, "operator_catalog_version": OPERATOR_CATALOG_VERSION,
        "complete": true, "mutations": mutations
    });
    let outcomes = json!({
        "schema_version": SCHEMA_VERSION, "checkpoint": 10, "status": "interrupted", "mutations": executions,
        "baseline": {"outcome": "passed", "logs": {"text": "logs/baseline.log"}},
        "discovery": {"outcome": "passed", "logs": {"text": "logs/discovery.log"}}
    });
    for (name, value) in [
        ("run.json", run),
        ("mutants.json", inventory),
        ("outcomes.json", outcomes),
    ] {
        fs::write(temp.path().join(name), serde_json::to_vec(&value).unwrap()).unwrap();
    }
    fs::write(temp.path().join("logs/baseline.log"), "baseline passed\n").unwrap();
    fs::write(
        temp.path().join("logs/discovery.log"),
        "packages discovered\n",
    )
    .unwrap();
    temp
}

#[cfg(test)]
pub(crate) fn fixture_with_dismissals() -> tempfile::TempDir {
    use serde_json::{Value, json};
    use std::fs;
    let temp = fixture();
    let path = temp.path().join("mutants.json");
    let mut inventory: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    for mutation in inventory["mutations"].as_array_mut().unwrap() {
        mutation["source_fingerprint"] = json!(format!("sha256:{}", "a".repeat(64)));
    }
    let dismissal = |index: usize, id: usize| {
        let mutation: SavedMutation =
            serde_json::from_value(inventory["mutations"][index].clone()).unwrap();
        json!({"id": id, "reason": "accepted test gap", "note": "Reviewed café / 目录\n\u{001b}[2J remains text",
            "mutation": crate::identity::MutationIdentity::from(&mutation)})
    };
    let first = dismissal(0, 5);
    let interrupted = dismissal(2, 7);
    let mut missing = dismissal(1, 8);
    missing["mutation"]["file"] = json!("removed.go");
    inventory["dismissals"] = json!({
        "matched": [{"mutation_id": 1, "dismissal": first}, {"mutation_id": 3, "dismissal": interrupted}],
        "needs_review": [{"status": "missing", "dismissal": missing}]
    });
    fs::write(path, serde_json::to_vec(&inventory).unwrap()).unwrap();
    temp
}

#[cfg(test)]
pub(crate) fn fixture_with_score() -> tempfile::TempDir {
    use serde_json::{Value, json};
    use std::fs;
    let temp = fixture_with_dismissals();
    let directory = temp.path().canonicalize().unwrap();
    let mut outcomes: Value = read_json(&directory, "outcomes.json").unwrap();
    outcomes["status"] = json!("completed");
    for (execution, outcome) in outcomes["mutations"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .zip(["missed", "killed", "killed", "build_failed"])
    {
        execution["state"] = json!("completed");
        execution["outcome"] = json!(outcome);
    }
    fs::write(
        temp.path().join("outcomes.json"),
        serde_json::to_vec(&outcomes).unwrap(),
    )
    .unwrap();
    let mut run: Value = read_json(&directory, "run.json").unwrap();
    run["status"] = json!("completed");
    run["effective_config"] = json!({"include":[], "exclude":[], "operators":[],
        "rerun":{"source_run":"/saved/older"}, "min_score":66.67});
    fs::write(
        temp.path().join("run.json"),
        serde_json::to_vec(&run).unwrap(),
    )
    .unwrap();
    let report = LoadedReport::load(temp.path()).unwrap();
    let score = MutationScore::calculate(
        ScoreScope::Rerun,
        report.run.status,
        report.complete,
        report
            .executions
            .iter()
            .map(|e| (e.outcome, report.dismissal(e.id).is_some())),
    );
    run["score_policy"] = serde_json::to_value(ScorePolicy::evaluate(
        Some("66.67".parse().unwrap()),
        &score,
    ))
    .unwrap();
    run["score"] = serde_json::to_value(score).unwrap();
    fs::write(
        temp.path().join("run.json"),
        serde_json::to_vec(&run).unwrap(),
    )
    .unwrap();
    temp
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn update(directory: &Path, name: &str, edit: impl FnOnce(&mut serde_json::Value)) {
        let mut value: serde_json::Value =
            serde_json::from_slice(&fs::read(directory.join(name)).unwrap()).unwrap();
        edit(&mut value);
        fs::write(directory.join(name), serde_json::to_vec(&value).unwrap()).unwrap();
    }

    #[test]
    fn score_and_threshold_are_validated_against_outcomes_review_and_selection() {
        use serde_json::json;
        let temp = fixture_with_score();
        let report = LoadedReport::load(temp.path()).unwrap();
        assert_eq!(report.run.score.unwrap().value_label(), "66.67%");
        assert!(report.run.score_policy.unwrap().failed());
        for (pointer, value) in [
            ("/score/percentage", json!(100)),
            ("/score/killed", json!(3)),
            ("/score/unfinished", json!(1)),
            ("/score/dismissed_in_score", json!(0)),
            ("/score/unavailable_reason", json!("incomplete_run")),
            ("/score/scope", json!("all_discovered")),
            ("/score_policy/status", json!("passed")),
            ("/score_policy/minimum", json!(50)),
            ("/effective_config/min_score", json!(50)),
            ("/effective_config/rerun", json!(null)),
            ("/score_policy", json!(null)),
            ("/score", json!(null)),
        ] {
            let temp = fixture_with_score();
            update(temp.path(), "run.json", |v| {
                *v.pointer_mut(pointer).unwrap() = value
            });
            assert!(LoadedReport::load(temp.path()).is_err(), "{pointer}");
        }
    }

    #[test]
    fn saved_score_requires_percentage_for_scored_outcomes() {
        let temp = fixture_with_score();
        let report = LoadedReport::load(temp.path()).unwrap();
        assert!(report.run.score.unwrap().percentage.is_some());

        // Both explicit null and an omitted field deserialize to None. The
        // outcomes still require a percentage, with every other score field intact.
        for omitted in [false, true] {
            update(temp.path(), "run.json", |run| {
                if omitted {
                    run["score"].as_object_mut().unwrap().remove("percentage");
                } else {
                    run["score"]["percentage"] = serde_json::Value::Null;
                }
            });
            let error = LoadedReport::load(temp.path())
                .expect_err("a scored report must retain its percentage");
            assert!(
                error
                    .root_cause()
                    .to_string()
                    .contains("saved mutation score disagrees"),
                "{error:#}"
            );
        }
    }

    #[test]
    fn saved_score_rejects_percentage_for_an_interrupted_run() {
        let temp = fixture();
        let report = LoadedReport::load(temp.path()).unwrap();
        let score = MutationScore::calculate(
            ScoreScope::AllDiscovered,
            report.run.status,
            report.complete,
            report.executions.iter().map(|e| (e.outcome, false)),
        );
        assert_eq!(score.percentage, None);
        update(temp.path(), "run.json", |run| {
            run["effective_config"] = serde_json::json!({});
            run["score_policy"] = serde_json::to_value(ScorePolicy::Disabled).unwrap();
            run["score"] = serde_json::to_value(&score).unwrap();
        });
        assert_eq!(
            LoadedReport::load(temp.path()).unwrap().run.score,
            Some(score)
        );

        // An interrupted run has no score even when some mutations finished.
        // Keep the counts and unavailable reason unchanged to isolate this check.
        for percentage in [0, 100] {
            update(temp.path(), "run.json", |run| {
                run["score"]["percentage"] = serde_json::json!(percentage);
            });
            let error = LoadedReport::load(temp.path())
                .expect_err("an interrupted report must not claim a percentage");
            assert!(
                error
                    .root_cause()
                    .to_string()
                    .contains("saved mutation score disagrees"),
                "{error:#}"
            );
        }
    }

    #[test]
    fn operator_totals_join_old_shuffled_records_and_allow_missing_timings() {
        let temp = fixture();
        update(temp.path(), "mutants.json", |v| {
            v["mutations"].as_array_mut().unwrap().reverse();
        });
        update(temp.path(), "outcomes.json", |v| {
            let executions = v["mutations"].as_array_mut().unwrap();
            executions[0]
                .as_object_mut()
                .unwrap()
                .remove("duration_seconds");
            executions[2]["duration_seconds"] = serde_json::json!(0.5);
            executions.rotate_left(1);
        });
        let report = LoadedReport::load(temp.path()).unwrap();
        assert!(report.run.score.is_none());
        let summary = report.operator_summary();
        assert!(summary.contains(
            "arithmetic_replace\n  Planned: 1 | Completed: 1\n  Killed: 1 | Missed: 0 | Timeout: 0"
        ));
        assert!(summary.contains(
            "boolean_flip\n  Planned: 3 | Completed: 1\n  Killed: 0 | Missed: 1 | Timeout: 0"
        ));
        assert!(summary.contains("Time: 0.25s (1 recorded, 0 missing)"));
        assert!(summary.contains("Time: 0.50s (1 recorded, 1 missing)"));
    }

    #[test]
    fn joins_shuffled_records_without_the_original_checkout() {
        let temp = fixture();
        update(temp.path(), "mutants.json", |v| {
            v["mutations"].as_array_mut().unwrap().reverse()
        });
        let report = LoadedReport::load(temp.path()).unwrap();
        assert_eq!(
            report.mutations.iter().map(|m| m.id).collect::<Vec<_>>(),
            vec![1, 2, 3, 4]
        );
        assert_eq!(report.mutations[1].file, Path::new("目录/b.go"));
        assert_eq!(report.executions[1].outcome, Some(Outcome::Killed));
        assert!(!report.run.module_root.unwrap().exists());
        assert!(report.dismissals.is_none());
    }

    #[test]
    fn validates_log_retention_and_accepts_legacy_records_without_it() {
        use serde_json::json;
        for path in ["/baseline/logs", "/discovery/logs", "/mutations/0/logs"] {
            let temp = fixture();
            assert!(LoadedReport::load(temp.path()).is_ok());
            let full =
                json!({"total_bytes": 12, "head_bytes": 12, "tail_bytes": 0, "omitted_bytes": 0});
            let cut =
                json!({"total_bytes": 100, "head_bytes": 5, "tail_bytes": 5, "omitted_bytes": 90});
            update(temp.path(), "outcomes.json", |v| {
                v.pointer_mut(path).unwrap()["retention"] =
                    json!({"text":cut, "stdout":full, "stderr":full});
            });
            assert!(LoadedReport::load(temp.path()).is_ok());
            update(temp.path(), "outcomes.json", |v| {
                v.pointer_mut(path).unwrap()["retention"]["text"]["omitted_bytes"] = json!(99);
            });
            assert!(
                format!("{:#}", LoadedReport::load(temp.path()).unwrap_err())
                    .contains("log retention")
            );
        }
    }

    #[test]
    fn saved_dismissals_are_validated_and_sorted_without_a_checkout() {
        let temp = fixture_with_dismissals();
        update(temp.path(), "mutants.json", |v| {
            v["mutations"].as_array_mut().unwrap().reverse();
            v["dismissals"]["matched"].as_array_mut().unwrap().reverse();
        });
        let report = LoadedReport::load(temp.path()).unwrap();
        assert_eq!(report.dismissal(1).unwrap().id, 5);
        assert!(report.dismissal(2).is_none());
        assert_eq!(report.dismissal(3).unwrap().id, 7);
        assert_eq!(report.executions[0].outcome, Some(Outcome::Missed));
        assert_eq!(report.executions[2].state, ExecutionState::Interrupted);
        assert_eq!(
            report.dismissals.unwrap().needs_review[0]
                .dismissal
                .mutation
                .file,
            Path::new("removed.go")
        );
    }

    #[test]
    fn rejects_inconsistent_saved_dismissal_snapshots() {
        use serde_json::json;
        for (pointer, value, expected) in [
            (
                "/matched/0/mutation_id",
                json!(99),
                "unknown saved mutation",
            ),
            ("/matched/1/mutation_id", json!(1), "multiple dismissals"),
            (
                "/matched/1/dismissal/id",
                json!(5),
                "duplicate saved dismissal ID",
            ),
            (
                "/needs_review/0/dismissal/id",
                json!(5),
                "duplicate saved dismissal ID",
            ),
            (
                "/matched/0/dismissal/reason",
                json!(" "),
                "reason must not be blank",
            ),
            (
                "/matched/0/dismissal/note",
                json!(""),
                "note must not be blank",
            ),
            (
                "/matched/0/dismissal/mutation/replacement",
                json!("other"),
                "identity does not match",
            ),
            (
                "/matched/0/dismissal/mutation/file",
                json!("../a.go"),
                "invalid module-relative",
            ),
            (
                "/matched/0/dismissal/mutation/source_fingerprint",
                json!("sha256:bad"),
                "invalid source fingerprint",
            ),
            (
                "/needs_review/0/status",
                json!({"matched": 1}),
                "issue cannot be a matched",
            ),
            (
                "/needs_review/0/status",
                json!("unknown"),
                "unknown variant",
            ),
        ] {
            let temp = fixture_with_dismissals();
            update(temp.path(), "mutants.json", |v| {
                *v["dismissals"].pointer_mut(pointer).unwrap() = value;
            });
            let error = LoadedReport::load(temp.path()).unwrap_err();
            assert!(
                format!("{error:#}").contains(expected),
                "{pointer}: {error:#}"
            );
        }
        let temp = fixture_with_dismissals();
        update(temp.path(), "mutants.json", |v| {
            v["dismissals"]["needs_review"][0]["dismissal"]["mutation"] =
                v["dismissals"]["matched"][0]["dismissal"]["mutation"].clone();
        });
        assert!(
            format!("{:#}", LoadedReport::load(temp.path()).unwrap_err())
                .contains("matched and needing review")
        );
    }

    #[test]
    fn permits_failed_baseline_reports_but_reruns_require_an_inventory() {
        let temp = fixture();
        update(temp.path(), "run.json", |v| {
            v["status"] = "failed".into();
            v["planned_mutations"] = serde_json::Value::Null;
        });
        update(temp.path(), "mutants.json", |v| {
            v["complete"] = false.into();
            v["mutations"] = serde_json::json!([]);
        });
        update(temp.path(), "outcomes.json", |v| {
            v["status"] = "failed".into();
            v["mutations"] = serde_json::json!([]);
        });
        let report = LoadedReport::load(temp.path()).unwrap();
        assert!(!report.complete);
        assert!(
            report
                .artifact(Path::new("logs/baseline.log"))
                .unwrap()
                .contains("baseline passed")
        );
        // The browser can inspect it; the rerun reader still refuses it.
        let error = crate::rerun::RerunSelection::load(&crate::rerun::RerunRequest {
            source_run: temp.path().to_path_buf(),
            filter: crate::rerun::RerunFilter::new(crate::rerun::RerunMode::All, vec![], vec![]),
        })
        .err()
        .unwrap();
        assert!(format!("{error:#}").contains("incomplete mutation inventory"));
    }

    #[test]
    fn bounds_and_sanitizes_artifacts_and_rejects_escaping_paths() {
        let temp = fixture();
        let report = LoadedReport::load(temp.path()).unwrap();
        let text = report.artifact(Path::new("logs/000001.log")).unwrap();
        assert!(text.contains("�[2Jdata"));
        assert!(!text.contains('\x1b'));
        fs::write(temp.path().join("large"), vec![b'x'; 300_000]).unwrap();
        let text = report.artifact(Path::new("large")).unwrap();
        assert!(text.contains("Preview truncated at 256 KiB"));
        assert!(text.len() < 263_000);
        for path in ["../run.json", "/etc/passwd", "logs", "missing"] {
            assert!(report.artifact(Path::new(path)).is_err(), "{path}");
        }
        #[cfg(unix)]
        {
            let outside = tempfile::NamedTempFile::new().unwrap();
            std::os::unix::fs::symlink(outside.path(), temp.path().join("escape")).unwrap();
            assert!(
                report
                    .artifact(Path::new("escape"))
                    .unwrap_err()
                    .to_string()
                    .contains("escapes")
            );
        }
    }
}
