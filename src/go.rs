use std::{
    collections::HashSet,
    future::Future,
    io::{BufRead, BufReader, Write},
    num::NonZeroUsize,
    path::{Path, PathBuf},
    time::Duration,
};

use anyhow::{Context, Result, bail, ensure};
use serde::Deserialize;

use crate::{cancellation::Cancellation, capture::Capture, process};

pub mod selection;

use crate::command::Failure;
pub use crate::command::{CommandOutput, TestPhase};
#[cfg(test)]
use crate::{
    command::{classify, text},
    report::Outcome,
};

#[derive(Default)]
struct Evidence {
    build_failed: bool,
    setup_failed: bool,
    test_failed: bool,
}

impl Evidence {
    fn observe(&mut self, event: &Event) {
        self.build_failed |= event.action == "build-fail"
            || !event.failed_build.is_empty()
            || event.output.trim_end().ends_with("[build failed]");
        self.setup_failed |= event.output.trim_end().ends_with("[setup failed]");
        self.test_failed |= event.action == "fail" && !event.package.is_empty();
    }
}

impl CommandOutput {
    /// Parse every event before applying retention. Only one JSON record is in
    /// memory at a time; the complete raw and rendered streams stay file-backed.
    pub(crate) fn prepare(
        mut raw: process::CommandOutput,
        log_limit: Option<NonZeroUsize>,
    ) -> Result<Self> {
        let mut rendered = tempfile::NamedTempFile::new()?;
        let mut reader = BufReader::new(raw.stdout.reader()?);
        let mut line = Vec::new();
        let mut evidence = Evidence::default();
        let mut last = None;
        while reader.read_until(b'\n', &mut line)? != 0 {
            let decoded = String::from_utf8_lossy(&line);
            let decoded = match decoded.strip_suffix('\n') {
                Some(line) => line.strip_suffix('\r').unwrap_or(line),
                None => &decoded,
            };
            match serde_json::from_str::<Event>(decoded) {
                Ok(event) => {
                    evidence.observe(&event);
                    rendered.write_all(event.output.as_bytes())?;
                    last = event.output.as_bytes().last().copied().or(last);
                }
                Err(_) => {
                    writeln!(rendered, "{decoded}")?;
                    last = Some(b'\n');
                }
            }
            line.clear();
        }
        if raw.stderr.retention()?.total_bytes > 0 {
            if last.is_some_and(|c| c != b'\n') {
                writeln!(rendered)?;
            }
            crate::command::write_lossy(&mut raw.stderr.reader()?, &mut rendered)?;
        }
        if let Some(error) = &raw.capture_error {
            writeln!(rendered, "\n[Output capture error: {error}]")?;
        }
        raw.stdout.set_limit(log_limit);
        raw.stderr.set_limit(log_limit);
        let mut text = Capture::utf8(rendered)?;
        text.set_limit(log_limit);
        Ok(Self {
            raw,
            text,
            failure: if evidence.build_failed {
                Failure::Build
            } else if !evidence.setup_failed && evidence.test_failed {
                Failure::Test
            } else {
                Failure::Unknown
            },
            tool: "Go",
        })
    }
}

async fn prepare(
    raw: process::CommandOutput,
    log_limit: Option<NonZeroUsize>,
) -> Result<CommandOutput> {
    tokio::task::spawn_blocking(move || CommandOutput::prepare(raw, log_limit))
        .await
        .context("processing Go command output panicked")?
}

pub async fn discover(
    root: &Path,
    scope: crate::cli::TestScope,
    limit: Duration,
    log_limit: Option<NonZeroUsize>,
    capture_limit: Option<NonZeroUsize>,
    cancellation: &Cancellation,
) -> Result<CommandOutput> {
    prepare(
        process::run(
            "go",
            list_args(scope),
            root,
            limit,
            capture_limit,
            cancellation,
        )
        .await?,
        log_limit,
    )
    .await
}

