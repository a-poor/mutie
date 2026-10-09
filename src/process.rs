use std::{
    future::{Future, Ready, pending},
    io::{Read, Seek, SeekFrom},
    path::Path,
    process::{ExitStatus, Stdio},
    time::{Duration, Instant},
};

use anyhow::{Context, Result};
use tokio::{io::AsyncReadExt, process::Command};

use crate::cancellation::Cancellation;

pub struct CommandOutput {
    pub status: ExitStatus,
    pub timed_out: bool,
    pub interrupted: bool,
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
    cancellation: &Cancellation,
) -> Result<CommandOutput> {
    run_inner(
        program,
        args,
        root,
        limit,
        cancellation,
        None::<fn(&str) -> Ready<()>>,
    )
    .await
}

/// Observe complete stdout lines while retaining the original file-backed
/// captures. The callback is advisory; deadlines and cancellation can interrupt it.
pub async fn run_observed<F, Fut>(
    program: &str,
    args: &[&str],
    root: &Path,
    limit: Duration,
    cancellation: &Cancellation,
    observer: F,
) -> Result<CommandOutput>
where
    F: FnMut(&str) -> Fut + Send,
    Fut: Future<Output = ()> + Send,
{
    run_inner(program, args, root, limit, cancellation, Some(observer)).await
}

async fn run_inner<F, Fut>(
    program: &str,
    args: &[&str],
    root: &Path,
    limit: Duration,
    cancellation: &Cancellation,
    observer: Option<F>,
) -> Result<CommandOutput>
where
    F: FnMut(&str) -> Fut + Send,
    Fut: Future<Output = ()> + Send,
{
    cancellation.check()?;
    let mut stdout = tempfile::NamedTempFile::new()?;
    let mut stderr = tempfile::tempfile()?;
    // Reopen, do not clone: a cloned descriptor shares the child's write offset.
    let reader = observer.as_ref().map(|_| stdout.reopen()).transpose()?;
    let mut command = Command::new(program);
    command
        .args(args)
        .current_dir(root)
        .stdin(Stdio::null())
        .stdout(stdout.as_file().try_clone()?)
        .stderr(stderr.try_clone()?)
        .kill_on_drop(true);
    // Outlive the child and its process-group guard so they issue any required
    // kills before temporary builds are removed, including on spawn/wait errors
    // or when this future is dropped.
    let _go_build_directory = if program == "go" {
        // v1 runs one module; do not pick up a workspace outside the snapshot.
        command.env("GOWORK", "off");
        let parent = std::env::var_os("GOTMPDIR")
            .filter(|value| !value.is_empty())
            .map(std::path::PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        let parent = parent.canonicalize().with_context(|| {
            format!(
                "resolving Go temporary build directory {}",
                parent.display()
            )
        })?;
        let directory = tempfile::Builder::new()
            .prefix("mutie-go-")
            .tempdir_in(&parent)
            .with_context(|| {
                format!(
                    "creating Go temporary build directory in {}",
                    parent.display()
                )
            })?;
        // Go cannot remove its go-build* trees after SIGKILL. Own a private
        // parent for every invocation; never prune caller-owned files or GOCACHE.
        command.env("GOTMPDIR", directory.path());
        Some(directory)
    } else {
        None
    };
    #[cfg(unix)]
    command.process_group(0);
    let started = Instant::now();
    let mut child = command
        .spawn()
        .with_context(|| format!("could not start {program}; is it installed and on PATH?"))?;
    let mut group = ProcessGroup { id: child.id() };
    let mut timed_out = false;
    let mut interrupted = false;
    let observation = async {
        if let (Some(reader), Some(observer)) = (reader, observer) {
            // Progress is best effort. If its independent reader fails, retain
            // normal execution and the authoritative captures read after exit.
            let _ = observe_lines(reader, observer).await;
        }
        pending::<()>().await
    };
    let status = tokio::select! {
        biased;
        result = tokio::time::timeout(limit, child.wait()) => match result {
            Ok(status) => status.context("waiting for child process")?,
            Err(_) => {
                timed_out = true;
                group.kill();
                child.start_kill()?;
                child.wait().await?
            }
        },
        _ = cancellation.cancelled() => {
            interrupted = true;
            group.kill();
            child.start_kill()?;
            child.wait().await?
        },
        _ = observation => unreachable!("observation never completes"),
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
        interrupted,
        elapsed,
        stdout: read(stdout.as_file_mut())?,
        stderr: read(&mut stderr)?,
    })
}

const MAX_PROGRESS_LINE: usize = 64 * 1024;

/// A bounded, incremental decoder; oversized/invalid lines are omitted only
/// from progress observation, never from saved output.
#[derive(Default)]
struct ProgressLines {
    partial: Vec<u8>,
    skipping: bool,
}

impl ProgressLines {
    fn push(&mut self, bytes: &[u8]) -> Vec<String> {
        let mut lines = Vec::new();
        for part in bytes.split_inclusive(|byte| *byte == b'\n') {
            if !self.skipping {
                if self.partial.len() + part.len() > MAX_PROGRESS_LINE {
                    self.partial.clear();
                    self.skipping = true;
                } else {
                    self.partial.extend_from_slice(part);
                }
            }
            if part.ends_with(b"\n") {
                if !self.skipping
                    && let Ok(line) = std::str::from_utf8(&self.partial)
                {
                    lines.push(line.trim_end_matches(['\r', '\n']).to_owned());
                }
                self.partial.clear();
                self.skipping = false;
            }
        }
        lines
    }
}

