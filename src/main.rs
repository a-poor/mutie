mod browser;
mod bun;
mod cancellation;
mod capture;
mod cli;
mod command;
mod config;
mod coordinator;
mod dismissals;
mod go;
mod identity;
mod javascript;
mod language;
mod mutation;
mod node;
mod operator_summary;
mod process;
mod python;
mod report;
mod report_lock;
mod report_reader;
mod report_retention;
mod rerun;
mod rust;
mod saved_report;
mod score;
mod status;
mod vitest;
mod workspace;
mod workspace_metrics;

use std::{collections::VecDeque, path::PathBuf, process::ExitCode, sync::Arc, time::Duration};

use anyhow::{Context, Result, bail, ensure};
use clap::Parser;
use tokio::{sync::mpsc, task::JoinSet};

use cancellation::{Cancellation, Interrupted};
use cli::{Cli, Lang, TestScope, UiMode};
use config::RunConfig;
use coordinator::{Coordinator, Phase, RunEvent, WorkerPhase, WorkerProgress};
use mutation::{Mutation, Scope};
use report::{ExecutionState, MutationResult, Outcome, PolicyEvaluation, RunPlan};
use workspace::Workspace;
use workspace_metrics::{WorkspaceKind, WorkspaceMeasurement};

#[tokio::main]
async fn main() -> Result<ExitCode> {
    let mut cli = Cli::parse();
    if let Some(command) = cli.command.take() {
        let cancellation = Cancellation::default();
        let _signals = cancellation
            .listen()
            .context("installing cancellation handlers")?;
        let result = match command {
            cli::Command::PythonList { path, python, json } => {
                python::list(&path, &python, json, &cancellation).await
            }
            cli::Command::PythonCheck {
                path,
                python,
                timeout,
                max_capture_bytes,
                json,
            } => {
                python::check(
                    &path,
                    &python,
                    Duration::from_secs(timeout.get()),
                    max_capture_bytes,
                    json,
                    &cancellation,
                )
                .await
            }
            cli::Command::BunList {
                path,
                include,
                exclude,
                json,
            } => bun::list(&path, &include, &exclude, json, &cancellation),
            cli::Command::BunCheck {
                json,
                path,
                timeout,
                max_capture_bytes,
            } => {
                bun::check(
                    &path,
                    Duration::from_secs(timeout.get()),
                    max_capture_bytes,
                    json,
                    &cancellation,
                )
                .await
            }
            cli::Command::VitestCheck {
                path,
                timeout,
                max_capture_bytes,
            } => {
                vitest::check(
                    &path,
                    Duration::from_secs(timeout.get()),
                    max_capture_bytes,
                    &cancellation,
                )
                .await
            }
            cli::Command::VitestList {
                path,
                include,
                exclude,
            } => vitest::list(&path, &include, &exclude, &cancellation).await,
            cli::Command::Runs { directory } => tokio::task::spawn_blocking(move || {
                report_retention::run(&directory, None, false, &cancellation)
            })
            .await
            .context("report listing panicked")?,
            cli::Command::Prune {
                directory,
                keep_last,
                older_than,
                apply,
            } => {
                let policy = report_retention::Policy::new(keep_last, older_than)?;
                tokio::task::spawn_blocking(move || {
                    report_retention::run(&directory, Some(policy), apply, &cancellation)
                })
                .await
                .context("report pruning panicked")?
            }
            cli::Command::Report { run_dir, module } => tokio::task::spawn_blocking(move || {
                browser::run(&run_dir, module.as_deref(), &cancellation)
            })
            .await
            .context("report browser panicked")?,
            cli::Command::Dismiss {
                module,
                lang,
                python,
                action,
            } => {
                if matches!(action, cli::DismissAction::Check) {
                    cli.path = module;
                    cli.lang = lang;
                    cli.python = python;
                    cli.list = true;
                    let config = RunConfig::resolve(cli)?;
                    dismissals::check(&config, &cancellation).await
                } else {
                    dismissals::manage(&module, lang.unwrap_or(Lang::Auto), action)
                }
            }
        };
        return result.map(|()| ExitCode::SUCCESS);
    }
    let config = RunConfig::resolve(cli)?;
    let scope = Scope::new(&config.include, &config.exclude)?;
    let rerun = config
        .rerun
        .as_ref()
        .map(rerun::RerunSelection::load)
        .transpose()?;
    if let Some(selection) = &rerun {
        selection.validate_language(config.lang)?;
    }
    let interactive = status::select(&config)?;
    let cancellation = Cancellation::default();
    let _signals = cancellation
        .listen()
        .context("installing cancellation handlers")?;
    let (mut coordinator, updates) = Coordinator::new(&config, interactive)?;
    let mut terminal = if interactive {
        match status::LiveTerminal::start(config.jobs.get(), cancellation.clone()) {
            Ok(terminal) => Some(terminal),
            Err(error) if config.ui == UiMode::Auto => {
                eprintln!("Live status unavailable: {error:#}; using plain output.");
                coordinator.use_plain_output();
                None
            }
            Err(error) => {
                return finish_run(coordinator, Err(error));
            }
        }
    } else {
        None
    };
    let engine_cancellation = cancellation.clone();
    let engine = tokio::spawn(async move {
        // Runner futures carry capture state; keep nested dispatch off worker stacks.
        let result = Box::pin(execute(
            &config,
            scope,
            rerun,
            &mut coordinator,
            &engine_cancellation,
        ))
        .await;
        (coordinator, result)
    });
    let (coordinator, result) =
        status::drive(engine, updates, &mut terminal, &cancellation).await?;
    finish_run(coordinator, result)
}

