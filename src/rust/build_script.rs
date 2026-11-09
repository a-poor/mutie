//! Owned build-script inputs and pristine generated-output identity.
//! Cargo executes trusted project code; this is validation, not a sandbox.
use super::ExecutionScope;
use crate::{cancellation::Cancellation, capture::Capture};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs,
    io::{BufRead, BufReader, Read},
    path::{Component, Path, PathBuf},
};

const RECIPE: &str = "owned-cargo-build-inputs-and-outputs-v1";
const MAX_BYTES: u64 = 256 * 1024 * 1024;
const MAX_FILES: usize = 20_000;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Script {
    pub package: String,
    pub path: PathBuf,
    pub fingerprint: String,
    pub recipe: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Outputs {
    // Package names and hashes only: no absolute paths or environment values.
    pub packages: BTreeMap<String, String>,
}

fn hash(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}
fn valid_hash(value: &str) -> bool {
    value
        .strip_prefix("sha256:")
        .is_some_and(|s| s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit()))
}

pub fn validate(scripts: &[Script], inputs: Option<&str>, outputs: Option<&Outputs>) -> Result<()> {
    ensure!(
        scripts.windows(2).all(|p| p[0].package < p[1].package),
        "invalid build-script inventory order"
    );
    for script in scripts {
        ensure!(
            !script.package.is_empty()
                && !script.path.as_os_str().is_empty()
                && script
                    .path
                    .components()
                    .all(|c| matches!(c, Component::Normal(_)))
                && valid_hash(&script.fingerprint)
                && script.recipe == RECIPE,
            "invalid build-script identity"
        );
    }
    ensure!(
        inputs.is_none_or(valid_hash),
        "invalid build-script input identity"
    );
    if let Some(outputs) = outputs {
        ensure!(
            outputs.packages.len() == scripts.len()
                && scripts.iter().all(|s| outputs
                    .packages
                    .get(&s.package)
                    .is_some_and(|h| valid_hash(h))),
            "invalid build-script output inventory"
        );
    }
    ensure!(
        !scripts.is_empty() || (inputs.is_none() && outputs.is_none()),
        "build evidence without build scripts"
    );
    Ok(())
}

pub fn load(root: &Path, package: &toml::Value, name: &str) -> Result<Vec<Script>> {
    let path = match package.get("build") {
        Some(v) if v.as_bool() == Some(false) => return Ok(Vec::new()),
        Some(v) if v.as_bool() == Some(true) => "build.rs",
        Some(v) => v
            .as_str()
            .context("package.build must be a boolean or build script path")?,
        None if root.join("build.rs").exists() => "build.rs",
        None => return Ok(Vec::new()),
    };
    ensure!(
        !path.is_empty()
            && Path::new(path)
                .components()
                .all(|c| matches!(c, Component::Normal(_) | Component::CurDir)),
        "build script must stay inside its package"
    );
    let path =
        super::project::inside(root, &root.join(path)).context("resolving project build script")?;
    let bytes = fs::read(root.join(&path)).context("reading project build script")?;
    Ok(vec![Script {
        package: name.into(),
        path,
        fingerprint: hash(&bytes),
        recipe: RECIPE.into(),
    }])
}

#[derive(Default)]
struct Budget {
    bytes: u64,
    files: usize,
}
impl Budget {
    fn add(&mut self, bytes: u64) -> Result<()> {
        self.bytes = self.bytes.saturating_add(bytes);
        self.files += 1;
        ensure!(
            self.bytes <= MAX_BYTES && self.files <= MAX_FILES,
            "build-script identity exceeds the 256 MiB / 20000-file bound"
        );
        Ok(())
    }
}

fn files(
    root: &Path,
    path: &Path,
    excluded: &[PathBuf],
    result: &mut BTreeMap<PathBuf, String>,
    budget: &mut Budget,
    cancellation: &Cancellation,
) -> Result<()> {
    cancellation.check()?;
    if excluded.iter().any(|p| path.starts_with(p)) {
        return Ok(());
    }
    let meta = fs::symlink_metadata(path)?;
    let relative = path.strip_prefix(root)?;
    if meta.is_symlink() {
        // Do not follow directory links or permit a fingerprint to escape the copy.
        ensure!(
            path.canonicalize()?.starts_with(root),
            "build input link escapes the owned root"
        );
        budget.add(0)?;
        result.insert(
            relative.into(),
            format!("link:{}", fs::read_link(path)?.display()),
        );
    } else if meta.is_dir() {
        for entry in fs::read_dir(path)? {
            files(root, &entry?.path(), excluded, result, budget, cancellation)?;
        }
    } else {
        ensure!(meta.is_file(), "unsupported build-script input file");
        budget.add(meta.len())?;
        let mut file = fs::File::open(path)?;
        let mut digest = Sha256::new();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            digest.update((meta.permissions().mode() & 0o777).to_le_bytes());
        }
        let mut buffer = [0; 65536];
        let mut read = 0u64;
        loop {
            cancellation.check()?;
            let n = file.read(&mut buffer)?;
            if n == 0 {
                break;
            }
            read += n as u64;
            ensure!(
                read <= meta.len(),
                "build-script input changed during fingerprinting"
            );
            digest.update(&buffer[..n]);
        }
        ensure!(
            read == meta.len(),
            "build-script input changed during fingerprinting"
        );
        result.insert(relative.into(), format!("sha256:{:x}", digest.finalize()));
    }
    Ok(())
}

