//! Static workspace inventory, checked against Cargo metadata in an owned copy.
use super::project::{self, ExecutionScope, ProductionTarget, Project};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Component, Path, PathBuf},
};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Member {
    pub name: String,
    pub directory: PathBuf,
    pub manifest_fingerprint: String,
    pub targets: Vec<ProductionTarget>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceScope {
    pub members: Vec<Member>,
    pub selected: Vec<String>,
}

impl WorkspaceScope {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            !self.members.is_empty() && !self.selected.is_empty(),
            "empty Rust workspace inventory or selection"
        );
        let mut names = BTreeSet::new();
        let mut directories = BTreeSet::new();
        for member in &self.members {
            ensure!(
                !member.name.is_empty() && names.insert(&member.name),
                "duplicate or empty workspace package"
            );
            ensure!(
                directories.insert(&member.directory)
                    && member
                        .directory
                        .components()
                        .all(|c| matches!(c, Component::Normal(_))),
                "invalid workspace member directory"
            );
            ensure!(
                member
                    .manifest_fingerprint
                    .strip_prefix("sha256:")
                    .is_some_and(|s| s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit())),
                "invalid workspace manifest identity"
            );
            project::validate_targets(&member.targets)?;
        }
        ensure!(
            self.members.windows(2).all(|p| p[0].name < p[1].name),
            "workspace members must be sorted"
        );
        ensure!(
            self.selected.windows(2).all(|p| p[0] < p[1])
                && self.selected.iter().all(|name| names.contains(name)),
            "invalid workspace package selection"
        );
        Ok(())
    }
}

fn patterns(value: Option<&toml::Value>) -> Result<Vec<globset::GlobMatcher>> {
    value
        .map(|v| {
            v.as_array()
                .context("workspace paths must be an array")?
                .iter()
                .map(|v| {
                    let text = v.as_str().context("workspace path must be a string")?;
                    ensure!(
                        !text.is_empty()
                            && Path::new(text)
                                .components()
                                .all(|c| matches!(c, Component::Normal(_) | Component::CurDir)),
                        "workspace members must stay inside the root"
                    );
                    Ok(globset::GlobBuilder::new(text.trim_start_matches("./"))
                        .literal_separator(true)
                        .build()?
                        .compile_matcher())
                })
                .collect()
        })
        .unwrap_or_else(|| Ok(Vec::new()))
}

fn manifest_directories(root: &Path) -> Result<BTreeSet<PathBuf>> {
    let mut pending = vec![PathBuf::new()];
    let mut found = BTreeSet::new();
    let mut count = 0;
    while let Some(relative) = pending.pop() {
        count += 1;
        ensure!(
            count <= 10_000 && relative.components().count() < 64,
            "workspace directory inventory exceeds limit"
        );
        if root.join(&relative).join("Cargo.toml").is_file() {
            found.insert(relative.clone());
        }
        for entry in fs::read_dir(root.join(&relative))? {
            let entry = entry?;
            if entry.file_type()?.is_dir()
                && !matches!(
                    entry.file_name().to_str(),
                    Some("target" | ".git" | ".mutie" | "node_modules")
                )
            {
                pending.push(relative.join(entry.file_name()));
            }
        }
    }
    Ok(found)
}

fn inherited_manifest(mut manifest: toml::Value, workspace: &toml::Value) -> Result<toml::Value> {
    let package = manifest
        .get_mut("package")
        .and_then(toml::Value::as_table_mut)
        .context("workspace member requires [package]")?;
    for (key, value) in package.iter_mut() {
        if value.get("workspace").is_some() {
            ensure!(
                value.as_table().is_some_and(|v| v.len() == 1)
                    && value["workspace"].as_bool() == Some(true),
                "invalid inherited package field {key}"
            );
            *value = workspace
                .get("package")
                .and_then(|v| v.get(key))
                .with_context(|| format!("missing workspace.package.{key}"))?
                .clone();
        }
    }
    Ok(manifest)
}