fn finish_run(mut coordinator: Coordinator, result: Result<()>) -> Result<ExitCode> {
    let result = coordinator.finish(result);
    status::print_summary(&coordinator);
    if coordinator.report_finished
        && let Some(score) = coordinator.score()
    {
        print!("{}", score.details());
    }
    if coordinator.report_finished
        && let Some(policy) = coordinator.policy()
        && !matches!(policy, PolicyEvaluation::Disabled)
    {
        println!("{policy}");
    }
    if coordinator.report_finished
        && let Some(policy) = coordinator.score_policy()
        && !matches!(policy, score::ScorePolicy::Disabled)
    {
        println!("{policy}");
    }
    // Persistence and execution errors take precedence over findings. Returning
    // an ExitCode lets all terminal, report-lock, and runtime guards drop normally.
    result?;
    Ok(
        if coordinator.policy().is_some_and(PolicyEvaluation::failed)
            || coordinator
                .score_policy()
                .is_some_and(score::ScorePolicy::failed)
        {
            ExitCode::from(3)
        } else {
            ExitCode::SUCCESS
        },
    )
}

async fn create_workspace(
    source: PathBuf,
    excluded: Vec<PathBuf>,
    cancellation: Cancellation,
) -> Result<Workspace> {
    tokio::task::spawn_blocking(move || Workspace::create(&source, &excluded, &cancellation))
        .await
        .context("workspace preparation panicked")?
}

async fn measured_workspace(
    source: PathBuf,
    excluded: Vec<PathBuf>,
    kind: WorkspaceKind,
    coordinator: &mut Coordinator,
    cancellation: &Cancellation,
) -> Result<Workspace> {
    let task_cancellation = cancellation.clone();
    let (result, measurement) = tokio::task::spawn_blocking(move || {
        Workspace::create_measured(&source, &excluded, &task_cancellation)
    })
    .await
    .context("workspace preparation panicked")?;
    coordinator.emit(
        RunEvent::Workspace {
            kind,
            measurement: &measurement,
        },
        cancellation,
    )?;
    result
}

