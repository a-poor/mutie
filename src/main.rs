mod cancellation;
mod cli;
mod config;
mod coordinator;
mod go;
mod mutation;
mod process;
mod report;
mod rerun;
mod saved_report;
mod status;
mod workspace;

use std::{collections::VecDeque, path::PathBuf, sync::Arc, time::Duration};

use anyhow::{Context, Result, bail, ensure};
use clap::Parser;
use tokio::task::JoinSet;

use cancellation::{Cancellation, Interrupted};
use cli::{Cli, UiMode};
use config::RunConfig;
use coordinator::{Coordinator, Phase, RunEvent};
use mutation::{Mutation, Scope};
use report::{ExecutionState, MutationResult, Outcome};
use workspace::Workspace;

#[tokio::main]
async fn main() -> Result<()> {
    let config = RunConfig::resolve(Cli::parse())?;
    let scope = Scope::new(&config.include, &config.exclude)?;
    let rerun = config
        .rerun_missed
        .as_deref()
        .map(rerun::RerunSelection::load)
        .transpose()?;
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
                let result = coordinator.finish(Err(error));
                status::print_summary(&coordinator);
                return result;
            }
        }
    } else {
        None
    };
    let engine_cancellation = cancellation.clone();
    let engine = tokio::spawn(async move {
        let result = execute(
            &config,
            scope,
            rerun,
            &mut coordinator,
            &engine_cancellation,
        )
        .await;
        (coordinator, result)
    });
    let (mut coordinator, result) =
        status::drive(engine, updates, &mut terminal, &cancellation).await?;
    let result = coordinator.finish(result);
    status::print_summary(&coordinator);
    result
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