fn local_dependencies(
    root: &Path,
    directory: &Path,
    manifest: &toml::Value,
    workspace: &toml::Value,
) -> Result<BTreeSet<PathBuf>> {
    let mut result = BTreeSet::new();
    let mut tables = vec![manifest];
    if let Some(targets) = manifest.get("target") {
        tables.extend(
            targets
                .as_table()
                .context("invalid target dependencies")?
                .values(),
        );
    }
    for table in tables {
        for group in ["dependencies", "dev-dependencies", "build-dependencies"] {
            if let Some(dependencies) = table.get(group) {
                for (name, original) in dependencies
                    .as_table()
                    .context("invalid dependency table")?
                {
                    let inherited = original.get("workspace").is_some();
                    let dependency = if inherited {
                        ensure!(
                            original.get("workspace").and_then(toml::Value::as_bool) == Some(true),
                            "invalid inherited dependency {name}"
                        );
                        ensure!(
                            original.as_table().unwrap().keys().all(|k| matches!(
                                k.as_str(),
                                "workspace" | "features" | "optional" | "default-features"
                            )),
                            "unsupported inherited dependency override {name}"
                        );
                        workspace
                            .get("dependencies")
                            .and_then(|v| v.get(name))
                            .with_context(|| format!("missing workspace dependency {name}"))?
                    } else {
                        original
                    };
                    ensure!(
                        dependency.get("git").is_none(),
                        "git dependencies require separate support"
                    );
                    if let Some(path) = dependency.get("path") {
                        let path = path.as_str().context("dependency path must be a string")?;
                        ensure!(
                            !Path::new(path).is_absolute(),
                            "absolute path dependencies are unsupported"
                        );
                        let base = if inherited {
                            root.to_owned()
                        } else {
                            root.join(directory)
                        };
                        let relative = project::inside(root, &base.join(path))?;
                        ensure!(
                            root.join(&relative).join("Cargo.toml").is_file(),
                            "local dependency has no Cargo.toml"
                        );
                        result.insert(relative);
                    }
                }
            }
        }
    }
    Ok(result)
}

