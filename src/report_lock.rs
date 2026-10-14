//! Persistent advisory locks shared by report writers and retention commands.
use std::{
    fs::{self, File, OpenOptions, TryLockError},
    io::ErrorKind,
    path::Path,
};

use anyhow::{Context, Result, bail, ensure};

pub(crate) const NAME: &str = ".mutie.lock";

/// Closing one descriptor does not release a lock while a fork/dup copy of the
/// same open-file description survives. End ownership explicitly before close.
pub(crate) struct Lock(File);

impl Lock {
    pub(crate) fn as_file(&self) -> &File {
        &self.0
    }
}

impl Drop for Lock {
    fn drop(&mut self) {
        if let Err(error) = self.0.unlock() {
            tracing::warn!(%error, "releasing report lock");
        }
    }
}

/// Inspectors never create files. Applying retention can add a lock to a legacy
/// finalized report. Never unlink a live lock: another opener must see its inode.
pub(crate) fn acquire(directory: &Path, create: bool) -> Result<Option<Lock>> {
    let path = directory.join(NAME);
    let file = match fs::symlink_metadata(&path) {
        Ok(metadata) => {
            ensure!(metadata.is_file(), "report lock is not a regular file");
            File::open(&path).context("opening report lock")?
        }
        Err(error) if error.kind() == ErrorKind::NotFound => {
            if !create {
                return Ok(None);
            }
            match OpenOptions::new().write(true).create_new(true).open(&path) {
                Ok(file) => file,
                // A concurrent pruner won creation; open the same lock inode.
                Err(error) if error.kind() == ErrorKind::AlreadyExists => {
                    return acquire(directory, false);
                }
                Err(error) => return Err(error).context("creating report lock"),
            }
        }
        Err(error) => return Err(error).context("inspecting report lock"),
    };
    match file.try_lock() {
        Ok(()) => Ok(Some(Lock(file))),
        Err(TryLockError::WouldBlock) => bail!("report is in use (locked)"),
        Err(TryLockError::Error(error)) => Err(error).context("locking report"),
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn releasing_owner_unlocks_even_while_an_inherited_descriptor_exists() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let owner = acquire(directory.path(), true)?.unwrap();
        // dup shares the same open-file description, as an inherited fork
        // descriptor does, without needing an unsafe fork in the test harness.
        let inherited = owner.as_file().try_clone()?;
        assert!(acquire(directory.path(), false).is_err());
        drop(owner);
        let next = acquire(directory.path(), false)?;
        assert!(next.is_some());
        drop(inherited);
        assert!(acquire(directory.path(), false).is_err());
        drop(next);
        assert!(acquire(directory.path(), false)?.is_some());
        Ok(())
    }
}