async fn execute(
    config: &RunConfig,
    scope: Scope,
    rerun: Option<rerun::RerunSelection>,
    coordinator: &mut Coordinator,
    cancellation: &Cancellation,
) -> Result<()> {
    let root = &config.module_root;
    let baseline_limit = Duration::from_secs(config.baseline_timeout.get());
    let mut rust_project = (config.lang == Lang::Rust)
        .then(|| rust::Project::load(root))
        .transpose()?;
    let bun_project = (config.lang == Lang::Bun)
        .then(|| bun::project::Project::load(root, cancellation))
        .transpose()?;
    let node_project = (config.lang == Lang::Node)
        .then(|| node::project::Project::load(root, cancellation))
        .transpose()?;
    let python_project = if config.lang == Lang::Python {
        Some(
            python::project::Project::load(
                root,
                config
                    .python
                    .as_deref()
                    .context("missing Python launcher")?,
                cancellation,
            )
            .await?,
        )
    } else {
        None
    };
    coordinator.line(format!("Snapshotting {}", root.display()));
    let mut excluded = vec![root.join(".mutie"), config.output_dir.clone()];
    excluded.extend(
        config
            .rerun
            .iter()
            .map(|request| request.source_run.clone()),
    );
    if let Some(project) = &rust_project {
        excluded.extend(project.excluded.clone());
    }
    let snapshot = Arc::new(
        measured_workspace(
            root.to_owned(),
            excluded,
            WorkspaceKind::Snapshot,
            coordinator,
            cancellation,
        )
        .await?,
    );
    if let Some(original) = &node_project
        && original.default_discovery
    {
        let copied = node::project::Project::load(&snapshot.path(), cancellation)?;
        anyhow::ensure!(
            original.tests == copied.tests,
            "Node default test selection changed during snapshotting; an excluded output/report directory must not contain tests"
        );
    }
    if let Some(original) = &bun_project {
        let copied = bun::project::Project::load(&snapshot.path(), cancellation)?;
        anyhow::ensure!(
            original.tests == copied.tests
                && original
                    .files
                    .iter()
                    .filter(|(p, _)| p.starts_with("src/")
                        || p.starts_with("test/")
                        || p.starts_with("tests/")
                        || matches!(
                            p.as_str(),
                            "package.json" | "tsconfig.json" | "bun.lock" | "bun.lockb"
                        ))
                    .all(|(p, h)| copied.files.get(p) == Some(h)),
            "Bun source, config or test inventory changed during snapshotting; report exclusions cannot hide executable inputs"
        );
    }
    if let Some(original) = &python_project {
        let copied = python::project::Project::load(
            &snapshot.path(),
            config
                .python
                .as_deref()
                .context("missing Python launcher")?,
            cancellation,
        )
        .await?;
        original.validate_snapshot(&copied)?;
    }
    if config.lang == Lang::Python && config.list {
        let project = python::project::Project::load(
            &snapshot.path(),
            config
                .python
                .as_deref()
                .context("missing Python launcher")?,
            cancellation,
        )
        .await?;
        let identity = python::discovery_scope(
            &project,
            config
                .python
                .as_deref()
                .context("missing Python launcher")?,
            baseline_limit,
            config
                .max_capture_bytes
                .context("Python requires bounded captures")?,
            cancellation,
        )
        .await?;
        if let Some(selection) = &rerun {
            selection.validate_python_scope(&identity)?;
        }
    }
    let python_scope = if config.lang == Lang::Python && !config.list {
        let (identity, mut collection) = python::execution_scope(
            &snapshot.path(),
            config
                .python
                .as_deref()
                .context("missing Python launcher")?,
            baseline_limit,
            config
                .max_capture_bytes
                .context("Python requires bounded captures")?,
            cancellation,
        )
        .await?;
        collection.raw.stdout.set_limit(config.max_log_bytes);
        collection.raw.stderr.set_limit(config.max_log_bytes);
        collection.text.set_limit(config.max_log_bytes);
        coordinator.emit(
            RunEvent::Command {
                phase: Phase::Discovery,
                output: &collection,
            },
            cancellation,
        )?;
        ensure!(
            collection.classify()? == Outcome::Missed,
            "Python pristine collection did not pass"
        );
        identity.validate_ready()?;
        if let Some(selection) = &rerun {
            selection.validate_python_scope(&identity)?;
        }
        coordinator.emit(RunEvent::PythonScope(&identity), cancellation)?;
        Some(identity)
    } else {
        None
    };
    let dismissal_store = if config.list {
        None
    } else {
        Some(dismissals::Store::load(&snapshot.path())?)
    };
    if rust_project.is_some() {
        // Scope and lockfile identity must describe the captured input, not a
        // manifest that could have changed while the snapshot was being copied.
        rust_project = Some(rust::Project::load(&snapshot.path())?);
    }
    let vitest_scope = if config.lang == Lang::Vitest {
        let identity = vitest::execution_scope(&snapshot.path(), cancellation).await?;
        coordinator.emit(RunEvent::VitestScope(&identity), cancellation)?;
        if let Some(selection) = &rerun {
            selection.validate_vitest_scope(&identity)?;
        }
        Some(identity)
    } else {
        None
    };
    let bun_scope = if config.lang == Lang::Bun {
        let identity = bun::execution_scope(
            &snapshot.path(),
            baseline_limit,
            config
                .max_capture_bytes
                .context("Bun requires bounded captures")?,
            cancellation,
        )
        .await?;
        coordinator.emit(RunEvent::BunScope(&identity), cancellation)?;
        if let Some(selection) = &rerun {
            selection.validate_bun_scope(&identity)?;
        }
        Some(identity)
    } else {
        None
    };
    let node_scope = if config.lang == Lang::Node {
        let identity = node::execution_scope(&snapshot.path(), cancellation).await?;
        coordinator.emit(RunEvent::NodeScope(&identity), cancellation)?;
        if let Some(selection) = &rerun {
            selection.validate_node_scope(&identity)?;
        }
        Some(identity)
    } else {
        None
    };
    let baseline_workspace = measured_workspace(
        snapshot.path(),
        vec![],
        WorkspaceKind::BaselineSetup,
        coordinator,
        cancellation,
    )
    .await?;
    if let Some(project) = &mut rust_project {
        coordinator.emit(RunEvent::Phase(Phase::Discovery), cancellation)?;
        let output = rust::discover(
            &baseline_workspace.path(),
            baseline_limit,
            config.max_log_bytes,
            config.max_capture_bytes,
            cancellation,
        )
        .await?;
        coordinator.emit(
            RunEvent::Command {
                phase: Phase::Discovery,
                output: &output,
            },
            cancellation,
        )?;
        if output.interrupted {
            return Err(Interrupted.into());
        }
        project.validate_metadata(&baseline_workspace.path(), &rust::metadata(&output)?)?;
        rust::versions(
            &mut project.scope,
            &baseline_workspace.path(),
            baseline_limit,
            cancellation,
        )
        .await?;
        coordinator.emit(RunEvent::RustScope(&project.scope), cancellation)?;
        if let Some(selection) = &rerun {
            selection.validate_rust_scope(&project.scope)?;
        }
    }
    let mut timeout = config
        .timeout
        .map(|seconds| Duration::from_secs(seconds.get()));
    if !config.list {
        coordinator.emit(RunEvent::Phase(Phase::Baseline), cancellation)?;
        let baseline = Box::pin(language::test(
            config.lang,
            &baseline_workspace.path(),
            &["./...".into()],
            baseline_limit,
            config.max_log_bytes,
            config.max_capture_bytes,
            None,
            vitest_scope.as_ref(),
            node_scope.as_ref(),
            bun_scope.as_ref(),
            python_scope.as_ref(),
            cancellation,
            |_| async {},
        ))
        .await?;
        coordinator.emit(
            RunEvent::Command {
                phase: Phase::Baseline,
                output: &baseline,
            },
            cancellation,
        )?;
        if baseline.interrupted {
            return Err(Interrupted.into());
        }
        if let Some(error) = &baseline.capture_error {
            bail!(
                "baseline: {error}; no mutations were run\n{}",
                command::text(&baseline)?
            );
        }
        if baseline.timed_out {
            bail!(
                "baseline timed out after {:.1}s; no mutations were run\n{}",
                baseline_limit.as_secs_f64(),
                command::text(&baseline)?
            );
        }
        if command::classify(&baseline)
            .context("baseline evidence invalid; no mutations were run")?
            != Outcome::Missed
        {
            bail!(
                "baseline tests failed; no mutations were run\n{}",
                command::text(&baseline)?
            );
        }
        timeout.get_or_insert_with(|| (baseline.elapsed * 5).max(Duration::from_secs(10)));
    }
    cancellation.check()?;
    coordinator.emit(RunEvent::Phase(Phase::Discovery), cancellation)?;
    // Package discovery must see the pristine input, not baseline side effects.
    let baseline_workspace = if config.list {
        baseline_workspace
    } else {
        let source = snapshot.path();
        let reset_cancellation = cancellation.clone();
        let (result, measurement, baseline_workspace) = tokio::task::spawn_blocking(move || {
            let (result, measurement) =
                baseline_workspace.reset_measured(&source, &reset_cancellation);
            (result, measurement, baseline_workspace)
        })
        .await
        .context("baseline workspace reset panicked")?;
        coordinator.emit(
            RunEvent::Workspace {
                kind: WorkspaceKind::DiscoveryReset,
                measurement: &measurement,
            },
            cancellation,
        )?;
        result?;
        baseline_workspace
    };
    let (mutations, graph) = if let Some(project) = &mut rust_project {
        drop(baseline_workspace);
        let source = snapshot.path();
        let entries: Vec<_> = project
            .scope
            .targets
            .iter()
            .map(|target| target.source.clone())
            .collect();
        let discovery_cancellation = cancellation.clone();
        let found = tokio::task::spawn_blocking(move || {
            rust::discovery::discover(&source, &entries, &scope, &discovery_cancellation)
        })
        .await
        .context("Rust discovery panicked")??;
        project.scope.discovery_exclusions = found.exclusions;
        for exclusion in &project.scope.discovery_exclusions {
            coordinator.line(format!("Rust discovery excluded {exclusion}"));
        }
        coordinator.emit(RunEvent::RustScope(&project.scope), cancellation)?;
        (found.mutations, None)
    } else if config.lang == Lang::Python {
        drop(baseline_workspace);
        let project = python::project::Project::load(
            &snapshot.path(),
            config
                .python
                .as_deref()
                .context("missing Python launcher")?,
            cancellation,
        )
        .await?;
        (project.discover(&scope)?, None)
    } else if matches!(config.lang, Lang::Vitest | Lang::Node | Lang::Bun) {
        drop(baseline_workspace);
        let found = if config.lang == Lang::Bun {
            javascript::discover_bun(&snapshot.path(), &scope, cancellation)?
        } else {
            javascript::discover(&snapshot.path(), &scope, cancellation)?
        };
        for exclusion in &found.exclusions {
            coordinator.line(format!("JavaScript discovery excluded {exclusion}"));
        }
        (found.mutations, None)
    } else {
        let discovery = go::discover(
            &baseline_workspace.path(),
            config.test_scope,
            baseline_limit,
            config.max_log_bytes,
            config.max_capture_bytes,
            cancellation,
        )
        .await?;
        coordinator.emit(
            RunEvent::Command {
                phase: Phase::Discovery,
                output: &discovery,
            },
            cancellation,
        )?;
        if discovery.interrupted {
            return Err(Interrupted.into());
        }
        let packages = go::packages(&discovery)?;
        drop(discovery);
        let graph = if config.test_scope == TestScope::Affected {
            Some(go::selection::PackageGraph::new(
                &baseline_workspace.path(),
                &packages,
            )?)
        } else {
            None
        };
        let mut files = match &graph {
            Some(graph) => graph.files.clone(),
            None => go::files_in(&baseline_workspace.path(), &packages)?,
        };
        drop(baseline_workspace);
        let mutations = {
            let snapshot = Arc::clone(&snapshot);
            let cancellation = cancellation.clone();
            tokio::task::spawn_blocking(move || {
                mutation::discover_cancellable(&snapshot.path(), &mut files, &scope, &cancellation)
            })
            .await
            .context("mutation discovery panicked")??
        };
        (mutations, graph)
    };
    cancellation.check()?;
    let mut dismissals = dismissal_store
        .map(|store| store.snapshot(config.lang, &mutations, cancellation))
        .transpose()?
        .unwrap_or_default();
    let (mutations, rerun) = match rerun {
        Some(selection) => {
            let cancellation = cancellation.clone();
            let (selected, provenance) =
                tokio::task::spawn_blocking(move || selection.select(mutations, &cancellation))
                    .await
                    .context("rerun selection panicked")??;
            coordinator.line(format!(
                "Selected {} mutations from {} ({})",
                selected.len(),
                provenance.source_run.display(),
                provenance.filter.description(),
            ));
            (selected, Some(provenance))
        }
        None => (
            mutations
                .into_iter()
                .filter(|m| config.operators.is_empty() || config.operators.contains(&m.operator))
                .collect(),
            None,
        ),
    };
    coordinator.line(format!("Found {} mutations", mutations.len()));
    if mutations.is_empty() {
        coordinator.line(match &rerun {
            Some(provenance) => format!(
                "No saved mutations matched the rerun selection: {}.",
                provenance.filter.description()
            ),
            None => "No eligible mutations matched the selected scope.".to_owned(),
        });
    }
    let test_packages: Vec<Vec<String>> = mutations
        .iter()
        .map(|mutation| match &graph {
            Some(graph) => graph.packages_for(&mutation.file.path),
            None if matches!(
                config.lang,
                Lang::Vitest | Lang::Node | Lang::Bun | Lang::Python
            ) =>
            {
                Ok(vec![])
            }
            None => Ok(vec![
                if config.lang == Lang::Rust {
                    "--tests"
                } else {
                    "./..."
                }
                .to_owned(),
            ]),
        })
        .collect::<Result<_>>()?;
    if config.list {
        for (index, mutation) in mutations.iter().enumerate() {
            cancellation.check()?;
            if let Some(provenance) = &rerun {
                println!(
                    "[{:06}] (source [{:06}]) {} {}",
                    index + 1,
                    provenance.mutations[index].source_id,
                    mutation.operator,
                    mutation.description()
                );
            } else {
                println!("{}", mutation.description());
            }
            if config.test_scope == TestScope::Affected {
                println!("  Tests: {}", test_packages[index].join(" "));
            }
        }
        return Ok(());
    }
    let timeout = timeout.context("missing mutation timeout")?;
    let jobs = config.jobs.get().min(mutations.len());
    dismissals.select(&mutations);
    coordinator.emit(RunEvent::Phase(Phase::Inventory), cancellation)?;
    coordinator.emit(
        RunEvent::Plan(&RunPlan {
            mutations: &mutations,
            test_packages: &test_packages,
            workers: jobs,
            timeout,
            rerun: rerun.as_ref(),
            dismissals: &dismissals,
        }),
        cancellation,
    )?;
    cancellation.check()?;
    if mutations.is_empty() {
        return Ok(());
    }
    coordinator.emit(RunEvent::Phase(Phase::Mutations), cancellation)?;
    coordinator.line(format!(
        "Running {jobs} worker(s), {:.2}s timeout per mutation",
        timeout.as_secs_f64()
    ));
    let context = WorkerContext {
        lang: config.lang,
        continue_on_error: config.continue_on_error,
        rust_cache_package: rust_project
            .as_ref()
            .filter(|_| config.rust_build_cache == cli::RustBuildCache::Worker)
            .map(|project| project.scope.package.clone()),
        vitest_scope: vitest_scope.map(Arc::new),
        node_scope: node_scope.map(Arc::new),
        bun_scope: bun_scope.map(Arc::new),
        python_scope: python_scope.map(Arc::new),
        snapshot,
        test_packages: Arc::new(test_packages),
        timeout,
        max_log_bytes: config.max_log_bytes,
        max_capture_bytes: config.max_capture_bytes,
        cancellation: cancellation.clone(),
    };
    run_workers(context, mutations, jobs, coordinator).await
}

