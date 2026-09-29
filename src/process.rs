use std::{
    io::{Read, Seek, SeekFrom},
    path::Path,
    process::{ExitStatus, Stdio},
    time::{Duration, Instant},
};

use anyhow::{Context, Result, bail};
use tokio::process::Command;

pub struct CommandOutput {
    pub status: ExitStatus,
    pub timed_out: bool,
    pub elapsed: Duration,
    pub stdout: String,
    pub stderr: String,
}

/// Capture to files so a full pipe or a descendant holding it open cannot hang us.
pub async fn run(
    program: &str,
    args: &[&str],
    root: &Path,
    limit: Duration,
) -> Result<CommandOutput> {
    let mut stdout = tempfile::tempfile()?;
    let mut stderr = tempfile::tempfile()?;
    let mut command = Command::new(program);
    command
        .args(args)
        .current_dir(root)
        .stdin(Stdio::null())
        .stdout(stdout.try_clone()?)
        .stderr(stderr.try_clone()?)
        .kill_on_drop(true);
    if program == "go" {
        // v1 runs one module; do not pick up a workspace outside the snapshot.
        command.env("GOWORK", "off");
    }
    #[cfg(unix)]
    command.process_group(0);
    let started = Instant::now();
    let mut child = command
        .spawn()
        .with_context(|| format!("could not start {program}; is it installed and on PATH?"))?;
    let mut group = ProcessGroup { id: child.id() };
    let mut timed_out = false;
    let status = tokio::select! {
        result = tokio::time::timeout(limit, child.wait()) => match result {
            Ok(status) => status.context("waiting for child process")?,
            Err(_) => {
                timed_out = true;
                group.kill();
                child.start_kill()?;
                child.wait().await?
            }
        },
        signal = tokio::signal::ctrl_c() => {
            signal?;
            group.kill();
            child.start_kill()?;
            child.wait().await?;
            bail!("interrupted");
        }
    };
    group.kill();
    let elapsed = started.elapsed();
    fn read(file: &mut std::fs::File) -> Result<String> {
        file.seek(SeekFrom::Start(0))?;
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)?;
        Ok(String::from_utf8_lossy(&bytes).into_owned())
    }
    Ok(CommandOutput {
        status,
        timed_out,
        elapsed,
        stdout: read(&mut stdout)?,
        stderr: read(&mut stderr)?,
    })
}

struct ProcessGroup {
    id: Option<u32>,
}

impl ProcessGroup {
    fn kill(&mut self) {
        if let Some(id) = self.id.take() {
            #[cfg(unix)]
            // SAFETY: the child created its own process group with its PID. A
            // negative PID targets only that group; no pointers are involved.
            unsafe {
                libc::kill(-(id as i32), libc::SIGKILL);
            }
            #[cfg(not(unix))]
            let _ = id;
        }
    }
}

impl Drop for ProcessGroup {
    fn drop(&mut self) {
        self.kill();
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[tokio::test]
    async fn timeout_kills_descendants_before_they_can_write() -> Result<()> {
        let root = tempfile::tempdir()?;
        let output = run(
            "sh",
            &["-c", "(sleep 0.5; touch leaked) & wait"],
            root.path(),
            Duration::from_millis(100),
        )
        .await?;
        assert!(output.timed_out);
        tokio::time::sleep(Duration::from_millis(600)).await;
        assert!(!root.path().join("leaked").exists());
        Ok(())
    }
}
