use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result, ensure};
use serde::Serialize;

use crate::{
    cancellation::{Cancellation, Interrupted},
    config::RunConfig,
    go,
    mutation::{Mutation, OPERATOR_CATALOG_VERSION, Operator},
    process::CommandOutput,
    report::{ExecutionState, MutationResult, Outcome, RunStatus, Summary},
    rerun::RerunProvenance,
};

pub(crate) const SCHEMA_VERSION: u32 = 1;

#[derive(Serialize)]
struct RunReport {
    schema_version: u32,
    checkpoint: u64,
    mutie_version: &'static str,
    operator_catalog_version: u32,
    module_root: PathBuf,
    effective_config: RunConfig,
    started_at_unix_seconds: u64,
    finished_at_unix_seconds: Option<u64>,
    duration_seconds: f64,
    status: RunStatus,
    phase: &'static str,
    error: Option<String>,
    command: Vec<&'static str>,
    include: Vec<String>,
    exclude: Vec<String>,
    requested_workers: usize,
    workers: Option<usize>,
    baseline_timeout_seconds: u64,
    mutation_timeout_seconds: Option<f64>,
    planned_mutations: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    rerun: Option<RerunProvenance>,
}

#[derive(Serialize)]
struct Inventory {
    schema_version: u32,
    operator_catalog_version: u32,
    complete: bool,
    mutations: Vec<PlannedMutation>,
}

#[derive(Serialize)]
struct PlannedMutation {
    id: usize,
    file: PathBuf,
    operator: Operator,
    source_fingerprint: String,
    line: usize,
    column: usize,
    end_line: usize,
    end_column: usize,
    byte_start: usize,
    byte_end: usize,
    original: String,
    replacement: &'static str,
    diff: String,
}

#[derive(Serialize)]
struct Outcomes {
    schema_version: u32,
    checkpoint: u64,
    status: RunStatus,
    baseline: Option<CommandRecord>,
    discovery: Option<CommandRecord>,
    summary: Summary,
    mutations: Vec<Execution>,
}

#[derive(Serialize)]
struct CommandRecord {
    outcome: &'static str,
    duration_seconds: f64,
    exit_code: Option<i32>,
    logs: Logs,
}

/// Artifact paths are relative to the run directory so reports can be moved.
#[derive(Serialize)]
struct Logs {
    text: String,
    stdout: String,
    stderr: String,
}

#[derive(Serialize)]
struct Execution {
    id: usize,
    state: ExecutionState,
    worker: Option<usize>,
    outcome: Option<Outcome>,
    error: Option<String>,
    duration_seconds: Option<f64>,
    exit_code: Option<i32>,
    logs: Option<Logs>,
}

pub struct SavedReport {
    directory: PathBuf,
    started: Instant,
    run: RunReport,
    inventory: Inventory,
    outcomes: Outcomes,
}

