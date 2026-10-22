//! Audited package-manager installation formats; never invokes installers.
use super::{json, read, validate_tree, yaml};
use crate::cancellation::Cancellation;
use anyhow::{Context, Result, ensure};
use serde_json::Value as Json;
use serde_yaml_ng::Value as Yaml;
use std::{fs, path::Path};

pub(super) fn installation_fingerprint(root: &Path, cancellation: &Cancellation) -> Result<String> {
    crate::dependency_tree::fingerprint(root, cancellation, true)
}

pub(super) fn json_trailing_commas(text: &str) -> Result<Json> {
    let mut bytes = text.as_bytes().to_vec();
    let (mut quoted, mut escape) = (false, false);
    for index in 0..bytes.len() {
        let byte = bytes[index];
        if quoted {
            if escape {
                escape = false;
            } else if byte == b'\\' {
                escape = true;
            } else if byte == b'"' {
                quoted = false;
            }
        } else if byte == b'"' {
            quoted = true;
        } else if byte == b','
            && bytes[index + 1..]
                .iter()
                .find(|b| !b.is_ascii_whitespace())
                .is_some_and(|b| matches!(b, b'}' | b']'))
        {
            bytes[index] = b' ';
        }
    }
    serde_json::from_slice(&bytes)
        .context("Bun text lockfile must be JSON with optional trailing commas")
}

fn validate_installed_packages(
    root: &Path,
    identities: &std::collections::BTreeSet<String>,
    cancellation: &Cancellation,
) -> Result<()> {
    let modules = root.join("node_modules");
    ensure!(
        fs::symlink_metadata(&modules)?.file_type().is_dir(),
        "node_modules must be a local directory"
    );
    validate_tree(&modules, &modules, cancellation, &mut 0)?;
    let mut pending = vec![modules.clone()];
    let mut count = 0;
    while let Some(directory) = pending.pop() {
        cancellation.check()?;
        for entry in fs::read_dir(&directory)? {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') {
                continue;
            }
            count += 1;
            ensure!(count < 10000, "installed package inventory exceeds limit");
            let path = entry.path();
            ensure!(
                entry.file_type()?.is_dir(),
                "hoisted packages must be local directories: {}",
                path.display()
            );
            if name.starts_with('@') {
                pending.push(path);
                continue;
            }
            let package = json(&path.join("package.json"))?;
            let package_name = package["name"]
                .as_str()
                .context("installed package name missing")?;
            let version = package["version"]
                .as_str()
                .context("installed package version missing")?;
            let expected = if directory
                .file_name()
                .is_some_and(|n| n.to_string_lossy().starts_with('@'))
            {
                format!(
                    "{}/{name}",
                    directory.file_name().unwrap().to_string_lossy()
                )
            } else {
                name
            };
            ensure!(
                package_name == expected
                    && identities.contains(&format!("{package_name}@{version}")),
                "unlocked or mismatched installed package: {package_name}@{version}"
            );
            if path.join("node_modules").exists() {
                pending.push(path.join("node_modules"));
            }
        }
    }
    Ok(())
}

fn registry_name(name: &str) -> bool {
    let parts: Vec<_> = name.split('/').collect();
    let scoped = name.starts_with('@');
    if parts.len() != if scoped { 2 } else { 1 } {
        return false;
    }
    parts.iter().enumerate().all(|(index, part)| {
        let part = if scoped && index == 0 {
            &part[1..]
        } else {
            part
        };
        !part.is_empty()
            && !part.contains("..")
            && part
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
    })
}

const YARN_FSEVENTS_PATCH: &str = "fsevents@patch:fsevents@npm%3A2.3.3#optional!builtin<compat/fsevents>::version=2.3.3&hash=df0bf1";

fn yarn_registry_identity(locator: &str) -> Result<(&str, &str)> {
    if locator == YARN_FSEVENTS_PATCH {
        return Ok(("fsevents", "2.3.3"));
    }
    let (name, version) = locator.rsplit_once("@npm:").context(
        "Yarn requires registry resolutions; only the audited built-in fsevents patch is supported",
    )?;
    ensure!(
        registry_name(name) && !version.contains(':') && !version.is_empty(),
        "invalid Yarn registry identity"
    );
    Ok((name, version))
}