pub(super) fn load(root: &Path, manifest: toml::Value, selected: &[String]) -> Result<Project> {
    for key in ["patch", "replace"] {
        ensure!(
            manifest.get(key).is_none(),
            "workspace Cargo dependency overrides and custom profiles require separate support"
        );
    }
    project::validate_profiles(&manifest)?;
    let workspace = &manifest["workspace"];
    ensure!(workspace.is_table(), "workspace must be a table");
    let includes = patterns(workspace.get("members"))?;
    let excludes = patterns(workspace.get("exclude"))?;
    let all = manifest_directories(root)?;
    let excluded_member = |p: &Path| excludes.iter().any(|g| g.is_match(p));
    let mut pending: BTreeSet<_> = all
        .iter()
        .filter(|p| {
            includes.iter().any(|g| {
                g.is_match(if p.as_os_str().is_empty() {
                    Path::new(".")
                } else {
                    p
                })
            }) && !excluded_member(p)
        })
        .cloned()
        .collect();
    for pattern in &includes {
        ensure!(
            all.iter()
                .any(|p| pattern.is_match(if p.as_os_str().is_empty() {
                    Path::new(".")
                } else {
                    p
                })),
            "workspace member pattern matches no package"
        );
    }
    if manifest.get("package").is_some() {
        pending.insert(PathBuf::new());
    }
    let mut members = BTreeMap::new();
    let mut build_scripts = Vec::new();
    let mut inactive_target_flags = BTreeMap::new();
    let mut seen = BTreeSet::new();
    let mut excluded = BTreeSet::from([root.join("target")]);
    let mut single = None;
    while let Some(directory) = pending.pop_first() {
        if !seen.insert(directory.clone()) {
            continue;
        }
        ensure!(seen.len() <= 256, "workspace has too many members");
        ensure!(
            !excluded_member(&directory),
            "excluded local dependency packages require separate support"
        );
        let member_root = root.join(&directory);
        let raw = project::read_manifest(&member_root.join("Cargo.toml"))?;
        ensure!(
            directory.as_os_str().is_empty() || raw.get("workspace").is_none(),
            "nested workspaces are unsupported"
        );
        if let Some(path) = raw.get("package").and_then(|p| p.get("workspace")) {
            ensure!(
                member_root
                    .join(path.as_str().context("package.workspace must be a path")?)
                    .canonicalize()?
                    == root,
                "package.workspace escapes selected workspace"
            );
        }
        pending.extend(local_dependencies(root, &directory, &raw, workspace)?);
        let resolved = inherited_manifest(raw, workspace)?;
        let project = Project::load_manifest(&member_root, &resolved, Some(root))?;
        for (target, flags) in &project.scope.inactive_target_flags {
            if let Some(previous) = inactive_target_flags.insert(target.clone(), flags.clone()) {
                ensure!(
                    previous == *flags,
                    "conflicting inactive Cargo target configuration"
                );
            }
        }
        build_scripts.extend(
            project
                .scope
                .build_scripts
                .iter()
                .cloned()
                .map(|mut script| {
                    script.path = directory.join(script.path);
                    script
                }),
        );
        excluded.extend(project.excluded.iter().cloned());
        let member = Member {
            name: project.scope.package.clone(),
            directory: directory.clone(),
            manifest_fingerprint: project.scope.manifest_fingerprint.clone().unwrap(),
            targets: project.scope.targets.clone(),
        };
        ensure!(
            members.insert(member.name.clone(), member).is_none(),
            "duplicate workspace package name"
        );
        if directory.as_os_str().is_empty() {
            single = Some(project);
        }
    }
    let chosen: Vec<_> = if selected.is_empty() {
        members.keys().cloned().collect()
    } else {
        selected.to_vec()
    };
    let scope = WorkspaceScope {
        members: members.into_values().collect(),
        selected: chosen,
    };
    scope.validate()?;
    // Keep existing standalone [workspace] packages and report identities compatible.
    if scope.members.len() == 1
        && let Some(project) = single
    {
        return Ok(project);
    }
    let lock = project::inside(root, &root.join("Cargo.lock"))
        .context("Rust requires a checked-in usable Cargo.lock")?;
    build_scripts.sort_by(|a, b| a.package.cmp(&b.package));
    Ok(Project { scope: ExecutionScope {
        package: "workspace".into(), targets: Vec::new(), workspace: Some(scope),
        manifest_fingerprint: Some(format!("sha256:{:x}", Sha256::digest(fs::read(root.join("Cargo.toml"))?))),
        lockfile_fingerprint: format!("sha256:{:x}", Sha256::digest(fs::read(root.join(lock))?)),
        default_features: true, test_scope: "all workspace library/binary unit and integration tests; doctests and ignored tests excluded".into(),
        profile: "test".into(), rustc: None, cargo: None, discovery_exclusions: Vec::new(),
        build_scripts, build_inputs: None, build_outputs: None,
        inactive_target_flags,
    }, excluded: excluded.into_iter().collect() })
}