impl SavedReport {
    pub fn create(config: &RunConfig) -> Result<Self> {
        let root = &config.module_root;
        let output_root = &config.output_dir;
        fs::create_dir_all(output_root)
            .with_context(|| format!("creating report directory {}", output_root.display()))?;
        let output_root = output_root.canonicalize()?;
        ensure!(
            !root.starts_with(&output_root),
            "the report directory must not be the module root or one of its ancestors"
        );
        let started_at = unix_seconds()?;
        let directory = tempfile::Builder::new()
            .prefix(&format!("run-{started_at}-"))
            .tempdir_in(&output_root)?;
        fs::create_dir(directory.path().join("diff"))?;
        fs::create_dir(directory.path().join("logs"))?;
        let mut report = Self {
            directory: directory.path().to_owned(),
            started: Instant::now(),
            run: RunReport {
                schema_version: SCHEMA_VERSION,
                checkpoint: 0,
                mutie_version: env!("CARGO_PKG_VERSION"),
                operator_catalog_version: OPERATOR_CATALOG_VERSION,
                module_root: root.to_owned(),
                effective_config: RunConfig {
                    output_dir: output_root,
                    ..config.clone()
                },
                started_at_unix_seconds: started_at,
                finished_at_unix_seconds: None,
                duration_seconds: 0.0,
                status: RunStatus::Running,
                phase: "snapshot",
                error: None,
                command: std::iter::once("go")
                    .chain(go::TEST_ARGS.iter().copied())
                    .collect(),
                include: config.include.clone(),
                exclude: config.exclude.clone(),
                requested_workers: config.jobs.get(),
                workers: None,
                baseline_timeout_seconds: config.baseline_timeout.get(),
                mutation_timeout_seconds: config.timeout.map(|value| value.get() as f64),
                planned_mutations: None,
                rerun: None,
            },
            inventory: Inventory {
                schema_version: SCHEMA_VERSION,
                operator_catalog_version: OPERATOR_CATALOG_VERSION,
                complete: false,
                mutations: Vec::new(),
            },
            outcomes: Outcomes {
                schema_version: SCHEMA_VERSION,
                checkpoint: 0,
                status: RunStatus::Running,
                baseline: None,
                discovery: None,
                summary: Summary::default(),
                mutations: Vec::new(),
            },
        };
        report.write(
            "mutants.json",
            &serde_json::to_vec_pretty(&report.inventory)?,
        )?;
        report.flush()?;
        report.directory = directory.keep();
        Ok(report)
    }

    pub fn path(&self) -> &Path {
        &self.directory
    }

    pub fn phase(&mut self, phase: &'static str) -> Result<()> {
        self.run.phase = phase;
        self.flush()
    }

    pub fn command(&mut self, name: &str, output: &CommandOutput) -> Result<()> {
        ensure!(
            matches!(name, "baseline" | "discovery"),
            "unknown command {name}"
        );
        let record = CommandRecord {
            outcome: if output.interrupted {
                "interrupted"
            } else if output.timed_out {
                "timeout"
            } else if output.status.success() {
                "passed"
            } else {
                "failed"
            },
            duration_seconds: output.elapsed.as_secs_f64(),
            exit_code: output.status.code(),
            logs: self.logs(&format!("logs/{name}"), output)?,
        };
        if name == "baseline" {
            self.outcomes.baseline = Some(record);
        } else {
            self.outcomes.discovery = Some(record);
        }
        self.flush()
    }

    pub fn plan(
        &mut self,
        mutations: &[Mutation],
        workers: usize,
        timeout: Duration,
        rerun: Option<&RerunProvenance>,
        cancellation: &Cancellation,
    ) -> Result<()> {
        ensure!(
            !self.inventory.complete,
            "mutation inventory already published"
        );
        let mut inventory = Vec::with_capacity(mutations.len());
        for (index, mutation) in mutations.iter().enumerate() {
            cancellation.check()?;
            let id = index + 1;
            let diff = format!("diff/{id:06}.diff");
            self.write(&diff, mutation.diff().as_bytes())?;
            inventory.push(PlannedMutation {
                id,
                file: mutation.file.path.clone(),
                operator: mutation.operator,
                source_fingerprint: mutation.file.fingerprint.clone(),
                line: mutation.line,
                column: mutation.column,
                end_line: mutation.end_line,
                end_column: mutation.end_column,
                byte_start: mutation.range.start,
                byte_end: mutation.range.end,
                original: mutation.original().to_owned(),
                replacement: mutation.replacement,
                diff,
            });
        }
        self.inventory.mutations = inventory;
        self.inventory.complete = true;
        self.write("mutants.json", &serde_json::to_vec_pretty(&self.inventory)?)?;
        self.outcomes.mutations = self
            .inventory
            .mutations
            .iter()
            .map(|mutation| Execution {
                id: mutation.id,
                state: ExecutionState::Pending,
                worker: None,
                outcome: None,
                error: None,
                duration_seconds: None,
                exit_code: None,
                logs: None,
            })
            .collect();
        self.run.planned_mutations = Some(mutations.len());
        self.run.rerun = rerun.cloned();
        self.run.workers = Some(workers);
        self.run.mutation_timeout_seconds = Some(timeout.as_secs_f64());
        self.flush()
    }

