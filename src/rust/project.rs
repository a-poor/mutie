use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(try_from = "ScopeRecord")]
pub struct ExecutionScope {
    pub package: String,
    pub targets: Vec<ProductionTarget>,
    // Reports lacking this evidence remain inspectable, but cannot validate a rerun.
    pub manifest_fingerprint: Option<String>,
    pub lockfile_fingerprint: String,
    pub default_features: bool,
    pub test_scope: String,
    pub profile: String,
    pub rustc: Option<String>,
    pub cargo: Option<String>,
    pub discovery_exclusions: Vec<String>,
}

// Accept the original single-binary report shape without emitting it in new reports.
#[derive(Deserialize)]
struct ScopeRecord {
    package: String,
    targets: Option<Vec<ProductionTarget>>,
    binary: Option<String>,
    source: Option<PathBuf>,
    edition: Option<String>,
    // Reports lacking this evidence remain inspectable, but cannot validate a rerun.
    manifest_fingerprint: Option<String>,
    lockfile_fingerprint: String,
    default_features: bool,
    test_scope: String,
    profile: String,
    rustc: Option<String>,
    cargo: Option<String>,
    discovery_exclusions: Vec<String>,
}

impl TryFrom<ScopeRecord> for ExecutionScope {
    type Error = anyhow::Error;