/// Integration-test trees can be strengthened; the subsequent pristine build
/// must still produce identical directives/generated files. Other snapshot
/// inputs are deliberately conservative, including helper modules and data.
pub fn inputs(
    root: &Path,
    scope: &ExecutionScope,
    include_tests: bool,
    cancellation: &Cancellation,
) -> Result<Option<String>> {
    if scope.build_scripts.is_empty() {
        return Ok(None);
    }
    let excluded: Vec<_> = if include_tests {
        Vec::new()
    } else {
        std::iter::once(root.join("tests"))
            .chain(scope.workspace.iter().flat_map(|w| {
                w.members
                    .iter()
                    .map(|m| root.join(&m.directory).join("tests"))
            }))
            .collect()
    };
    let mut result = BTreeMap::new();
    files(
        root,
        root,
        &excluded,
        &mut result,
        &mut Budget::default(),
        cancellation,
    )?;
    Ok(Some(hash(&serde_json::to_vec(&result)?)))
}

fn normalize(value: &str, root: &Path, artifacts: &Path, out: &Path) -> String {
    value
        .replace(out.to_string_lossy().as_ref(), "$OUT_DIR")
        .replace(artifacts.to_string_lossy().as_ref(), "$ARTIFACTS")
        .replace(root.to_string_lossy().as_ref(), "$SOURCE")
}

