use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result, ensure};
use serde::Serialize;

use crate::{
    cli::Cli,
    go,
    process::CommandOutput,
    report::{MutationResult, Outcome, Summary},
};

#[derive(Serialize)]
struct RunReport {
    schema_version: u32,
    mutie_version: &'static str,
    module_root: PathBuf,
    started_at_unix_seconds: u64,
    finished_at_unix_seconds: Option<u64>,
    duration_seconds: f64,
    status: &'static str,
    error: Option<String>,
    command: Vec<&'static str>,
    include: Vec<String>,
    exclude: Vec<String>,
    requested_workers: usize,
    workers: Option<usize>,
    baseline_timeout_seconds: u64,
    mutation_timeout_seconds: Option<f64>,
    planned_mutations: Option<usize>,
    baseline: Option<Baseline>,
    summary: Summary,
    mutations: Vec<SavedMutation>,
}

#[derive(Serialize)]
struct Baseline {
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
struct SavedMutation {
    id: usize,
    file: PathBuf,
    line: usize,
    column: usize,
    byte_start: usize,
    byte_end: usize,
    original: String,
    replacement: &'static str,
    outcome: Outcome,
    error: Option<String>,
    duration_seconds: f64,
    exit_code: Option<i32>,
    diff: String,
    logs: Logs,
}

pub struct SavedReport {
    directory: PathBuf,
    output_root: PathBuf,
    started: Instant,
    run: RunReport,
}

impl SavedReport {
    pub fn create(root: &Path, cli: &Cli) -> Result<Self> {
        let output_root = cli
            .output_dir
            .clone()
            .unwrap_or_else(|| root.join(".mutie"));
        fs::create_dir_all(&output_root)
            .with_context(|| format!("creating report directory {}", output_root.display()))?;
        let output_root = output_root.canonicalize()?;
        ensure!(
            !root.starts_with(&output_root),
            "the report directory must not be the module root or one of its ancestors"
        );
        let started_at = unix_seconds()?;
        // Random suffixes keep simultaneous runs and repeated invocations separate.
        let directory = tempfile::Builder::new()
            .prefix(&format!("run-{started_at}-"))
            .tempdir_in(&output_root)?;
        fs::create_dir(directory.path().join("mutations"))?;
        let mut report = Self {
            directory: directory.path().to_owned(),
            output_root,
            started: Instant::now(),
            run: RunReport {
                schema_version: 1,
                mutie_version: env!("CARGO_PKG_VERSION"),
                module_root: root.to_owned(),
                started_at_unix_seconds: started_at,
                finished_at_unix_seconds: None,
                duration_seconds: 0.0,
                status: "running",
                error: None,
                command: std::iter::once("go")
                    .chain(go::TEST_ARGS.iter().copied())
                    .collect(),
                include: cli.include.clone(),
                exclude: cli.exclude.clone(),
                requested_workers: cli.jobs.get(),
                workers: None,
                baseline_timeout_seconds: cli.baseline_timeout.get(),
                mutation_timeout_seconds: cli.timeout.map(|value| value.get() as f64),
                planned_mutations: None,
                baseline: None,
                summary: Summary::default(),
                mutations: Vec::new(),
            },
        };
        report.flush()?;
        report.directory = directory.keep();
        Ok(report)
    }

    pub fn path(&self) -> &Path {
        &self.directory
    }

    pub fn output_root(&self) -> &Path {
        &self.output_root
    }

    pub fn baseline(&mut self, output: &CommandOutput) -> Result<()> {
        let logs = self.logs("baseline", output)?;
        self.run.baseline = Some(Baseline {
            outcome: if output.timed_out {
                "timeout"
            } else if output.status.success() {
                "passed"
            } else {
                "failed"
            },
            duration_seconds: output.elapsed.as_secs_f64(),
            exit_code: output.status.code(),
            logs,
        });
        self.flush()
    }

    pub fn plan(&mut self, count: usize, workers: usize, timeout: Duration) -> Result<()> {
        self.run.planned_mutations = Some(count);
        self.run.workers = Some(workers);
        self.run.mutation_timeout_seconds = Some(timeout.as_secs_f64());
        self.flush()
    }

    pub fn record(&mut self, result: &MutationResult) -> Result<()> {
        ensure!(
            result.id > 0 && !self.run.mutations.iter().any(|item| item.id == result.id),
            "duplicate or invalid mutation report ID {}",
            result.id
        );
        let prefix = format!("mutations/{:06}", result.id);
        let diff = format!("{prefix}.diff");
        self.write(&diff, result.mutation.diff().as_bytes())?;
        let logs = self.logs(&prefix, &result.output)?;
        self.run.mutations.push(SavedMutation {
            id: result.id,
            file: result.mutation.file.path.clone(),
            line: result.mutation.line,
            column: result.mutation.column,
            byte_start: result.mutation.range.start,
            byte_end: result.mutation.range.end,
            original: result.mutation.original().to_owned(),
            replacement: result.mutation.replacement,
            outcome: result.outcome,
            error: result.error.clone(),
            duration_seconds: result.output.elapsed.as_secs_f64(),
            exit_code: result.output.status.code(),
            diff,
            logs,
        });
        self.run.mutations.sort_by_key(|item| item.id);
        self.run.summary.record(result);
        self.flush()
    }

    pub fn finish(&mut self, error: Option<&anyhow::Error>) -> Result<()> {
        self.run.status = if error.is_some() {
            "failed"
        } else {
            "completed"
        };
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
        self.write("report.json", &serde_json::to_vec_pretty(&self.run)?)?;
        self.write("summary.txt", self.summary().as_bytes())
    }

    /// Publish each file only after the full contents have been written. Reports
    /// from earlier checkpoints remain valid if writing a new checkpoint fails.
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

