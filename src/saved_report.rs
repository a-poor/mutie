use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    time::{Instant, SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result, ensure};
use serde::Serialize;

use crate::{
    cancellation::{Cancellation, Interrupted},
    capture::Retention,
    command::CommandOutput,
    config::RunConfig,
    dismissals::DismissalSnapshot,
    language,
    mutation::{OPERATOR_CATALOG_VERSION, Operator},
    report::{
        ExecutionState, MutationResult, Outcome, PolicyEvaluation, RunPlan, RunStatus, Summary,
    },
    report_lock,
    rerun::RerunProvenance,
    score::{MutationScore, ScorePolicy},
    workspace_metrics::WorkspaceMetrics,
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
    policy: PolicyEvaluation,
    score: MutationScore,
    score_policy: ScorePolicy,
    baseline_command: Vec<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    compilation_command: Option<Vec<&'static str>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    rust_scope: Option<crate::rust::ExecutionScope>,
    #[serde(skip_serializing_if = "Option::is_none")]
    vitest_scope: Option<crate::vitest::ExecutionScope>,
    node_scope: Option<crate::node::ExecutionScope>,
    bun_scope: Option<crate::bun::ExecutionScope>,
    #[serde(skip_serializing_if = "Option::is_none")]
    python_scope: Option<crate::python::ExecutionScope>,
    mutation_command_prefix: Vec<&'static str>,
    discovery_command: Vec<&'static str>,
    include: Vec<String>,
    exclude: Vec<String>,
    requested_workers: usize,
    workers: Option<usize>,
    baseline_timeout_seconds: u64,
    mutation_timeout_seconds: Option<f64>,
    planned_mutations: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    rerun: Option<RerunProvenance>,
    workspace_metrics: WorkspaceMetrics,
}

#[derive(Serialize)]
struct Inventory {
    schema_version: u32,
    operator_catalog_version: u32,
    complete: bool,
    mutations: Vec<PlannedMutation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    dismissals: Option<DismissalSnapshot>,
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
    replacement: String,
    diff: String,
    test_packages: Vec<String>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    capture_error: Option<String>,
    logs: Logs,
}

/// Artifact paths are relative to the run directory so reports can be moved.
#[derive(Serialize)]
struct Logs {
    text: String,
    stdout: String,
    stderr: String,
    retention: LogRetention,
}

#[derive(Clone, Debug, serde::Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LogRetention {
    pub text: Retention,
    pub stdout: Retention,
    pub stderr: Retention,
}

impl LogRetention {
    pub fn validate(&self) -> Result<()> {
        self.text.validate()?;
        self.stdout.validate()?;
        self.stderr.validate()
    }

    pub fn notices(&self) -> String {
        [
            ("Text", &self.text),
            ("Stdout", &self.stdout),
            ("Stderr", &self.stderr),
        ]
        .into_iter()
        .filter(|(_, r)| r.omitted_bytes > 0)
        .map(|(name, r)| format!("{name}: {}\n", r.notice()))
        .collect()
    }
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
    // Hold through the final multi-file flush, not merely until status changes.
    _lock: report_lock::Lock,
    started: Instant,
    run: RunReport,
    inventory: Inventory,
    outcomes: Outcomes,
}

impl SavedReport {
    pub fn python_scope(&mut self, scope: &crate::python::ExecutionScope) -> Result<()> {
        self.run.python_scope = Some(scope.clone());
        self.flush()
    }
    pub fn bun_scope(&mut self, scope: &crate::bun::ExecutionScope) -> Result<()> {
        self.run.bun_scope = Some(scope.clone());
        self.flush()
    }
    pub fn node_scope(&mut self, scope: &crate::node::ExecutionScope) -> Result<()> {
        self.run.baseline_command = scope.test_command();
        self.run.mutation_command_prefix = scope.test_command();
        self.run.node_scope = Some(scope.clone());
        self.flush()
    }

