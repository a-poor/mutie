//! Cargo adapter: one ordinary package and owned build artifacts.
use std::{
    ffi::OsStr,
    future::Future,
    io::{BufRead, BufReader, Write},
    num::NonZeroUsize,
    path::Path,
    time::{Duration, Instant},
};

use anyhow::{Context, Result, ensure};

use crate::{
    cancellation::{Cancellation, Interrupted},
    capture::Capture,
    command::{CommandOutput, Failure, TestPhase},
    process,
};

pub mod discovery;
pub mod project;
pub use project::{ExecutionScope, Project};

pub const TEST_ARGS: &[&str] = &[
    "test",
    "--locked",
    "--tests",
    "--no-fail-fast",
    "--message-format=json",
];
pub const METADATA_ARGS: &[&str] = &["metadata", "--format-version", "1", "--no-deps", "--locked"];

/// Owned by one worker for one run; never shared with the baseline or another worker.
pub struct BuildCache {
    directory: tempfile::TempDir,
    package: String,
}

impl BuildCache {
    pub fn new(package: String) -> Result<Self> {
        Ok(Self {
            directory: tempfile::Builder::new()
                .prefix("mutie-cargo-worker-")
                .tempdir()
                .context("creating worker-local Cargo build cache")?,
            package,
        })
    }
}

async fn cargo(
    args: &[&str],
    root: &Path,
    artifacts: &Path,
    limit: Duration,
    capture_limit: Option<NonZeroUsize>,
    cancellation: &Cancellation,
) -> Result<process::CommandOutput> {
    let directory = artifacts
        .to_str()
        .context("Cargo artifact path is not UTF-8")?;
    let value = toml::Value::String(directory.into()).to_string();
    let final_dir = format!("build.target-dir={value}");
    let build_dir = format!("build.build-dir={value}");
    let args: Vec<_> = args
        .iter()
        .copied()
        .chain(["--config", &final_dir, "--config", &build_dir])
        .collect();
    process::run_with_env(
        "cargo",
        &args,
        root,
        limit,
        capture_limit,
        cancellation,
        &[
            ("CARGO_TARGET_DIR", artifacts.as_os_str()),
            ("CARGO_BUILD_TARGET_DIR", artifacts.as_os_str()),
            ("CARGO_BUILD_BUILD_DIR", artifacts.as_os_str()),
            ("CARGO_INCREMENTAL", OsStr::new("0")),
        ],
    )
    .await
}

pub async fn discover(
    root: &Path,
    limit: Duration,
    log_limit: Option<NonZeroUsize>,
    capture_limit: Option<NonZeroUsize>,
    cancellation: &Cancellation,
) -> Result<CommandOutput> {
    let artifacts = tempfile::Builder::new()
        .prefix("mutie-cargo-metadata-")
        .tempdir()?;
    let raw = cargo(
        METADATA_ARGS,
        root,
        artifacts.path(),
        limit,
        capture_limit,
        cancellation,
    )
    .await?;
    prepare(raw, log_limit, Failure::Unknown).await
}

pub fn metadata(output: &CommandOutput) -> Result<serde_json::Value> {
    ensure!(!output.interrupted, "Cargo metadata interrupted");
    // Capture failures must remain actionable even when retained logs omit the
    // error notice. Never parse a potentially valid-looking captured prefix.
    if let Some(error) = &output.capture_error {
        anyhow::bail!("Cargo metadata: {error}");
    }
    ensure!(
        !output.timed_out && output.status.success(),
        "Cargo metadata failed:\n{}",
        output.text.text()?
    );
    serde_json::from_reader(output.stdout.reader()?).context("decoding Cargo metadata")
}

pub async fn versions(
    scope: &mut ExecutionScope,
    root: &Path,
    limit: Duration,
    cancellation: &Cancellation,
) -> Result<()> {
    for (program, arg, destination) in [
        ("rustc", "-vV", &mut scope.rustc),
        ("cargo", "--version", &mut scope.cargo),
    ] {
        let output = process::run(
            program,
            &[arg],
            root,
            limit,
            NonZeroUsize::new(64 * 1024),
            cancellation,
        )
        .await?;
        if output.interrupted {
            return Err(Interrupted.into());
        }
        ensure!(
            !output.timed_out && output.capture_error.is_none() && output.status.success(),
            "reading {program} version failed: {}",
            output.stderr.text()?
        );
        *destination = Some(output.stdout.text()?.trim().to_owned());
    }
    Ok(())
}