    fn try_from(record: ScopeRecord) -> Result<Self> {
        let targets = match (record.targets, record.binary, record.source, record.edition) {
            (Some(targets), None, None, None) => targets,
            (None, Some(name), Some(source), Some(edition)) => vec![ProductionTarget {
                kind: TargetKind::Bin,
                name,
                source,
                edition,
            }],
            _ => anyhow::bail!("Rust report requires one unambiguous target identity"),
        };
        validate_targets(&targets)?;
        Ok(Self {
            targets,
            package: record.package,
            manifest_fingerprint: record.manifest_fingerprint,
            lockfile_fingerprint: record.lockfile_fingerprint,
            default_features: record.default_features,
            test_scope: record.test_scope,
            profile: record.profile,
            rustc: record.rustc,
            cargo: record.cargo,
            discovery_exclusions: record.discovery_exclusions,
        })
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum TargetKind {
    Bin,
    Lib,
}

impl TargetKind {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Bin => "binary",
            Self::Lib => "library",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct ProductionTarget {
    pub kind: TargetKind,
    pub name: String,
    pub source: PathBuf,
    pub edition: String,
}

fn validate_targets(targets: &[ProductionTarget]) -> Result<()> {
    ensure!(!targets.is_empty(), "Rust target inventory is empty");
    let mut kinds = BTreeSet::new();
    for target in targets {
        ensure!(
            kinds.insert(&target.kind),
            "duplicate Rust production target kind"
        );
        ensure!(!target.name.is_empty(), "Rust target name is empty");
        ensure!(
            matches!(target.edition.as_str(), "2015" | "2018" | "2021" | "2024"),
            "unsupported Rust target edition"
        );
        ensure!(
            !target.source.as_os_str().is_empty()
                && target
                    .source
                    .components()
                    .all(|c| matches!(c, std::path::Component::Normal(_))),
            "Rust target source must be a relative path without traversal"
        );
    }
    ensure!(
        targets.windows(2).all(|pair| pair[0] < pair[1]),
        "Rust target inventory is not sorted"
    );
    Ok(())
}

impl ExecutionScope {
    pub fn validate_rerun_record(&self) -> Result<()> {
        ensure!(
            self.manifest_fingerprint
                .as_ref()
                .is_some_and(|s| !s.is_empty())
                && self.rustc.as_ref().is_some_and(|s| !s.is_empty())
                && self.cargo.as_ref().is_some_and(|s| !s.is_empty()),
            "saved Rust run lacks manifest or toolchain identity; start a fresh scan before rerunning"
        );
        Ok(())
    }

    pub fn validate_rerun(&self, current: &Self) -> Result<()> {
        self.validate_rerun_record()?;
        for (matches, setting) in [
            (self.package == current.package, "package"),
            (self.targets == current.targets, "production targets"),
            (
                self.manifest_fingerprint == current.manifest_fingerprint,
                "Cargo.toml fingerprint",
            ),
            (
                self.lockfile_fingerprint == current.lockfile_fingerprint,
                "Cargo.lock fingerprint",
            ),
            (
                self.default_features == current.default_features,
                "features",
            ),
            (self.test_scope == current.test_scope, "test scope"),
            (self.profile == current.profile, "profile"),
            (self.rustc == current.rustc, "rustc toolchain"),
            (self.cargo == current.cargo, "Cargo toolchain"),
        ] {
            ensure!(
                matches,
                "Rust rerun {setting} changed; start a fresh scan; no mutations were run"
            );
        }
        // Discovery exclusions are diagnostics, not execution settings. Tests
        // and unselected files may change; selected identities are checked later.
        Ok(())
    }
}

pub struct Project {
    pub scope: ExecutionScope,
    pub excluded: Vec<PathBuf>,
}

impl Project {
    pub fn load(root: &Path) -> Result<Self> {
        ensure!(cfg!(unix), "initial Rust support requires a Unix host");
        let manifest = read_manifest(&root.join("Cargo.toml"))?;
        let package = manifest
            .get("package")
            .context("Rust requires one package, not a virtual workspace")?;
        let name = package
            .get("name")
            .and_then(toml::Value::as_str)
            .context("Cargo package.name must be a string")?;
        ensure!(
            package.get("workspace").is_none(),
            "enclosing Cargo workspaces are not supported yet"
        );
        ensure!(
            package
                .get("build")
                .is_none_or(|v| v.as_bool() == Some(false))
                && !root.join("build.rs").exists(),
            "project build scripts are not supported yet"
        );
        ensure!(
            manifest.get("patch").is_none() && manifest.get("replace").is_none(),
            "Cargo dependency overrides are not supported yet"
        );
        ensure!(
            manifest.get("profile").is_none(),
            "custom Cargo profiles are not supported yet"
        );
        validate_dependencies(&manifest)?;
        if let Some(workspace) = manifest.get("workspace") {
            for key in ["members", "default-members"] {
                ensure!(
                    workspace.get(key).is_none_or(|v| v
                        .as_array()
                        .is_some_and(|v| v.iter().all(|m| m.as_str() == Some(".")))),
                    "multi-package Cargo workspaces are not supported yet"
                );
            }
        } else {
            for ancestor in root.ancestors().skip(1) {
                let path = ancestor.join("Cargo.toml");
                if path.is_file() {
                    ensure!(
                        read_manifest(&path)?.get("workspace").is_none(),
                        "package belongs to an enclosing workspace; workspace member selection is not supported yet"
                    );
                }
            }
        }
        for kind in ["bin", "test", "example", "bench"] {
            if let Some(targets) = manifest.get(kind).and_then(toml::Value::as_array) {
                ensure!(
                    !matches!(kind, "example" | "bench"),
                    "Rust examples and benchmarks are outside the initial test scope"
                );
                for target in targets {
                    ensure!(
                        target
                            .get("harness")
                            .is_none_or(|v| v.as_bool() == Some(true))
                            && target.get("test").is_none_or(|v| v.as_bool() == Some(true)),
                        "Rust targets require the standard enabled test harness"
                    );
                    ensure!(
                        target.get("required-features").is_none(),
                        "feature-gated Cargo targets are not supported yet"
                    );
                }
            }
        }
        let targets = production_targets(root, &manifest, package, name)?;
        let lock = inside(root, &root.join("Cargo.lock"))
            .context("Rust requires a checked-in usable Cargo.lock")?;
        let mut excluded = BTreeSet::from([root.join("target")]);
        validate_environment(root, &mut excluded)?;
        let mut configs = BTreeSet::new();
        for ancestor in root.ancestors() {
            ensure!(
                !ancestor.join("rust-toolchain").exists()
                    && !ancestor.join("rust-toolchain.toml").exists(),
                "Rust toolchain override files are not supported yet"
            );
            configs.insert(ancestor.join(".cargo/config"));
            configs.insert(ancestor.join(".cargo/config.toml"));
        }
        if let Some(home) = std::env::var_os("CARGO_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|p| PathBuf::from(p).join(".cargo")))
        {
            configs.insert(home.join("config"));
            configs.insert(home.join("config.toml"));
        }
        for path in configs.into_iter().filter(|p| p.is_file()) {
            let config = read_manifest(&path)?;
            for (key, value) in config.as_table().context("invalid Cargo config")? {
                match key.as_str() {
                    "net" | "term" => {}
                    "build" => {
                        for (key, value) in
                            value.as_table().context("invalid Cargo build config")?
                        {
                            ensure!(
                                matches!(key.as_str(), "target-dir" | "build-dir"),
                                "unsupported Cargo setting build.{key} in {}",
                                path.display()
                            );
                            let value = value
                                .as_str()
                                .context("Cargo artifact directory must be a string")?;
                            ensure!(
                                !value.contains(['{', '}']),
                                "templated Cargo artifact paths are not supported yet"
                            );
                            let base = path
                                .parent()
                                .and_then(Path::parent)
                                .context("invalid Cargo config path")?;
                            excluded.insert(crate::config::resolve_output_path(&base.join(value))?);
                        }
                    }
                    _ => anyhow::bail!(
                        "unsupported Cargo configuration section {key} in {}",
                        path.display()
                    ),
                }
            }
        }
        for path in &excluded {
            ensure!(
                !root.starts_with(path),
                "Cargo artifact directory overlaps project root: {}",
                path.display()
            );
            // A conventional artifact directory can be inside the root; never
            // silently exclude the selected source or manifest/lockfile.
            for input in targets
                .iter()
                .map(|target| target.source.as_path())
                .chain([Path::new("Cargo.toml"), &lock])
            {
                ensure!(
                    !root.join(input).starts_with(path),
                    "Cargo artifact directory overlaps source input: {}",
                    path.display()
                );
            }
            ensure!(
                !path.starts_with(root) || path.starts_with(root.join("target")),
                "Cargo artifact directory overlaps possible source/test inputs: {}; use target/ or a directory outside the project",
                path.display()
            );
        }
        let test_scope = if targets.iter().any(|target| target.kind == TargetKind::Lib) {
            "library/binary unit and integration tests; doctests and ignored tests excluded"
        } else {
            // Preserve the execution identity of existing binary-only reports.
            "binary unit and integration tests; ignored tests excluded"
        };
        Ok(Self {
            scope: ExecutionScope {
                package: name.into(),
                targets,
                manifest_fingerprint: Some(format!(
                    "sha256:{:x}",
                    Sha256::digest(fs::read(root.join("Cargo.toml"))?)
                )),
                lockfile_fingerprint: format!(
                    "sha256:{:x}",
                    Sha256::digest(fs::read(root.join(lock))?)
                ),
                default_features: true,
                test_scope: test_scope.into(),
                profile: "test".into(),
                rustc: None,
                cargo: None,
                discovery_exclusions: Vec::new(),
            },
            excluded: excluded.into_iter().collect(),
        })
    }

    pub fn validate_metadata(&self, root: &Path, value: &serde_json::Value) -> Result<()> {
        let members = value["workspace_members"]
            .as_array()
            .context("Cargo metadata has no workspace members")?;
        let packages = value["packages"]
            .as_array()
            .context("Cargo metadata has no packages")?;
        ensure!(
            members.len() == 1 && packages.len() == 1,
            "initial Rust support requires one Cargo package"
        );
        ensure!(
            value["workspace_root"].as_str().map(Path::new) == Some(root),
            "Cargo resolved a workspace outside the snapshot"
        );
        let package = &packages[0];
        ensure!(
            package["name"].as_str() == Some(&self.scope.package) && members[0] == package["id"],
            "Cargo package identity differs from the selected manifest"
        );
        ensure!(
            package["manifest_path"].as_str().map(Path::new)
                == Some(root.join("Cargo.toml").as_path()),
            "Cargo manifest escaped the snapshot"
        );
        let mut targets = Vec::new();
        for target in package["targets"]
            .as_array()
            .context("Cargo metadata has no targets")?
        {
            let kind = target["kind"]
                .as_array()
                .context("Cargo target has no kind")?;
            ensure!(
                kind.len() == 1
                    && matches!(kind[0].as_str(), Some("bin" | "test" | "lib" | "rlib")),
                "only ordinary libraries, one binary and integration tests are supported Rust targets"
            );
            ensure!(target["test"] == true, "Rust targets must enable tests");
            let path = inside(
                root,
                Path::new(
                    target["src_path"]
                        .as_str()
                        .context("Cargo target has no source")?,
                ),
            )?;
            ensure!(
                !self
                    .excluded
                    .iter()
                    .filter_map(|p| p.strip_prefix(root).ok())
                    .any(|p| path.starts_with(p)),
                "Cargo target lies inside an excluded artifact directory"
            );
            if kind[0] != "test" {
                ensure!(
                    target["crate_types"] == target["kind"],
                    "unsupported Cargo production crate type"
                );
                targets.push(ProductionTarget {
                    kind: if kind[0] == "bin" {
                        TargetKind::Bin
                    } else {
                        TargetKind::Lib
                    },
                    name: target["name"]
                        .as_str()
                        .context("Cargo target has no name")?
                        .into(),
                    source: path,
                    edition: target["edition"]
                        .as_str()
                        .context("Cargo target has no edition")?
                        .into(),
                });
            }
        }
        targets.sort();
        validate_targets(&targets)?;
        ensure!(
            targets == self.scope.targets,
            "Cargo production targets differ from the selected manifest"
        );
        Ok(())
    }
}

fn production_targets(
    root: &Path,
    manifest: &toml::Value,
    package: &toml::Value,
    package_name: &str,
) -> Result<Vec<ProductionTarget>> {
    let mut targets = Vec::new();
    let empty = toml::Value::Table(Default::default());
    let edition = |target: &toml::Value| -> Result<String> {
        Ok(target
            .get("edition")
            .or_else(|| package.get("edition"))
            .map(|value| {
                value
                    .as_str()
                    .context("inherited Cargo editions are not supported yet")
            })
            .transpose()?
            .unwrap_or("2015")
            .into())
    };
    let string = |target: &toml::Value, key: &str| -> Result<Option<String>> {
        target
            .get(key)
            .map(|value| {
                value
                    .as_str()
                    .map(str::to_owned)
                    .with_context(|| format!("Cargo target {key} must be a string"))
            })
            .transpose()
    };
    if manifest.get("lib").is_some()
        || (package.get("autolib").and_then(toml::Value::as_bool) != Some(false)
            && root.join("src/lib.rs").is_file())
    {
        let library = manifest.get("lib").unwrap_or(&empty);
        ensure!(library.as_table().is_some(), "Cargo lib must be a table");
        ensure!(
            library
                .get("proc-macro")
                .is_none_or(|v| v.as_bool() == Some(false)),
            "Rust proc macros are not supported yet"
        );
        ensure!(
            library
                .get("crate-type")
                .is_none_or(|v| v
                    .as_array()
                    .is_some_and(|types| types.len() == 1
                        && matches!(types[0].as_str(), Some("lib" | "rlib")))),
            "only ordinary lib/rlib crate types are supported"
        );
        ensure!(
            library
                .get("harness")
                .is_none_or(|v| v.as_bool() == Some(true))
                && library
                    .get("test")
                    .is_none_or(|v| v.as_bool() == Some(true)),
            "Rust targets require the standard enabled test harness"
        );
        ensure!(
            library.get("required-features").is_none(),
            "feature-gated Cargo targets are not supported yet"
        );
        targets.push(ProductionTarget {
            kind: TargetKind::Lib,
            name: string(library, "name")?.unwrap_or_else(|| package_name.replace('-', "_")),
            source: inside(
                root,
                &root.join(string(library, "path")?.unwrap_or_else(|| "src/lib.rs".into())),
            )?,
            edition: edition(library)?,
        });
    }
    let mut binaries = std::collections::BTreeMap::new();
    {
        if root.join("src/main.rs").is_file() {
            binaries.insert(package_name.to_owned(), PathBuf::from("src/main.rs"));
        }
        let directory = root.join("src/bin");
        if directory.is_dir() {
            for entry in fs::read_dir(directory)? {
                let path = entry?.path();
                let candidate = if path.is_file() && path.extension().is_some_and(|e| e == "rs") {
                    Some((path.file_stem(), path.clone()))
                } else if path.is_dir() && path.join("main.rs").is_file() {
                    Some((path.file_name(), path.join("main.rs")))
                } else {
                    None
                };
                if let Some((name, path)) = candidate {
                    let name = name
                        .and_then(|n| n.to_str())
                        .context("Cargo binary name is not UTF-8")?;
                    ensure!(
                        binaries
                            .insert(name.to_owned(), path.strip_prefix(root)?.to_owned())
                            .is_none(),
                        "ambiguous automatic Cargo binary target {name}"
                    );
                }
            }
        }
    }
    let mut explicit_names = BTreeSet::new();
    if let Some(bins) = manifest.get("bin") {
        for binary in bins.as_array().context("Cargo bin must be an array")? {
            let name = string(binary, "name")?.context("explicit Cargo binary requires a name")?;
            ensure!(
                explicit_names.insert(name.clone()),
                "duplicate Cargo binary target {name}"
            );
            let path = string(binary, "path")?
                .map(PathBuf::from)
                .or_else(|| binaries.get(&name).cloned())
                .context(
                    "explicit Cargo binary requires a path when not automatically discovered",
                )?;
            let source = inside(root, &root.join(path))?;
            binaries.remove(&name);
            binaries.retain(|_, candidate| {
                root.join(candidate).canonicalize().ok() != Some(root.join(&source))
            });
            targets.push(ProductionTarget {
                kind: TargetKind::Bin,
                name,
                source,
                edition: edition(binary)?,
            });
        }
    }
    if package.get("autobins").and_then(toml::Value::as_bool) == Some(false) {
        binaries.clear();
    }
    for (name, source) in binaries {
        targets.push(ProductionTarget {
            kind: TargetKind::Bin,
            name,
            source: inside(root, &root.join(source))?,
            edition: edition(&empty)?,
        });
    }
    targets.sort();
    validate_targets(&targets)?;
    Ok(targets)
}

pub fn inside(root: &Path, path: &Path) -> Result<PathBuf> {
    let physical = path
        .canonicalize()
        .with_context(|| format!("opening Rust input {}", path.display()))?;
    Ok(physical
        .strip_prefix(root)
        .with_context(|| format!("Rust input {} is outside the project", path.display()))?
        .to_owned())
}

fn read_manifest(path: &Path) -> Result<toml::Value> {
    toml::from_str(
        &fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?,
    )
    .with_context(|| format!("parsing {}", path.display()))
}

fn validate_dependencies(value: &toml::Value) -> Result<()> {
    if let Some(table) = value.as_table() {
        for (key, value) in table {
            if matches!(
                key.as_str(),
                "dependencies" | "dev-dependencies" | "build-dependencies"
            ) {
                for dependency in value
                    .as_table()
                    .context("invalid Cargo dependency table")?
                    .values()
                {
                    ensure!(
                        dependency.get("path").is_none()
                            && dependency.get("git").is_none()
                            && dependency.get("workspace").is_none(),
                        "Rust local, git, and inherited dependencies are not supported yet"
                    );
                }
            } else if key == "target" {
                for target in value
                    .as_table()
                    .context("invalid Cargo target dependency table")?
                    .values()
                {
                    validate_dependencies(target)?;
                }
            }
        }
    }
    Ok(())
}

fn validate_environment(root: &Path, excluded: &mut BTreeSet<PathBuf>) -> Result<()> {
    for (key, value) in std::env::vars_os().filter(|(_, v)| !v.is_empty()) {
        let Some(key) = key.to_str() else { continue };
        if matches!(
            key,
            "CARGO_TARGET_DIR" | "CARGO_BUILD_TARGET_DIR" | "CARGO_BUILD_BUILD_DIR"
        ) {
            let base = std::env::current_dir()?;
            excluded.insert(crate::config::resolve_output_path(&base.join(value))?);
        } else {
            ensure!(
                !matches!(
                    key,
                    "RUSTFLAGS"
                        | "RUSTDOCFLAGS"
                        | "CARGO_ENCODED_RUSTFLAGS"
                        | "CARGO_ENCODED_RUSTDOCFLAGS"
                        | "RUSTC"
                        | "RUSTDOC"
                        | "RUSTC_WRAPPER"
                        | "RUSTC_WORKSPACE_WRAPPER"
                        | "CARGO_BUILD_TARGET"
                        | "CARGO_BUILD_RUSTC"
                        | "CARGO_BUILD_RUSTDOC"
                        | "CARGO_BUILD_RUSTC_WRAPPER"
                        | "CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER"
                        | "CARGO_BUILD_RUSTFLAGS"
                        | "CARGO_BUILD_RUSTDOCFLAGS"
                ) && !key.starts_with("CARGO_PROFILE_")
                    && !(key.starts_with("CARGO_TARGET_")
                        && (key.ends_with("_RUNNER")
                            || key.ends_with("_RUSTFLAGS")
                            || key.ends_with("_LINKER"))),
                "unsupported Rust execution override {key}"
            );
        }
    }
    // Resolve the conventional directory too, including existing internal links.
    excluded.insert(crate::config::resolve_output_path(&root.join("target"))?);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(extra: &str) -> Result<tempfile::TempDir> {
        let temp = tempfile::tempdir_in(std::env::temp_dir().canonicalize()?)?;
        fs::create_dir(temp.path().join("src"))?;
        fs::write(temp.path().join("src/main.rs"), "fn main() {}")?;
        fs::write(
            temp.path().join("Cargo.lock"),
            "version = 4\n[[package]]\nname = 'sample'\nversion = '0.0.0'\n",
        )?;
        fs::write(
            temp.path().join("Cargo.toml"),
            format!("[package]\nname = 'sample'\nversion = '0.0.0'\nedition = '2024'\n{extra}"),
        )?;
        Ok(temp)
    }

    #[test]
    fn reruns_require_matching_execution_identity_but_allow_discovery_changes() -> Result<()> {
        let temp = fixture("")?;
        let mut scope = Project::load(temp.path())?.scope;
        scope.rustc = Some("rustc test-version with host".into());
        scope.cargo = Some("cargo test-version".into());
        let saved = serde_json::to_value(&scope)?;
        let mut current = scope.clone();
        current
            .discovery_exclusions
            .push("new test-only module".into());
        scope.validate_rerun(&current)?;
        for (field, value, expected) in [
            ("package", serde_json::json!("other"), "package"),
            (
                "manifest_fingerprint",
                serde_json::json!("changed"),
                "Cargo.toml",
            ),
            (
                "lockfile_fingerprint",
                serde_json::json!("changed"),
                "Cargo.lock",
            ),
            ("default_features", serde_json::json!(false), "features"),
            ("test_scope", serde_json::json!("library"), "test scope"),
            ("profile", serde_json::json!("release"), "profile"),
            ("rustc", serde_json::json!("other rustc"), "rustc toolchain"),
            ("cargo", serde_json::json!("other cargo"), "Cargo toolchain"),
        ] {
            let mut changed = saved.clone();
            changed[field] = value;
            let changed: ExecutionScope = serde_json::from_value(changed)?;
            let error = scope.validate_rerun(&changed).unwrap_err();
            assert!(error.to_string().contains(expected), "{field}: {error}");
        }
        for field in ["manifest_fingerprint", "rustc", "cargo"] {
            let mut legacy = saved.clone();
            legacy.as_object_mut().unwrap().remove(field);
            let legacy: ExecutionScope = serde_json::from_value(legacy)?;
            assert!(
                legacy
                    .validate_rerun_record()
                    .unwrap_err()
                    .to_string()
                    .contains("fresh scan")
            );
        }
        Ok(())
    }

    #[test]
    fn legacy_binary_reports_normalize_and_preserve_rerun_requirements() -> Result<()> {
        let temp = fixture("")?;
        let mut scope = Project::load(temp.path())?.scope;
        scope.rustc = Some("rustc test-version".into());
        scope.cargo = Some("cargo test-version".into());
        let mut legacy = serde_json::to_value(&scope)?;
        legacy.as_object_mut().unwrap().remove("targets");
        legacy["binary"] = "sample".into();
        legacy["source"] = "src/main.rs".into();
        legacy["edition"] = "2024".into();
        let restored: ExecutionScope = serde_json::from_value(legacy.clone())?;
        restored.validate_rerun(&scope)?;
        scope.validate_rerun(&restored)?;
        let rewritten = serde_json::to_value(restored)?;
        assert!(rewritten.get("binary").is_none());
        assert_eq!(rewritten["targets"], serde_json::to_value(&scope.targets)?);
        for field in ["manifest_fingerprint", "rustc", "cargo"] {
            let mut incomplete = legacy.clone();
            incomplete.as_object_mut().unwrap().remove(field);
            let readable: ExecutionScope = serde_json::from_value(incomplete)?;
            assert!(readable.validate_rerun(&scope).is_err());
        }
        Ok(())
    }

    #[test]
    fn target_identity_changes_require_a_fresh_scan() -> Result<()> {
        let temp = fixture("")?;
        let mut scope = Project::load(temp.path())?.scope;
        scope.rustc = Some("rustc test-version".into());
        scope.cargo = Some("cargo test-version".into());
        let saved = serde_json::to_value(&scope)?;
        for (field, value) in [
            ("name", "renamed"),
            ("source", "src/other.rs"),
            ("edition", "2021"),
            ("kind", "lib"),
        ] {
            let mut changed = saved.clone();
            changed["targets"][0][field] = value.into();
            let changed: ExecutionScope = serde_json::from_value(changed)?;
            assert!(
                scope
                    .validate_rerun(&changed)
                    .unwrap_err()
                    .to_string()
                    .contains("production targets")
            );
        }
        let mut expanded = scope.clone();
        let mut library = scope.targets[0].clone();
        library.kind = TargetKind::Lib;
        library.source = "src/lib.rs".into();
        expanded.targets.push(library);
        // An added target and a removed target both invalidate a saved run.
        assert!(scope.validate_rerun(&expanded).is_err());
        assert!(expanded.validate_rerun(&scope).is_err());
        let roundtrip: ExecutionScope = serde_json::from_value(serde_json::to_value(&expanded)?)?;
        roundtrip.validate_rerun(&expanded)?;
        Ok(())
    }

    #[test]
    fn rejects_ambiguous_or_malformed_saved_target_inventories() -> Result<()> {
        let temp = fixture("")?;
        let saved = serde_json::to_value(Project::load(temp.path())?.scope)?;
        for (field, value) in [
            ("name", ""),
            ("source", ""),
            ("source", "/tmp/main.rs"),
            ("source", "../main.rs"),
            ("edition", "future"),
            ("kind", "proc-macro"),
        ] {
            let mut changed = saved.clone();
            changed["targets"][0][field] = value.into();
            assert!(
                serde_json::from_value::<ExecutionScope>(changed).is_err(),
                "{field}: {value}"
            );
        }
        let mut duplicate = saved.clone();
        duplicate["targets"]
            .as_array_mut()
            .unwrap()
            .push(saved["targets"][0].clone());
        assert!(serde_json::from_value::<ExecutionScope>(duplicate).is_err());
        let mut empty = saved.clone();
        empty["targets"] = serde_json::json!([]);
        assert!(serde_json::from_value::<ExecutionScope>(empty).is_err());
        let mut ambiguous = saved.clone();
        ambiguous["binary"] = "other".into();
        assert!(serde_json::from_value::<ExecutionScope>(ambiguous).is_err());
        let mut reversed = saved.clone();
        let mut library = saved["targets"][0].clone();
        library["kind"] = "lib".into();
        reversed["targets"] = serde_json::json!([library, saved["targets"][0]]);
        assert!(serde_json::from_value::<ExecutionScope>(reversed).is_err());
        Ok(())
    }

    #[test]
    fn metadata_must_match_selected_target_identity_and_crate_type() -> Result<()> {
        let temp = fixture("[[bin]]\nname = 'custom'\npath = 'src/main.rs'\nedition = '2021'")?;
        let root = temp.path();
        let project = Project::load(root)?;
        assert_eq!(project.scope.targets[0].edition, "2021");
        let metadata = serde_json::json!({
            "workspace_members": ["package-id"], "workspace_root": root,
            "packages": [{"id": "package-id", "name": "sample",
                "manifest_path": root.join("Cargo.toml"),
                "targets": [{"kind": ["bin"], "crate_types": ["bin"], "test": true,
                    "name": "custom", "src_path": root.join("src/main.rs"), "edition": "2021"}]}]
        });
        project.validate_metadata(root, &metadata)?;
        for (field, value) in [
            ("name", serde_json::json!("other")),
            ("edition", serde_json::json!("2024")),
            ("src_path", serde_json::json!(root.join("Cargo.toml"))),
            ("crate_types", serde_json::json!(["rlib"])),
            ("kind", serde_json::json!(["lib"])),
            ("test", serde_json::json!(false)),
        ] {
            let mut changed = metadata.clone();
            changed["packages"][0]["targets"][0][field] = value;
            assert!(
                project.validate_metadata(root, &changed).is_err(),
                "{field}"
            );
        }
        for targets in [
            serde_json::json!([]),
            serde_json::json!([
                metadata["packages"][0]["targets"][0],
                metadata["packages"][0]["targets"][0]
            ]),
        ] {
            let mut changed = metadata.clone();
            changed["packages"][0]["targets"] = targets;
            assert!(project.validate_metadata(root, &changed).is_err());
        }
        Ok(())
    }

    #[test]
    fn rejects_unsupported_layouts_before_building() -> Result<()> {
        for (extra, expected) in [
            ("build = 'generate.rs'", "build scripts"),
            ("[lib]\nproc-macro = true", "proc macros"),
            ("[workspace]\nmembers = ['sibling']", "multi-package"),
            (
                "[dependencies]\nother = { path = '../other' }",
                "dependencies",
            ),
            (
                "[target.'cfg(unix)'.dependencies]\nother = { path = 'other' }",
                "dependencies",
            ),
            (
                "[dependencies]\nother = { git = 'https://example.invalid/other' }",
                "dependencies",
            ),
            ("[patch.crates-io]\nother = { path = 'other' }", "overrides"),
            ("[[test]]\nname = 'custom'\nharness = false", "harness"),
            ("[profile.test]\nopt-level = 1", "profiles"),
        ] {
            let temp = fixture(extra)?;
            let error = Project::load(temp.path())
                .err()
                .context("unsupported project was accepted")?;
            assert!(error.to_string().contains(expected), "{extra}: {error:#}");
        }
        let temp = fixture("")?;
        let project = Project::load(temp.path())?;
        assert_eq!(project.scope.package, "sample");
        assert_eq!(project.scope.targets[0].source, Path::new("src/main.rs"));
        assert!(project.excluded.contains(&temp.path().join("target")));
        Ok(())
    }

    #[test]
    fn library_targets_follow_names_paths_editions_and_auto_discovery() -> Result<()> {
        let temp = fixture("")?;
        let root = temp.path();
        fs::write(root.join("src/lib.rs"), "pub fn enabled() -> bool { true }")?;
        let combined = Project::load(root)?;
        assert_eq!(combined.scope.targets.len(), 2);
        assert_eq!(combined.scope.targets[1].kind, TargetKind::Lib);
        fs::remove_file(root.join("src/main.rs"))?;
        assert_eq!(Project::load(root)?.scope.targets.len(), 1);
        fs::write(
            root.join("Cargo.toml"),
            "[package]\nname = 'sample-package'\nversion = '0.0.0'\nedition = '2024'\nautobins = false\n[lib]\nname = 'api'\npath = 'src/lib.rs'\nedition = '2021'\ncrate-type = ['rlib']",
        )?;
        let library = Project::load(root)?;
        assert_eq!(library.scope.targets[0].name, "api");
        assert_eq!(library.scope.targets[0].edition, "2021");
        fs::write(root.join("src/main.rs"), "fn main() {}")?;
        assert_eq!(Project::load(root)?.scope.targets.len(), 1);
        fs::write(
            root.join("Cargo.toml"),
            "[package]\nname = 'sample-package'\nversion = '0.0.0'\nedition = '2024'\nautolib = false",
        )?;
        assert_eq!(Project::load(root)?.scope.targets[0].kind, TargetKind::Bin);
        fs::create_dir(root.join("src/bin"))?;
        fs::write(root.join("src/bin/extra.rs"), "fn main() {}")?;
        assert!(Project::load(root).is_err());
        fs::remove_file(root.join("src/main.rs"))?;
        assert_eq!(Project::load(root)?.scope.targets[0].name, "extra");
        Ok(())
    }

    #[test]
    fn rejects_unsupported_library_settings_and_paths() -> Result<()> {
        for extra in [
            "proc-macro = true",
            "crate-type = ['cdylib']",
            "crate-type = ['lib', 'rlib']",
            "test = false",
            "harness = false",
            "required-features = ['optional']",
            "path = '../outside.rs'",
        ] {
            let temp = fixture(&format!("[lib]\n{extra}"))?;
            fs::write(temp.path().join("src/lib.rs"), "")?;
            assert!(Project::load(temp.path()).is_err(), "{extra}");
        }
        Ok(())
    }

    #[test]
    fn validates_workspace_membership_and_cargo_configuration() -> Result<()> {
        let parent = fixture("[workspace]\nmembers = ['child']")?;
        let child = parent.path().join("child");
        fs::create_dir_all(child.join("src"))?;
        fs::write(
            child.join("Cargo.toml"),
            "[package]\nname = 'child'\nversion = '0.0.0'",
        )?;
        fs::write(child.join("src/main.rs"), "fn main() {}")?;
        assert!(
            Project::load(&child)
                .err()
                .unwrap()
                .to_string()
                .contains("enclosing workspace")
        );
        let temp = fixture("")?;
        fs::create_dir(temp.path().join(".cargo"))?;
        fs::write(
            temp.path().join(".cargo/config.toml"),
            "[build]\nrustflags = ['--cfg', 'unexpected']",
        )?;
        assert!(
            Project::load(temp.path())
                .err()
                .unwrap()
                .to_string()
                .contains("build.rustflags")
        );
        fs::write(
            temp.path().join(".cargo/config.toml"),
            "[build]\ntarget-dir = 'src'",
        )?;
        assert!(
            Project::load(temp.path())
                .err()
                .unwrap()
                .to_string()
                .contains("overlaps source")
        );
        Ok(())
    }
}