    pub fn vitest_scope(&mut self, scope: &crate::vitest::ExecutionScope) -> Result<()> {
        self.run.vitest_scope = Some(scope.clone());
        self.flush()
    }

    pub fn rust_scope(&mut self, scope: &crate::rust::ExecutionScope) -> Result<()> {
        self.run.baseline_command = scope.test_command();
        self.run.mutation_command_prefix = scope.test_command();
        self.run.rust_scope = Some(scope.clone());
        self.flush()
    }

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
        let lock =
            report_lock::acquire(directory.path(), true)?.context("new report has no lock")?;
        fs::create_dir(directory.path().join("diff"))?;
        fs::create_dir(directory.path().join("logs"))?;
        let mut report = Self {
            directory: directory.path().to_owned(),
            _lock: lock,
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
                policy: PolicyEvaluation::pending(config.fail_on_missed),
                score: MutationScore::calculate(
                    config.score_scope(),
                    RunStatus::Running,
                    false,
                    [],
                ),
                score_policy: ScorePolicy::Disabled,
                baseline_command: language::test_command(config.lang),
                compilation_command: (config.lang == crate::cli::Lang::Rust).then(|| {
                    language::test_command(config.lang)
                        .into_iter()
                        .chain(["--no-run"])
                        .collect()
                }),
                rust_scope: None,
                vitest_scope: None,
                node_scope: None,
                bun_scope: None,
                python_scope: None,
                mutation_command_prefix: language::test_command(config.lang)
                    .into_iter()
                    .filter(|arg| {
                        if config.lang == crate::cli::Lang::Rust {
                            *arg != "--tests"
                        } else {
                            *arg != "./..."
                        }
                    })
                    .collect(),
                discovery_command: language::discovery_command(config.lang, config.test_scope),
                include: config.include.clone(),
                exclude: config.exclude.clone(),
                requested_workers: config.jobs.get(),
                workers: None,
                baseline_timeout_seconds: config.baseline_timeout.get(),
                mutation_timeout_seconds: config.timeout.map(|value| value.get() as f64),
                planned_mutations: None,
                rerun: None,
                workspace_metrics: WorkspaceMetrics::default(),
            },
            inventory: Inventory {
                schema_version: SCHEMA_VERSION,
                operator_catalog_version: OPERATOR_CATALOG_VERSION,
                complete: false,
                mutations: Vec::new(),
                dismissals: None,
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

    pub fn policy(&self) -> &PolicyEvaluation {
        &self.run.policy
    }

    pub fn score(&self) -> &MutationScore {
        &self.run.score
    }

    pub fn score_policy(&self) -> &ScorePolicy {
        &self.run.score_policy
    }

    pub fn workspace_metrics(&mut self, metrics: &WorkspaceMetrics) {
        self.run.workspace_metrics = metrics.clone();
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
            } else if output.capture_error.is_some() {
                "failed"
            } else if output.timed_out {
                "timeout"
            } else if crate::command::classify(output).ok() == Some(Outcome::Missed) {
                "passed"
            } else {
                "failed"
            },
            duration_seconds: output.elapsed.as_secs_f64(),
            exit_code: output.status.code(),
            capture_error: output.capture_error.clone(),
            logs: self.logs(&format!("logs/{name}"), output)?,
        };
        if name == "baseline" {
            self.outcomes.baseline = Some(record);
        } else {
            self.outcomes.discovery = Some(record);
        }
        self.flush()
    }

    pub fn plan(&mut self, plan: &RunPlan<'_>, cancellation: &Cancellation) -> Result<()> {
        let RunPlan {
            mutations,
            test_packages,
            workers,
            timeout,
            rerun,
            dismissals,
        } = plan;
        ensure!(
            mutations.len() == test_packages.len()
                && test_packages.iter().all(|p| {
                    if matches!(
                        self.run.effective_config.lang,
                        crate::cli::Lang::Vitest
                            | crate::cli::Lang::Node
                            | crate::cli::Lang::Bun
                            | crate::cli::Lang::Python
                    ) {
                        p.is_empty()
                    } else {
                        !p.is_empty()
                    }
                }),
            "every mutation must have a test package selection"
        );
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
                replacement: mutation.replacement.to_string(),
                diff,
                test_packages: test_packages[index].clone(),
            });
        }
        self.inventory.mutations = inventory;
        self.inventory.dismissals = Some((*dismissals).clone());
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
        self.run.workers = Some(*workers);
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
        self.run.policy =
            if self.run.effective_config.fail_on_missed && status == RunStatus::Completed {
                let dismissed = self.dismissed_missed();
                PolicyEvaluation::completed(self.outcomes.summary.missed - dismissed, dismissed)
            } else {
                PolicyEvaluation::pending(self.run.effective_config.fail_on_missed)
            };
        self.flush()
    }

    fn dismissed_missed(&self) -> usize {
        self.outcomes
            .mutations
            .iter()
            .filter(|execution| {
                execution.state == ExecutionState::Completed
                    && execution.outcome == Some(Outcome::Missed)
                    && self
                        .inventory
                        .dismissals
                        .as_ref()
                        .is_some_and(|dismissals| dismissals.dismissal(execution.id).is_some())
            })
            .count()
    }

    fn logs(&self, prefix: &str, output: &CommandOutput) -> Result<Logs> {
        let logs = Logs {
            text: format!("{prefix}.log"),
            stdout: format!(
                "{prefix}.stdout.{}",
                if self.run.effective_config.lang != crate::cli::Lang::Go {
                    "log"
                } else {
                    "jsonl"
                }
            ),
            stderr: format!("{prefix}.stderr.log"),
            retention: LogRetention {
                text: output.text.retention()?,
                stdout: output.stdout.retention()?,
                stderr: output.stderr.retention()?,
            },
        };
        output.text.save(&self.directory.join(&logs.text))?;
        output.stdout.save(&self.directory.join(&logs.stdout))?;
        output.stderr.save(&self.directory.join(&logs.stderr))?;
        Ok(logs)
    }

    fn flush(&mut self) -> Result<()> {
        self.run.score = MutationScore::calculate(
            self.run.effective_config.score_scope(),
            self.run.status,
            self.inventory.complete,
            self.outcomes.mutations.iter().map(|execution| {
                (
                    execution.outcome,
                    self.inventory
                        .dismissals
                        .as_ref()
                        .is_some_and(|d| d.dismissal(execution.id).is_some()),
                )
            }),
        );
        self.run.score_policy =
            ScorePolicy::evaluate(self.run.effective_config.min_score, &self.run.score);
        self.run.duration_seconds = self.started.elapsed().as_secs_f64();
        self.run.checkpoint += 1;
        self.outcomes.checkpoint = self.run.checkpoint;
        // Each snapshot is atomic, not the whole set. Matching checkpoint numbers
        // let readers detect a crash between publication of outcomes and metadata.
        self.write("outcomes.json", &serde_json::to_vec_pretty(&self.outcomes)?)?;
        self.write("run.json", &serde_json::to_vec_pretty(&self.run)?)?;
        self.write_derived("summary.txt", self.summary().as_bytes())?;
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
            self.write_derived(&format!("{name}.txt"), text.as_bytes())?;
        }
        Ok(())
    }

    /// Compare against the published file, not a cache: missing or edited output
    /// must be repaired even when the newly generated contents are unchanged.
    fn write_derived(&self, name: &str, contents: &[u8]) -> Result<()> {
        let path = self.directory.join(name);
        match fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.is_file() && metadata.len() == contents.len() as u64 => {
                if fs::read(&path).with_context(|| format!("reading {}", path.display()))?
                    == contents
                {
                    return Ok(());
                }
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(error).with_context(|| format!("inspecting {}", path.display()));
            }
        }
        self.write(name, contents)
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
        text.push_str(&format!("{}\n", self.run.workspace_metrics.summary()));
        text.push_str(&format!("{}\n", self.run.policy));
        text.push_str(&self.run.score.details());
        text.push_str(&format!("{}\n", self.run.score_policy));
        if self.run.effective_config.lang == crate::cli::Lang::Python {
            text.push_str("Runner: CPython 3.14.7 / pytest 9.0.2\nFull suite; serial; pristine test inventory; owned workspaces\n");
        } else if self.run.effective_config.lang == crate::cli::Lang::Bun {
            text.push_str("Runner: Bun 1.3.11 test runner (JavaScript/TypeScript)\nFull suite; serial; fresh owned command copies; bounded evidence\n");
        } else if self.run.effective_config.lang == crate::cli::Lang::Node {
            let typescript = self
                .run
                .node_scope
                .as_ref()
                .is_some_and(|s| s.typescript.is_some());
            text.push_str(if typescript {
                "Runner: Node test runner (JavaScript/native TypeScript)\nType stripping only; no type checking or tsconfig transforms\n"
            } else {
                "Runner: Node test runner (JavaScript)\n"
            });
            text.push_str("Full suite; serial files; fresh command copies\n");
            if let Some(scope) = &self.run.node_scope {
                text.push_str(&format!(
                    "Package: {}\nNode: {}\nManifest: {}\n",
                    scope.package, scope.node, scope.manifest_fingerprint
                ));
            }
        } else if self.run.effective_config.lang == crate::cli::Lang::Vitest {
            text.push_str(
                "Runner: Vitest (JavaScript/TypeScript)\nFull suite; fresh command copies\n",
            );
            if let Some(scope) = &self.run.vitest_scope {
                text.push_str(&format!("Package: {}\nNode: {}; pnpm: {}; Vitest: {}; Vite: {}\nManifest: {}\nLockfile: {}\n", scope.package, scope.node, scope.pnpm, scope.vitest, scope.vite, scope.manifest_fingerprint, scope.lockfile_fingerprint));
            }
        } else if self.run.effective_config.lang == crate::cli::Lang::Rust {
            text.push_str("Language: Rust\nMutation and baseline test scope: unit and integration tests, default features; doctests and ignored tests excluded\n");
            text.push_str(match self.run.effective_config.rust_build_cache {
                crate::cli::RustBuildCache::Fresh => "Build artifacts: fresh owned directories per baseline/mutation\n",
                crate::cli::RustBuildCache::Worker => "Build artifacts: fresh baseline; worker-local dependencies, package cleaned before each mutation build\n",
            });
            if let Some(scope) = &self.run.rust_scope {
                text.push_str(&format!(
                    "Cargo package: {}\nLockfile: {}\n",
                    scope.package, scope.lockfile_fingerprint
                ));
                if let Some(workspace) = &scope.workspace {
                    text.push_str(&format!(
                        "Mutation members: {}\nTests: all workspace members\n",
                        workspace.selected.join(", ")
                    ));
                    for member in &workspace.members {
                        text.push_str(&format!(
                            "Workspace member: {} | path: {} | manifest: {}\n",
                            member.name,
                            member.directory.display(),
                            member.manifest_fingerprint
                        ));
                    }
                }
                for target in &scope.targets {
                    text.push_str(&format!(
                        "Cargo {}: {} | source: {} | edition: {}\n",
                        target.kind.label(),
                        target.name,
                        target.source.display(),
                        target.edition
                    ));
                }
                for exclusion in &scope.discovery_exclusions {
                    text.push_str(&format!("Discovery excluded {exclusion}\n"));
                }
            }
        } else {
            text.push_str(&format!(
                "Mutation test scope: {} (baseline: full module)\n",
                match self.run.effective_config.test_scope {
                    crate::cli::TestScope::Full => "full",
                    crate::cli::TestScope::Affected => "affected",
                }
            ));
        }
        if let Some(root) = &self.run.workspace_metrics.temporary_root {
            text.push_str(&format!("Workspace temporary root: {}\n", root.display()));
        }
        if let Some(rerun) = &self.run.rerun {
            text.push_str(&format!(
                "Rerun: {} from {} (checkpoint {})\n",
                rerun.filter.description(),
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
        if let Some(dismissals) = &self.inventory.dismissals {
            let dismissed_missed = self.dismissed_missed();
            text.push_str(&format!(
                "Dismissals (saved at discovery): {} matched; {} need review\nMissed review: {} undismissed; {} dismissed (outcomes unchanged)\n",
                dismissals.matched.len(), dismissals.needs_review.len(),
                self.outcomes.summary.missed - dismissed_missed, dismissed_missed
            ));
            for issue in &dismissals.needs_review {
                text.push_str(&format!(
                    "  Needs review ({}): {}",
                    issue.status.label(),
                    issue.dismissal.description()
                ));
            }
        } else {
            text.push_str("Dismissal review: not recorded\n");
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
        text.push('\n');
        text.push_str(&crate::operator_summary::summarize(
            self.inventory.complete,
            self.outcomes.mutations.iter().map(|execution| {
                (
                    self.inventory.mutations[execution.id - 1].operator,
                    execution.state,
                    execution.outcome,
                    execution.duration_seconds,
                )
            }),
        ));
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
                self.append_dismissal(&mut text, execution.id);
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
            self.append_dismissal(&mut text, execution.id);
            if let Some(error) = &execution.error {
                text.push_str(&format!("    Error: {error}\n"));
            }
        }
        text
    }

    fn append_dismissal(&self, text: &mut String, id: usize) {
        if let Some(dismissal) = self
            .inventory
            .dismissals
            .as_ref()
            .and_then(|d| d.dismissal(id))
        {
            text.push_str("    Review: dismissed (saved at discovery)\n");
            for line in dismissal.description().lines() {
                text.push_str(&format!("    {line}\n"));
            }
        }
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
    use std::time::Duration;

    fn config(root: &Path) -> Result<RunConfig> {
        fs::write(root.join("go.mod"), "module example.com/test\n")?;
        RunConfig::resolve(Cli::parse_from([
            std::ffi::OsStr::new("mutie"),
            std::ffi::OsStr::new("run"),
            root.as_os_str(),
        ]))
    }

    fn read(report: &SavedReport, name: &str) -> serde_json::Value {
        serde_json::from_slice(&fs::read(report.path().join(name)).unwrap()).unwrap()
    }

    fn output() -> CommandOutput {
        CommandOutput::prepare(
            crate::process::CommandOutput {
                pid: 1,
                group_remaining: false,
                status: std::process::ExitStatus::from_raw(0),
                timed_out: false,
                interrupted: false,
                capture_error: None,
                elapsed: Duration::from_millis(125),
                stdout: crate::capture::Capture::from_bytes(
                    b"{\"Action\":\"output\",\"Output\":\"PASS\\n\"}\n",
                ),
                stderr: crate::capture::Capture::from_bytes(b"diagnostic\n"),
            },
            None,
        )
        .unwrap()
    }

    #[test]
    fn unchanged_lists_keep_their_files_while_authoritative_checkpoints_advance() -> Result<()> {
        use std::os::unix::fs::MetadataExt;

        let root = tempfile::tempdir()?;
        let mut report = SavedReport::create(&config(root.path())?)?;
        // Open handles prevent inode reuse from masking a replacement.
        let list = fs::File::open(report.path().join("missed.txt"))?;
        let run = fs::File::open(report.path().join("run.json"))?;
        let outcomes = fs::File::open(report.path().join("outcomes.json"))?;
        let checkpoint = read(&report, "run.json")["checkpoint"].as_u64().unwrap();
        report.flush()?;
        assert_eq!(
            list.metadata()?.ino(),
            fs::metadata(report.path().join("missed.txt"))?.ino()
        );
        assert_ne!(
            run.metadata()?.ino(),
            fs::metadata(report.path().join("run.json"))?.ino()
        );
        assert_ne!(
            outcomes.metadata()?.ino(),
            fs::metadata(report.path().join("outcomes.json"))?.ino()
        );
        assert_eq!(read(&report, "run.json")["checkpoint"], checkpoint + 1);
        assert_eq!(
            read(&report, "run.json")["checkpoint"],
            read(&report, "outcomes.json")["checkpoint"]
        );
        fs::remove_file(report.path().join("missed.txt"))?;
        report.flush()?;
        assert_eq!(fs::read(report.path().join("missed.txt"))?, b"");
        Ok(())
    }

    #[test]
    fn derived_files_repair_edits_and_links_and_retry_failed_publication() -> Result<()> {
        use std::os::unix::fs::{MetadataExt, symlink};

        let root = tempfile::tempdir()?;
        let report = SavedReport::create(&config(root.path())?)?;
        let path = report.path().join("missed.txt");
        report.write_derived("missed.txt", b"first")?;
        let original = fs::File::open(&path)?;
        report.write_derived("missed.txt", b"first")?;
        assert_eq!(original.metadata()?.ino(), fs::metadata(&path)?.ino());
        report.write_derived("missed.txt", b"other")?;
        assert_ne!(original.metadata()?.ino(), fs::metadata(&path)?.ino());
        fs::write(&path, b"edits")?;
        report.write_derived("missed.txt", b"other")?;
        assert_eq!(fs::read(&path)?, b"other");
        fs::remove_file(&path)?;
        let external = root.path().join("external");
        fs::write(&external, b"other")?;
        symlink(&external, &path)?;
        report.write_derived("missed.txt", b"other")?;
        assert!(!fs::symlink_metadata(&path)?.file_type().is_symlink());
        assert_eq!(fs::read(&external)?, b"other");
        fs::remove_file(&path)?;
        fs::create_dir(&path)?;
        assert!(report.write_derived("missed.txt", b"other").is_err());
        fs::remove_dir(&path)?;
        report.write_derived("missed.txt", b"other")?;
        assert_eq!(fs::read(&path)?, b"other");
        Ok(())
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
            &RunPlan {
                mutations: &mutations,
                test_packages: &vec![vec!["./...".into()]; mutations.len()],
                workers: 2,
                timeout: Duration::from_secs(10),
                rerun: None,
                dismissals: &DismissalSnapshot::default(),
            },
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
        interrupted.mark_interrupted();
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
        let summary = fs::read_to_string(report.path().join("summary.txt"))?;
        let loaded = crate::report_reader::LoadedReport::load(report.path())?;
        assert!(summary.contains(&loaded.operator_summary()));
        assert!(summary.contains("boolean_flip\n  Planned: 3 | Completed: 2"));
        assert!(summary.contains("Killed: 0 | Missed: 2 | Timeout: 0"));
        assert!(summary.contains("Time: 0.38s (3 recorded, 0 missing)"));
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
                    &RunPlan {
                        mutations: &mutations,
                        test_packages: &vec![vec!["./...".into()]; mutations.len()],
                        workers: 1,
                        timeout: Duration::from_secs(10),
                        rerun: None,
                        dismissals: &DismissalSnapshot::default(),
                    },
                    &Cancellation::default()
                )
                .is_err()
        );
        assert_eq!(read(&report, "mutants.json")["complete"], false);
        assert!(report.started(1, 1).is_err());
        report.finish(Some(&anyhow::anyhow!("report write failed")))?;
        assert_eq!(read(&report, "run.json")["status"], "failed");
        assert!(
            fs::read_to_string(report.path().join("summary.txt"))?
                .contains("Not available: inventory was not published.")
        );
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