pub fn outputs(
    stdout: &Capture,
    root: &Path,
    artifacts: &Path,
    scope: &ExecutionScope,
    cancellation: &Cancellation,
) -> Result<Option<Outputs>> {
    if scope.build_scripts.is_empty() {
        return Ok(None);
    }
    let mut ids = BTreeMap::new();
    let mut executed = Vec::new();
    for line in BufReader::new(stdout.reader()?).split(b'\n') {
        cancellation.check()?;
        let line = line?;
        let Ok(value) = serde_json::from_slice::<serde_json::Value>(&line) else {
            continue;
        };
        if value["reason"] == "compiler-artifact"
            && value["target"]["kind"] == serde_json::json!(["custom-build"])
        {
            for script in &scope.build_scripts {
                if value["target"]["src_path"].as_str().map(Path::new)
                    == Some(root.join(&script.path).as_path())
                {
                    let id = value["package_id"]
                        .as_str()
                        .context("build script lacks package ID")?
                        .to_owned();
                    ensure!(
                        ids.insert(id, script).is_none(),
                        "duplicate build-script artifact"
                    );
                }
            }
        }
        if value["reason"] == "build-script-executed" {
            executed.push(value);
        }
    }
    ensure!(
        ids.len() == scope.build_scripts.len(),
        "missing owned build-script artifact evidence"
    );
    let mut packages = BTreeMap::new();
    let mut budget = Budget::default();
    for event in executed {
        let Some(script) = event["package_id"].as_str().and_then(|id| ids.get(id)) else {
            continue;
        };
        let out = Path::new(
            event["out_dir"]
                .as_str()
                .context("build script lacks OUT_DIR")?,
        )
        .canonicalize()?;
        ensure!(
            out.starts_with(artifacts.canonicalize()?) && out.is_dir(),
            "build script output escaped owned Cargo artifacts"
        );
        let mut generated = BTreeMap::new();
        files(&out, &out, &[], &mut generated, &mut budget, cancellation)?;
        for (path, digest) in &mut generated {
            let path = out.join(path);
            ensure!(
                !fs::symlink_metadata(&path)?.is_symlink(),
                "linked build-script outputs are unsupported"
            );
            let bytes = fs::read(path)?;
            *digest = match std::str::from_utf8(&bytes) {
                Ok(text) => hash(normalize(text, root, artifacts, &out).as_bytes()),
                Err(_) => hash(&bytes),
            };
        }
        let output = out
            .parent()
            .context("missing build output parent")?
            .join("output");
        ensure!(
            output
                .canonicalize()?
                .starts_with(artifacts.canonicalize()?),
            "build directives escaped owned artifacts"
        );
        budget.add(fs::metadata(&output)?.len())?;
        let text = fs::read_to_string(output)?;
        let mut directives = Vec::new();
        let mut declared = BTreeMap::new();
        let mut environment = BTreeMap::new();
        let package_root = scope
            .workspace
            .as_ref()
            .and_then(|w| w.members.iter().find(|m| m.name == script.package))
            .map(|m| root.join(&m.directory))
            .unwrap_or_else(|| root.to_owned());
        for line in text.lines() {
            let Some(directive) = line
                .strip_prefix("cargo::")
                .or_else(|| line.strip_prefix("cargo:"))
            else {
                continue;
            };
            // Warnings are diagnostics, not compiler configuration.
            if directive.starts_with("warning=") {
                continue;
            }
            directives.push(normalize(directive, root, artifacts, &out));
            if let Some(path) = directive.strip_prefix("rerun-if-changed=") {
                let path = package_root.join(path);
                let path = path
                    .canonicalize()
                    .context("missing declared build-script input")?;
                ensure!(
                    path.starts_with(root),
                    "declared build-script inputs must stay inside the snapshot"
                );
                files(root, &path, &[], &mut declared, &mut budget, cancellation)?;
            }
            if let Some(name) = directive.strip_prefix("rerun-if-env-changed=") {
                ensure!(!name.is_empty(), "empty build-script environment input");
                let value = std::env::var_os(name);
                let bytes = value.as_ref().map(|v| v.as_encoded_bytes());
                environment.insert(name.to_owned(), bytes.map(hash));
            }
        }
        // Preserve ordered directives, including link arguments absent from Cargo JSON.
        let digest = hash(&serde_json::to_vec(&(
            RECIPE,
            directives,
            generated,
            declared,
            environment,
        ))?);
        ensure!(
            packages.insert(script.package.clone(), digest).is_none(),
            "duplicate build-script execution evidence"
        );
    }
    ensure!(
        packages.len() == scope.build_scripts.len(),
        "missing owned build-script completion evidence"
    );
    Ok(Some(Outputs { packages }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn script_records_require_complete_sorted_recipe_bound_evidence() -> Result<()> {
        let script = Script {
            package: "sample".into(),
            path: "build.rs".into(),
            fingerprint: hash(b"script"),
            recipe: RECIPE.into(),
        };
        let outputs = Outputs {
            packages: [("sample".into(), hash(b"outputs"))].into(),
        };
        validate(
            std::slice::from_ref(&script),
            Some(&hash(b"inputs")),
            Some(&outputs),
        )?;
        for (key, value) in [
            ("path", "../build.rs"),
            ("fingerprint", "unknown"),
            ("recipe", "old"),
            ("package", ""),
        ] {
            let mut record = serde_json::to_value(&script)?;
            record[key] = value.into();
            let changed: Script = serde_json::from_value(record)?;
            assert!(validate(&[changed], None, None).is_err());
        }
        assert!(validate(&[script.clone(), script], None, None).is_err());
        assert!(validate(&[], None, Some(&outputs)).is_err());
        Ok(())
    }

    #[test]
    fn snapshot_inputs_allow_integration_tests_but_bind_helpers_data_and_modes() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let root = temp.path().canonicalize()?;
        fs::create_dir(root.join("src"))?;
        fs::create_dir(root.join("tests"))?;
        fs::write(
            root.join("Cargo.toml"),
            "[package]\nname='sample'\nversion='0.0.0'\nedition='2024'\n",
        )?;
        fs::write(root.join("Cargo.lock"), "lock")?;
        fs::write(root.join("src/lib.rs"), "pub fn x() {}")?;
        fs::write(root.join("build.rs"), "fn main() {}")?;
        let project = super::super::Project::load(&root)?;
        let cancel = Cancellation::default();
        let before = inputs(&root, &project.scope, false, &cancel)?;
        fs::write(root.join("tests/new.rs"), "#[test] fn x() {}")?;
        assert_eq!(before, inputs(&root, &project.scope, false, &cancel)?);
        assert_ne!(before, inputs(&root, &project.scope, true, &cancel)?);
        fs::write(root.join("helper.rs"), "fn helper() {}")?;
        assert_ne!(before, inputs(&root, &project.scope, false, &cancel)?);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let before = inputs(&root, &project.scope, false, &cancel)?;
            fs::set_permissions(root.join("helper.rs"), fs::Permissions::from_mode(0o755))?;
            assert_ne!(before, inputs(&root, &project.scope, false, &cancel)?);
        }
        Ok(())
    }
}
