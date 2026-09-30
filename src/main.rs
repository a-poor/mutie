mod cli;
mod go;
mod mutation;
mod process;
mod report;
mod saved_report;
mod workspace;

use std::{
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use anyhow::{Context, Result, bail, ensure};
use clap::Parser;
use tokio::{sync::mpsc, task::JoinSet};

use cli::{Cli, Lang};
use mutation::{Mutation, Scope};
use report::{MutationResult, Outcome, Summary};
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
    let mut saved = if cli.list {
        None
    } else {
        Some(SavedReport::create(&root, &cli)?)
    };
    if let Some(report) = &saved {
        println!("Saving report to {}", report.path().display());
    }
    let result = execute(&cli, &root, &scope, saved.as_mut()).await;
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

async fn execute(
    cli: &Cli,
    root: &Path,
    scope: &Scope,
    mut saved: Option<&mut SavedReport>,
) -> Result<()> {
    let baseline_limit = Duration::from_secs(cli.baseline_timeout.get());

    println!("Snapshotting {}", root.display());
    let mut excluded = vec![root.join(".mutie")];
    if let Some(report) = &saved {
        excluded.push(report.output_root().to_owned());
    }
    let snapshot = Arc::new(Workspace::new_excluding(root, &excluded)?);
    let baseline_workspace = Workspace::new(&snapshot.path())?;
    let mut timeout = cli
        .timeout
        .map(|seconds| Duration::from_secs(seconds.get()));
    if !cli.list {
        println!("Running baseline: go test ./... (test caching disabled)");
        let baseline = go::test(&baseline_workspace.path(), baseline_limit).await?;
        if let Some(report) = saved.as_deref_mut() {
            report.baseline(&baseline)?;
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
        // Package discovery must see the pristine input, not baseline side effects.
        baseline_workspace.reset(&snapshot.path())?;
    }
    let files = go::files(&baseline_workspace.path(), baseline_limit).await?;
    let mutations = mutation::discover(&snapshot.path(), files, scope)?;
    drop(baseline_workspace);
    println!("Found {} mutations", mutations.len());
    if cli.list {
        for mutation in &mutations {
            println!("{}", mutation.description());
        }
        return Ok(());
    }
    let timeout = timeout.context("missing mutation timeout")?;
    let jobs = cli.jobs.get().min(mutations.len());
    if let Some(report) = saved.as_deref_mut() {
        report.plan(mutations.len(), jobs, timeout)?;
    }
    if mutations.is_empty() {
        println!("No eligible mutations matched the selected scope.");
        return Ok(());
    }
    println!(
        "Running {jobs} worker(s), {:.2}s timeout per mutation",
        timeout.as_secs_f64()
    );
    let summary = run_workers(snapshot, mutations, jobs, timeout, cli.verbose, saved).await?;
    println!("\n{summary}");
    Ok(())
}

async fn run_workers(
    snapshot: Arc<Workspace>,
    mutations: Vec<Mutation>,
    jobs: usize,
    timeout: Duration,
    verbose: bool,
    mut saved: Option<&mut SavedReport>,
) -> Result<Summary> {
    let mutations = Arc::new(mutations);
    let next = Arc::new(AtomicUsize::new(0));
    let (sender, mut receiver) = mpsc::unbounded_channel();
    let mut workers = JoinSet::new();
    for _ in 0..jobs {
        let (snapshot, mutations, next, sender) = (
            Arc::clone(&snapshot),
            Arc::clone(&mutations),
            Arc::clone(&next),
            sender.clone(),
        );
        workers.spawn(async move {
            let workspace = Workspace::new(&snapshot.path())?;
            let mut first = true;
            loop {
                let index = next.fetch_add(1, Ordering::Relaxed);
                let Some(mutation) = mutations.get(index) else {
                    break;
                };
                if !first {
                    workspace.reset(&snapshot.path())?;
                }
                first = false;
                mutation.apply(&workspace.path())?;
                let output = go::test(&workspace.path(), timeout).await?;
                let (outcome, error) = match go::classify(&output) {
                    Ok(outcome) => (outcome, None),
                    Err(error) => (
                        Outcome::Error,
                        Some(format!("running {}: {error:#}", mutation.description())),
                    ),
                };
                let failed = error.is_some();
                let result = MutationResult {
                    id: index + 1,
                    mutation: mutation.clone(),
                    outcome,
                    output,
                    error,
                };
                sender.send(result).context("result receiver closed")?;
                if failed {
                    break;
                }
            }
            Ok::<_, anyhow::Error>(())
        });
    }
    drop(sender);
    let mut summary = Summary::default();
    let total = mutations.len();
    let result: Result<()> = async {
        while !workers.is_empty() {
            tokio::select! {
                Some(result) = receiver.recv() => record(&mut summary, &result, total, verbose, saved.as_deref_mut())?,
                Some(finished) = workers.join_next() => {
                    finished.context("mutation worker panicked")??;
                }
            }
        }
        while let Some(result) = receiver.recv().await {
            record(&mut summary, &result, total, verbose, saved.as_deref_mut())?;
        }
        ensure!(summary.total() == total, "workers did not report every mutation");
        Ok(())
    }.await;
    if let Err(mut error) = result {
        workers.abort_all();
        while workers.join_next().await.is_some() {}
        // A worker can fail while other completed results are still queued.
        while let Ok(result) = receiver.try_recv() {
            if let Err(save_error) =
                record(&mut summary, &result, total, verbose, saved.as_deref_mut())
            {
                error = error.context(format!(
                    "while collecting remaining results: {save_error:#}"
                ));
            }
        }
        return Err(error);
    }
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
    println!(
        "[{}/{}] {}: {} ({:.2}s)",
        summary.total(),
        total,
        result.outcome,
        result.mutation.description(),
        result.output.elapsed.as_secs_f64()
    );
    if verbose {
        let text = go::text(&result.output);
        if !text.is_empty() {
            println!("{}", text.trim_end());
        }
    }
    if let Some(error) = &result.error {
        bail!("{error}");
    }
    Ok(())
}