// Disable result caching so each mutation really executes tests. The outer
// deadline covers compilation too, and replaces Go's per-binary timeout.
pub const TEST_ARGS: &[&str] = &["test", "-json", "-count=1", "-timeout=0", "./..."];

pub fn list_args(scope: crate::cli::TestScope) -> &'static [&'static str] {
    match scope {
        crate::cli::TestScope::Full => &["list", "-json", "./..."],
        crate::cli::TestScope::Affected => &["list", "-deps", "-test", "-json", "./..."],
    }
}

pub async fn test_observed<F, Fut>(
    root: &Path,
    packages: &[String],
    limit: Duration,
    log_limit: Option<NonZeroUsize>,
    capture_limit: Option<NonZeroUsize>,
    cancellation: &Cancellation,
    mut progress: F,
) -> Result<CommandOutput>
where
    F: FnMut(TestPhase) -> Fut + Send,
    Fut: Future<Output = ()> + Send,
{
    ensure!(!packages.is_empty(), "mutation test selection is empty");
    let args: Vec<_> = TEST_ARGS[..TEST_ARGS.len() - 1]
        .iter()
        .copied()
        .chain(packages.iter().map(String::as_str))
        .collect();
    let mut phases = TestProgress::default();
    let raw = process::run_observed(
        "go",
        &args,
        root,
        limit,
        capture_limit,
        cancellation,
        move |line| {
            let update = phases.observe(line).map(&mut progress);
            async move {
                if let Some(update) = update {
                    update.await;
                }
            }
        },
    )
    .await?;
    prepare(raw, log_limit).await
}

#[derive(Default)]
struct TestProgress {
    active_packages: HashSet<String>,
    phase: TestPhase,
}

impl TestProgress {
    fn observe(&mut self, line: &str) -> Option<TestPhase> {
        let event: Event = serde_json::from_str(line).ok()?;
        // Package streams interleave. Keep testing while any observed package
        // is active; a different package's build output must not hide that.
        let phase = match event.action.as_str() {
            "start" | "run" | "cont" if !event.package.is_empty() => {
                self.active_packages.insert(event.package);
                TestPhase::Testing
            }
            "pass" | "fail" | "skip" if !event.package.is_empty() && event.test.is_empty() => {
                self.active_packages.remove(&event.package);
                if self.active_packages.is_empty() {
                    TestPhase::Running
                } else {
                    TestPhase::Testing
                }
            }
            "build-output" if !event.import_path.is_empty() && self.active_packages.is_empty() => {
                TestPhase::Building
            }
            _ => return None,
        };
        if self.phase == phase {
            return None;
        }
        self.phase = phase;
        Some(phase)
    }
}

pub fn files(root: &Path, output: &CommandOutput) -> Result<Vec<PathBuf>> {
    let packages = packages(output)?;
    files_in(root, &packages)
}

pub fn packages(output: &CommandOutput) -> Result<Vec<selection::Package>> {
    ensure!(!output.interrupted, "Go package discovery interrupted");
    if let Some(error) = &output.capture_error {
        bail!("Go package discovery: {error}");
    }
    ensure!(!output.timed_out, "Go package discovery timed out");
    ensure!(
        output.status.success(),
        "Go package discovery failed:\n{}",
        output.stderr.text()?
    );
    serde_json::Deserializer::from_reader(BufReader::new(output.stdout.reader()?))
        .into_iter::<selection::Package>()
        .collect::<std::result::Result<Vec<_>, _>>()
        .context("decoding go list output")
}

pub fn files_in(root: &Path, packages: &[selection::Package]) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    for package in packages {
        let relative = package
            .dir
            .strip_prefix(root)
            .context("go list returned a package outside the module")?;
        files.extend(
            package
                .go_files
                .iter()
                .chain(&package.cgo_files)
                .map(|name| relative.join(name)),
        );
    }
    Ok(files)
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "PascalCase")]
struct Event {
    #[serde(default)]
    action: String,
    #[serde(default)]
    package: String,
    #[serde(default)]
    test: String,
    #[serde(default)]
    import_path: String,
    #[serde(default)]
    failed_build: String,
    #[serde(default)]
    output: String,
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::process::ExitStatusExt;

