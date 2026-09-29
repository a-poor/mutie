use std::{
    path::{Path, PathBuf},
    time::Duration,
};

use anyhow::{Context, Result, bail, ensure};
use serde::Deserialize;

use crate::{
    process::{self, CommandOutput},
    report::Outcome,
};

// Disable result caching so each mutation really executes tests. The outer
// deadline covers compilation too, and replaces Go's per-binary timeout.
pub async fn test(root: &Path, limit: Duration) -> Result<CommandOutput> {
    process::run(
        "go",
        &["test", "-json", "-count=1", "-timeout=0", "./..."],
        root,
        limit,
    )
    .await
}

pub async fn files(root: &Path, limit: Duration) -> Result<Vec<PathBuf>> {
    let output = process::run("go", &["list", "-json", "./..."], root, limit).await?;
    ensure!(!output.timed_out, "Go package discovery timed out");
    ensure!(
        output.status.success(),
        "Go package discovery failed:\n{}",
        output.stderr
    );
    #[derive(Deserialize)]
    #[serde(rename_all = "PascalCase")]
    struct Package {
        dir: PathBuf,
        #[serde(default)]
        go_files: Vec<String>,
        #[serde(default)]
        cgo_files: Vec<String>,
    }
    let mut files = Vec::new();
    for package in serde_json::Deserializer::from_str(&output.stdout).into_iter::<Package>() {
        let package = package.context("decoding go list output")?;
        let relative = package
            .dir
            .strip_prefix(root)
            .context("go list returned a package outside the module")?;
        files.extend(
            package
                .go_files
                .into_iter()
                .chain(package.cgo_files)
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
    failed_build: String,
    #[serde(default)]
    output: String,
}

pub fn classify(output: &CommandOutput) -> Result<Outcome> {
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

    fn output(stdout: &str, code: i32) -> CommandOutput {
        CommandOutput {
            status: std::process::ExitStatus::from_raw(code << 8),
            timed_out: false,
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
        Ok(())
    }
}