pub(super) fn validate_yarn_installation(
    root: &Path,
    manifest: &Json,
    cancellation: &Cancellation,
) -> Result<(String, String, std::collections::BTreeSet<String>)> {
    let config: Json = serde_yaml_ng::from_str(&read(&root.join(".yarnrc.yml"))?)?;
    ensure!(
        config["nodeLinker"] == "node-modules",
        "Yarn requires nodeLinker: node-modules; PnP and pnpm linkers need separate isolation validation"
    );
    for (key, value) in config
        .as_object()
        .context("Yarn config must be a mapping")?
    {
        ensure!(
            key == "nodeLinker"
                || (matches!(key.as_str(), "enableScripts" | "enableGlobalCache")
                    && value == false),
            "unsupported Yarn configuration: {key}"
        );
    }
    let text = read(&root.join("yarn.lock"))?;
    let lock: Json = serde_yaml_ng::from_str(&text)?;
    ensure!(
        lock["__metadata"]["version"] == 10,
        "Yarn requires audited lockfile version 10"
    );
    let records = lock
        .as_object()
        .context("Yarn lockfile must be a mapping")?;
    let root_key = format!(
        "{}@workspace:.",
        manifest["name"].as_str().context("package name missing")?
    );
    let importer = records
        .get(&root_key)
        .context("Yarn root workspace record missing")?;
    let mut dependencies = serde_json::Map::new();
    for group in ["dependencies", "devDependencies", "optionalDependencies"] {
        if let Some(deps) = manifest.get(group) {
            for (name, spec) in deps
                .as_object()
                .context("dependency group must be an object")?
            {
                let spec = spec
                    .as_str()
                    .context("dependency specifier must be a string")?;
                ensure!(
                    !spec.contains(':') && !spec.contains('/') && !spec.is_empty(),
                    "only registry version dependencies supported"
                );
                dependencies.insert(name.clone(), Json::String(format!("npm:{spec}")));
            }
        }
    }
    ensure!(
        importer["dependencies"].as_object() == Some(&dependencies),
        "Yarn manifest/lockfile dependencies mismatch"
    );
    let mut identities = std::collections::BTreeSet::new();
    for (key, entry) in records {
        if key == "__metadata" || key == &root_key {
            continue;
        }
        let resolution = entry["resolution"]
            .as_str()
            .context("Yarn resolution missing")?;
        let (name, version) = yarn_registry_identity(resolution)?;
        ensure!(
            entry["version"] == version
                && (entry["checksum"].as_str().is_some_and(|s| !s.is_empty())
                    || entry["conditions"].as_str().is_some_and(|s| !s.is_empty())),
            "Yarn package version/checksum or platform condition missing"
        );
        identities.insert(format!("{name}@{version}"));
    }
    validate_installed_packages(root, &identities, cancellation)?;
    for (name, spec) in &dependencies {
        ensure!(registry_name(name), "invalid Yarn dependency name");
        let descriptor = format!(
            "{name}@{}",
            spec.as_str().context("invalid Yarn specifier")?
        );
        let entry = records
            .iter()
            .find(|(key, _)| key.split(", ").any(|part| part == descriptor))
            .map(|(_, value)| value)
            .context("Yarn root dependency descriptor is not locked")?;
        let package = root.join("node_modules").join(name).join("package.json");
        if !package.exists() && manifest["optionalDependencies"].get(name).is_some() {
            continue;
        }
        let metadata = json(&package)?;
        ensure!(
            metadata["name"] == name.as_str() && metadata["version"] == entry["version"],
            "Yarn direct dependency differs from lockfile"
        );
    }
    let state: Json = serde_yaml_ng::from_str(&read(&root.join("node_modules/.yarn-state.yml"))?)?;
    ensure!(
        state["__metadata"]["version"] == 1 && state["__metadata"]["nmMode"] == "classic",
        "unsupported Yarn installed-state layout"
    );
    for (locator, entry) in state.as_object().context("Yarn state must be a mapping")? {
        if locator == "__metadata" {
            continue;
        }
        if locator == &root_key {
            ensure!(
                entry["locations"] == serde_json::json!([""]),
                "Yarn root installed location must be empty"
            );
            continue;
        }
        let (name, version) = if !locator.contains("@virtual:") {
            yarn_registry_identity(locator)?
        } else {
            let (name, virtual_version) = locator
                .split_once("@virtual:")
                .context("unsupported Yarn installed locator")?;
            (
                name,
                virtual_version
                    .rsplit_once("#npm:")
                    .context("unsupported Yarn virtual locator")?
                    .1,
            )
        };
        ensure!(
            identities.contains(&format!("{name}@{version}")),
            "Yarn state contains unlocked package"
        );
        for location in entry["locations"]
            .as_array()
            .context("Yarn installed locations missing")?
        {
            let location = location.as_str().context("invalid Yarn location")?;
            ensure!(
                location.starts_with("node_modules/")
                    && !location.contains('\\')
                    && Path::new(location)
                        .components()
                        .all(|c| matches!(c, std::path::Component::Normal(_))),
                "invalid Yarn installed location"
            );
            let package = json(&root.join(location).join("package.json"))?;
            ensure!(
                package["name"] == name && package["version"] == version,
                "Yarn installed state differs from package"
            );
        }
    }
    let vitest = json(&root.join("node_modules/vitest/package.json"))?["version"]
        .as_str()
        .context("Vitest version missing")?
        .to_owned();
    ensure!(dependencies.contains_key("vitest"), "Vitest must be direct");
    Ok((text, vitest, identities))
}