async fn execute(
    config: &RunConfig,
    scope: Scope,
    rerun: Option<rerun::RerunSelection>,
    coordinator: &mut Coordinator,
    cancellation: &Cancellation,
) -> Result<()> {
    let root = &config.module_root;
    let baseline_limit = Duration::from_secs(config.baseline_timeout.get());
    coordinator.line(format!("Snapshotting {}", root.display()));
    let mut excluded = vec![root.join(".mutie"), config.output_dir.clone()];
    excluded.extend(config.rerun_missed.iter().cloned());
    let snapshot =
        Arc::new(create_workspace(root.to_owned(), excluded, cancellation.clone()).await?);
    let baseline_workspace =
        create_workspace(snapshot.path(), vec![], cancellation.clone()).await?;
    let mut timeout = config
        .timeout
        .map(|seconds| Duration::from_secs(seconds.get()));
    if !config.list {
        coordinator.emit(RunEvent::Phase(Phase::Baseline), cancellation)?;
        let baseline = go::test(&baseline_workspace.path(), baseline_limit, cancellation).await?;
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
        if baseline.timed_out {
            bail!(
                "baseline timed out after {:.1}s; no mutations were run\n{}",
                baseline_limit.as_secs_f64(),
                go::text(&baseline)
            );
        }
        if !baseline.status.success() {
            bail!(
                "baseline tests failed; no mutations were run\n{}",
                go::text(&baseline)
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
        let cancellation = cancellation.clone();
        tokio::task::spawn_blocking(move || {
            baseline_workspace.reset(&source, &cancellation)?;
            Ok::<_, anyhow::Error>(baseline_workspace)
        })
        .await
        .context("baseline workspace reset panicked")??
    };
    let discovery = process::run(
        "go",
        &["list", "-json", "./..."],
        &baseline_workspace.path(),
        baseline_limit,
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
    let mut files = go::files(&baseline_workspace.path(), &discovery)?;
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
    cancellation.check()?;
    let (mutations, rerun) = match rerun {
        Some(selection) => {
            let cancellation = cancellation.clone();
            let (selected, provenance) =
                tokio::task::spawn_blocking(move || selection.select(mutations, &cancellation))
                    .await
                    .context("rerun selection panicked")??;
            coordinator.line(format!(
                "Selected {} previously missed mutations from {}",
                selected.len(),
                provenance.source_run.display()
            ));
            (selected, Some(provenance))
        }
        None => (mutations, None),
    };
    coordinator.line(format!("Found {} mutations", mutations.len()));
    if config.list {
        for mutation in &mutations {
            cancellation.check()?;
            println!("{}", mutation.description());
        }
        return Ok(());
    }
    let timeout = timeout.context("missing mutation timeout")?;
    let jobs = config.jobs.get().min(mutations.len());
    coordinator.emit(RunEvent::Phase(Phase::Inventory), cancellation)?;
    coordinator.emit(
        RunEvent::Plan {
            mutations: &mutations,
            workers: jobs,
            timeout,
            rerun: rerun.as_ref(),
        },
        cancellation,
    )?;
    cancellation.check()?;
    if mutations.is_empty() {
        coordinator.line(if rerun.is_some() {
            "The saved run has no completed missed mutations to rerun."
        } else {
            "No eligible mutations matched the selected scope."
        });
        return Ok(());
    }
    coordinator.emit(RunEvent::Phase(Phase::Mutations), cancellation)?;
    coordinator.line(format!(
        "Running {jobs} worker(s), {:.2}s timeout per mutation",
        timeout.as_secs_f64()
    ));
    let context = WorkerContext {
        snapshot,
        timeout,
        cancellation: cancellation.clone(),
    };
    run_workers(context, mutations, jobs, coordinator).await
}

#[derive(Clone)]
struct WorkerContext {
    snapshot: Arc<Workspace>,
    timeout: Duration,
    cancellation: Cancellation,
}

struct WorkerJob {
    context: WorkerContext,
    id: usize,
    worker: usize,
    mutation: Mutation,
    workspace: Option<Workspace>,
}

impl WorkerJob {
    async fn run(self) -> (usize, Option<Workspace>, MutationResult) {
        let Self {
            context,
            id,
            worker,
            mutation,
            workspace,
        } = self;
        let prepared = {
            let context = context.clone();
            let mutation = mutation.clone();
            tokio::task::spawn_blocking(move || {
                context.cancellation.check()?;
                let workspace = match workspace {
                    Some(workspace) => {
                        workspace.reset(&context.snapshot.path(), &context.cancellation)?;
                        workspace
                    }
                    None => {
                        Workspace::create(&context.snapshot.path(), &[], &context.cancellation)?
                    }
                };
                context.cancellation.check()?;
                mutation.apply(&workspace.path())?;
                Ok::<_, anyhow::Error>(workspace)
            })
            .await
            .context("worker workspace preparation panicked")
            .and_then(|result| result)
        };
        let mut workspace = None;
        let output = match prepared {
            Ok(prepared) => {
                workspace = Some(prepared);
                go::test(
                    &workspace.as_ref().unwrap().path(),
                    context.timeout,
                    &context.cancellation,
                )
                .await
            }
            Err(error) => Err(error),
        };
        let (state, outcome, output, error) = match output {
            Ok(output) if output.interrupted => {
                (ExecutionState::Interrupted, None, Some(output), None)
            }
            Ok(output) => {
                let (outcome, error) = match go::classify(&output) {
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
        // Stop peers immediately on a tool failure, before the coordinator persists it.
        if outcome == Some(Outcome::Error) {
            context.cancellation.cancel();
        }
        (
            worker,
            workspace,
            MutationResult {
                id,
                mutation,
                state,
                outcome,
                output,
                error,
            },
        )
    }
}

async fn run_workers(
    context: WorkerContext,
    mutations: Vec<Mutation>,
    jobs: usize,
    coordinator: &mut Coordinator,
) -> Result<()> {
    let mut workers = JoinSet::new();
    let mut available: VecDeque<_> = (1..=jobs).map(|worker| (worker, None)).collect();
    let mut next = 0;
    let mut failure = None;
    let total = mutations.len();
    loop {
        while failure.is_none() && !context.cancellation.is_cancelled() && next < total {
            let Some((worker, workspace)) = available.pop_front() else {
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
                }
                .run(),
            );
            next += 1;
        }
        let Some(finished) = workers.join_next().await else {
            break;
        };
        let result = match finished {
            Ok((worker, workspace, result)) => {
                available.push_back((worker, workspace));
                coordinator
                    .emit(
                        RunEvent::Finished {
                            worker,
                            result: &result,
                        },
                        &context.cancellation,
                    )
                    .and_then(|()| match result.error {
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
    Ok(())
}
