use anyhow::{Context, Result, ensure};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs,
    path::{Component, Path, PathBuf},
};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Receipt {
    pub site: PathBuf,
    pub executable: PathBuf,
    pub package: String,
    pub environment_fingerprint: String,
    pub files: BTreeMap<String, String>,
    pub build_diagnostics: Vec<(String, String)>,
}

impl Receipt {
    pub fn validate(&self, owned: &Path, original_executable: &Path) -> Result<()> {
        ensure!(
            self.site.starts_with(owned)
                && self.executable.starts_with(owned)
                && self.site.is_absolute()
                && self.executable.is_absolute()
                && self.executable.canonicalize()? == original_executable.canonicalize()?
                && !self.package.is_empty()
                && !self.files.is_empty()
                && self.files.len() < 10000
                && self.build_diagnostics.len() <= 1,
            "invalid Python owned installation identity"
        );
        let configuration = fs::read(owned.join("venv/pyvenv.cfg"))?;
        ensure!(
            format!("sha256:{:x}", Sha256::digest(&configuration)) == self.environment_fingerprint,
            "Python installed environment configuration changed"
        );
        let mut observed = BTreeMap::new();
        fn visit(
            root: &Path,
            directory: &Path,
            files: &mut BTreeMap<String, String>,
            total: &mut u64,
        ) -> Result<()> {
            for entry in fs::read_dir(directory)? {
                let entry = entry?;
                let kind = entry.file_type()?;
                let path = entry.path();
                ensure!(!kind.is_symlink(), "Python installed symlink unsupported");
                if kind.is_dir() {
                    visit(root, &path, files, total)?;
                } else {
                    ensure!(
                        kind.is_file() && entry.metadata()?.len() <= 8 * 1024 * 1024,
                        "invalid Python installed file"
                    );
                    *total += entry.metadata()?.len();
                    ensure!(
                        *total <= 64 * 1024 * 1024 && files.len() < 10000,
                        "Python installation exceeds budget"
                    );
                    let relative = path
                        .strip_prefix(root)?
                        .to_str()
                        .context("non-UTF-8 installation path")?
                        .to_owned();
                    let bytes = fs::read(&path)?;
                    files.insert(relative, format!("sha256:{:x}", Sha256::digest(&bytes)));
                }
            }
            Ok(())
        }
        visit(&self.site, &self.site, &mut observed, &mut 0)?;
        ensure!(
            observed == self.files,
            "Python installed package/runtime bytes changed"
        );
        Ok(())
    }

    pub fn admits_import(&self, module: &str, path: &str) -> bool {
        if !self.files.contains_key(path)
            || !Path::new(path)
                .components()
                .all(|c| matches!(c, Component::Normal(_)))
        {
            return false;
        }
        if module == self.package || module.starts_with(&format!("{}.", self.package)) {
            let relative = module.replace('.', "/");
            path == format!("{relative}.py") || path == format!("{relative}/__init__.py")
        } else {
            !path.starts_with(&format!("{}/", self.package))
        }
    }
}