pub(super) fn validate_bun_installation(
    root: &Path,
    manifest: &Json,
    cancellation: &Cancellation,
) -> Result<(String, String, std::collections::BTreeSet<String>)> {
    ensure!(
        !root.join("node_modules/.bun").exists(),
        "Bun isolated linker requires separate validation; use a hoisted installation"
    );
    let text = read(&root.join("bun.lock"))?;
    let lock = json_trailing_commas(&text)?;
    ensure!(
        lock["lockfileVersion"] == 1 && lock["configVersion"] == 1,
        "Bun requires audited text lockfile/config version 1"
    );
    let workspaces = lock["workspaces"]
        .as_object()
        .context("Bun workspaces missing")?;
    ensure!(
        workspaces.len() == 1 && workspaces.contains_key(""),
        "only a root Bun package is supported"
    );
    let importer = &workspaces[""];
    for group in ["dependencies", "devDependencies", "optionalDependencies"] {
        ensure!(
            manifest.get(group) == importer.get(group),
            "Bun manifest/lockfile {group} mismatch"
        );
    }
    let records = lock["packages"]
        .as_object()
        .context("Bun package records missing")?;
    let mut identities = std::collections::BTreeSet::new();
    for (name, entry) in records {
        let values = entry
            .as_array()
            .context("Bun package record must be an array")?;
        ensure!(
            values.len() == 4
                && values[1] == ""
                && values[3].as_str().is_some_and(|s| s.starts_with("sha512-")),
            "Bun requires default-registry integrity records"
        );
        let identity = values[0].as_str().context("Bun package identity missing")?;
        let (package, version) = identity
            .rsplit_once('@')
            .context("invalid Bun package identity")?;
        ensure!(
            registry_name(name) && package == name && !version.is_empty(),
            "nested/aliased Bun lock packages need separate validation"
        );
        identities.insert(identity.to_owned());
    }
    validate_installed_packages(root, &identities, cancellation)?;
    for group in ["dependencies", "devDependencies", "optionalDependencies"] {
        if let Some(deps) = manifest.get(group) {
            for name in deps
                .as_object()
                .context("dependency group must be an object")?
                .keys()
            {
                ensure!(registry_name(name), "invalid dependency name: {name}");
                if group == "optionalDependencies" && !root.join("node_modules").join(name).exists()
                {
                    continue;
                }
                let package = json(&root.join("node_modules").join(name).join("package.json"))?;
                let identity = format!(
                    "{}@{}",
                    name,
                    package["version"]
                        .as_str()
                        .context("installed version missing")?
                );
                ensure!(
                    records.get(name).and_then(|entry| entry[0].as_str())
                        == Some(identity.as_str()),
                    "Bun direct dependency is not locked"
                );
            }
        }
    }
    ensure!(
        manifest["dependencies"].get("vitest").is_some()
            || manifest["devDependencies"].get("vitest").is_some(),
        "Vitest must be direct"
    );
    let vitest = json(&root.join("node_modules/vitest/package.json"))?["version"]
        .as_str()
        .context("Vitest version missing")?
        .to_owned();
    Ok((text, vitest, identities))
}