pub async fn test<F, Fut>(
    root: &Path,
    limit: Duration,
    log_limit: Option<NonZeroUsize>,
    capture_limit: Option<NonZeroUsize>,
    cache: Option<&BuildCache>,
    cancellation: &Cancellation,
    mut progress: F,
) -> Result<CommandOutput>
where
    F: FnMut(TestPhase) -> Fut + Send,
    Fut: Future<Output = ()> + Send,
{
    let started = Instant::now();
    // The guard spans compilation and execution, outside the resettable source.
    let fresh = cache
        .is_none()
        .then(|| {
            tempfile::Builder::new()
                .prefix("mutie-cargo-build-")
                .tempdir()
        })
        .transpose()?;
    let artifacts = cache
        .map(|cache| cache.directory.path())
        .unwrap_or_else(|| fresh.as_ref().unwrap().path());
    report_progress(progress(TestPhase::Building), limit, cancellation).await;
    // Cargo's own package cleanup removes *all* local target/fingerprint outputs,
    // including integration tests, independently of source mtimes or file size.
    // Registry dependencies remain in this worker's owned directory.
    let cleaned = if let Some(cache) = cache {
        let mut clean = cargo(
            &["clean", "--locked", "--package", &cache.package],
            root,
            artifacts,
            limit.saturating_sub(started.elapsed()),
            capture_limit,
            cancellation,
        )
        .await?;
        if !clean.status.success()
            || clean.timed_out
            || clean.interrupted
            || clean.capture_error.is_some()
        {
            if !clean.status.success()
                && !clean.timed_out
                && !clean.interrupted
                && clean.capture_error.is_none()
            {
                clean.capture_error =
                    Some("Cargo package cleanup failed before compilation".into());
            }
            clean.elapsed = started.elapsed();
            return prepare(clean, log_limit, Failure::Unknown).await;
        }
        Some(clean)
    } else {
        None
    };
    let build_args: Vec<_> = TEST_ARGS.iter().copied().chain(["--no-run"]).collect();
    let build = cargo(
        &build_args,
        root,
        artifacts,
        limit.saturating_sub(started.elapsed()),
        capture_limit,
        cancellation,
    )
    .await;
    let mut build = match build {
        Ok(build) => build,
        Err(error) => {
            let Some(mut clean) = cleaned else {
                return Err(error);
            };
            if error.is::<Interrupted>() {
                clean.interrupted = true;
            } else {
                clean.capture_error = Some(format!(
                    "starting Cargo compilation after cleanup: {error:#}"
                ));
            }
            clean.elapsed = started.elapsed();
            return prepare(clean, log_limit, Failure::Unknown).await;
        }
    };
    let evidence = Evidence::read(&build)?;
    if let Some(clean) = cleaned {
        build = merge(clean, build, "Cargo compilation")?;
    }
    if !build.status.success()
        || build.timed_out
        || build.interrupted
        || build.capture_error.is_some()
    {
        let failure = evidence.failure(build.status.code(), false);
        build.elapsed = started.elapsed();
        return prepare(build, log_limit, failure).await;
    }
    if evidence.build_finished != Some(true) || evidence.compiler_error {
        build.capture_error =
            Some("Cargo compilation returned no successful build-finished event".into());
        return prepare(build, log_limit, Failure::Unknown).await;
    }
    if cancellation.is_cancelled() {
        build.interrupted = true;
        return prepare(build, log_limit, Failure::Unknown).await;
    }
    if started.elapsed() >= limit {
        build.timed_out = true;
        return prepare(build, log_limit, Failure::Unknown).await;
    }
    report_progress(
        progress(TestPhase::Testing),
        limit.saturating_sub(started.elapsed()),
        cancellation,
    )
    .await;
    let tests = cargo(
        TEST_ARGS,
        root,
        artifacts,
        limit.saturating_sub(started.elapsed()),
        capture_limit,
        cancellation,
    )
    .await;
    let tests = match tests {
        Ok(tests) => tests,
        Err(error) => {
            if error.is::<Interrupted>() {
                build.interrupted = true;
            } else {
                build.capture_error =
                    Some(format!("starting Cargo tests after compilation: {error:#}"));
            }
            build.elapsed = started.elapsed();
            return prepare(build, log_limit, Failure::Unknown).await;
        }
    };
    let evidence = Evidence::read(&tests)?;
    let failure = evidence.failure(tests.status.code(), true);
    let mut raw = merge(build, tests, "Cargo test execution")?;
    raw.elapsed = started.elapsed();
    if raw.status.success()
        && !raw.timed_out
        && !raw.interrupted
        && raw.capture_error.is_none()
        && (evidence.build_finished != Some(true)
            || !evidence.test_passed
            || evidence.test_failed
            || evidence.compiler_error)
    {
        raw.capture_error = Some(
            "Cargo returned success without build and standard test-harness completion evidence"
                .into(),
        );
    }
    prepare(raw, log_limit, failure).await
}