    pub fn started(&mut self, id: usize, worker: usize) -> Result<()> {
        let execution = self.execution_mut(id)?;
        ensure!(
            execution.state == ExecutionState::Pending,
            "mutation {id} already started"
        );
        execution.state = ExecutionState::Running;
        execution.worker = Some(worker);
        self.flush()
    }

    pub fn record(&mut self, result: &MutationResult) -> Result<()> {
        ensure!(
            self.execution_mut(result.id)?.state == ExecutionState::Running,
            "mutation {} is not running",
            result.id
        );
        ensure!(
            matches!(
                (result.state, result.outcome),
                (ExecutionState::Completed, Some(_)) | (ExecutionState::Interrupted, None)
            ),
            "invalid execution result for mutation {}",
            result.id
        );
        let logs = result
            .output
            .as_ref()
            .map(|output| self.logs(&format!("logs/{:06}", result.id), output))
            .transpose()?;
        let execution = self.execution_mut(result.id)?;
        execution.state = result.state;
        execution.outcome = result.outcome;
        execution.error = result.error.clone();
        execution.duration_seconds = result
            .output
            .as_ref()
            .map(|output| output.elapsed.as_secs_f64());
        execution.exit_code = result
            .output
            .as_ref()
            .and_then(|output| output.status.code());
        execution.logs = logs;
        self.outcomes.summary.record(result);
        self.flush()
    }

    fn execution_mut(&mut self, id: usize) -> Result<&mut Execution> {
        id.checked_sub(1)
            .and_then(|index| self.outcomes.mutations.get_mut(index))
            .with_context(|| format!("unknown mutation ID {id}"))
    }

    pub fn finish(&mut self, error: Option<&anyhow::Error>) -> Result<()> {
        let status = match error {
            Some(error) if error.is::<Interrupted>() => RunStatus::Interrupted,
            Some(_) => RunStatus::Failed,
            None => RunStatus::Completed,
        };
        if status == RunStatus::Completed {
            ensure!(
                self.inventory.complete
                    && self
                        .outcomes
                        .mutations
                        .iter()
                        .all(|item| item.state == ExecutionState::Completed),
                "cannot complete a run with unfinished mutations"
            );
        }
        // A panic or persistence error can prevent a worker's final event from
        // being recorded. Never leave it apparently running in a finalized run.
        for execution in &mut self.outcomes.mutations {
            if execution.state == ExecutionState::Running {
                execution.state = ExecutionState::Interrupted;
                execution.error = Some("run stopped before this result was recorded".into());
            }
        }
        self.run.status = status;
        self.outcomes.status = status;
        self.run.error = error.map(|error| format!("{error:#}"));
        self.run.finished_at_unix_seconds = Some(unix_seconds()?);
        self.flush()
    }

    fn logs(&self, prefix: &str, output: &CommandOutput) -> Result<Logs> {
        let logs = Logs {
            text: format!("{prefix}.log"),
            stdout: format!("{prefix}.stdout.jsonl"),
            stderr: format!("{prefix}.stderr.log"),
        };
        self.write(&logs.text, go::text(output).as_bytes())?;
        self.write(&logs.stdout, output.stdout.as_bytes())?;
        self.write(&logs.stderr, output.stderr.as_bytes())?;
        Ok(logs)
    }

