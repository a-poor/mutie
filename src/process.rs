use std::{
    future::{Future, Ready, pending},
    io::Write,
    num::NonZeroUsize,
    path::Path,
    process::{ExitStatus, Stdio},
    time::{Duration, Instant},
};

use anyhow::{Context, Result};
use tokio::{
    io::AsyncReadExt,
    process::{ChildStderr, ChildStdout, Command},
};

use crate::{cancellation::Cancellation, capture::Capture};

pub struct CommandOutput {
    pub status: ExitStatus,
    pub timed_out: bool,
    pub interrupted: bool,
    pub capture_error: Option<String>,
    pub elapsed: Duration,
    pub stdout: Capture,
    pub stderr: Capture,
}

/// Unlimited captures go directly to files. Limited captures use drained pipes
/// so only Mutie can write to the files and enforce a shared byte budget.
pub async fn run(
    program: &str,
    args: &[&str],
    root: &Path,
    limit: Duration,
    capture_limit: Option<NonZeroUsize>,
    cancellation: &Cancellation,
) -> Result<CommandOutput> {
    run_inner(
        program,
        args,
        root,
        limit,
        capture_limit,
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
    capture_limit: Option<NonZeroUsize>,
    cancellation: &Cancellation,
    observer: F,
) -> Result<CommandOutput>
where
    F: FnMut(&str) -> Fut + Send,
    Fut: Future<Output = ()> + Send,
{
    run_inner(
        program,
        args,
        root,
        limit,
        capture_limit,
        cancellation,
        Some(observer),
    )
    .await
}

async fn run_inner<F, Fut>(
    program: &str,
    args: &[&str],
    root: &Path,
    limit: Duration,
    capture_limit: Option<NonZeroUsize>,
    cancellation: &Cancellation,
    observer: Option<F>,
) -> Result<CommandOutput>
where
    F: FnMut(&str) -> Fut + Send,
    Fut: Future<Output = ()> + Send,
{
    cancellation.check()?;
    let mut stdout = tempfile::NamedTempFile::new()?;
    let mut stderr = tempfile::NamedTempFile::new()?;
    // Reopen, do not clone: observation must not move the capture writer's offset.
    let reader = observer.as_ref().map(|_| stdout.reopen()).transpose()?;
    let mut command = Command::new(program);
    command
        .args(args)
        .current_dir(root)
        .stdin(Stdio::null())
        .stdout(if capture_limit.is_some() {
            Stdio::piped()
        } else {
            stdout.as_file().try_clone()?.into()
        })
        .stderr(if capture_limit.is_some() {
            Stdio::piped()
        } else {
            stderr.as_file().try_clone()?.into()
        })
        .kill_on_drop(true);
    // Outlive the child and its process-group guard so they issue any required
    // kills before temporary files are removed, including on spawn/wait errors
    // or when this future is dropped.
    let _go_directories = if program == "go" {
        // v1 runs one module; do not pick up a workspace outside the snapshot.
        command.env("GOWORK", "off");
        let test_parent = std::env::temp_dir();
        let test_parent = test_parent.canonicalize().with_context(|| {
            format!(
                "resolving Go test temporary directory {}",
                test_parent.display()
            )
        })?;
        let tests = tempfile::Builder::new()
            .prefix("mutie-test-")
            .tempdir_in(&test_parent)
            .with_context(|| {
                format!(
                    "creating Go test temporary directory in {}",
                    test_parent.display()
                )
            })?;
        // Go tests and their helpers can leave files outside the workspace.
        // Set all common aliases in the child only, including for discovery.
        for variable in ["TMPDIR", "TMP", "TEMP"] {
            command.env(variable, tests.path());
        }
        let parent = std::env::var_os("GOTMPDIR")
            .filter(|value| !value.is_empty())
            .map(std::path::PathBuf::from)
            .unwrap_or(test_parent);
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
        Some((tests, directory))
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
    let mut capture_error = None;
    let observation = async {
        if let (Some(reader), Some(observer)) = (reader, observer) {
            // Progress is best effort. If its independent reader fails, retain
            // normal execution and the authoritative captures read after exit.
            let _ = observe_lines(reader, observer).await;
        }
        pending::<()>().await
    };
    let stdout_pipe = child.stdout.take();
    let stderr_pipe = child.stderr.take();
    let status = {
        let capture = async {
            if let Some(budget) = capture_limit {
                capture_pipes(
                    stdout_pipe.context("missing stdout pipe")?,
                    stderr_pipe.context("missing stderr pipe")?,
                    stdout.as_file_mut(),
                    stderr.as_file_mut(),
                    budget.get(),
                )
                .await
            } else {
                pending::<Result<()>>().await
            }
        };
        let deadline = tokio::time::sleep(limit);
        tokio::pin!(capture, deadline, observation);
        let mut captured = false;
        let status = loop {
            tokio::select! {
                biased;
                result = &mut capture, if !captured => {
                    captured = true;
                    if let Err(error) = result {
                        capture_error = Some(format!("{error:#}"));
                        group.kill();
                        child.start_kill()?;
                        break child.wait().await?;
                    }
                },
                result = child.wait() => break result.context("waiting for child process")?,
                _ = &mut deadline => {
                    timed_out = true;
                    group.kill();
                    child.start_kill()?;
                    break child.wait().await?;
                },
                _ = cancellation.cancelled() => {
                    interrupted = true;
                    group.kill();
                    child.start_kill()?;
                    break child.wait().await?;
                },
                _ = &mut observation => unreachable!("observation never completes"),
            }
        };
        group.kill();
        // Drain bytes already in the pipes even when the child exits first.
        // A descendant outside the process group may still hold a pipe open;
        // bound that wait and explicitly fail rather than silently lose output.
        if capture_limit.is_some() && !captured {
            capture_error = match tokio::time::timeout(Duration::from_secs(1), &mut capture).await {
                Ok(Ok(())) => None,
                Ok(Err(error)) => Some(format!("{error:#}")),
                Err(_) => Some("output capture did not reach EOF after command shutdown".into()),
            };
        }
        status
    };
    let elapsed = started.elapsed();
    Ok(CommandOutput {
        status,
        timed_out,
        interrupted,
        capture_error,
        elapsed,
        stdout: Capture::new(stdout)?,
        stderr: Capture::new(stderr)?,
    })
}

/// Both streams share one budget. Read one byte beyond an exactly full budget
/// to distinguish overflow from output that ends precisely at the limit.
async fn capture_pipes(
    mut stdout: ChildStdout,
    mut stderr: ChildStderr,
    stdout_file: &mut std::fs::File,
    stderr_file: &mut std::fs::File,
    budget: usize,
) -> Result<()> {
    let mut out = [0; 16 * 1024];
    let mut err = [0; 16 * 1024];
    let (mut out_done, mut err_done) = (false, false);
    let mut remaining = budget;
    while !out_done || !err_done {
        let read_size = remaining.saturating_add(1).min(out.len());
        let (count, bytes, file) = tokio::select! {
            result = stdout.read(&mut out[..read_size]), if !out_done => {
                let count = result.context("reading command stdout")?;
                out_done = count == 0;
                (count, &out[..count], &mut *stdout_file)
            },
            result = stderr.read(&mut err[..read_size]), if !err_done => {
                let count = result.context("reading command stderr")?;
                err_done = count == 0;
                (count, &err[..count], &mut *stderr_file)
            },
        };
        let keep = count.min(remaining);
        // Synchronous bounded writes finish before cancellation can drop the
        // capture, so no background writer can race cleanup or saved lengths.
        file.write_all(&bytes[..keep])
            .context("writing command output capture")?;
        remaining -= keep;
        anyhow::ensure!(
            count == keep,
            "command output exceeded --max-capture-bytes ({budget} bytes combined stdout/stderr); command stopped, captured output is incomplete"
        );
    }
    Ok(())
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

    #[tokio::test]
    async fn capture_budget_is_shared_and_exact_boundary_succeeds_even_on_fast_exit() -> Result<()>
    {
        let root = tempfile::tempdir()?;
        for (script, budget, exceeded, stdout, stderr) in [
            (
                "printf 1234; printf 56789 >&2",
                9,
                false,
                Some("1234"),
                Some("56789"),
            ),
            ("printf 123456789", 8, true, Some("12345678"), Some("")),
            ("printf 123456789 >&2", 8, true, Some(""), Some("12345678")),
            ("printf 1234; printf 56789 >&2", 8, true, None, None),
            ("printf x", 1, false, Some("x"), Some("")),
            ("true", 1, false, Some(""), Some("")),
        ] {
            let output = run(
                "sh",
                &["-c", script],
                root.path(),
                Duration::from_secs(5),
                NonZeroUsize::new(budget),
                &Cancellation::default(),
            )
            .await?;
            assert_eq!(output.capture_error.is_some(), exceeded, "{script}");
            assert!(!output.timed_out && !output.interrupted);
            let captured =
                output.stdout.retention()?.total_bytes + output.stderr.retention()?.total_bytes;
            assert!(captured <= budget as u64);
            if exceeded {
                assert_eq!(captured, budget as u64);
                assert!(
                    output
                        .capture_error
                        .as_ref()
                        .unwrap()
                        .contains("--max-capture-bytes")
                );
            } else {
                assert!(output.status.success());
            }
            if let Some(expected) = stdout {
                assert_eq!(output.stdout.text()?, expected);
            }
            if let Some(expected) = stderr {
                assert_eq!(output.stderr.text()?, expected);
            }
        }
        Ok(())
    }

    #[tokio::test]
    async fn noisy_capture_stops_descendants_even_with_a_blocked_observer() -> Result<()> {
        let root = tempfile::tempdir()?;
        let release = root.path().join("release");
        let output = run_observed("sh", &["-c", "printf 'ready\\n'; while [ ! -f release ]; do sleep 0.01; done; (sleep 1; touch leaked) & while :; do printf 'stdout payload\\n'; printf 'stderr payload\\n' >&2; done"], root.path(), Duration::from_secs(5), NonZeroUsize::new(4096), &Cancellation::default(), |_| {
            std::fs::write(&release, "").unwrap();
            pending::<()>()
        }).await?;
        assert!(release.exists());
        assert!(output.capture_error.as_ref().unwrap().contains("4096"));
        assert!(!output.timed_out && !output.interrupted);
        assert_eq!(
            output.stdout.retention()?.total_bytes + output.stderr.retention()?.total_bytes,
            4096
        );
        assert!(output.stdout.text()?.starts_with("ready\n"));
        tokio::time::sleep(Duration::from_millis(1100)).await;
        assert!(!root.path().join("leaked").exists());
        Ok(())
    }

    #[tokio::test]
    async fn limited_capture_does_not_wait_for_descendants_after_parent_exit() -> Result<()> {
        let root = tempfile::tempdir()?;
        let output = run(
            "sh",
            &[
                "-c",
                "printf 'finished\\n'; (sleep 30; touch leaked) & exit 0",
            ],
            root.path(),
            Duration::from_secs(5),
            NonZeroUsize::new(4096),
            &Cancellation::default(),
        )
        .await?;
        assert!(output.status.success());
        assert!(output.capture_error.is_none());
        assert_eq!(output.stdout.text()?, "finished\n");
        assert!(output.elapsed < Duration::from_secs(3));
        Ok(())
    }

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
        let output = run_observed("sh", &["-c", "printf 'first\\n'; printf 'diagnostic\\n' >&2; while [ ! -f release ]; do sleep 0.02; done; printf 'last without newline'"], root.path(), Duration::from_secs(5), None, &Cancellation::default(), |line| {
            seen.push(line.to_owned());
            std::fs::write(&release, "").unwrap();
            std::future::ready(())
        }).await?;
        assert!(output.status.success() && !output.timed_out);
        assert!(seen.contains(&"first".to_owned()));
        assert_eq!(output.stdout.text()?, "first\nlast without newline");
        assert_eq!(output.stderr.text()?, "diagnostic\n");
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
                None,
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
            assert_eq!(output.stdout.text()?, "captured\n");
            assert_eq!(output.stderr.text()?, "error output\n");
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
            None,
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
        assert_eq!(output.stdout.text()?, "before cancellation\n");
        assert_eq!(output.stderr.text()?, "diagnostic\n");
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
            None,
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
            None,
            &Cancellation::default(),
        )
        .await?;
        assert!(output.timed_out);
        tokio::time::sleep(Duration::from_millis(600)).await;
        assert!(!root.path().join("leaked").exists());
        Ok(())
    }
}
