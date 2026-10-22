//! Owned dependency-tree identity shared by JavaScript adapters.
use crate::cancellation::Cancellation;
use anyhow::{Context, Result, ensure};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fs, path::Path};

pub const METADATA: &[&str] = &[
    "package-lock.json",
    "npm-shrinkwrap.json",
    "pnpm-lock.yaml",
    "yarn.lock",
    "bun.lock",
    "bun.lockb",
    ".yarnrc.yml",
    ".npmrc",
    "pnpm-workspace.yaml",
];
pub fn identity_key(name: &str) -> bool {
    METADATA.contains(&name) || matches!(name, "node_modules" | "development_installation_recipe")
}
pub fn development(
    root: &Path,
    manifest: &serde_json::Value,
    cancellation: &Cancellation,
) -> Result<BTreeMap<String, String>> {
    let mut result = BTreeMap::new();
    for name in [".pnp.cjs", ".pnp.loader.mjs"] {
        ensure!(
            !root.join(name).exists(),
            "Yarn PnP loaders require separate validation"
        );
    }
    for name in METADATA {
        let path = root.join(name);
        match fs::symlink_metadata(&path) {
            Ok(meta) => {
                ensure!(
                    meta.is_file() && meta.len() <= 8 * 1024 * 1024,
                    "package-manager metadata must be a bounded regular file: {name}"
                );
                result.insert(
                    (*name).into(),
                    format!("sha256:{:x}", Sha256::digest(fs::read(&path)?)),
                );
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error).context("reading package-manager metadata"),
        }
    }
    if root.join("pnpm-workspace.yaml").is_file() {
        let value: serde_yaml_ng::Value =
            serde_yaml_ng::from_str(&fs::read_to_string(root.join("pnpm-workspace.yaml"))?)?;
        ensure!(
            value.get("packages").is_none(),
            "pnpm workspace members require separate support"
        );
    }
    if root.join(".yarnrc.yml").is_file() {
        let value: serde_yaml_ng::Value =
            serde_yaml_ng::from_str(&fs::read_to_string(root.join(".yarnrc.yml"))?)?;
        ensure!(
            value
                .get("nodeLinker")
                .is_none_or(|value| value.as_str() == Some("node-modules")),
            "Yarn loader/linker requires separate validation"
        );
    }
    match fs::symlink_metadata(root.join("node_modules")) {
        Ok(meta) => {
            ensure!(
                meta.file_type().is_dir(),
                "development node_modules must be a local directory"
            );
            ensure!(
                manifest["devDependencies"]
                    .as_object()
                    .is_some_and(|deps| !deps.is_empty()),
                "installed Node/Bun packages require declared development dependencies; runtime dependency imports remain unsupported"
            );
            result.insert(
                "node_modules".into(),
                fingerprint(root, cancellation, false)?,
            );
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error).context("reading development installation"),
    }
    if !result.is_empty() {
        result.insert(
            "development_installation_recipe".into(),
            format!(
                "sha256:{:x}",
                Sha256::digest(
                    [
                        include_bytes!("dependency_tree.rs").as_slice(),
                        include_bytes!("node/project.rs").as_slice(),
                        include_bytes!("bun/project.rs").as_slice()
                    ]
                    .concat()
                )
            ),
        );
    }
    Ok(result)
}

pub(crate) fn fingerprint(
    root: &Path,
    cancellation: &Cancellation,
    exclude_caches: bool,
) -> Result<String> {
    use std::io::Read;
    let modules = root.join("node_modules");
    let mut paths = vec![modules.clone()];
    let mut files = Vec::new();
    while let Some(path) = paths.pop() {
        cancellation.check()?;
        let relative = path.strip_prefix(&modules)?;
        if exclude_caches
            && [".vite", ".vite-temp", ".cache"]
                .iter()
                .any(|name| relative.starts_with(name))
        {
            continue;
        }
        ensure!(
            path.components().count() - modules.components().count() < 100,
            "dependency nesting exceeds limit"
        );
        let meta = fs::symlink_metadata(&path)?;
        ensure!(
            files.len() + paths.len() < 200_000,
            "installation fingerprint exceeds file limit"
        );
        if meta.is_dir() {
            for entry in fs::read_dir(&path)? {
                paths.push(entry?.path());
            }
        }
        files.push(path);
    }
    files.sort();
    let mut digest = Sha256::new();
    let mut budget = 512_u64 * 1024 * 1024;
    for path in files {
        cancellation.check()?;
        let name = path
            .strip_prefix(&modules)?
            .to_str()
            .context("dependency path must be UTF-8")?;
        digest.update((name.len() as u64).to_le_bytes());
        digest.update(name.as_bytes());
        let meta = fs::symlink_metadata(&path)?;
        if meta.file_type().is_symlink() {
            digest.update(b"link");
            let resolved = path.canonicalize()?;
            let target = resolved
                .strip_prefix(&modules)
                .context("dependency link escapes installation")?
                .to_str()
                .context("dependency link must be UTF-8")?;
            digest.update((target.len() as u64).to_le_bytes());
            digest.update(target.as_bytes());
        } else if meta.is_dir() {
            digest.update(b"dir");
        } else {
            ensure!(meta.is_file(), "unsupported dependency file");
            digest.update(b"file");
            digest.update(meta.len().to_le_bytes());
            let mut file = fs::File::open(&path)?;
            let mut buffer = [0u8; 64 * 1024];
            let mut length = 0;
            loop {
                cancellation.check()?;
                let read = file.read(&mut buffer)?;
                if read == 0 {
                    break;
                }
                budget = budget
                    .checked_sub(read as u64)
                    .context("installation exceeds 512 MiB fingerprint budget")?;
                length += read as u64;
                digest.update(&buffer[..read]);
            }
            ensure!(
                length == meta.len(),
                "dependency changed while fingerprinting"
            );
        }
    }
    Ok(format!("sha256:{:x}", digest.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn development_installations_are_owned_and_content_bound() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let root = temp.path().canonicalize()?;
        fs::create_dir_all(root.join("node_modules/tool"))?;
        fs::write(root.join("node_modules/tool/index.js"), "true")?;
        assert!(development(&root, &serde_json::json!({}), &Cancellation::default()).is_err());
        let manifest = serde_json::json!({"devDependencies":{"tool":"1.0.0"}});
        let before = development(&root, &manifest, &Cancellation::default())?;
        fs::write(root.join("node_modules/tool/index.js"), "null")?;
        assert_ne!(
            before,
            development(&root, &manifest, &Cancellation::default())?
        );
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(&root, root.join("node_modules/escape"))?;
            assert!(development(&root, &manifest, &Cancellation::default()).is_err());
        }
        Ok(())
    }

    #[test]
    fn lockfile_drift_is_bound_and_pnp_is_rejected() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let root = temp.path();
        fs::write(root.join("yarn.lock"), "first")?;
        let manifest = serde_json::json!({});
        let before = development(root, &manifest, &Cancellation::default())?;
        fs::write(root.join("yarn.lock"), "other")?;
        assert_ne!(
            before,
            development(root, &manifest, &Cancellation::default())?
        );
        fs::write(root.join(".pnp.cjs"), "throw Error('must not execute')")?;
        assert!(development(root, &manifest, &Cancellation::default()).is_err());
        Ok(())
    }
}