    fn flush(&mut self) -> Result<()> {
        self.run.duration_seconds = self.started.elapsed().as_secs_f64();
        self.run.checkpoint += 1;
        self.outcomes.checkpoint = self.run.checkpoint;
        // Each snapshot is atomic, not the whole set. Matching checkpoint numbers
        // let readers detect a crash between publication of outcomes and metadata.
        self.write("outcomes.json", &serde_json::to_vec_pretty(&self.outcomes)?)?;
        self.write("run.json", &serde_json::to_vec_pretty(&self.run)?)?;
        self.write("summary.txt", self.summary().as_bytes())?;
        for (name, outcome) in [
            ("killed", Outcome::Killed),
            ("missed", Outcome::Missed),
            ("timeout", Outcome::Timeout),
            ("build_failed", Outcome::BuildFailed),
            ("errors", Outcome::Error),
        ] {
            let mut text = String::new();
            for execution in &self.outcomes.mutations {
                if execution.outcome == Some(outcome) {
                    text.push_str(&self.description(execution.id));
                    text.push('\n');
                }
            }
            self.write(&format!("{name}.txt"), text.as_bytes())?;
        }
        Ok(())
    }

    /// Publish each file only after its full contents have been written.
    fn write(&self, name: &str, contents: &[u8]) -> Result<()> {
        let path = self.directory.join(name);
        let mut file =
            tempfile::NamedTempFile::new_in(path.parent().context("missing report parent")?)?;
        file.write_all(contents)?;
        file.as_file().sync_all()?;
        file.persist(&path)
            .with_context(|| format!("saving {}", path.display()))?;
        Ok(())
    }

    fn description(&self, id: usize) -> String {
        let mutation = &self.inventory.mutations[id - 1];
        format!(
            "[{id:06}] {}:{}:{}: {:?} -> {:?}",
            mutation.file.display(),
            mutation.line,
            mutation.column,
            mutation.original,
            mutation.replacement
        )
    }

    fn summary(&self) -> String {
        let mut text = format!(
            "mutie {}\nModule: {}\nStatus: {}\nPhase: {}\nDuration: {:.2}s\n\n{}\n",
            self.run.mutie_version,
            self.run.module_root.display(),
            self.run.status,
            self.run.phase,
            self.run.duration_seconds,
            self.outcomes.summary
        );
        if let Some(rerun) = &self.run.rerun {
            text.push_str(&format!(
                "Rerun: missed mutations from {} (checkpoint {})\n",
                rerun.source_run.display(),
                rerun.source_checkpoint
            ));
            for mapping in &rerun.mutations {
                text.push_str(&format!(
                    "  [{:06}] <- source [{:06}]\n",
                    mapping.id, mapping.source_id
                ));
            }
        }
        if let Some(planned) = self.run.planned_mutations {
            text.push_str(&format!(
                "Reported: {}/{planned}\n",
                self.outcomes.summary.total()
            ));
            for (name, state) in [
                ("Pending", ExecutionState::Pending),
                ("Running", ExecutionState::Running),
                ("Interrupted", ExecutionState::Interrupted),
            ] {
                text.push_str(&format!(
                    "{name}: {}\n",
                    self.outcomes
                        .mutations
                        .iter()
                        .filter(|item| item.state == state)
                        .count()
                ));
            }
        }
        if let Some(baseline) = &self.outcomes.baseline {
            text.push_str(&format!(
                "Baseline: {} ({:.2}s), output: {}\n",
                baseline.outcome, baseline.duration_seconds, baseline.logs.text
            ));
        }
        if let Some(error) = &self.run.error {
            text.push_str(&format!("\nError: {error}\n"));
        }
        text.push_str("\nMissed mutations:\n");
        if self.outcomes.summary.missed == 0 {
            text.push_str("  None reported.\n");
        }
        for execution in &self.outcomes.mutations {
            if execution.outcome == Some(Outcome::Missed) {
                text.push_str(&format!(
                    "  {}\n    Diff: {}\n",
                    self.description(execution.id),
                    self.inventory.mutations[execution.id - 1].diff
                ));
                if let Some(logs) = &execution.logs {
                    text.push_str(&format!("    Output: {}\n", logs.text));
                }
            }
        }
        text.push_str("\nAll mutations (source paths are module-relative; artifact paths are report-relative):\n");
        for execution in &self.outcomes.mutations {
            let state = execution
                .outcome
                .map(|outcome| outcome.to_string())
                .unwrap_or_else(|| {
                    match execution.state {
                        ExecutionState::Pending => "pending",
                        ExecutionState::Running => "running",
                        ExecutionState::Interrupted => "interrupted",
                        ExecutionState::Completed => "completed",
                    }
                    .into()
                });
            text.push_str(&format!(
                "  {}: {state}\n    Diff: {}\n",
                self.description(execution.id),
                self.inventory.mutations[execution.id - 1].diff
            ));
            if let Some(logs) = &execution.logs {
                text.push_str(&format!("    Output: {}\n", logs.text));
            }
            if let Some(error) = &execution.error {
                text.push_str(&format!("    Error: {error}\n"));
            }
        }
        text
    }
}