#[derive(Clone)]
struct WorkerContext {
    vitest_scope: Option<Arc<vitest::ExecutionScope>>,
    node_scope: Option<Arc<node::ExecutionScope>>,
    bun_scope: Option<Arc<bun::ExecutionScope>>,
    python_scope: Option<Arc<python::ExecutionScope>>,
    lang: Lang,
    continue_on_error: Option<std::num::NonZeroUsize>,
    rust_cache_package: Option<String>,
    snapshot: Arc<Workspace>,
    test_packages: Arc<Vec<Vec<String>>>,
    timeout: Duration,
    max_log_bytes: Option<std::num::NonZeroUsize>,
    max_capture_bytes: Option<std::num::NonZeroUsize>,
    cancellation: Cancellation,
}

struct WorkerJob {
    context: WorkerContext,
    id: usize,
    worker: usize,
    mutation: Mutation,
    workspace: Option<Workspace>,
    cache: Option<rust::BuildCache>,
    progress: mpsc::Sender<WorkerProgress>,
}

struct FinishedWorker {
    worker: usize,
    workspace: Option<Workspace>,
    cache: Option<rust::BuildCache>,
    result: MutationResult,
    continuable_error: bool,
    preparation: Option<(WorkspaceKind, WorkspaceMeasurement)>,
}

