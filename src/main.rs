mod cancellation;
mod cli;
mod go;
mod mutation;
mod process;
mod report;
mod saved_report;
mod workspace;

use std::{
    collections::VecDeque,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use anyhow::{Context, Result, bail, ensure};
use clap::Parser;
use tokio::task::JoinSet;

use cancellation::{Cancellation, Interrupted};
use cli::{Cli, Lang};
use mutation::{Mutation, Scope};
use report::{ExecutionState, MutationResult, Outcome, Summary};
use saved_report::SavedReport;
use workspace::Workspace;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    ensure!(
        cli.lang != Lang::Rust,
        "Rust support is planned; only Go is supported for now"
    );
    let root = cli
        .path
        .canonicalize()
        .with_context(|| format!("opening {}", cli.path.display()))?;
    ensure!(
        root.join("go.mod").is_file(),
        "{} must be a Go module root containing go.mod",
        root.display()
    );
    let scope = Scope::new(&cli.include, &cli.exclude)?;
    let cancellation = Cancellation::default();
    let _signals = cancellation
        .listen()
        .context("installing cancellation handlers")?;
    let mut saved = if cli.list {
        None
    } else {
        Some(SavedReport::create(&root, &cli)?)
    };
    if let Some(report) = &saved {
        println!("Saving report to {}", report.path().display());
    }
    let result = execute(&cli, &root, scope, saved.as_mut(), &cancellation).await;
    if let Some(report) = &mut saved {
        if let Err(save_error) = report.finish(result.as_ref().err()) {
            return Err(match result {
                Err(run_error) => {
                    run_error.context(format!("could not finish saved report: {save_error:#}"))
                }
                Ok(()) => save_error,
            });
        }
        println!(
            "Report saved: {}",
            report.path().join("summary.txt").display()
        );
    }
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
    cli: &Cli,
    root: &Path,
    scope: Scope,
    mut saved: Option<&mut SavedReport>,
    cancellation: &Cancellation,
) -> Result<()> {
    let baseline_limit = Duration::from_secs(cli.baseline_timeout.get());
    println!("Snapshotting {}", root.display());
    let mut excluded = vec![root.join(".mutie")];
    if let Some(report) = &saved {
        excluded.push(report.output_root().to_owned());
    }
    let snapshot =
        Arc::new(create_workspace(root.to_owned(), excluded, cancellation.clone()).await?);
    let baseline_workspace =
        create_workspace(snapshot.path(), vec![], cancellation.clone()).await?;
    let mut timeout = cli
        .timeout
        .map(|seconds| Duration::from_secs(seconds.get()));
    if !cli.list {
        if let Some(report) = saved.as_deref_mut() {
            report.phase("baseline")?;
        }
        println!("Running baseline: go test ./... (test caching disabled)");
        let baseline = go::test(&baseline_workspace.path(), baseline_limit, cancellation).await?;
        if let Some(report) = saved.as_deref_mut() {
            report.command("baseline", &baseline)?;
        }
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
        println!("Baseline passed in {:.2}s", baseline.elapsed.as_secs_f64());
        if cli.verbose {
            print!("{}", go::text(&baseline));
        }
    }
    cancellation.check()?;
    if let Some(report) = saved.as_deref_mut() {
        report.phase("discovery")?;
    }
    // Package discovery must see the pristine input, not baseline side effects.
    let baseline_workspace = if cli.list {
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
    if let Some(report) = saved.as_deref_mut() {
        report.command("discovery", &discovery)?;
    }
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
    println!("Found {} mutations", mutations.len());
    if cli.list {
        for mutation in &mutations {
            cancellation.check()?;
            println!("{}", mutation.description());
        }
        return Ok(());
    }
    let timeout = timeout.context("missing mutation timeout")?;
    let jobs = cli.jobs.get().min(mutations.len());
    if let Some(report) = saved.as_deref_mut() {
        report.phase("inventory")?;
        report.plan(&mutations, jobs, timeout, cancellation)?;
    }
    cancellation.check()?;
    if mutations.is_empty() {
        println!("No eligible mutations matched the selected scope.");
        return Ok(());
    }
    if let Some(report) = saved.as_deref_mut() {
        report.phase("mutations")?;
    }
    println!(
        "Running {jobs} worker(s), {:.2}s timeout per mutation",
        timeout.as_secs_f64()
    );
    let context = WorkerContext {
        snapshot,
        timeout,
        cancellation: cancellation.clone(),
    };
    let summary = run_workers(context, mutations, jobs, cli.verbose, saved).await?;
    println!("\n{summary}");
    Ok(())
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
    verbose: bool,
    mut saved: Option<&mut SavedReport>,
) -> Result<Summary> {
    let mut workers = JoinSet::new();
    let mut available: VecDeque<_> = (1..=jobs).map(|worker| (worker, None)).collect();
    let mut next = 0;
    let mut summary = Summary::default();
    let mut failure = None;
    let total = mutations.len();
    loop {
        while failure.is_none() && !context.cancellation.is_cancelled() && next < total {
            let Some((worker, workspace)) = available.pop_front() else {
                break;
            };
            let id = next + 1;
            if let Some(report) = saved.as_deref_mut()
                && let Err(error) = report.started(id, worker)
            {
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
                record(&mut summary, &result, total, verbose, saved.as_deref_mut())
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
        summary.total() == total,
        "workers did not report every mutation"
    );
    Ok(summary)
}

fn record(
    summary: &mut Summary,
    result: &MutationResult,
    total: usize,
    verbose: bool,
    saved: Option<&mut SavedReport>,
) -> Result<()> {
    if let Some(report) = saved {
        report.record(result)?;
    }
    summary.record(result);
    let outcome = result
        .outcome
        .map(|outcome| outcome.to_string())
        .unwrap_or_else(|| "interrupted".into());
    let duration = result
        .output
        .as_ref()
        .map(|output| format!(" ({:.2}s)", output.elapsed.as_secs_f64()))
        .unwrap_or_default();
    println!(
        "[{}/{}] {}: {}{}",
        summary.total(),
        total,
        outcome,
        result.mutation.description(),
        duration
    );
    if verbose && let Some(output) = &result.output {
        let text = go::text(output);
        if !text.is_empty() {
            println!("{}", text.trim_end());
        }
    }
    if let Some(error) = &result.error {
        bail!("{error}");
    }
    Ok(())
}