pub(super) fn validate_npm_installation(
    root: &Path,
    manifest: &Json,
    cancellation: &Cancellation,
) -> Result<(String, String, std::collections::BTreeSet<String>)> {
    let text = read(&root.join("package-lock.json"))?;
    let lock: Json = serde_json::from_str(&text)?;
    ensure!(
        lock["lockfileVersion"] == 3,
        "npm requires lockfileVersion 3"
    );
    let records = lock["packages"]
        .as_object()
        .context("npm lockfile packages missing")?;
    let importer = records.get("").context("npm root package record missing")?;
    for group in ["dependencies", "devDependencies", "optionalDependencies"] {
        ensure!(
            manifest.get(group) == importer.get(group),
            "npm manifest/lockfile {group} mismatch"
        );
        if let Some(deps) = manifest.get(group) {
            for (name, value) in deps
                .as_object()
                .context("dependency group must be an object")?
            {
                let spec = value
                    .as_str()
                    .context("dependency specifier must be a string")?;
                ensure!(
                    !spec.is_empty()
                        && spec
                            .chars()
                            .all(|c| c.is_ascii_alphanumeric() || " .^~<>=|*-+".contains(c)),
                    "only registry dependency versions are supported: {name}"
                );
            }
        }
    }
    let modules = root.join("node_modules");
    ensure!(
        fs::symlink_metadata(&modules)?.file_type().is_dir(),
        "node_modules must be a local directory"
    );
    validate_tree(&modules, &modules, cancellation, &mut 0)?;
    let installed = json(&modules.join(".package-lock.json"))?;
    ensure!(
        installed["lockfileVersion"] == 3,
        "npm installed lockfileVersion must be 3"
    );
    let actual = installed["packages"]
        .as_object()
        .context("npm installed packages missing")?;
    let mut identities = std::collections::BTreeSet::new();
    for (path, entry) in records {
        if path.is_empty() {
            continue;
        }
        cancellation.check()?;
        ensure!(
            path.starts_with("node_modules/")
                && !path.contains('\\')
                && Path::new(path)
                    .components()
                    .all(|p| matches!(p, std::path::Component::Normal(_))),
            "invalid npm package path: {path}"
        );
        ensure!(
            entry.get("link").is_none()
                && entry["resolved"]
                    .as_str()
                    .is_some_and(|s| s.starts_with("https://registry.npmjs.org/"))
                && entry["integrity"]
                    .as_str()
                    .is_some_and(|s| s.starts_with("sha512-")),
            "npm requires integrity-pinned registry packages: {path}"
        );
        let version = entry["version"]
            .as_str()
            .context("npm package version missing")?;
        let name = path
            .rsplit_once("node_modules/")
            .context("invalid npm package path")?
            .1;
        identities.insert(format!("{name}@{version}"));
        let package = root.join(path).join("package.json");
        if !package.exists() && entry["optional"] == true {
            continue;
        }
        let metadata = json(&package)?;
        ensure!(
            metadata["name"] == name && metadata["version"] == version,
            "npm installed package differs from lockfile: {path}"
        );
        let captured = actual
            .get(path)
            .context("npm installed lock omits a package")?;
        for field in ["version", "resolved", "integrity"] {
            ensure!(
                captured.get(field) == entry.get(field),
                "npm installed lock differs for {path}: {field}"
            );
        }
    }
    ensure!(
        actual.keys().all(|path| records.contains_key(path)),
        "npm installed lock contains unlocked packages"
    );
    ensure!(
        manifest["dependencies"].get("vitest").is_some()
            || manifest["devDependencies"].get("vitest").is_some(),
        "Vitest must be a direct dependency"
    );
    let vitest = records
        .get("node_modules/vitest")
        .context("locked direct Vitest is missing")?["version"]
        .as_str()
        .context("locked Vitest version missing")?
        .to_owned();
    Ok((text, vitest, identities))
}

