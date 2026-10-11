//! Persistent advisory locks shared by report writers and retention commands.
use std::{
    fs::{self, File, OpenOptions, TryLockError},
    io::ErrorKind,
    path::Path,
};

use anyhow::{Context, Result, bail, ensure};

pub(crate) const NAME: &str = ".mutie.lock";

/// Inspectors never create files. Applying retention can add a lock to a legacy
/// finalized report. Never unlink a live lock: another opener must see its inode.
pub(crate) fn acquire(directory: &Path, create: bool) -> Result<Option<File>> {
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
        Ok(()) => Ok(Some(file)),
        Err(TryLockError::WouldBlock) => bail!("report is in use (locked)"),
        Err(TryLockError::Error(error)) => Err(error).context("locking report"),
    }
}