    #[test]
    fn incremental_stderr_decoding_matches_lossy_utf8_across_chunks_and_eof() -> Result<()> {
        let mut bytes = vec![b'a'; 16 * 1024 - 1];
        bytes.extend_from_slice("界é\r\n".as_bytes());
        bytes.extend_from_slice(b"\xff\xc0\xe2\x82");
        let mut decoded = Vec::new();
        crate::command::write_lossy(&mut bytes.as_slice(), &mut decoded)?;
        assert_eq!(String::from_utf8(decoded)?, String::from_utf8_lossy(&bytes));
        Ok(())
    }

    #[test]
    fn retained_logs_never_determine_classification_even_for_oversized_events() -> Result<()> {
        let noise = "{\"Action\":\"output\",\"Output\":\"padding é界\\n\"}\n".repeat(200);
        for (event, expected) in [
            (
                "{\"Action\":\"fail\",\"Package\":\"p\"}",
                Some(Outcome::Killed),
            ),
            (
                "{\"Action\":\"build-fail\",\"ImportPath\":\"p\"}",
                Some(Outcome::BuildFailed),
            ),
            (
                "{\"Action\":\"fail\",\"Package\":\"p\",\"FailedBuild\":\"p\"}",
                Some(Outcome::BuildFailed),
            ),
            (
                "{\"Action\":\"output\",\"Output\":\"FAIL p [build failed]\\n\"}",
                Some(Outcome::BuildFailed),
            ),
            (
                "{\"Action\":\"output\",\"Output\":\"FAIL p [setup failed]\\n\"}\n{\"Action\":\"fail\",\"Package\":\"p\"}",
                None,
            ),
            ("tool error", None),
        ] {
            let raw = format!("{noise}{event}\n{noise}");
            let mut limited = CommandOutput::prepare(output(&raw, 1).raw, NonZeroUsize::new(64))?;
            assert!(!limited.stdout.text()?.contains(event));
            assert_eq!(classify(&limited).ok(), expected);
            assert_eq!(classify(&limited).ok(), classify(&output(&raw, 1)).ok());
            assert!(text(&limited)?.len() < 256);
            limited.raw.timed_out = true;
            assert_eq!(classify(&limited)?, Outcome::Timeout);
            limited.raw.interrupted = true;
            assert!(classify(&limited).is_err());
        }
        let oversized = serde_json::json!({"Action":"output", "Output":format!("{} [setup failed]\n", "x".repeat(2 * 1024 * 1024))}).to_string();
        let raw = format!("{noise}{oversized}\n{{\"Action\":\"fail\",\"Package\":\"p\"}}\n{noise}");
        let limited = CommandOutput::prepare(output(&raw, 1).raw, NonZeroUsize::new(64))?;
        assert!(classify(&limited).is_err());
        assert!(limited.stdout.retention()?.omitted_bytes > 2 * 1024 * 1024);
        Ok(())
    }

    #[test]
    fn discovery_decodes_complete_machine_output_and_text_preserves_stream_boundaries() -> Result<()>
    {
        let raw = "{\"Dir\":\"/module\",\"GoFiles\":[\"a.go\"],\"Ignored\":\"padding\"}\n{\"Dir\":\"/module/sub\",\"GoFiles\":[\"b.go\"]}";
        let limited = CommandOutput::prepare(output(raw, 0).raw, NonZeroUsize::new(1))?;
        assert_eq!(
            files(Path::new("/module"), &limited)?,
            [PathBuf::from("a.go"), PathBuf::from("sub/b.go")]
        );
        assert!(limited.stdout.retention()?.omitted_bytes > 0);
        let mut raw = output("{\"Action\":\"output\",\"Output\":\"é\"}\n", 0).raw;
        raw.stderr = Capture::from_bytes(b"diagnostic\r\n");
        let prepared = CommandOutput::prepare(raw, None)?;
        assert_eq!(text(&prepared)?, "é\ndiagnostic\r\n");
        assert_eq!(text(&output("plain\r\nlast\r", 0))?, "plain\nlast\r\n");
        Ok(())
    }

