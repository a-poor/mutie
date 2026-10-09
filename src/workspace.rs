use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail, ensure};
use tempfile::TempDir;

use crate::{cancellation::Cancellation, workspace_metrics::WorkspaceMeasurement};

/// Owns only temporary data. The input working tree is never a mutation target.
pub struct Workspace {
    directory: TempDir,
}

impl Workspace {
    #[cfg(test)]
    pub fn new(source: &Path) -> Result<Self> {
        Self::new_excluding(source, &[])
    }

    #[cfg(test)]
    pub fn new_excluding(source: &Path, excluded: &[PathBuf]) -> Result<Self> {
        Self::create(source, excluded, &Cancellation::default())
    }

    pub fn create(
        source: &Path,
        excluded: &[PathBuf],
        cancellation: &Cancellation,
    ) -> Result<Self> {
        Self::create_measured(source, excluded, cancellation).0
    }

    pub fn create_measured(
        source: &Path,
        excluded: &[PathBuf],
        cancellation: &Cancellation,
    ) -> (Result<Self>, WorkspaceMeasurement) {
        Self::create_using(source, excluded, cancellation, &mut |from, to| {
            reflink_copy::reflink_or_copy(from, to)
        })
    }

    fn create_using(
        source: &Path,
        excluded: &[PathBuf],
        cancellation: &Cancellation,
        copier: &mut impl FnMut(&Path, &Path) -> std::io::Result<Option<u64>>,
    ) -> (Result<Self>, WorkspaceMeasurement) {
        WorkspaceMeasurement::measure(|measurement| {
            cancellation.check()?;
            // Go and symlink resolution return physical paths. Keep workspace paths
            // physical too (not macOS's /var alias for /private/var).
            let temporary_root = std::env::temp_dir()
                .canonicalize()
                .context("resolving temporary workspace directory")?;
            measurement.temporary_root = Some(temporary_root.clone());
            let directory = tempfile::Builder::new()
                .prefix("mutie-")
                .tempdir_in(temporary_root)?;
            ensure!(
                !directory.path().canonicalize()?.starts_with(source),
                "temporary directory is inside the module; set TMPDIR to a directory outside it"
            );
            let workspace = Self { directory };
            let destination = workspace.path();
            DirectoryCopy {
                excluded,
                cancellation,
                measurement,
                copier,
            }
            .copy(source, &destination, source, &destination)?;
            Ok(workspace)
        })
    }

    pub fn path(&self) -> std::path::PathBuf {
        self.directory.path().join("repo")
    }

    /// Restore the entire workspace, including files changed or created by tests.
    #[cfg(test)]
    pub fn reset(&self, source: &Path, cancellation: &Cancellation) -> Result<()> {
        self.reset_measured(source, cancellation).0
    }

    pub fn reset_measured(
        &self,
        source: &Path,
        cancellation: &Cancellation,
    ) -> (Result<()>, WorkspaceMeasurement) {
        self.reset_using(source, cancellation, &mut |from, to| {
            reflink_copy::reflink_or_copy(from, to)
        })
    }

    fn reset_using(
        &self,
        source: &Path,
        cancellation: &Cancellation,
        copier: &mut impl FnMut(&Path, &Path) -> std::io::Result<Option<u64>>,
    ) -> (Result<()>, WorkspaceMeasurement) {
        WorkspaceMeasurement::measure(|measurement| {
            cancellation.check()?;
            measurement.temporary_root = self.directory.path().parent().map(Path::to_owned);
            let destination = self.path();
            if destination.exists() {
                fs::remove_dir_all(&destination).context("could not reset temporary workspace")?;
            }
            DirectoryCopy {
                excluded: &[],
                cancellation,
                measurement,
                copier,
            }
            .copy(source, &destination, source, &destination)
        })
    }
}

struct DirectoryCopy<'a, C> {
    excluded: &'a [PathBuf],
    cancellation: &'a Cancellation,
    measurement: &'a mut WorkspaceMeasurement,
    copier: &'a mut C,
}

