use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail, ensure};
use tempfile::TempDir;

/// Owns only temporary data. The input working tree is never a mutation target.
pub struct Workspace {
    directory: TempDir,
}

impl Workspace {
    pub fn new(source: &Path) -> Result<Self> {
        Self::new_excluding(source, &[])
    }

    pub fn new_excluding(source: &Path, excluded: &[PathBuf]) -> Result<Self> {
        let directory = tempfile::Builder::new().prefix("mutie-").tempdir()?;
        ensure!(
            !directory.path().canonicalize()?.starts_with(source),
            "temporary directory is inside the module; set TMPDIR to a directory outside it"
        );
        let workspace = Self { directory };
        let destination = workspace.path();
        copy_directory(source, &destination, source, &destination, excluded)?;
        Ok(workspace)
    }

    pub fn path(&self) -> std::path::PathBuf {
        self.directory.path().join("repo")
    }

    /// Restore the entire workspace, including files changed or created by tests.
    pub fn reset(&self, source: &Path) -> Result<()> {
        let destination = self.path();
        if destination.exists() {
            fs::remove_dir_all(&destination).context("could not reset temporary workspace")?;
        }
        copy_directory(source, &destination, source, &destination, &[])
    }
}

fn copy_directory(
    source: &Path,
    destination: &Path,
    root: &Path,
    copy_root: &Path,
    excluded: &[PathBuf],
) -> Result<()> {
    fs::create_dir_all(destination)?;
    for entry in fs::read_dir(source).with_context(|| format!("reading {}", source.display()))? {
        let entry = entry?;
        if entry.file_name() == ".git" {
            continue;
        }
        let from = entry.path();
        if excluded.contains(&from) {
            continue;
        }
        let to = destination.join(entry.file_name());
        let kind = entry.file_type()?;
        if kind.is_dir() {
            copy_directory(&from, &to, root, copy_root, excluded)?;
        } else if kind.is_file() {
            reflink_copy::reflink_or_copy(&from, &to)
                .with_context(|| format!("copying {}", from.display()))?;
            fs::set_permissions(&to, entry.metadata()?.permissions())?;
        } else if kind.is_symlink() {
            if from
                .canonicalize()
                .is_ok_and(|target| excluded.iter().any(|path| target.starts_with(path)))
            {
                continue;
            }
            copy_symlink(&from, &to, root, copy_root)?;
        } else {
            bail!(
                "cannot copy special file {} into a workspace",
                from.display()
            );
        }
    }
    Ok(())
}

fn copy_symlink(from: &Path, to: &Path, root: &Path, copy_root: &Path) -> Result<()> {
    let resolved = from
        .canonicalize()
        .with_context(|| format!("resolving symlink {}", from.display()))?;
    let relative = resolved.strip_prefix(root).with_context(|| {
        format!(
            "symlink {} points outside the module; external symlinks are not supported",
            from.display()
        )
    })?;
    let target = fs::read_link(from)?;
    // Absolute internal links must point into this copy, never the input tree.
    let target = if target.is_absolute() {
        copy_root.join(relative)
    } else {
        target
    };
    #[cfg(unix)]
    std::os::unix::fs::symlink(target, to)?;
    #[cfg(windows)]
    if resolved.is_dir() {
        std::os::windows::fs::symlink_dir(target, to)?;
    } else {
        std::os::windows::fs::symlink_file(target, to)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_directories_are_omitted_from_snapshots() -> Result<()> {
        let source = tempfile::tempdir()?;
        fs::write(source.path().join("code.go"), "original")?;
        let excluded = vec![
            source.path().join(".mutie"),
            source.path().join("artifacts/reports"),
        ];
        for directory in &excluded {
            fs::create_dir_all(directory)?;
            fs::write(directory.join("previous-run"), "report")?;
        }
        #[cfg(unix)]
        std::os::unix::fs::symlink("artifacts/reports", source.path().join("report-link"))?;
        let workspace = Workspace::new_excluding(source.path(), &excluded)?;
        assert!(workspace.path().join("code.go").is_file());
        assert!(!workspace.path().join(".mutie").exists());
        assert!(!workspace.path().join("artifacts/reports").exists());
        assert!(!workspace.path().join("report-link").exists());
        Ok(())
    }

    #[test]
    fn reset_restores_files_and_removes_test_artifacts_without_touching_input() -> Result<()> {
        let source = tempfile::tempdir()?;
        fs::write(source.path().join("code.go"), "original")?;
        fs::create_dir(source.path().join(".git"))?;
        fs::write(source.path().join(".git/config"), "private")?;
        let workspace = Workspace::new(source.path())?;
        assert!(!workspace.path().join(".git").exists());
        fs::write(workspace.path().join("code.go"), "mutated")?;
        fs::write(workspace.path().join("artifact"), "test output")?;
        assert_eq!(
            fs::read_to_string(source.path().join("code.go"))?,
            "original"
        );
        workspace.reset(source.path())?;
        assert_eq!(
            fs::read_to_string(workspace.path().join("code.go"))?,
            "original"
        );
        assert!(!workspace.path().join("artifact").exists());
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn internal_symlinks_stay_in_copy_and_external_links_are_rejected() -> Result<()> {
        let source = tempfile::tempdir()?;
        fs::write(source.path().join("data"), "original")?;
        std::os::unix::fs::symlink(source.path().join("data"), source.path().join("link"))?;
        let workspace = Workspace::new(source.path())?;
        fs::write(workspace.path().join("link"), "changed")?;
        assert_eq!(fs::read_to_string(source.path().join("data"))?, "original");
        assert_eq!(
            fs::read_to_string(workspace.path().join("data"))?,
            "changed"
        );
        let outside = tempfile::NamedTempFile::new()?;
        std::os::unix::fs::symlink(outside.path(), source.path().join("external"))?;
        assert!(Workspace::new(source.path()).is_err());
        Ok(())
    }
}
