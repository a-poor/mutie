use std::{
    collections::{BTreeMap, BTreeSet},
    num::NonZeroUsize,
    path::{Component, Path, PathBuf},
    time::Duration,
};

use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Runtime {
    executable: PathBuf,
    python: String,
    prefix: PathBuf,
    versions: BTreeMap<String, String>,
    environment: String,
    executable_fingerprint: String,
    library_fingerprint: String,
    stdlib: String,
    packages: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ExecutionScope {
    runtime: Runtime,
    package: String,
    manifest_fingerprint: String,
    recipe_fingerprint: String,
    test_files: Vec<String>,
    test_nodes: Vec<String>,
    timeout_millis: u128,
    capture_bytes: usize,
    evidence_file_bytes: u64,
}

fn fingerprint(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

fn recipe() -> String {
    fingerprint(
        &[
            include_bytes!("../python.rs").as_slice(),
            include_bytes!("identity.rs").as_slice(),
            include_bytes!("scope.py").as_slice(),
            include_bytes!("runtime.py").as_slice(),
            include_bytes!("driver.py").as_slice(),
            include_bytes!("protocol.rs").as_slice(),
            include_bytes!("project.rs").as_slice(),
            include_bytes!("../process.rs").as_slice(),
            include_bytes!("../capture.rs").as_slice(),
            include_bytes!("../workspace.rs").as_slice(),
            include_bytes!("../command.rs").as_slice(),
            include_bytes!("../../Cargo.lock").as_slice(),
        ]
        .concat(),
    )
}

impl ExecutionScope {
    pub(crate) fn executable(&self) -> &Path {
        &self.runtime.executable
    }

    pub(crate) fn admits_source(&self, path: &Path) -> bool {
        path.starts_with(&self.package)
            && path.extension().is_some_and(|extension| extension == "py")
            && path.components().all(|c| matches!(c, Component::Normal(_)))
    }

    pub(crate) fn validate_ready(&self) -> Result<()> {
        self.validate_record()?;
        let unique: BTreeSet<_> = self.test_nodes.iter().collect();
        let files: BTreeSet<_> = self
            .test_nodes
            .iter()
            .filter_map(|node| node.split_once("::").map(|(file, _)| file))
            .collect();
        ensure!(
            !unique.is_empty()
                && unique.len() == self.test_nodes.len()
                && self
                    .test_nodes
                    .iter()
                    .all(|node| node.contains("::") && !node.contains('\0'))
                && files == self.test_files.iter().map(String::as_str).collect(),
            "Python identity lacks a complete pristine test inventory"
        );
        Ok(())
    }

    pub(super) fn bind(&mut self, nodes: Vec<String>) -> Result<()> {
        self.test_nodes = nodes;
        self.validate_ready()
    }

    pub(super) fn validate_inventory(&self, nodes: &[String]) -> Result<()> {
        self.validate_ready()?;
        ensure!(
            self.test_nodes == nodes,
            "Python candidate collection differs from pristine baseline"
        );
        Ok(())
    }

    pub(super) fn capture(
        runtime: Value,
        project: &Value,
        timeout: Duration,
        capture: NonZeroUsize,
    ) -> Result<Self> {
        let result = Self {
            runtime: serde_json::from_value(runtime).context("invalid Python runtime identity")?,
            package: project["package"]
                .as_str()
                .context("missing Python package")?
                .into(),
            manifest_fingerprint: project["files"]["pyproject.toml"]
                .as_str()
                .context("missing Python manifest identity")?
                .into(),
            recipe_fingerprint: recipe(),
            test_files: serde_json::from_value(project["tests"].clone())?,
            test_nodes: Vec::new(),
            timeout_millis: timeout.as_millis(),
            capture_bytes: capture.get(),
            evidence_file_bytes: 4 * 1024 * 1024,
        };
        result.validate_record()?;
        Ok(result)
    }

    pub(crate) fn validate_record(&self) -> Result<()> {
        ensure!(
            self.runtime.executable.is_absolute()
                && self.runtime.prefix.is_absolute()
                && self.runtime.python.starts_with("3.14.7 "),
            "unsupported saved Python runtime"
        );
        let pinned = [
            ("pytest", "9.0.2"),
            ("pluggy", "1.7.0"),
            ("packaging", "26.3"),
            ("iniconfig", "2.3.1"),
            ("pygments", "2.21.0"),
        ];
        ensure!(
            pinned.iter().all(|(name, version)| self
                .runtime
                .versions
                .get(*name)
                .is_some_and(|v| v == version))
                && self
                    .runtime
                    .versions
                    .keys()
                    .all(|name| name == "pip" || pinned.iter().any(|(p, _)| p == name)),
            "unsupported saved Python dependencies"
        );
        let valid = |value: &str| {
            value
                .strip_prefix("sha256:")
                .is_some_and(|hex| hex.len() == 64 && hex.bytes().all(|b| b.is_ascii_hexdigit()))
        };
        ensure!(
            [
                &self.runtime.environment,
                &self.runtime.executable_fingerprint,
                &self.runtime.library_fingerprint,
                &self.runtime.stdlib,
                &self.runtime.packages,
                &self.manifest_fingerprint,
                &self.recipe_fingerprint
            ]
            .into_iter()
            .all(|v| valid(v)),
            "invalid saved Python fingerprints"
        );
        ensure!(
            !self.package.is_empty()
                && Path::new(&self.package).components().count() == 1
                && Path::new(&self.package)
                    .components()
                    .all(|c| matches!(c, Component::Normal(_))),
            "invalid saved Python package"
        );
        let unique: BTreeSet<_> = self.test_files.iter().collect();
        ensure!(
            !unique.is_empty()
                && unique.len() == self.test_files.len()
                && self.test_files.iter().all(|file| {
                    let path = Path::new(file);
                    path.starts_with("tests")
                        && path.components().all(|c| matches!(c, Component::Normal(_)))
                        && path
                            .file_name()
                            .and_then(|n| n.to_str())
                            .is_some_and(|n| n.starts_with("test_") && n.ends_with(".py"))
                }),
            "invalid saved Python test inventory"
        );
        ensure!(
            self.timeout_millis > 0
                && self.capture_bytes > 0
                && self.evidence_file_bytes == 4 * 1024 * 1024,
            "invalid saved Python limits"
        );
        Ok(())
    }

    pub(crate) fn validate_rerun(&self, current: &Self) -> Result<()> {
        self.validate_record()?;
        current.validate_record()?;
        // Stronger tests and different command budgets are permitted. Shared
        // source identities will separately bind each selected mutation.
        ensure!(
            self.runtime == current.runtime
                && self.package == current.package
                && self.manifest_fingerprint == current.manifest_fingerprint
                && self.recipe_fingerprint == current.recipe_fingerprint,
            "Python execution identity changed; start a fresh run"
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn identity_rejects_drift_but_allows_stronger_tests_and_new_budgets() -> Result<()> {
        let hash = fingerprint(b"fixture");
        let runtime = json!({
            "executable":"/venv/bin/python", "prefix":"/venv", "python":"3.14.7 test",
            "versions":{"pytest":"9.0.2","pluggy":"1.7.0","packaging":"26.3",
                "iniconfig":"2.3.1","pygments":"2.21.0"},
            "environment":hash, "executable_fingerprint":hash, "library_fingerprint":hash,
            "stdlib":hash, "packages":hash,
        });
        let project = json!({"package":"sample","files":{"pyproject.toml":hash},
            "tests":["tests/test_sample.py"]});
        let original = ExecutionScope::capture(
            runtime,
            &project,
            Duration::from_secs(5),
            NonZeroUsize::new(4096).unwrap(),
        )?;
        let mut stronger = original.clone();
        stronger.test_files.push("tests/test_stronger.py".into());
        stronger.timeout_millis = 9000;
        stronger.capture_bytes = 8192;
        original.validate_rerun(&stronger)?;
        for field in ["manifest_fingerprint", "recipe_fingerprint"] {
            let mut value = serde_json::to_value(&stronger)?;
            value[field] = json!(fingerprint(b"changed"));
            let changed = serde_json::from_value(value)?;
            assert!(original.validate_rerun(&changed).is_err());
        }
        for field in [
            "environment",
            "executable_fingerprint",
            "library_fingerprint",
            "stdlib",
            "packages",
        ] {
            let mut value = serde_json::to_value(&stronger)?;
            value["runtime"][field] = json!(fingerprint(b"changed"));
            assert!(
                original
                    .validate_rerun(&serde_json::from_value(value)?)
                    .is_err()
            );
        }
        stronger.test_files.push("../hidden.py".into());
        assert!(stronger.validate_record().is_err());
        let mut value = serde_json::to_value(original)?;
        value["extra"] = json!(true);
        assert!(serde_json::from_value::<ExecutionScope>(value).is_err());
        Ok(())
    }
}