impl<C: FnMut(&Path, &Path) -> std::io::Result<Option<u64>>> DirectoryCopy<'_, C> {
    fn copy(
        &mut self,
        source: &Path,
        destination: &Path,
        root: &Path,
        copy_root: &Path,
    ) -> Result<()> {
        self.cancellation.check()?;
        fs::create_dir_all(destination)?;
        for entry in
            fs::read_dir(source).with_context(|| format!("reading {}", source.display()))?
        {
            let entry = entry?;
            self.cancellation.check()?;
            if entry.file_name() == ".git" {
                continue;
            }
            let from = entry.path();
            if self.excluded.contains(&from) {
                continue;
            }
            let to = destination.join(entry.file_name());
            let kind = entry.file_type()?;
            if kind.is_dir() {
                self.copy(&from, &to, root, copy_root)?;
            } else if kind.is_file() {
                let copied = (self.copier)(&from, &to)
                    .with_context(|| format!("copying {}", from.display()))?;
                self.measurement.copies.record(copied);
                fs::set_permissions(&to, entry.metadata()?.permissions())?;
            } else if kind.is_symlink() {
                if from
                    .canonicalize()
                    .is_ok_and(|target| self.excluded.iter().any(|path| target.starts_with(path)))
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
    use crate::workspace_metrics::OperationStatus;

    #[test]
    fn measured_copies_distinguish_reflinks_empty_fallbacks_and_exclusions() -> Result<()> {
        let source = tempfile::tempdir_in(std::env::temp_dir().canonicalize()?)?;
        fs::create_dir(source.path().join("nested"))?;
        fs::create_dir(source.path().join(".git"))?;
        fs::create_dir(source.path().join("reports"))?;
        fs::write(source.path().join("clone"), "original")?;
        fs::write(source.path().join("nested/fallback"), "copied")?;
        fs::write(source.path().join("nested/empty"), "")?;
        fs::write(source.path().join(".git/config"), "excluded")?;
        fs::write(source.path().join("reports/report"), "excluded")?;
        #[cfg(unix)]
        std::os::unix::fs::symlink("clone", source.path().join("link"))?;
        // Simulate both library return variants, independently of the host filesystem.
        let (result, measured) = Workspace::create_using(
            source.path(),
            &[source.path().join("reports")],
            &Cancellation::default(),
            &mut |from, to| {
                let bytes = fs::copy(from, to)?;
                Ok(if from.file_name().unwrap() == "clone" {
                    None
                } else {
                    Some(bytes)
                })
            },
        );
        let workspace = result?;
        assert_eq!(measured.status, OperationStatus::Completed);
        assert_eq!(measured.copies.reflink_files, 1);
        assert_eq!(measured.copies.copied_files, 2);
        assert_eq!(measured.copies.copied_bytes, 6);
        assert!(measured.duration_seconds.is_finite() && measured.duration_seconds >= 0.0);
        assert_eq!(
            measured.temporary_root,
            Some(std::env::temp_dir().canonicalize()?)
        );
        assert!(!workspace.path().join(".git").exists());
        assert!(!workspace.path().join("reports").exists());
        fs::write(workspace.path().join("clone"), "changed")?;
        assert_eq!(fs::read_to_string(source.path().join("clone"))?, "original");
        Ok(())
    }

    #[test]
    fn failed_and_cancelled_copies_keep_partial_counts_and_clean_up() -> Result<()> {
        for interrupt in [false, true] {
            let source = tempfile::tempdir_in(std::env::temp_dir().canonicalize()?)?;
            for name in ["a", "b", "c"] {
                fs::write(source.path().join(name), "abc")?;
            }
            let cancellation = Cancellation::default();
            let mut destination = None;
            let mut calls = 0;
            let (result, measured) =
                Workspace::create_using(source.path(), &[], &cancellation, &mut |from, to| {
                    calls += 1;
                    destination = to.parent().map(Path::to_owned);
                    if calls == 2 {
                        return Err(std::io::Error::other("injected copy failure"));
                    }
                    let bytes = fs::copy(from, to)?;
                    if interrupt {
                        cancellation.cancel();
                    }
                    Ok(Some(bytes))
                });
            assert!(result.is_err());
            assert_eq!(
                measured.status,
                if interrupt {
                    OperationStatus::Interrupted
                } else {
                    OperationStatus::Failed
                }
            );
            assert_eq!(measured.copies.copied_files, 1);
            assert_eq!(measured.copies.copied_bytes, 3);
            assert_eq!(measured.copies.reflink_files, 0);
            assert!(!destination.unwrap().exists());
            for name in ["a", "b", "c"] {
                assert_eq!(fs::read_to_string(source.path().join(name))?, "abc");
            }
        }
        Ok(())
    }

    #[test]
    fn reset_measurements_cover_deletion_recopy_and_removal_errors() -> Result<()> {
        let source = tempfile::tempdir_in(std::env::temp_dir().canonicalize()?)?;
        fs::write(source.path().join("source"), "pristine")?;
        let workspace = Workspace::new(source.path())?;
        fs::write(workspace.path().join("source"), "mutated")?;
        fs::write(workspace.path().join("test-artifact"), "side effect")?;
        let (result, measured) =
            workspace.reset_using(source.path(), &Cancellation::default(), &mut |from, to| {
                fs::copy(from, to).map(Some)
            });
        result?;
        assert_eq!(measured.status, OperationStatus::Completed);
        assert_eq!(measured.copies.copied_files, 1);
        assert_eq!(measured.copies.copied_bytes, 8);
        assert!(!workspace.path().join("test-artifact").exists());
        assert_eq!(
            fs::read_to_string(workspace.path().join("source"))?,
            "pristine"
        );
        // A plain file at the workspace directory forces remove_dir_all to fail.
        fs::remove_dir_all(workspace.path())?;
        fs::write(workspace.path(), "not a directory")?;
        let (result, measured) = workspace.reset_measured(source.path(), &Cancellation::default());
        assert!(result.is_err());
        assert_eq!(measured.status, OperationStatus::Failed);
        assert_eq!(
            measured.copies.reflink_files + measured.copies.copied_files,
            0
        );
        assert_eq!(
            fs::read_to_string(source.path().join("source"))?,
            "pristine"
        );
        Ok(())
    }

    #[test]
    fn report_directories_are_omitted_from_snapshots() -> Result<()> {
        let source = tempfile::tempdir_in(std::env::temp_dir().canonicalize()?)?;
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
        let source = tempfile::tempdir_in(std::env::temp_dir().canonicalize()?)?;
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
        workspace.reset(source.path(), &Cancellation::default())?;
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
        let source = tempfile::tempdir_in(std::env::temp_dir().canonicalize()?)?;
        fs::write(source.path().join("data"), "original")?;
        std::os::unix::fs::symlink(source.path().join("data"), source.path().join("link"))?;
        let workspace = Workspace::new(source.path())?;
        assert_eq!(workspace.path(), workspace.path().canonicalize()?);
        let worker = Workspace::new(&workspace.path())?;
        assert_eq!(fs::read_to_string(worker.path().join("link"))?, "original");
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