fn unix_seconds() -> Result<u64> {
    Ok(SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs())
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use crate::{
        cli::Cli,
        mutation::{self, Scope},
    };
    use clap::Parser;
    use std::os::unix::process::ExitStatusExt;

    fn config(root: &Path) -> Result<RunConfig> {
        fs::write(root.join("go.mod"), "module example.com/test\n")?;
        RunConfig::resolve(Cli::parse_from([
            std::ffi::OsStr::new("mutie"),
            root.as_os_str(),
        ]))
    }

    fn read(report: &SavedReport, name: &str) -> serde_json::Value {
        serde_json::from_slice(&fs::read(report.path().join(name)).unwrap()).unwrap()
    }

    fn output() -> CommandOutput {
        CommandOutput {
            status: std::process::ExitStatus::from_raw(0),
            timed_out: false,
            interrupted: false,
            elapsed: Duration::from_millis(125),
            stdout: "{\"Action\":\"output\",\"Output\":\"PASS\\n\"}\n".into(),
            stderr: "diagnostic\n".into(),
        }
    }

    #[test]
    fn publishes_inventory_before_execution_and_checkpoints_results_by_id() -> Result<()> {
        let root = tempfile::tempdir()?;
        fs::write(
            root.path().join("code.go"),
            "package p\nvar a = true\nvar b = false\nvar c = true\n",
        )?;
        let mutations =
            mutation::discover(root.path(), vec!["code.go".into()], &Scope::new(&[], &[])?)?;
        let config = config(root.path())?;
        let mut report = SavedReport::create(&config)?;
        assert_eq!(read(&report, "mutants.json")["complete"], false);
        report.command("baseline", &output())?;
        report.plan(
            &mutations,
            2,
            Duration::from_secs(10),
            None,
            &Cancellation::default(),
        )?;
        let inventory = fs::read(report.path().join("mutants.json"))?;
        let planned = read(&report, "mutants.json");
        assert_eq!(planned["schema_version"], 1);
        assert_eq!(planned["operator_catalog_version"], 1);
        assert_eq!(planned["complete"], true);
        for (index, mutation) in planned["mutations"].as_array().unwrap().iter().enumerate() {
            assert_eq!(mutation["id"], index + 1);
            assert_eq!(mutation["operator"], "boolean_flip");
            assert!(
                report
                    .path()
                    .join(mutation["diff"].as_str().unwrap())
                    .is_file()
            );
            assert_eq!(
                read(&report, "outcomes.json")["mutations"][index]["state"],
                "pending"
            );
        }
        for id in [2, 1] {
            report.started(id, id)?;
            let started = read(&report, "outcomes.json");
            assert_eq!(started["mutations"][id - 1]["state"], "running");
            assert!(started["mutations"][id - 1]["outcome"].is_null());
            let result = MutationResult {
                id,
                mutation: mutations[id - 1].clone(),
                state: ExecutionState::Completed,
                outcome: Some(Outcome::Missed),
                output: Some(output()),
                error: None,
            };
            report.record(&result)?;
            assert!(report.record(&result).is_err());
            assert_eq!(read(&report, "outcomes.json")["summary"]["missed"], 3 - id);
            assert_eq!(
                read(&report, "outcomes.json")["mutations"][id - 1]["duration_seconds"],
                0.125
            );
        }
        assert!(report.started(0, 1).is_err());
        assert!(report.started(4, 1).is_err());
        assert!(report.finish(None).is_err());
        report.started(3, 1)?;
        let mut interrupted = output();
        interrupted.interrupted = true;
        report.record(&MutationResult {
            id: 3,
            mutation: mutations[2].clone(),
            state: ExecutionState::Interrupted,
            outcome: None,
            output: Some(interrupted),
            error: None,
        })?;
        report.finish(Some(&Interrupted.into()))?;
        let run = read(&report, "run.json");
        let outcomes = read(&report, "outcomes.json");
        assert_eq!(run["status"], "interrupted");
        assert_eq!(outcomes["status"], "interrupted");
        assert_eq!(run["checkpoint"], outcomes["checkpoint"]);
        assert!(run["finished_at_unix_seconds"].is_number());
        assert_eq!(outcomes["summary"]["missed"], 2);
        assert_eq!(outcomes["summary"]["errors"], 0);
        assert!(outcomes["mutations"][2]["outcome"].is_null());
        assert_eq!(
            fs::read_to_string(report.path().join("logs/000003.log"))?,
            "PASS\ndiagnostic\n"
        );
        assert!(
            fs::read_to_string(report.path().join("diff/000001.diff"))?
                .contains("-var a = true\n+var a = false\n")
        );
        let missed = fs::read_to_string(report.path().join("missed.txt"))?;
        assert_eq!(missed.lines().count(), 2);
        assert!(missed.starts_with("[000001]"));
        assert!(fs::read_to_string(report.path().join("errors.txt"))?.is_empty());
        assert!(fs::read_to_string(report.path().join("summary.txt"))?.contains("Interrupted: 1"));
        assert_eq!(fs::read(report.path().join("mutants.json"))?, inventory);
        let previous = fs::read(report.path().join("run.json"))?;
        let next = SavedReport::create(&config)?;
        assert_ne!(report.path(), next.path());
        assert_eq!(fs::read(report.path().join("run.json"))?, previous);
        let next_path = next.path().to_owned();
        drop(next);
        assert!(next_path.join("run.json").is_file());
        Ok(())
    }

    #[test]
    fn refuses_to_run_without_a_published_inventory() -> Result<()> {
        let root = tempfile::tempdir()?;
        fs::write(root.path().join("code.go"), "package p\nvar a = true\n")?;
        let mutations =
            mutation::discover(root.path(), vec!["code.go".into()], &Scope::new(&[], &[])?)?;
        let mut report = SavedReport::create(&config(root.path())?)?;
        assert!(report.started(1, 1).is_err());
        // Force an artifact write failure without permission-dependent behavior.
        fs::create_dir(report.path().join("diff/000001.diff"))?;
        assert!(
            report
                .plan(
                    &mutations,
                    1,
                    Duration::from_secs(10),
                    None,
                    &Cancellation::default()
                )
                .is_err()
        );
        assert_eq!(read(&report, "mutants.json")["complete"], false);
        assert!(report.started(1, 1).is_err());
        report.finish(Some(&anyhow::anyhow!("report write failed")))?;
        assert_eq!(read(&report, "run.json")["status"], "failed");
        Ok(())
    }

    #[test]
    fn rejects_output_paths_that_would_hide_the_module_or_overwrite_a_file() -> Result<()> {
        let root = tempfile::tempdir()?;
        let mut config = config(root.path())?;
        config.output_dir = root.path().to_owned();
        assert!(SavedReport::create(&config).is_err());
        let sentinel = root.path().join("existing");
        fs::write(&sentinel, "keep me")?;
        config.output_dir = sentinel.clone();
        assert!(SavedReport::create(&config).is_err());
        assert_eq!(fs::read_to_string(sentinel)?, "keep me");
        Ok(())
    }
}