pub(super) fn validate_pnpm_installation(
    root: &Path,
    manifest: &Json,
    cancellation: &Cancellation,
) -> Result<(String, String, std::collections::BTreeSet<String>)> {
    let lock_text = read(&root.join("pnpm-lock.yaml"))?;
    let lock: Yaml = serde_yaml_ng::from_str(&lock_text).context("parsing pnpm-lock.yaml")?;
    ensure!(
        lock["lockfileVersion"].as_str() == Some("9.0"),
        "expected pnpm lockfileVersion 9.0"
    );
    let importers = lock["importers"]
        .as_mapping()
        .context("lockfile requires importers")?;
    ensure!(
        importers.len() == 1 && importers.contains_key(Yaml::String(".".into())),
        "only one root lockfile importer is supported"
    );
    let importer = &lock["importers"]["."];
    for group in ["dependencies", "devDependencies", "optionalDependencies"] {
        let deps = manifest
            .get(group)
            .map(|v| v.as_object().context("dependency group must be an object"))
            .transpose()?;
        let locked = importer[group].as_mapping();
        ensure!(
            deps.map_or(0, |d| d.len()) == locked.map_or(0, |d| d.len()),
            "manifest/lockfile {group} mismatch; install with the frozen lockfile"
        );
        if let Some(deps) = deps {
            for (name, spec) in deps {
                let spec = spec
                    .as_str()
                    .context("dependency specifier must be a string")?;
                ensure!(!spec.is_empty() && spec.chars().all(|c| c.is_ascii_digit() || " .^~<>=|*-+xXvabcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ".contains(c)), "only registry version dependencies are supported: {name}");
                ensure!(
                    importer[group][name.as_str()]["specifier"].as_str() == Some(spec),
                    "manifest/lockfile specifier mismatch for {name}"
                );
            }
        }
    }
    let packages = lock["packages"]
        .as_mapping()
        .context("lockfile requires registry packages")?;
    for (_, package) in packages {
        let resolution = package["resolution"]
            .as_mapping()
            .context("package requires a registry integrity resolution")?;
        ensure!(
            resolution.len() == 1
                && package["resolution"]["integrity"]
                    .as_str()
                    .is_some_and(|s| s.starts_with("sha512-")),
            "only integrity-pinned registry dependencies are supported"
        );
    }
    let modules = root.join("node_modules");
    ensure!(fs::symlink_metadata(&modules).context("install dependencies with pnpm install --frozen-lockfile --ignore-scripts before listing")?.file_type().is_dir(), "node_modules must be a local directory");
    validate_tree(&modules, &modules, cancellation, &mut 0)?;
    let layout = yaml(&modules.join(".modules.yaml"))?;
    ensure!(
        layout["packageManager"].as_str() == Some("pnpm@11.10.0")
            && layout["nodeLinker"].as_str() == Some("isolated")
            && layout["virtualStoreDir"].as_str() == Some(".pnpm"),
        "installed dependencies require pnpm 11.10.0 with a local isolated virtual store"
    );
    for group in ["dependencies", "devDependencies", "optionalDependencies"] {
        if let Some(deps) = importer[group].as_mapping() {
            for (name, entry) in deps {
                let name = name.as_str().context("dependency name must be a string")?;
                ensure!(
                    !name.contains("..") && !name.starts_with('/') && !name.contains('\\'),
                    "invalid dependency name: {name}"
                );
                let version = entry["version"]
                    .as_str()
                    .context("locked dependency version is missing")?
                    .split('(')
                    .next()
                    .unwrap();
                ensure!(
                    packages.contains_key(Yaml::String(format!("{name}@{version}"))),
                    "dependency is not a locked registry package: {name}"
                );
                let installed = modules.join(name).join("package.json");
                if group == "optionalDependencies" && !installed.exists() {
                    continue;
                }
                let data = json(
                    &installed
                        .canonicalize()
                        .with_context(|| format!("missing installed dependency {name}"))?,
                )?;
                ensure!(
                    data["name"] == name && data["version"] == version,
                    "installed dependency differs from lockfile: {name}"
                );
            }
        }
    }
    let installed_lock = yaml(&modules.join(".pnpm/lock.yaml"))?;
    ensure!(
        installed_lock == lock,
        "installed dependency lock differs from pnpm-lock.yaml; reinstall with the frozen lockfile"
    );

    let locked_vitest = ["devDependencies", "dependencies"]
        .iter()
        .find_map(|group| importer[*group]["vitest"]["version"].as_str())
        .context("Vitest must be a direct dependency")?
        .to_owned();
    let identities = packages
        .keys()
        .map(|key| {
            key.as_str()
                .context("invalid package identity")
                .map(str::to_owned)
        })
        .collect::<Result<_>>()?;
    Ok((lock_text, locked_vitest, identities))
}