    #[test]
    fn progress_uses_observed_events_and_tracks_interleaved_packages() {
        let mut progress = TestProgress::default();
        assert_eq!(progress.observe("not JSON"), None);
        assert_eq!(
            progress.observe(r#"{"Action":"output","Output":"compiling or testing"}"#),
            None
        );
        assert_eq!(
            progress.observe(r#"{"Action":"build-output","ImportPath":"p"}"#),
            Some(TestPhase::Building)
        );
        assert_eq!(
            progress.observe(r#"{"Action":"build-output","ImportPath":"p"}"#),
            None
        );
        assert_eq!(
            progress.observe(r#"{"Action":"start","Package":"p"}"#),
            Some(TestPhase::Testing)
        );
        assert_eq!(
            progress.observe(r#"{"Action":"start","Package":"q"}"#),
            None
        );
        assert_eq!(
            progress.observe(r#"{"Action":"build-output","ImportPath":"r"}"#),
            None
        );
        assert_eq!(
            progress.observe(r#"{"Action":"pass","Package":"p","Test":"TestExample"}"#),
            None
        );
        assert_eq!(progress.observe(r#"{"Action":"pass","Package":"p"}"#), None);
        assert_eq!(
            progress.observe(r#"{"Action":"skip","Package":"q"}"#),
            Some(TestPhase::Running)
        );
        assert_eq!(progress.observe(r#"{"Action":"start"}"#), None);
        assert_eq!(progress.observe(r#"{"Action":"build-output"}"#), None);
    }

    fn output(stdout: &str, code: i32) -> CommandOutput {
        CommandOutput::prepare(
            process::CommandOutput {
                status: std::process::ExitStatus::from_raw(code << 8),
                timed_out: false,
                interrupted: false,
                capture_error: None,
                elapsed: Duration::ZERO,
                stdout: Capture::from_bytes(stdout.as_bytes()),
                stderr: Capture::from_bytes(b""),
            },
            None,
        )
        .unwrap()
    }

    #[test]
    fn distinguishes_test_build_setup_and_tool_failures() -> Result<()> {
        assert_eq!(classify(&output("", 0))?, Outcome::Missed);
        assert_eq!(
            classify(&output(
                r#"{"Action":"fail","Package":"p","Test":"TestX"}"#,
                1
            ))?,
            Outcome::Killed
        );
        assert_eq!(
            classify(&output(r#"{"Action":"build-fail","ImportPath":"p"}"#, 1))?,
            Outcome::BuildFailed
        );
        assert_eq!(
            classify(&output(
                r#"{"Action":"fail","Package":"p","FailedBuild":"p"}"#,
                1
            ))?,
            Outcome::BuildFailed
        );
        assert_eq!(
            classify(&output(
                r#"{"Action":"output","Output":"FAIL p [build failed]\n"}"#,
                1
            ))?,
            Outcome::BuildFailed
        );
        assert!(classify(&output("go: missing dependency", 1)).is_err());
        assert!(classify(&output("{\"Action\":\"output\",\"Output\":\"FAIL p [setup failed]\\n\"}\n{\"Action\":\"fail\",\"Package\":\"p\"}", 1)).is_err());
        let mut timed_out = output("", 1);
        timed_out.raw.timed_out = true;
        assert_eq!(classify(&timed_out)?, Outcome::Timeout);
        let mut interrupted = output("", 0);
        interrupted.raw.interrupted = true;
        assert!(classify(&interrupted).is_err());
        Ok(())
    }
}