impl WorkerJob {
    async fn run(self) -> FinishedWorker {
        let Self {
            context,
            id,
            worker,
            mutation,
            workspace,
            mut cache,
            progress,
        } = self;
        let kind = if workspace.is_some() {
            WorkspaceKind::WorkerReset
        } else {
            WorkspaceKind::WorkerSetup
        };
        let prepared = {
            let context = context.clone();
            let mutation = mutation.clone();
            tokio::task::spawn_blocking(move || {
                let (result, measurement) = match workspace {
                    Some(workspace) => {
                        let (result, measurement) = workspace
                            .reset_measured(&context.snapshot.path(), &context.cancellation);
                        (result.map(|()| workspace), measurement)
                    }
                    None => Workspace::create_measured(
                        &context.snapshot.path(),
                        &[],
                        &context.cancellation,
                    ),
                };
                let result = result.and_then(|workspace| {
                    context.cancellation.check()?;
                    mutation.apply(&workspace.path())?;
                    Ok(workspace)
                });
                (result, measurement)
            })
            .await
            .context("worker workspace preparation panicked")
        };
        let (prepared, preparation) = match prepared {
            Ok((result, measurement)) => (result, Some((kind, measurement))),
            Err(error) => (Err(error), None),
        };
        let mut workspace = None;
        let output = match prepared {
            Ok(prepared) => {
                workspace = Some(prepared);
                let _ = progress
                    .send(WorkerProgress {
                        worker,
                        id,
                        phase: WorkerPhase::Running,
                    })
                    .await;
                let cache_ready = if cache.is_none() {
                    context
                        .rust_cache_package
                        .as_ref()
                        .map(|package| {
                            rust::BuildCache::new(package.clone()).map(|new| cache = Some(new))
                        })
                        .transpose()
                } else {
                    Ok(None)
                };
                match cache_ready {
                    Err(error) => Err(error),
                    Ok(_) => {
                        Box::pin(language::test(
                            context.lang,
                            &workspace.as_ref().unwrap().path(),
                            &context.test_packages[id - 1],
                            context.timeout,
                            context.max_log_bytes,
                            context.max_capture_bytes,
                            cache.as_ref(),
                            context.vitest_scope.as_deref(),
                            context.node_scope.as_deref(),
                            context.bun_scope.as_deref(),
                            context.python_scope.as_deref(),
                            &context.cancellation,
                            move |phase| {
                                let progress = progress.clone();
                                async move {
                                    let _ = progress
                                        .send(WorkerProgress {
                                            worker,
                                            id,
                                            phase: phase.into(),
                                        })
                                        .await;
                                }
                            },
                        ))
                        .await
                    }
                }
            }
            Err(error) => Err(error),
        };
        let (state, outcome, output, error) = match output {
            Ok(output) if output.interrupted => {
                (ExecutionState::Interrupted, None, Some(output), None)
            }
            Ok(output) => {
                let (outcome, error) = match command::classify(&output) {
                    Ok(outcome) => (outcome, None),
                    Err(error) => (
                        Outcome::Error,
                        Some(format!("running {}: {error:#}", mutation.description())),
                    ),
                };
                (
                    ExecutionState::Completed,
                    Some(outcome),
                    Some(output),
                    error,
                )
            }
            Err(error) if error.is::<Interrupted>() => {
                (ExecutionState::Interrupted, None, None, None)
            }
            Err(error) => (
                ExecutionState::Completed,
                Some(Outcome::Error),
                None,
                Some(format!("running {}: {error:#}", mutation.description())),
            ),
        };
        let continuable_error = context.continue_on_error.is_some()
            && outcome == Some(Outcome::Error)
            && output
                .as_ref()
                .is_some_and(command::CommandOutput::isolated_candidate_error);
        // Only adapter-proven body errors may avoid cancellation. All other
        // failures stop peers before the coordinator persists the result.
        if outcome == Some(Outcome::Error) && !continuable_error {
            context.cancellation.cancel();
        }
        FinishedWorker {
            continuable_error,
            worker,
            workspace,
            cache,
            result: MutationResult {
                id,
                mutation,
                state,
                outcome,
                output,
                error,
            },
            preparation,
        }
    }
}

