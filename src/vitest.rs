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
    println!("Vitest discovery preview only; use --lang vitest for scored runs.");
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
    let output = execution::run(
        &project,
        &node,
        None,
        timeout,
        Some(capture_limit),
        cancellation,
    )
    .await?;
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

/// Persist the fixed runner recipe independently of temporary workspace paths.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutionScope {
    pub package: String,
    pub node: String,
    pub node_executable: std::path::PathBuf,
    pub pnpm: String,
    pub vitest: String,
    pub vite: String,
    pub manifest_fingerprint: String,
    pub lockfile_fingerprint: String,
    pub config: serde_json::Value,
    pub project_config_fingerprints: std::collections::BTreeMap<String, String>,
    pub reporter_fingerprint: String,
    pub syntax: Vec<String>,
}

pub async fn execution_scope(root: &Path, cancellation: &Cancellation) -> Result<ExecutionScope> {
    use sha2::{Digest, Sha256};
    let project = project::Project::load(root, cancellation)?;
    let node = resolve_node(root, cancellation).await?;
    let mut project_config_fingerprints = std::collections::BTreeMap::new();
    for name in ["tsconfig.json", "pnpm-workspace.yaml"] {
        let path = project.root.join(name);
        if path.exists() {
            project_config_fingerprints.insert(
                name.to_owned(),
                format!(
                    "sha256:{:x}",
                    Sha256::digest(
                        std::fs::read(&path)
                            .with_context(|| format!("reading {}", path.display()))?
                    )
                ),
            );
        }
    }
    Ok(ExecutionScope {
        project_config_fingerprints,
        package: project.name,
        node: "24.22.0".into(),
        node_executable: node.executable,
        pnpm: "11.10.0".into(),
        vitest: project.vitest,
        vite: project.vite,
        manifest_fingerprint: project.manifest_fingerprint,
        lockfile_fingerprint: project.lockfile_fingerprint,
        config: execution::settings(
            Path::new("<project>"),
            Path::new("<private-cache>"),
            Path::new("<private-reporter>"),
        ),
        reporter_fingerprint: format!(
            "sha256:{:x}",
            Sha256::digest(include_bytes!("vitest/reporter.mjs"))
        ),
        syntax: vec!["javascript".into(), "typescript".into()],
    })
}

pub async fn test(
    root: &Path,
    limit: Duration,
    log_limit: Option<NonZeroUsize>,
    capture_limit: Option<NonZeroUsize>,
    expected: &ExecutionScope,
    cancellation: &Cancellation,
) -> Result<crate::command::CommandOutput> {
    let actual = execution_scope(root, cancellation).await?;
    ensure!(
        &actual == expected,
        "Vitest runner or package identity changed during the run"
    );
    let project = project::Project::load(root, cancellation)?;
    let node = NodeTool {
        executable: expected.node_executable.clone(),
    };
    Ok(
        execution::run(&project, &node, None, limit, capture_limit, cancellation)
            .await?
            .into_command(log_limit),
    )
}

impl ExecutionScope {
    pub fn validate_rerun(&self, current: &Self) -> Result<()> {
        self.validate_record()?;
        current.validate_record()?;
        ensure!(
            self == current,
            "Vitest package, dependency, runner or configuration identity changed; start a fresh scan; no mutations were run"
        );
        Ok(())
    }

    pub fn validate_record(&self) -> Result<()> {
        ensure!(
            !self.package.is_empty() && self.node_executable.is_absolute(),
            "invalid Vitest package or Node identity"
        );
        ensure!(
            self.node == "24.22.0"
                && self.pnpm == "11.10.0"
                && self.vitest == "4.1.9"
                && matches!(self.vite.as_str(), "8.1.3" | "8.3.4"),
            "unsupported saved Vitest tool identity"
        );
        ensure!(
            self.project_config_fingerprints
                .keys()
                .all(|name| matches!(name.as_str(), "tsconfig.json" | "pnpm-workspace.yaml")),
            "unsupported saved Vitest project configuration"
        );
        for fingerprint in [
            &self.manifest_fingerprint,
            &self.lockfile_fingerprint,
            &self.reporter_fingerprint,
        ]
        .into_iter()
        .chain(self.project_config_fingerprints.values())
        {
            ensure!(
                fingerprint
                    .strip_prefix("sha256:")
                    .is_some_and(|digest| digest.len() == 64
                        && digest.bytes().all(|b| b.is_ascii_hexdigit())),
                "invalid saved Vitest fingerprint"
            );
        }
        ensure!(
            self.syntax == ["javascript", "typescript"],
            "unsupported saved Vitest syntax scope"
        );
        ensure!(
            self.config
                == execution::settings(
                    Path::new("<project>"),
                    Path::new("<private-cache>"),
                    Path::new("<private-reporter>")
                ),
            "unsupported saved Vitest runner settings"
        );
        Ok(())
    }
}

#[cfg(test)]
pub(crate) fn fixture_scope() -> ExecutionScope {
    ExecutionScope {
        package: "fixture".into(),
        node: "24.22.0".into(),
        node_executable: "/tools/node".into(),
        pnpm: "11.10.0".into(),
        vitest: "4.1.9".into(),
        vite: "8.3.4".into(),
        manifest_fingerprint: format!("sha256:{}", "a".repeat(64)),
        lockfile_fingerprint: format!("sha256:{}", "b".repeat(64)),
        config: execution::settings(
            Path::new("<project>"),
            Path::new("<private-cache>"),
            Path::new("<private-reporter>"),
        ),
        project_config_fingerprints: Default::default(),
        reporter_fingerprint: format!("sha256:{}", "c".repeat(64)),
        syntax: vec!["javascript".into(), "typescript".into()],
    }
}

#[cfg(test)]
mod scope_tests {
    use super::*;
    #[test]
    fn reruns_require_the_complete_saved_execution_identity() {
        let original = fixture_scope();
        original.validate_rerun(&original).unwrap();
        let value = serde_json::to_value(&original).unwrap();
        for (field, changed) in [
            ("package", serde_json::json!("renamed")),
            ("node", serde_json::json!("24.23.0")),
            ("node_executable", serde_json::json!("/another/node")),
            ("pnpm", serde_json::json!("11.11.0")),
            ("vitest", serde_json::json!("4.1.10")),
            ("vite", serde_json::json!("8.1.3")),
            (
                "manifest_fingerprint",
                serde_json::json!(format!("sha256:{}", "d".repeat(64))),
            ),
            (
                "lockfile_fingerprint",
                serde_json::json!(format!("sha256:{}", "d".repeat(64))),
            ),
            (
                "reporter_fingerprint",
                serde_json::json!(format!("sha256:{}", "d".repeat(64))),
            ),
            (
                "project_config_fingerprints",
                serde_json::json!({"tsconfig.json": format!("sha256:{}", "d".repeat(64))}),
            ),
            ("config", serde_json::json!({"test":{"retry":1}})),
            ("syntax", serde_json::json!(["typescript"])),
        ] {
            let mut changed_scope = value.clone();
            changed_scope[field] = changed;
            let changed_scope = serde_json::from_value(changed_scope).unwrap();
            assert!(original.validate_rerun(&changed_scope).is_err(), "{field}");
        }
    }
}
