//! Vitest discovery preview and isolated baseline diagnostics.
mod config;
pub mod discovery;
pub mod execution;
mod lifecycle;
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
        println!(
            "{}: {} {}",
            index + 1,
            mutation.operator,
            mutation.description()
        );
    }
    println!("{} mutations", found.mutations.len());
    Ok(())
}

struct NodeTool {
    executable: std::path::PathBuf,
}
async fn resolve_node(root: &Path, cancellation: &Cancellation) -> Result<NodeTool> {
    identify_node("node", root, cancellation).await
}

async fn identify_node(
    program: &str,
    root: &Path,
    cancellation: &Cancellation,
) -> Result<NodeTool> {
    let output = process::run_with_env(
        program,
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
    validate_node_identity(&output, cancellation)
}

fn validate_node_identity(
    output: &process::CommandOutput,
    cancellation: &Cancellation,
) -> Result<NodeTool> {
    cancellation.check()?;
    ensure!(
        !output.interrupted
            && !output.timed_out
            && output.capture_error.is_none()
            && output.status.success(),
        "could not identify Node for Vitest"
    );
    let node: serde_json::Value =
        serde_json::from_str(&output.stdout.text()?).context("invalid Node identity output")?;
    ensure!(
        node["version"] == "v24.22.0",
        "Vitest currently requires Node 24.22.0; found {}",
        node["version"]
    );
    let executable = node["executable"]
        .as_str()
        .context("Node executable identity missing")?;
    ensure!(
        Path::new(executable).is_absolute(),
        "Node executable identity must be absolute"
    );
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub package_manager: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub installation_fingerprint: Option<String>,
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
    execution_scope_with_node(root, "node", cancellation).await
}

async fn execution_scope_with_node(
    root: &Path,
    program: &str,
    cancellation: &Cancellation,
) -> Result<ExecutionScope> {
    use sha2::{Digest, Sha256};
    let project = project::Project::load(root, cancellation)?;
    let node = identify_node(program, root, cancellation).await?;
    Ok(ExecutionScope {
        project_config_fingerprints: project.project_config_fingerprints,
        installation_fingerprint: Some(project.installation_fingerprint),
        package: project.name,
        node: "24.22.0".into(),
        node_executable: node.executable,
        pnpm: if project.package_manager == "pnpm@11.10.0" {
            "11.10.0".into()
        } else {
            String::new()
        },
        package_manager: if project.package_manager == "pnpm@11.10.0" {
            None
        } else {
            Some(project.package_manager)
        },
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
            Sha256::digest(
                [
                    include_bytes!("vitest/reporter.mjs").as_slice(),
                    include_bytes!("vitest/lifecycle.rs").as_slice(),
                    include_bytes!("vitest/project.rs").as_slice(),
                    include_bytes!("vitest/installation.rs").as_slice(),
                    include_bytes!("dependency_tree.rs").as_slice()
                ]
                .concat()
            )
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
    // Resolve through PATH only at run startup. Recheck the pinned executable and
    // reload project identity for every command; neither validation is cached.
    expected.validate_record()?;
    let program = expected
        .node_executable
        .to_str()
        .context("Node path is not UTF-8")?;
    let actual = execution_scope_with_node(root, program, cancellation).await?;
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
                && ((self.pnpm == "11.10.0" && self.package_manager.is_none())
                    || (self.pnpm.is_empty()
                        && matches!(
                            self.package_manager.as_deref(),
                            Some("npm@11.19.1" | "yarn@4.18.1" | "bun@1.3.11")
                        )))
                && self.vitest == "4.1.9"
                && matches!(self.vite.as_str(), "8.1.3" | "8.3.4"),
            "unsupported saved Vitest tool identity"
        );
        ensure!(
            self.package_manager.is_none() || self.installation_fingerprint.is_some(),
            "package-manager identity requires installed dependency fingerprint"
        );
        ensure!(
            self.project_config_fingerprints.keys().all(|name| matches!(
                name.as_str(),
                "tsconfig.json" | "pnpm-workspace.yaml" | ".yarnrc.yml"
            ) || config::FILES
                .contains(&name.as_str())),
            "unsupported saved Vitest project configuration"
        );
        for fingerprint in [
            &self.manifest_fingerprint,
            &self.lockfile_fingerprint,
            &self.reporter_fingerprint,
        ]
        .into_iter()
        .chain(self.project_config_fingerprints.values())
        .chain(self.installation_fingerprint.iter())
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
        package_manager: None,
        installation_fingerprint: None,
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
    #[cfg(unix)]
    #[tokio::test]
    async fn node_identity_rechecks_replacements_and_rejects_failures() {
        use std::{fs, os::unix::fs::PermissionsExt};
        let temp = tempfile::tempdir().unwrap();
        let executable = temp.path().join("node with spaces");
        let write = |body: &str| {
            fs::write(&executable, format!("#!/bin/sh\n{body}\n")).unwrap();
            fs::set_permissions(&executable, fs::Permissions::from_mode(0o755)).unwrap();
        };
        // The mock ignores Node's eval arguments. Invoke its script explicitly
        // to avoid fresh-executable loader stalls, then use the production
        // validator on real exit/capture evidence with the same ten-second bound.
        async fn probe_script(
            program: &str,
            root: &Path,
            cancellation: &Cancellation,
        ) -> Result<NodeTool> {
            let output = process::run_with_env(
                "/bin/sh",
                &[program],
                root,
                Duration::from_secs(10),
                NonZeroUsize::new(4096),
                cancellation,
                &[("NODE_OPTIONS", "".as_ref()), ("NODE_PATH", "".as_ref())],
            )
            .await?;
            validate_node_identity(&output, cancellation)
        }
        let cancellation = Cancellation::default();
        let program = executable.to_str().unwrap();
        let valid = format!(
            "printf '%s\\n' '{}'",
            serde_json::json!({"version":"v24.22.0", "executable":program})
        );
        write(&valid);
        assert_eq!(
            probe_script(program, temp.path(), &cancellation)
                .await
                .unwrap()
                .executable,
            executable
        );
        for (body, expected) in [
            (
                "echo '{\"version\":\"v24.23.0\",\"executable\":\"/node\"}'",
                "Vitest currently requires Node 24.22.0",
            ),
            (
                "echo '{\"version\":\"v24.22.0\",\"executable\":\"relative/node\"}'",
                "Node executable identity must be absolute",
            ),
            (
                "echo '{\"version\":\"v24.22.0\"}'",
                "Node executable identity missing",
            ),
            ("echo invalid-json", "invalid Node identity output"),
            ("exit 1", "could not identify Node for Vitest"),
            ("kill -TERM $$", "could not identify Node for Vitest"),
        ] {
            write(body);
            let error = probe_script(program, temp.path(), &cancellation)
                .await
                .err()
                .expect("invalid identity must fail");
            assert!(error.to_string().contains(expected), "{body}: {error:#}");
        }
        write(&valid);
        assert_eq!(
            probe_script(program, temp.path(), &cancellation)
                .await
                .unwrap()
                .executable,
            executable
        );
        // Keep real spawn failure coverage: an explicit interpreter would read
        // a non-executable script, so these cases use the production launcher.
        fs::set_permissions(&executable, fs::Permissions::from_mode(0o644)).unwrap();
        assert!(
            identify_node(program, temp.path(), &cancellation)
                .await
                .is_err()
        );
        fs::remove_file(&executable).unwrap();
        assert!(
            identify_node(program, temp.path(), &cancellation)
                .await
                .is_err()
        );
        write(&valid);
        cancellation.cancel();
        assert!(
            identify_node(program, temp.path(), &cancellation)
                .await
                .is_err()
        );
    }

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