    fn summary(&self) -> String {
        let mut text = format!(
            "mutie {}\nModule: {}\nStatus: {}\nDuration: {:.2}s\n\n{}\n",
            self.run.mutie_version,
            self.run.module_root.display(),
            self.run.status,
            self.run.duration_seconds,
            self.run.summary
        );
        if let Some(planned) = self.run.planned_mutations {
            text.push_str(&format!(
                "Reported: {}/{planned}\n",
                self.run.mutations.len()
            ));
        }
        if let Some(baseline) = &self.run.baseline {
            text.push_str(&format!(
                "Baseline: {} ({:.2}s), output: {}\n",
                baseline.outcome, baseline.duration_seconds, baseline.logs.text
            ));
        }
        if let Some(error) = &self.run.error {
            text.push_str(&format!("\nError: {error}\n"));
        }
        text.push_str("\nMissed mutations:\n");
        let missed: Vec<_> = self
            .run
            .mutations
            .iter()
            .filter(|item| item.outcome == Outcome::Missed)
            .collect();
        if missed.is_empty() {
            text.push_str("  None reported.\n");
        }
        for mutation in missed {
            text.push_str(&format!(
                "  [{:06}] {}:{}:{}: {:?} -> {:?}\n    Diff: {}\n    Output: {}\n",
                mutation.id,
                mutation.file.display(),
                mutation.line,
                mutation.column,
                mutation.original,
                mutation.replacement,
                mutation.diff,
                mutation.logs.text
            ));
        }
        text.push_str("\nAll results (source paths are module-relative; artifact paths are report-relative):\n");
        for mutation in &self.run.mutations {
            text.push_str(&format!(
                "  [{:06}] {}: {}:{}:{} ({:.2}s)\n    Diff: {}\n    Output: {}\n",
                mutation.id,
                mutation.outcome,
                mutation.file.display(),
                mutation.line,
                mutation.column,
                mutation.duration_seconds,
                mutation.diff,
                mutation.logs.text
            ));
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
    use crate::mutation::{self, Scope};
    use clap::Parser;
    use std::os::unix::process::ExitStatusExt;

    fn read(report: &SavedReport) -> serde_json::Value {
        serde_json::from_slice(&fs::read(report.path().join("report.json")).unwrap()).unwrap()
    }

    #[test]
    fn checkpoints_results_and_preserves_previous_runs() -> Result<()> {
        let root = tempfile::tempdir()?;
        fs::write(
            root.path().join("code.go"),
            "package p\nvar a = true\nvar b = false\n",
        )?;
        let mutations =
            mutation::discover(root.path(), vec!["code.go".into()], &Scope::new(&[], &[])?)?;
        let cli = Cli::parse_from(["mutie"]);
        let mut report = SavedReport::create(root.path(), &cli)?;
        report.plan(2, 2, Duration::from_secs(10))?;
        for id in [2, 1] {
            let output = CommandOutput {
                status: std::process::ExitStatus::from_raw(0),
                timed_out: false,
                elapsed: Duration::from_millis(125),
                stdout: "{\"Action\":\"output\",\"Output\":\"PASS\\n\"}\n".into(),
                stderr: "diagnostic\n".into(),
            };
            if id == 2 {
                report.baseline(&output)?;
            }
            report.record(&MutationResult {
                id,
                mutation: mutations[id - 1].clone(),
                outcome: Outcome::Missed,
                output,
                error: None,
            })?;
            let checkpoint = read(&report);
            assert_eq!(checkpoint["status"], "running");
            assert_eq!(checkpoint["mutations"].as_array().unwrap().len(), 3 - id);
            assert_eq!(checkpoint["mutations"][0]["duration_seconds"], 0.125);
        }
        let snapshot = read(&report);
        assert_eq!(snapshot["mutations"][0]["id"], 1);
        assert_eq!(snapshot["mutations"][1]["id"], 2);
        assert_eq!(
            fs::read_to_string(report.path().join("mutations/000001.log"))?,
            "PASS\ndiagnostic\n"
        );
        assert_eq!(
            fs::read_to_string(report.path().join("mutations/000001.stderr.log"))?,
            "diagnostic\n"
        );
        assert!(
            fs::read_to_string(report.path().join("mutations/000001.diff"))?
                .contains("-var a = true\n+var a = false\n")
        );
        report.finish(Some(&anyhow::anyhow!("worker interrupted")))?;
        let finished = read(&report);
        assert_eq!(finished["status"], "failed");
        assert_eq!(finished["summary"]["missed"], 2);
        assert_eq!(finished["error"], "worker interrupted");
        assert!(finished["finished_at_unix_seconds"].is_number());
        let previous = fs::read(report.path().join("report.json"))?;
        let next = SavedReport::create(root.path(), &cli)?;
        assert_ne!(report.path(), next.path());
        assert_eq!(fs::read(report.path().join("report.json"))?, previous);
        let path = next.path().to_owned();
        drop(next);
        assert!(path.join("report.json").is_file());
        Ok(())
    }

    #[test]
    fn rejects_output_paths_that_would_hide_the_module_or_overwrite_a_file() -> Result<()> {
        let root = tempfile::tempdir()?;
        let mut cli = Cli::parse_from(["mutie"]);
        cli.output_dir = Some(root.path().to_owned());
        assert!(SavedReport::create(root.path(), &cli).is_err());
        let sentinel = root.path().join("existing");
        fs::write(&sentinel, "keep me")?;
        cli.output_dir = Some(sentinel.clone());
        assert!(SavedReport::create(root.path(), &cli).is_err());
        assert_eq!(fs::read_to_string(sentinel)?, "keep me");
        Ok(())
    }
}