// Phase hints are advisory, just like Go's stdout observer. A slow coordinator
// must not keep a command alive past cancellation or its deadline.
async fn report_progress(
    update: impl Future<Output = ()>,
    remaining: Duration,
    cancellation: &Cancellation,
) {
    tokio::select! {
        biased;
        _ = cancellation.cancelled() => {},
        _ = tokio::time::sleep(remaining) => {},
        _ = update => {},
    }
}

#[derive(Default)]
struct Evidence {
    build_finished: Option<bool>,
    compiler_error: bool,
    test_failed: bool,
    test_passed: bool,
}

impl Evidence {
    fn failure(&self, code: Option<i32>, testing: bool) -> Failure {
        if code != Some(101) {
            return Failure::Unknown;
        }
        if self.build_finished == Some(false) && self.compiler_error {
            Failure::Build
        } else if testing && self.build_finished == Some(true) && self.test_failed {
            Failure::Test
        } else {
            Failure::Unknown
        }
    }

    fn read(raw: &process::CommandOutput) -> Result<Self> {
        let mut evidence = Self::default();
        let mut line = Vec::new();
        let mut reader = BufReader::new(raw.stdout.reader()?);
        while reader.read_until(b'\n', &mut line)? != 0 {
            if let Ok(value) = serde_json::from_slice::<serde_json::Value>(&line) {
                if value["reason"] == "build-finished" {
                    evidence.build_finished = value["success"].as_bool();
                }
                if value["reason"] == "compiler-message" && value["message"]["level"] == "error" {
                    evidence.compiler_error = true;
                }
            } else {
                evidence.test_failed |= line.starts_with(b"test result: FAILED.");
                evidence.test_passed |= line.starts_with(b"test result: ok.");
            }
            line.clear();
        }
        Ok(evidence)
    }
}

fn merge(
    first: process::CommandOutput,
    mut last: process::CommandOutput,
    phase: &str,
) -> Result<process::CommandOutput> {
    fn join(first: Capture, last: Capture, phase: &str) -> Result<Capture> {
        let mut file = tempfile::NamedTempFile::new()?;
        std::io::copy(&mut first.reader()?, &mut file)?;
        writeln!(file, "\n--- {phase} ---")?;
        std::io::copy(&mut last.reader()?, &mut file)?;
        Ok(Capture::new(file)?)
    }
    last.stdout = join(first.stdout, last.stdout, phase)?;
    last.stderr = join(first.stderr, last.stderr, phase)?;
    Ok(last)
}