pub(super) fn validate_metadata(
    root: &Path,
    scope: &WorkspaceScope,
    scripts: &[super::build_script::Script],
    value: &serde_json::Value,
) -> Result<()> {
    ensure!(
        value["workspace_root"].as_str().map(Path::new) == Some(root),
        "Cargo workspace escaped the snapshot"
    );
    let packages = value["packages"]
        .as_array()
        .context("Cargo metadata has no packages")?;
    let members = value["workspace_members"]
        .as_array()
        .context("Cargo metadata has no workspace members")?;
    ensure!(
        packages.len() == scope.members.len() && members.len() == packages.len(),
        "Cargo workspace membership differs from static inventory"
    );
    let mut seen = BTreeSet::new();
    for member in &scope.members {
        let package = packages
            .iter()
            .find(|p| p["name"].as_str() == Some(&member.name))
            .context("Cargo workspace member missing")?;
        let directory = root.join(&member.directory);
        ensure!(
            package["manifest_path"].as_str().map(Path::new)
                == Some(directory.join("Cargo.toml").as_path()),
            "Cargo member manifest escaped its package"
        );
        ensure!(
            members.contains(&package["id"])
                && seen.insert(package["id"].as_str().context("Cargo package has no ID")?),
            "invalid Cargo workspace member ID"
        );
        let script = scripts
            .iter()
            .find(|s| s.package == member.name)
            .cloned()
            .map(|mut s| {
                s.path = s.path.strip_prefix(&member.directory).unwrap().to_owned();
                s
            });
        project::validate_package_metadata(
            &directory,
            package,
            &member.targets,
            &[directory.join("target")],
            script.as_ref(),
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> Result<tempfile::TempDir> {
        fn copy(from: &Path, to: &Path) -> Result<()> {
            fs::create_dir_all(to)?;
            for entry in fs::read_dir(from)? {
                let entry = entry?;
                if entry.file_name() == "target" {
                    continue;
                }
                if entry.file_type()?.is_dir() {
                    copy(&entry.path(), &to.join(entry.file_name()))?;
                } else {
                    fs::copy(entry.path(), to.join(entry.file_name()))?;
                }
            }
            Ok(())
        }
        let temp = tempfile::tempdir_in(std::env::temp_dir().canonicalize()?)?;
        copy(
            &Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/rust-workspace"),
            temp.path(),
        )?;
        Ok(temp)
    }

    #[test]
    fn inherits_metadata_discovers_implicit_members_and_binds_selection() -> Result<()> {
        let temp = fixture()?;
        let root = temp.path();
        let mut project = Project::load_selected(root, &["workspace-core".into()])?;
        let workspace = project.scope.workspace.as_ref().unwrap();
        assert_eq!(workspace.members.len(), 2);
        assert_eq!(workspace.selected, ["workspace-core"]);
        assert_eq!(
            project.scope.entries(),
            [PathBuf::from("crates/core/src/lib.rs")]
        );
        assert!(
            workspace
                .members
                .iter()
                .all(|m| m.targets[0].edition == "2024")
        );
        project.scope.rustc = Some("test rustc".into());
        project.scope.cargo = Some("test cargo".into());
        let restored: ExecutionScope =
            serde_json::from_value(serde_json::to_value(&project.scope)?)?;
        restored.validate_rerun(&project.scope)?;
        let mut changed = restored.clone();
        changed.workspace.as_mut().unwrap().selected = vec!["workspace-consumer".into()];
        assert!(restored.validate_rerun(&changed).is_err());
        let text =
            fs::read_to_string(root.join("Cargo.toml"))?.replace("crates/*", "crates/consumer");
        fs::write(root.join("Cargo.toml"), text)?;
        assert_eq!(
            Project::load(root)?.scope.workspace.unwrap().members.len(),
            2
        );
        assert!(Project::load_selected(root, &["missing".into()]).is_err());
        Ok(())
    }

    #[test]
    fn root_packages_are_members_and_dependency_escape_is_rejected() -> Result<()> {
        let temp = fixture()?;
        let root = temp.path();
        let text = fs::read_to_string(root.join("Cargo.toml"))?;
        fs::write(
            root.join("Cargo.toml"),
            format!(
                "{text}\n[package]\nname = 'root-package'\nversion.workspace = true\nedition.workspace = true\n"
            ),
        )?;
        fs::create_dir(root.join("src"))?;
        fs::write(root.join("src/lib.rs"), "pub fn enabled() -> bool { true }")?;
        let workspace = Project::load(root)?.scope.workspace.unwrap();
        assert_eq!(workspace.members.len(), 3);
        assert_eq!(workspace.members[0].directory, PathBuf::new());
        fs::write(
            root.join("Cargo.toml"),
            text.replace("path = \"crates/core\"", "path = \"../outside\""),
        )?;
        assert!(Project::load(root).is_err());
        Ok(())
    }

    #[test]
    fn saved_inventory_rejects_duplicate_members_traversal_and_unknown_selection() -> Result<()> {
        let temp = fixture()?;
        let scope = Project::load(temp.path())?.scope;
        for field in ["duplicate", "path", "selection", "target"] {
            let mut value = serde_json::to_value(&scope)?;
            match field {
                "duplicate" => {
                    let member = value["workspace"]["members"][0].clone();
                    value["workspace"]["members"]
                        .as_array_mut()
                        .unwrap()
                        .push(member);
                }
                "path" => value["workspace"]["members"][0]["directory"] = "../escape".into(),
                "selection" => value["workspace"]["selected"] = serde_json::json!(["missing"]),
                _ => value["workspace"]["members"][0]["targets"] = serde_json::json!([]),
            }
            assert!(
                serde_json::from_value::<ExecutionScope>(value).is_err(),
                "{field}"
            );
        }
        Ok(())
    }
}
