//! Vitest discovery preview and isolated baseline diagnostics.
pub mod discovery;
pub mod execution;
pub mod project;
mod protocol;

use crate::{cancellation::Cancellation, mutation::Scope, process};
use anyhow::{Context, Result, ensure};
use std::{num::NonZeroUsize, path::Path, time::Duration};

pub async fn list(
    root: &Path,
    include: &[String],
    exclude: &[String],
    cancellation: &Cancellation,
) -> Result<()> {
    let project = project::Project::load(root, cancellation)?;
    let node = resolve_node(&project.root, cancellation).await?;
    let found = discovery::discover(&project.root, &Scope::new(include, exclude)?, cancellation)?;
    println!("Vitest discovery preview only; tests, reports and reruns are not implemented.");
    println!(
        "{}; Node v24.22.0; Vitest {}; Vite {}",
        project.name, project.vitest, project.vite
    );
    println!("Node executable {}", node.executable.display());
    println!(
        "Manifest {}; lockfile {}",
        project.manifest_fingerprint, project.lockfile_fingerprint
    );
    for exclusion in found.exclusions {
        println!("Excluded {exclusion}");
    }
    for (index, mutation) in found.mutations.iter().enumerate() {
        println!("{}: boolean_flip {}", index + 1, mutation.description());
    }
    println!("{} mutations", found.mutations.len());
    Ok(())
}

struct NodeTool {
    executable: std::path::PathBuf,
}
async fn resolve_node(root: &Path, cancellation: &Cancellation) -> Result<NodeTool> {
    let output = process::run_with_env(
        "node",
        &[
            "--eval",
            "console.log(JSON.stringify({version:process.version,executable:process.execPath}))",
        ],
        root,
        Duration::from_secs(10),
        NonZeroUsize::new(4096),
        cancellation,
        &[("NODE_OPTIONS", "".as_ref()), ("NODE_PATH", "".as_ref())],
    )
    .await?;
    cancellation.check()?;
    ensure!(
        !output.interrupted
            && !output.timed_out
            && output.capture_error.is_none()
            && output.status.success(),
        "could not identify Node for Vitest preview"
    );
    let node: serde_json::Value =
        serde_json::from_str(&output.stdout.text()?).context("invalid Node identity output")?;
    ensure!(
        node["version"] == "v24.22.0",
        "Vitest preview currently requires Node 24.22.0; found {}",
        node["version"]
    );
    let executable = node["executable"]
        .as_str()
        .context("Node executable identity missing")?;
    Ok(NodeTool {
        executable: executable.into(),
    })
}

pub async fn check(
    root: &Path,
    timeout: Duration,
    capture_limit: NonZeroUsize,
    cancellation: &Cancellation,
) -> Result<()> {
    let project = project::Project::load(root, cancellation)?;
    let node = resolve_node(&project.root, cancellation).await?;
    println!("Checking isolated Vitest baseline; no mutations or saved report.");
    let output =
        execution::run(&project, &node, None, timeout, capture_limit, cancellation).await?;
    let counts = output
        .evidence
        .as_ref()
        .ok()
        .map(|evidence| evidence.counts());
    let output = output.into_command(NonZeroUsize::new(64 * 1024));
    let outcome = output.classify().with_context(|| {
        format!(
            "Vitest baseline failed:\n{}",
            output.text.text().unwrap_or_default()
        )
    })?;
    ensure!(
        outcome == crate::report::Outcome::Missed,
        "Vitest baseline did not pass ({outcome:?}):\n{}",
        output.text.text()?
    );
    let (passed, _, skipped) = counts.context("passing baseline missing test counts")?;
    println!(
        "Vitest baseline passed: {passed} passed, {skipped} skipped/todo ({:.2}s command time).",
        output.raw.elapsed.as_secs_f64()
    );
    Ok(())
}
