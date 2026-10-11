use std::{
    collections::HashSet,
    future::Future,
    path::{Path, PathBuf},
    time::Duration,
};

use anyhow::{Context, Result, bail, ensure};
use serde::Deserialize;

use crate::{
    cancellation::Cancellation,
    process::{self, CommandOutput},
    report::Outcome,
};

pub mod selection;

// Disable result caching so each mutation really executes tests. The outer
// deadline covers compilation too, and replaces Go's per-binary timeout.
pub const TEST_ARGS: &[&str] = &["test", "-json", "-count=1", "-timeout=0", "./..."];

pub fn list_args(scope: crate::cli::TestScope) -> &'static [&'static str] {
    match scope {
        crate::cli::TestScope::Full => &["list", "-json", "./..."],
        crate::cli::TestScope::Affected => &["list", "-deps", "-test", "-json", "./..."],
    }
}

pub async fn test(
    root: &Path,
    limit: Duration,
    cancellation: &Cancellation,
) -> Result<CommandOutput> {
    process::run("go", TEST_ARGS, root, limit, cancellation).await
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TestPhase {
    #[default]
    Running,
    Building,
    Testing,
}

pub async fn test_observed<F, Fut>(
    root: &Path,
    packages: &[String],
    limit: Duration,
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
    process::run_observed("go", &args, root, limit, cancellation, move |line| {
        let update = phases.observe(line).map(&mut progress);
        async move {
            if let Some(update) = update {
                update.await;
            }
        }
    })
    .await
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
    ensure!(!output.timed_out, "Go package discovery timed out");
    ensure!(
        output.status.success(),
        "Go package discovery failed:\n{}",
        output.stderr
    );
    serde_json::Deserializer::from_str(&output.stdout)
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

pub fn classify(output: &CommandOutput) -> Result<Outcome> {
    ensure!(
        !output.interrupted,
        "cannot classify an interrupted command"
    );
    if output.timed_out {
        return Ok(Outcome::Timeout);
    }
    if output.status.success() {
        return Ok(Outcome::Missed);
    }
    let events: Vec<Event> = output
        .stdout
        .lines()
        .filter_map(|line| serde_json::from_str(line).ok())
        .collect();
    if events.iter().any(|event| {
        event.action == "build-fail"
            || !event.failed_build.is_empty()
            || event.output.trim_end().ends_with("[build failed]")
    }) {
        return Ok(Outcome::BuildFailed);
    }
    // Setup/tool failures must not inflate the number of killed mutations.
    if !events
        .iter()
        .any(|event| event.output.trim_end().ends_with("[setup failed]"))
        && events
            .iter()
            .any(|event| event.action == "fail" && !event.package.is_empty())
    {
        return Ok(Outcome::Killed);
    }
    bail!(
        "Go command failed without a test or build failure ({}):\n{}",
        output.status,
        text(output)
    );
}

pub fn text(output: &CommandOutput) -> String {
    let mut text = String::new();
    for line in output.stdout.lines() {
        match serde_json::from_str::<Event>(line) {
            Ok(event) => text.push_str(&event.output),
            Err(_) => {
                text.push_str(line);
                text.push('\n');
            }
        }
    }
    if !output.stderr.is_empty() {
        if !text.is_empty() && !text.ends_with('\n') {
            text.push('\n');
        }
        text.push_str(&output.stderr);
    }
    text
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::process::ExitStatusExt;

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
        CommandOutput {
            status: std::process::ExitStatus::from_raw(code << 8),
            timed_out: false,
            interrupted: false,
            elapsed: Duration::ZERO,
            stdout: stdout.into(),
            stderr: String::new(),
        }
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
        timed_out.timed_out = true;
        assert_eq!(classify(&timed_out)?, Outcome::Timeout);
        let mut interrupted = output("", 0);
        interrupted.interrupted = true;
        assert!(classify(&interrupted).is_err());
        Ok(())
    }
}
