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
    #[serde(default, skip_serializing_if = "String::is_empty")]
    source_root: String,
    manifest_fingerprint: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    conftest_fingerprints: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    support_fingerprints: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    data_fingerprints: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    data_directories: Vec<String>,
    recipe_fingerprint: String,
    test_files: Vec<String>,
    test_nodes: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    expected_failure_nodes: Vec<String>,
    timeout_millis: u128,
    capture_bytes: usize,
    evidence_file_bytes: u64,
}

fn fingerprint(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

pub(super) fn is_test_file(file: &str) -> bool {
    let path = Path::new(file);
    !file.contains('\0')
        && (path.starts_with("tests") || path.components().count() == 1)
        && path.components().all(|c| matches!(c, Component::Normal(_)))
        && path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.starts_with("test_") && n.ends_with(".py"))
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
        path.starts_with(Path::new(&self.source_root).join(&self.package))
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

    pub(super) fn bind_expected_failures(&mut self, nodes: Vec<String>) -> Result<()> {
        self.expected_failure_nodes = nodes;
        self.validate_ready()
    }

    pub(super) fn validate_expected_failures(&self, nodes: &[String]) -> Result<()> {
        ensure!(
            self.expected_failure_nodes == nodes,
            "Python expected failures differ from pristine baseline"
        );
        Ok(())
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
            source_root: project
                .get("source_root")
                .map(|value| value.as_str().context("invalid Python source root"))
                .transpose()?
                .unwrap_or("")
                .into(),
            manifest_fingerprint: project["files"]["pyproject.toml"]
                .as_str()
                .context("missing Python manifest identity")?
                .into(),
            support_fingerprints: ["pytest.ini", "tests/__init__.py"]
                .into_iter()
                .filter_map(|path| project["files"].get(path).map(|hash| (path, hash)))
                .map(|(path, hash)| {
                    Ok((
                        path.to_owned(),
                        hash.as_str()
                            .context("invalid Python support file identity")?
                            .to_owned(),
                    ))
                })
                .collect::<Result<_>>()?,
            recipe_fingerprint: recipe(),
            data_directories: project
                .get("directories")
                .map(|value| serde_json::from_value(value.clone()))
                .transpose()?
                .unwrap_or_default(),
            data_fingerprints: project
                .get("data_files")
                .map(|value| serde_json::from_value::<Vec<String>>(value.clone()))
                .transpose()?
                .unwrap_or_default()
                .into_iter()
                .map(|path| {
                    let hash = project["files"][&path]
                        .as_str()
                        .context("missing Python data identity")?
                        .to_owned();
                    Ok((path, hash))
                })
                .collect::<Result<_>>()?,
            conftest_fingerprints: project
                .get("conftests")
                .and_then(Value::as_array)
                .context("missing Python conftest inventory")?
                .iter()
                .map(|path| {
                    let path = path.as_str().context("invalid Python conftest path")?;
                    let hash = project["files"][path]
                        .as_str()
                        .context("missing Python conftest identity")?;
                    Ok((path.to_owned(), hash.to_owned()))
                })
                .collect::<Result<_>>()?,
            test_files: serde_json::from_value(project["tests"].clone())?,
            test_nodes: Vec::new(),
            expected_failure_nodes: Vec::new(),
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
            self.support_fingerprints
                .iter()
                .all(
                    |(path, hash)| matches!(path.as_str(), "pytest.ini" | "tests/__init__.py")
                        && valid(hash)
                ),
            "invalid saved Python support identity"
        );
        ensure!(
            self.conftest_fingerprints
                .iter()
                .all(
                    |(path, hash)| matches!(path.as_str(), "conftest.py" | "tests/conftest.py")
                        && valid(hash)
                ),
            "invalid saved Python conftest identity"
        );
        ensure!(
            self.data_directories.windows(2).all(|w| w[0] < w[1])
                && self.data_directories.iter().all(|path| !path.is_empty()
                    && !path.contains('\0')
                    && Path::new(path)
                        .components()
                        .all(|c| matches!(c, Component::Normal(_)))),
            "invalid saved Python data directories"
        );
        ensure!(
            self.data_fingerprints
                .iter()
                .all(|(path, hash)| !path.is_empty()
                    && !path.contains('\0')
                    && Path::new(path)
                        .components()
                        .all(|c| matches!(c, Component::Normal(_)))
                    && !matches!(
                        Path::new(path).extension().and_then(|s| s.to_str()),
                        Some("py" | "pyc")
                    )
                    && valid(hash)),
            "invalid saved Python data identity"
        );
        ensure!(
            matches!(self.source_root.as_str(), "" | "src")
                && !self.package.is_empty()
                && Path::new(&self.package).components().count() == 1
                && Path::new(&self.package)
                    .components()
                    .all(|c| matches!(c, Component::Normal(_))),
            "invalid saved Python package"
        );
        let unique: BTreeSet<_> = self.test_files.iter().collect();
        let expected: BTreeSet<_> = self.expected_failure_nodes.iter().collect();
        ensure!(
            expected.len() == self.expected_failure_nodes.len()
                && expected.iter().all(|node| self.test_nodes.contains(node))
                && (expected.is_empty() || expected.len() < self.test_nodes.len()),
            "invalid saved Python expected failures"
        );
        ensure!(
            !unique.is_empty()
                && unique.len() == self.test_files.len()
                && self.test_files.iter().all(|file| is_test_file(file)),
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
                && self.source_root == current.source_root
                && self.manifest_fingerprint == current.manifest_fingerprint
                && self.conftest_fingerprints == current.conftest_fingerprints
                && self.support_fingerprints == current.support_fingerprints
                && self.data_fingerprints == current.data_fingerprints
                && self.data_directories == current.data_directories
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
            "tests":["tests/test_sample.py"], "conftests":[]});
        let mut missing_inventory = project.clone();
        missing_inventory
            .as_object_mut()
            .unwrap()
            .remove("conftests");
        assert!(
            ExecutionScope::capture(
                runtime.clone(),
                &missing_inventory,
                Duration::from_secs(5),
                NonZeroUsize::new(4096).unwrap(),
            )
            .is_err()
        );
        let original = ExecutionScope::capture(
            runtime,
            &project,
            Duration::from_secs(5),
            NonZeroUsize::new(4096).unwrap(),
        )?;
        let mut stronger = original.clone();
        let mut with_expected = original.clone();
        with_expected.bind(vec![
            "tests/test_sample.py::test_a".into(),
            "tests/test_sample.py::test_b".into(),
        ])?;
        with_expected.bind_expected_failures(vec!["tests/test_sample.py::test_a".into()])?;
        with_expected.validate_expected_failures(&["tests/test_sample.py::test_a".into()])?;
        assert!(with_expected.validate_expected_failures(&[]).is_err());
        assert!(
            with_expected
                .validate_expected_failures(&["tests/test_sample.py::test_b".into()])
                .is_err()
        );
        assert!(
            with_expected
                .bind_expected_failures(with_expected.test_nodes.clone())
                .is_err()
        );
        let mut with_data = original.clone();
        with_data
            .data_fingerprints
            .insert("tests/data/input.toml".into(), hash.clone());
        with_data.validate_record()?;
        assert!(original.validate_rerun(&with_data).is_err());
        let mut with_directory = original.clone();
        with_directory.data_directories = vec!["tests/data/empty".into()];
        with_directory.validate_record()?;
        assert!(original.validate_rerun(&with_directory).is_err());
        for directories in [vec!["../escape"], vec!["b", "a"], vec!["same", "same"]] {
            with_directory.data_directories = directories.into_iter().map(str::to_owned).collect();
            assert!(with_directory.validate_record().is_err());
        }
        for path in ["../escape", "/absolute", "tests/code.py", ""] {
            let mut invalid = with_data.clone();
            invalid.data_fingerprints.insert(path.into(), hash.clone());
            assert!(invalid.validate_record().is_err());
        }
        stronger.test_files.push("tests/test_stronger.py".into());
        stronger.timeout_millis = 9000;
        stronger.capture_bytes = 8192;
        original.validate_rerun(&stronger)?;
        let mut fixtures = original.clone();
        fixtures
            .conftest_fingerprints
            .insert("tests/conftest.py".into(), hash.clone());
        fixtures.validate_record()?;
        assert!(original.validate_rerun(&fixtures).is_err());
        let mut changed_fixtures = fixtures.clone();
        changed_fixtures
            .conftest_fingerprints
            .insert("tests/conftest.py".into(), fingerprint(b"changed fixture"));
        assert!(fixtures.validate_rerun(&changed_fixtures).is_err());
        changed_fixtures
            .conftest_fingerprints
            .insert("../conftest.py".into(), hash.clone());
        assert!(changed_fixtures.validate_record().is_err());
        for path in ["pytest.ini", "tests/__init__.py"] {
            let mut support = original.clone();
            support
                .support_fingerprints
                .insert(path.into(), hash.clone());
            support.validate_record()?;
            assert!(original.validate_rerun(&support).is_err());
            let mut changed = support.clone();
            changed
                .support_fingerprints
                .insert(path.into(), fingerprint(b"changed"));
            assert!(support.validate_rerun(&changed).is_err());
            assert!(support.validate_rerun(&original).is_err());
            changed
                .support_fingerprints
                .insert("other.py".into(), hash.clone());
            assert!(changed.validate_record().is_err());
        }
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
        let mut relocated = original.clone();
        relocated.source_root = "src".into();
        relocated.validate_record()?;
        assert!(relocated.admits_source(Path::new("src/sample/value.py")));
        assert!(!relocated.admits_source(Path::new("sample/value.py")));
        assert!(!relocated.admits_source(Path::new("src/sample/../outside.py")));
        assert!(original.validate_rerun(&relocated).is_err());
        for invalid in [".", "../src", "/src", "src/", "other"] {
            relocated.source_root = invalid.into();
            assert!(relocated.validate_record().is_err());
        }
        stronger.test_files.push("../hidden.py".into());
        assert!(stronger.validate_record().is_err());
        let mut value = serde_json::to_value(original)?;
        value["extra"] = json!(true);
        assert!(serde_json::from_value::<ExecutionScope>(value).is_err());
        Ok(())
    }
}