async fn observe_lines<F, Fut>(reader: std::fs::File, mut observer: F) -> std::io::Result<()>
where
    F: FnMut(&str) -> Fut,
    Fut: Future<Output = ()>,
{
    let mut reader = tokio::fs::File::from_std(reader);
    let mut decoder = ProgressLines::default();
    let mut bytes = [0; 16 * 1024];
    let mut tick = tokio::time::interval(Duration::from_millis(50));
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        tick.tick().await;
        // Bound each batch so noisy commands cannot monopolize the executor.
        for _ in 0..16 {
            let count = reader.read(&mut bytes).await?;
            if count == 0 {
                break;
            }
            for line in decoder.push(&bytes[..count]) {
                observer(&line).await;
            }
        }
    }
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

    #[test]
    fn progress_decoder_handles_split_unicode_crlf_and_oversized_lines() {
        let mut lines = ProgressLines::default();
        assert!(lines.push(b"first\r").is_empty());
        assert_eq!(lines.push(b"\n\xe7\x95"), ["first"]);
        assert_eq!(lines.push(b"\x8c\nnext\n"), ["界", "next"]);
        assert!(lines.push(&vec![b'x'; MAX_PROGRESS_LINE]).is_empty());
        assert!(lines.push(b"more").is_empty());
        assert!(lines.partial.is_empty());
        assert_eq!(lines.push(b"\n\xff\nrecovered\n"), ["recovered"]);
    }

    #[tokio::test]
    async fn observes_output_before_exit_without_changing_captured_bytes() -> Result<()> {
        let root = tempfile::tempdir()?;
        let mut seen = Vec::new();
        let release = root.path().join("release");
        let output = run_observed("sh", &["-c", "printf 'first\\n'; printf 'diagnostic\\n' >&2; while [ ! -f release ]; do sleep 0.02; done; printf 'last without newline'"], root.path(), Duration::from_secs(5), &Cancellation::default(), |line| {
            seen.push(line.to_owned());
            std::fs::write(&release, "").unwrap();
            std::future::ready(())
        }).await?;
        assert!(output.status.success() && !output.timed_out);
        assert!(seen.contains(&"first".to_owned()));
        assert_eq!(output.stdout, "first\nlast without newline");
        assert_eq!(output.stderr, "diagnostic\n");
        Ok(())
    }

    #[tokio::test]
    async fn blocked_progress_observer_cannot_block_cancellation_or_deadline() -> Result<()> {
        use std::sync::atomic::{AtomicBool, Ordering};
        for cancel in [true, false] {
            let root = tempfile::tempdir()?;
            let cancellation = Cancellation::default();
            let observing = AtomicBool::new(false);
            let command = run_observed(
                "sh",
                &[
                    "-c",
                    "printf 'captured\\n'; printf 'error output\\n' >&2; (sleep 1; touch leaked) & sleep 30",
                ],
                root.path(),
                Duration::from_millis(400),
                &cancellation,
                |_| {
                    observing.store(true, Ordering::SeqCst);
                    pending::<()>()
                },
            );
            let stop = async {
                tokio::time::timeout(Duration::from_secs(3), async {
                    while !observing.load(Ordering::SeqCst) {
                        tokio::time::sleep(Duration::from_millis(5)).await;
                    }
                })
                .await
                .unwrap();
                if cancel {
                    cancellation.cancel();
                }
            };
            let (output, ()) = tokio::join!(command, stop);
            let output = output?;
            assert_eq!(output.interrupted, cancel);
            assert_eq!(output.timed_out, !cancel);
            assert_eq!(output.stdout, "captured\n");
            assert_eq!(output.stderr, "error output\n");
            tokio::time::sleep(Duration::from_millis(1050)).await;
            assert!(!root.path().join("leaked").exists());
        }
        Ok(())
    }

    #[tokio::test]
    async fn cancellation_retains_output_and_kills_descendants() -> Result<()> {
        let root = tempfile::tempdir()?;
        let cancellation = Cancellation::default();
        let command = run(
            "sh",
            &[
                "-c",
                "printf 'before cancellation\\n'; printf 'diagnostic\\n' >&2; (sleep 0.5; touch leaked) & touch ready; wait",
            ],
            root.path(),
            Duration::from_secs(10),
            &cancellation,
        );
        let stop = async {
            tokio::time::timeout(Duration::from_secs(5), async {
                while !root.path().join("ready").exists() {
                    tokio::time::sleep(Duration::from_millis(5)).await;
                }
            })
            .await
            .expect("child did not start");
            cancellation.cancel();
        };
        let (output, ()) = tokio::join!(command, stop);
        let output = output?;
        assert!(output.interrupted);
        assert!(!output.timed_out);
        assert_eq!(output.stdout, "before cancellation\n");
        assert_eq!(output.stderr, "diagnostic\n");
        tokio::time::sleep(Duration::from_millis(600)).await;
        assert!(!root.path().join("leaked").exists());
        Ok(())
    }

    #[tokio::test]
    async fn cancellation_before_start_does_not_spawn_a_process() -> Result<()> {
        let root = tempfile::tempdir()?;
        let cancellation = Cancellation::default();
        cancellation.cancel();
        let error = run(
            "sh",
            &["-c", "touch started"],
            root.path(),
            Duration::from_secs(1),
            &cancellation,
        )
        .await
        .err()
        .unwrap();
        assert!(error.is::<crate::cancellation::Interrupted>());
        assert!(!root.path().join("started").exists());
        Ok(())
    }

    #[tokio::test]
    async fn timeout_kills_descendants_before_they_can_write() -> Result<()> {
        let root = tempfile::tempdir()?;
        let output = run(
            "sh",
            &["-c", "(sleep 0.5; touch leaked) & wait"],
            root.path(),
            Duration::from_millis(100),
            &Cancellation::default(),
        )
        .await?;
        assert!(output.timed_out);
        tokio::time::sleep(Duration::from_millis(600)).await;
        assert!(!root.path().join("leaked").exists());
        Ok(())
    }
}