async fn run_workers(
    context: WorkerContext,
    mutations: Vec<Mutation>,
    jobs: usize,
    coordinator: &mut Coordinator,
) -> Result<()> {
    let mut workers = JoinSet::new();
    let (progress, mut progress_events) = mpsc::channel(jobs.saturating_mul(2).clamp(1, 256));
    let mut available: VecDeque<_> = (1..=jobs).map(|worker| (worker, None, None)).collect();
    let mut next = 0;
    let mut failure = None;
    let mut candidate_errors = 0usize;
    let mut deferred_error = None;
    let total = mutations.len();
    loop {
        while failure.is_none() && !context.cancellation.is_cancelled() && next < total {
            let Some((worker, workspace, cache)) = available.pop_front() else {
                break;
            };
            let id = next + 1;
            if let Err(error) = coordinator.emit(
                RunEvent::Started {
                    id,
                    worker,
                    mutation: &mutations[next],
                },
                &context.cancellation,
            ) {
                failure = Some(error);
                context.cancellation.cancel();
                break;
            }
            workers.spawn(
                WorkerJob {
                    context: context.clone(),
                    id,
                    worker,
                    mutation: mutations[next].clone(),
                    workspace,
                    cache,
                    progress: progress.clone(),
                }
                .run(),
            );
            next += 1;
        }
        let finished = tokio::select! {
            biased;
            finished = workers.join_next() => finished,
            Some(progress) = progress_events.recv() => {
                if let Err(error) = coordinator.emit(RunEvent::Progress(progress), &context.cancellation) {
                    failure.get_or_insert(error);
                    context.cancellation.cancel();
                }
                continue;
            }
        };
        let Some(finished) = finished else {
            break;
        };
        let result = match finished {
            Ok(FinishedWorker {
                worker,
                workspace,
                cache,
                result,
                continuable_error,
                preparation,
            }) => {
                available.push_back((worker, workspace, cache));
                let measured = if let Some((kind, measurement)) = preparation {
                    coordinator.emit(
                        RunEvent::Workspace {
                            kind,
                            measurement: &measurement,
                        },
                        &context.cancellation,
                    )
                } else {
                    Ok(())
                };
                measured
                    .and_then(|()| {
                        coordinator.emit(
                            RunEvent::Finished {
                                worker,
                                result: &result,
                            },
                            &context.cancellation,
                        )
                    })
                    .and_then(|()| match result.error {
                        Some(error) if continuable_error => {
                            candidate_errors += 1;
                            deferred_error.get_or_insert_with(|| error.clone());
                            if candidate_errors >= context.continue_on_error.unwrap().get() {
                                Err(anyhow::anyhow!(error).context(format!(
                                    "candidate error limit reached ({candidate_errors})"
                                )))
                            } else {
                                Ok(())
                            }
                        }
                        Some(error) => Err(anyhow::anyhow!(error)),
                        None => Ok(()),
                    })
            }
            Err(error) => Err(anyhow::Error::new(error).context("mutation worker panicked")),
        };
        if let Err(error) = result {
            failure = Some(match failure {
                Some(previous) => {
                    previous.context(format!("while collecting remaining results: {error:#}"))
                }
                None => error,
            });
            context.cancellation.cancel();
        }
        // Always drain every worker, even after errors or cancellation. Dropping
        // their futures here would discard active output and skip process reaping.
    }
    if let Some(error) = failure {
        return Err(error);
    }
    context.cancellation.check()?;
    ensure!(
        coordinator.state.summary.total() == total,
        "workers did not report every mutation"
    );
    if let Some(error) = deferred_error {
        anyhow::bail!(
            "completed inventory with {candidate_errors} candidate error(s); first error: {error}"
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn findings_exit_waits_for_report_persistence_and_nonmissed_outcomes_pass() -> Result<()> {
        for fail_save in [false, true] {
            let root = tempfile::tempdir()?;
            fs::write(root.path().join("go.mod"), "module example.com/policy\n")?;
            fs::write(
                root.path().join("a.go"),
                "package p\nvar a = true\nvar b = true\nvar c = true\n",
            )?;
            let config = RunConfig::resolve(Cli::parse_from([
                std::ffi::OsStr::new("mutie"),
                root.path().as_os_str(),
                std::ffi::OsStr::new("--fail-on-missed"),
                std::ffi::OsStr::new("--min-score"),
                std::ffi::OsStr::new("100"),
            ]))?;
            let mutations =
                mutation::discover(root.path(), vec!["a.go".into()], &Scope::new(&[], &[])?)?;
            let (mut coordinator, _) = Coordinator::new(&config, true)?;
            let directory = coordinator.report_path().unwrap();
            let initial: serde_json::Value =
                serde_json::from_slice(&fs::read(directory.join("run.json"))?)?;
            assert_eq!(initial["policy"]["status"], "not_evaluated");
            assert_eq!(initial["score_policy"]["status"], "not_evaluated");
            let cancellation = Cancellation::default();
            coordinator.emit(
                RunEvent::Plan(&RunPlan {
                    mutations: &mutations,
                    test_packages: &vec![vec!["./...".into()]; mutations.len()],
                    workers: 1,
                    timeout: Duration::from_secs(1),
                    rerun: None,
                    dismissals: &dismissals::DismissalSnapshot::default(),
                }),
                &cancellation,
            )?;
            for (index, outcome) in [
                Outcome::Killed,
                Outcome::Timeout,
                if fail_save {
                    Outcome::Missed
                } else {
                    Outcome::BuildFailed
                },
            ]
            .into_iter()
            .enumerate()
            {
                coordinator.emit(
                    RunEvent::Started {
                        id: index + 1,
                        worker: 1,
                        mutation: &mutations[index],
                    },
                    &cancellation,
                )?;
                coordinator.emit(
                    RunEvent::Finished {
                        worker: 1,
                        result: &MutationResult {
                            id: index + 1,
                            mutation: mutations[index].clone(),
                            state: ExecutionState::Completed,
                            outcome: Some(outcome),
                            output: None,
                            error: None,
                        },
                    },
                    &cancellation,
                )?;
            }
            if fail_save {
                // Fail the final text-summary write after JSON has a findings failure.
                fs::remove_file(directory.join("summary.txt"))?;
                fs::create_dir(directory.join("summary.txt"))?;
            }
            let result = finish_run(coordinator, Ok(()));
            if fail_save {
                assert!(format!("{:#}", result.unwrap_err()).contains("finishing saved report"));
            } else {
                assert_eq!(result?, ExitCode::SUCCESS);
            }
            let metadata: serde_json::Value =
                serde_json::from_slice(&fs::read(directory.join("run.json"))?)?;
            assert_eq!(
                metadata["policy"]["status"],
                if fail_save { "failed" } else { "passed" }
            );
            assert_eq!(
                metadata["score_policy"]["status"],
                if fail_save { "failed" } else { "passed" }
            );
            assert_eq!(
                metadata["policy"]["undismissed_missed"],
                usize::from(fail_save)
            );
        }
        Ok(())
    }
}
