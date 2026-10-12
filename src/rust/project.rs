use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ExecutionScope {
    pub package: String,
    pub binary: String,
    pub source: PathBuf,
    pub edition: String,
    pub lockfile_fingerprint: String,
    pub default_features: bool,
    pub test_scope: String,
    pub profile: String,
    pub rustc: Option<String>,
    pub cargo: Option<String>,
    pub discovery_exclusions: Vec<String>,
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
            manifest.get("lib").is_none() && !root.join("src/lib.rs").exists(),
            "initial Rust support requires a binary package without a library"
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
        let bins = manifest.get("bin").and_then(toml::Value::as_array);
        ensure!(
            bins.is_none_or(|v| v.len() == 1),
            "initial Rust support requires exactly one binary"
        );
        let binary = bins
            .and_then(|v| v[0].get("name"))
            .and_then(toml::Value::as_str)
            .unwrap_or(name);
        let source = bins
            .and_then(|v| v[0].get("path"))
            .and_then(toml::Value::as_str)
            .unwrap_or("src/main.rs");
        let source = inside(root, &root.join(source))?;
        let lock = inside(root, &root.join("Cargo.lock"))
            .context("Rust requires a checked-in usable Cargo.lock")?;
        let edition = match package.get("edition") {
            Some(edition) => edition
                .as_str()
                .context("inherited Cargo editions are not supported yet")?,
            None => "2015",
        };
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
            for input in [&source, Path::new("Cargo.toml"), &lock] {
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
        Ok(Self {
            scope: ExecutionScope {
                package: name.into(),
                binary: binary.into(),
                source,
                edition: edition.into(),
                lockfile_fingerprint: format!(
                    "sha256:{:x}",
                    Sha256::digest(fs::read(root.join(lock))?)
                ),
                default_features: true,
                test_scope: "binary unit and integration tests; ignored tests excluded".into(),
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
        let mut binaries = 0;
        for target in package["targets"]
            .as_array()
            .context("Cargo metadata has no targets")?
        {
            let kind = target["kind"]
                .as_array()
                .context("Cargo target has no kind")?;
            ensure!(
                kind.len() == 1 && matches!(kind[0].as_str(), Some("bin" | "test")),
                "only a binary and integration tests are supported Rust targets"
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
            if kind[0] == "bin" {
                binaries += 1;
                ensure!(
                    target["name"].as_str() == Some(&self.scope.binary)
                        && path == self.scope.source,
                    "Cargo binary differs from the selected target"
                );
            }
        }
        ensure!(
            binaries == 1,
            "initial Rust support requires exactly one binary"
        );
        Ok(())
    }
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
    fn rejects_unsupported_layouts_before_building() -> Result<()> {
        for (extra, expected) in [
            ("build = 'generate.rs'", "build scripts"),
            ("[lib]\npath = 'src/main.rs'", "without a library"),
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
        assert_eq!(project.scope.source, Path::new("src/main.rs"));
        assert!(project.excluded.contains(&temp.path().join("target")));
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