async fn prepare(
    raw: process::CommandOutput,
    log_limit: Option<NonZeroUsize>,
    failure: Failure,
) -> Result<CommandOutput> {
    tokio::task::spawn_blocking(move || {
        let mut raw = raw;
        let mut rendered = tempfile::NamedTempFile::new()?;
        let mut line = Vec::new();
        let mut reader = BufReader::new(raw.stdout.reader()?);
        while reader.read_until(b'\n', &mut line)? != 0 {
            match serde_json::from_slice::<serde_json::Value>(&line) {
                Ok(value) if value["reason"] == "compiler-message" => {
                    if let Some(text) = value["message"]["rendered"].as_str() {
                        rendered.write_all(text.as_bytes())?;
                    }
                }
                Ok(value) if value.get("reason").is_some() => {}
                _ => rendered.write_all(String::from_utf8_lossy(&line).as_bytes())?,
            }
            line.clear();
        }
        writeln!(rendered)?;
        crate::command::write_lossy(&mut raw.stderr.reader()?, &mut rendered)?;
        if let Some(error) = &raw.capture_error {
            writeln!(rendered, "\n[Output capture error: {error}]")?;
        }
        raw.stdout.set_limit(log_limit);
        raw.stderr.set_limit(log_limit);
        let mut text = Capture::utf8(rendered)?;
        text.set_limit(log_limit);
        Ok(CommandOutput {
            raw,
            text,
            evidence: crate::command::OutcomeEvidence::ExitStatus(failure),
            tool: "Cargo",
        })
    })
    .await
    .context("processing Cargo output panicked")?
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use crate::{command::classify, report::Outcome};
    use std::os::unix::process::ExitStatusExt;

    #[tokio::test]
    async fn metadata_capture_failure_survives_log_truncation_and_valid_json_prefix() -> Result<()>
    {
        let reason =
            "command output exceeded --max-capture-bytes=16; captured output is incomplete";
        for code in [0, 1] {
            let raw = process::CommandOutput {
                status: std::process::ExitStatus::from_raw(code << 8),
                timed_out: false,
                interrupted: false,
                capture_error: Some(reason.into()),
                elapsed: Duration::from_millis(1),
                stdout: Capture::from_bytes(b"{\"packages\":[]}"),
                stderr: Capture::from_bytes(b""),
            };
            let output = prepare(raw, NonZeroUsize::new(1), Failure::Unknown).await?;
            assert!(!output.text.text()?.contains(reason));
            assert!(metadata(&output).unwrap_err().to_string().contains(reason));
        }
        Ok(())
    }

    #[tokio::test]
    async fn blocked_phase_updates_respect_cancellation_and_deadlines() {
        let cancellation = Cancellation::default();
        cancellation.cancel();
        tokio::time::timeout(
            Duration::from_secs(1),
            report_progress(
                std::future::pending(),
                Duration::from_secs(60),
                &cancellation,
            ),
        )
        .await
        .unwrap();
        tokio::time::timeout(
            Duration::from_secs(1),
            report_progress(
                std::future::pending(),
                Duration::from_millis(1),
                &Cancellation::default(),
            ),
        )
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn cargo_evidence_distinguishes_build_test_and_tool_failures_before_retention()
    -> Result<()> {
        let build_error = "{\"reason\":\"compiler-message\",\"message\":{\"level\":\"error\",\"rendered\":\"type mismatch\\n\"}}\n{\"reason\":\"build-finished\",\"success\":false}\n";
        let failed_test =
            "{\"reason\":\"build-finished\",\"success\":true}\n\ntest result: FAILED. 1 failed\n";
        for (stdout, code, testing, expected) in [
            (build_error, 101, false, Some(Outcome::BuildFailed)),
            (build_error, 1, false, None),
            (failed_test, 101, true, Some(Outcome::Killed)),
            (failed_test, 101, false, None),
            ("test result: FAILED. 1 failed\n", 101, true, None),
            (
                "{\"reason\":\"build-finished\",\"success\":false}\n",
                101,
                false,
                None,
            ),
            ("cargo: registry or linker tool failure\n", 101, true, None),
        ] {
            let raw = process::CommandOutput {
                status: std::process::ExitStatus::from_raw(code << 8),
                timed_out: false,
                interrupted: false,
                capture_error: None,
                elapsed: Duration::from_millis(10),
                stdout: Capture::from_bytes(stdout.as_bytes()),
                stderr: Capture::from_bytes(b""),
            };
            let evidence = Evidence::read(&raw)?;
            let mut output = prepare(
                raw,
                NonZeroUsize::new(1),
                evidence.failure(Some(code), testing),
            )
            .await?;
            assert_eq!(classify(&output).ok(), expected, "{stdout}");
            output.raw.capture_error = Some("exceeded --max-capture-bytes".into());
            assert!(
                classify(&output)
                    .unwrap_err()
                    .to_string()
                    .contains("--max-capture-bytes"),
                "capture failure must override compiler/test failure evidence"
            );
        }
        Ok(())
    }
}

#[cfg(all(test, unix))]
#[path = "../tests/support/rust_adapter_probe.rs"]
mod adapter_probe;
