mod cli;
mod go;
mod mutation;
mod process;
mod report;
mod workspace;

use std::{
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
use report::{MutationResult, Summary};
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
    let baseline_limit = Duration::from_secs(cli.baseline_timeout.get());

    println!("Snapshotting {}", root.display());
    let snapshot = Arc::new(Workspace::new(&root)?);
    let baseline_workspace = Workspace::new(&snapshot.path())?;
    let mut timeout = cli
        .timeout
        .map(|seconds| Duration::from_secs(seconds.get()));
    if !cli.list {
        println!("Running baseline: go test ./... (test caching disabled)");
        let baseline = go::test(&baseline_workspace.path(), baseline_limit).await?;
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
    let mutations = mutation::discover(&snapshot.path(), files, &scope)?;
    drop(baseline_workspace);
    println!("Found {} mutations", mutations.len());
    if cli.list {
        for mutation in &mutations {
            println!("{}", mutation.description());
        }
        return Ok(());
    }
    if mutations.is_empty() {
        println!("No eligible mutations matched the selected scope.");
        return Ok(());
    }
    let timeout = timeout.context("missing mutation timeout")?;
    let jobs = cli.jobs.get().min(mutations.len());
    println!(
        "Running {jobs} worker(s), {:.2}s timeout per mutation",
        timeout.as_secs_f64()
    );
    let summary = run_workers(snapshot, mutations, jobs, timeout, cli.verbose).await?;
    println!("\n{summary}");
    Ok(())
}

async fn run_workers(
    snapshot: Arc<Workspace>,
    mutations: Vec<Mutation>,
    jobs: usize,
    timeout: Duration,
    verbose: bool,
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
            while let Some(mutation) = mutations.get(next.fetch_add(1, Ordering::Relaxed)) {
                if !first {
                    workspace.reset(&snapshot.path())?;
                }
                first = false;
                mutation.apply(&workspace.path())?;
                let output = go::test(&workspace.path(), timeout).await?;
                let result = MutationResult {
                    mutation: mutation.clone(),
                    outcome: go::classify(&output)
                        .with_context(|| format!("running {}", mutation.description()))?,
                    duration: output.elapsed,
                    output: if verbose {
                        go::text(&output)
                    } else {
                        String::new()
                    },
                };
                sender.send(result).context("result receiver closed")?;
            }
            Ok::<_, anyhow::Error>(())
        });
    }
    drop(sender);
    let mut summary = Summary::default();
    let total = mutations.len();
    while !workers.is_empty() {
        tokio::select! {
            Some(result) = receiver.recv() => display(&mut summary, &result, total, verbose),
            Some(finished) = workers.join_next() => {
                if let Err(error) = finished.context("mutation worker panicked").and_then(|result| result) {
                    workers.abort_all();
                    while workers.join_next().await.is_some() {}
                    return Err(error);
                }
            }
        }
    }
    while let Some(result) = receiver.recv().await {
        display(&mut summary, &result, total, verbose);
    }
    ensure!(
        summary.total() == total,
        "workers did not report every mutation"
    );
    Ok(summary)
}

fn display(summary: &mut Summary, result: &MutationResult, total: usize, verbose: bool) {
    summary.record(result);
    println!(
        "[{}/{}] {}: {} ({:.2}s)",
        summary.total(),
        total,
        result.outcome,
        result.mutation.description(),
        result.duration.as_secs_f64()
    );
    if verbose && !result.output.is_empty() {
        println!("{}", result.output.trim_end());
    }
}
