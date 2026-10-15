//! Read-only preview of the first Vitest scope. Execution is not implemented.
pub mod discovery;
pub mod project;

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
    let output = process::run_with_env(
        "node",
        &[
            "--eval",
            "console.log(JSON.stringify({version:process.version,executable:process.execPath}))",
        ],
        &project.root,
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
    let found = discovery::discover(&project.root, &Scope::new(include, exclude)?, cancellation)?;
    println!("Vitest discovery preview only; tests, reports and reruns are not implemented.");
    println!(
        "{}; Node {}; Vitest {}; Vite {}",
        project.name,
        node["version"].as_str().unwrap(),
        project.vitest,
        project.vite
    );
    println!("Node executable {executable}");
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
