//! Narrow Python admission. Project code is never imported during discovery.
mod identity;
#[cfg(test)]
mod profile;
pub(crate) mod project;
mod protocol;
pub(crate) use identity::ExecutionScope;
#[cfg(all(test, unix))]
mod tests;

use std::{
    io::{Read, Write},
    num::NonZeroUsize,
    path::Path,
    time::Duration,
};

use anyhow::{Context, Result, ensure};
use serde_json::Value;

use crate::{cancellation::Cancellation, process};

const RUNTIME: &str = include_str!("python/runtime.py");
const SCOPE: &str = include_str!("python/scope.py");
const DRIVER: &str = include_str!("python/driver.py");

fn read_evidence(
    output: &process::CommandOutput,
    path: &Path,
    nonce: &str,
    root: &str,
    collect_only: bool,
    conftests: &[String],
) -> Result<(Vec<String>, crate::report::Outcome)> {
    ensure!(
        !output.interrupted && !output.timed_out && output.capture_error.is_none(),
        "Python process did not complete cleanly"
    );
    ensure!(
        !output.group_remaining,
        "Python descendants remained after interpreter exit"
    );
    ensure!(
        output.stderr.text()?.is_empty(),
        "Python emitted uncaptured stderr"
    );
    ensure!(
        std::fs::symlink_metadata(path)?.file_type().is_file(),
        "Python evidence must be a regular file"
    );
    let mut bytes = Vec::new();
    std::fs::File::open(path)?
        .take(4 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .context("reading bounded Python evidence")?;
    protocol::classify(
        &bytes,
        output.pid,
        nonce,
        root,
        collect_only,
        output
            .status
            .code()
            .context("Python terminated without an exit code")?,
        conftests,
    )
}

async fn helper(
    executable: &Path,
    script: &str,
    arguments: &[&str],
    cancellation: &Cancellation,
) -> Result<Value> {
    let owned = tempfile::tempdir().context("creating Python validation workspace")?;
    let script_path = owned.path().join("validate.py");
    std::fs::write(&script_path, script)?;
    let cache = format!("pycache_prefix={}", owned.path().join("cache").display());
    let mut args = vec![
        "-I",
        "-S",
        "-B",
        "-X",
        &cache,
        script_path
            .to_str()
            .context("Python helper path is not UTF-8")?,
    ];
    args.extend_from_slice(arguments);
    // Keep nested capture futures off Tokio's ordinary worker stack.
    let output = Box::pin(process::run(
        executable
            .to_str()
            .context("Python launcher path is not UTF-8")?,
        &args,
        owned.path(),
        Duration::from_secs(60),
        NonZeroUsize::new(4 * 1024 * 1024),
        cancellation,
    ))
    .await?;
    cancellation.check()?;
    ensure!(
        !output.timed_out && !output.interrupted && output.capture_error.is_none(),
        "Python validation did not complete: timeout={}, interrupted={}, capture={:?}",
        output.timed_out,
        output.interrupted,
        output.capture_error
    );
    ensure!(
        !output.group_remaining,
        "Python validation left descendants running"
    );
    let stderr = output.stderr.text()?;
    ensure!(
        output.status.success() && stderr.is_empty(),
        "Python validation failed: {stderr}"
    );
    serde_json::from_reader(output.stdout.reader()?).context("reading Python validation evidence")
}

pub async fn list(
    root: &Path,
    executable: &Path,
    json: bool,
    cancellation: &Cancellation,
) -> Result<()> {
    // Resolve relative launchers before changing cwd, but preserve venv symlinks.
    let executable = std::path::absolute(executable)?;
    let root = root.canonicalize().context("resolving Python project")?;
    let identity = helper(&executable, RUNTIME, &[], cancellation).await?;
    let project = project::Project::load(&root, &executable, cancellation).await?;
    let scope = project.value()?;
    let found = project.discover(&crate::mutation::Scope::new(&[], &[])?)?;
    ensure!(
        identity == helper(&executable, RUNTIME, &[], cancellation).await?,
        "Python runtime changed during discovery"
    );
    ensure!(
        scope
            == helper(
                &executable,
                SCOPE,
                &[root.to_str().context("Python root is not UTF-8")?],
                cancellation
            )
            .await?,
        "Python project changed during discovery"
    );
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "kind": "python-discovery",
                "identity": identity,
                "scope": scope,
            }))?
        );
    } else {
        for (index, mutation) in found.iter().enumerate() {
            println!("{}: boolean_flip {}", index + 1, mutation.description());
        }
        println!("{} Python boolean mutations", found.len());
    }
    Ok(())
}

pub async fn check(
    root: &Path,
    executable: &Path,
    timeout: Duration,
    capture_limit: NonZeroUsize,
    json: bool,
    cancellation: &Cancellation,
) -> Result<()> {
    let result = execute(root, executable, timeout, capture_limit, cancellation).await?;
    let outcome = result.command.classify().with_context(|| {
        format!(
            "Python baseline failed:\n{}",
            result.command.text.text().unwrap_or_default()
        )
    })?;
    ensure!(
        outcome == crate::report::Outcome::Missed,
        "Python baseline did not pass ({outcome:?}):\n{}",
        result.command.text.text()?
    );
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "kind":"python-baseline-check", "identity":result.identity,
                "tests":result.nodes, "seconds":result.command.elapsed.as_secs_f64(),
                "scope":result.project,
            }))?
        );
    } else {
        println!(
            "Python baseline passed: {} tests ({:.2}s)",
            result.nodes.len(),
            result.command.elapsed.as_secs_f64()
        );
    }
    Ok(())
}

struct Execution {
    #[cfg(test)]
    timings: Vec<(&'static str, f64)>,
    command: crate::command::CommandOutput,
    identity: ExecutionScope,
    project: Value,
    nodes: Vec<String>,
}

async fn execute(
    root: &Path,
    executable: &Path,
    timeout: Duration,
    capture_limit: NonZeroUsize,
    cancellation: &Cancellation,
) -> Result<Execution> {
    Box::pin(execute_mode(
        root,
        executable,
        timeout,
        capture_limit,
        cancellation,
        false,
        None,
    ))
    .await
}

pub(crate) async fn execution_scope(
    root: &Path,
    executable: &Path,
    timeout: Duration,
    capture_limit: NonZeroUsize,
    cancellation: &Cancellation,
) -> Result<(ExecutionScope, crate::command::CommandOutput)> {
    let result = Box::pin(execute_mode(
        root,
        executable,
        timeout,
        capture_limit,
        cancellation,
        true,
        None,
    ))
    .await?;
    Ok((result.identity, result.command))
}

pub(crate) async fn discovery_scope(
    project: &project::Project,
    executable: &Path,
    timeout: Duration,
    capture_limit: NonZeroUsize,
    cancellation: &Cancellation,
) -> Result<ExecutionScope> {
    let executable = std::path::absolute(executable)?;
    ExecutionScope::capture(
        helper(&executable, RUNTIME, &[], cancellation).await?,
        &project.value()?,
        timeout,
        capture_limit,
    )
}

pub(crate) async fn test(
    root: &Path,
    timeout: Duration,
    log_limit: Option<NonZeroUsize>,
    capture_limit: Option<NonZeroUsize>,
    expected: &ExecutionScope,
    cancellation: &Cancellation,
) -> Result<crate::command::CommandOutput> {
    expected.validate_ready()?;
    let mut result = Box::pin(execute_mode(
        root,
        expected.executable(),
        timeout,
        capture_limit.context("Python requires bounded captures")?,
        cancellation,
        false,
        Some(expected),
    ))
    .await?;
    result.command.raw.stdout.set_limit(log_limit);
    result.command.raw.stderr.set_limit(log_limit);
    result.command.text.set_limit(log_limit);
    Ok(result.command)
}

async fn execute_mode(
    root: &Path,
    executable: &Path,
    timeout: Duration,
    capture_limit: NonZeroUsize,
    cancellation: &Cancellation,
    collection_only: bool,
    expected: Option<&ExecutionScope>,
) -> Result<Execution> {
    #[cfg(test)]
    let mut timings = profile::PhaseTimings::new();
    let executable = std::path::absolute(executable)?;
    let root = root.canonicalize().context("resolving Python project")?;
    let identity = helper(&executable, RUNTIME, &[], cancellation).await?;
    #[cfg(test)]
    timings.finish("runtime_before");
    let root_text = root.to_str().context("Python root is not UTF-8")?;
    let original = helper(&executable, SCOPE, &[root_text], cancellation).await?;
    #[cfg(test)]
    timings.finish("scope_original");
    let mut execution_identity =
        identity::ExecutionScope::capture(identity.clone(), &original, timeout, capture_limit)?;
    if let Some(expected) = expected {
        expected.validate_rerun(&execution_identity)?;
    }
    let source = root.clone();
    #[cfg(test)]
    timings.finish("execution_identity");
    let cancel = cancellation.clone();
    let workspace = tokio::task::spawn_blocking(move || {
        crate::workspace::Workspace::create(&source, &[], &cancel)
    })
    .await??;
    #[cfg(test)]
    timings.finish("workspace_copy");
    let copied = workspace.path().canonicalize()?;
    let copied_text = copied.to_str().context("Python workspace is not UTF-8")?;
    ensure!(
        original == helper(&executable, SCOPE, &[copied_text], cancellation).await?,
        "Python inputs changed while creating the workspace"
    );
    #[cfg(test)]
    timings.finish("scope_copy");
    let owned = tempfile::tempdir().context("creating Python command workspace")?;
    let directory = owned.path().canonicalize()?;
    let driver = directory.join("driver.py");
    let config = directory.join("pytest.ini");
    let home = directory.join("home");
    let tmp = directory.join("tmp");
    std::fs::write(&driver, DRIVER)?;
    std::fs::write(&config, "[pytest]\n")?;
    std::fs::create_dir(&home)?;
    std::fs::create_dir(&tmp)?;
    #[cfg(test)]
    timings.finish("private_environment");
    let mut inventory = None;
    let mut elapsed = Duration::ZERO;
    for collect_only in [true, false] {
        let evidence = directory.join(if collect_only {
            "collect.jsonl"
        } else {
            "test.jsonl"
        });
        let nonce = evidence
            .to_str()
            .context("Python evidence path is not UTF-8")?;
        // Separate fresh prefixes prevent preexisting bytecode from masking source.
        let cache = format!(
            "pycache_prefix={}",
            directory
                .join(if collect_only {
                    "collect-cache"
                } else {
                    "test-cache"
                })
                .display()
        );
        let mut args = vec![
            "-I",
            "-B",
            "-X",
            &cache,
            driver.to_str().context("Python driver path is not UTF-8")?,
            copied_text,
            nonce,
            config.to_str().context("Python config path is not UTF-8")?,
            nonce,
            original["source_root"]
                .as_str()
                .context("missing Python source root")?,
            original["package"]
                .as_str()
                .context("missing Python package")?,
        ];
        for file in original["tests"]
            .as_array()
            .context("Python scope omitted test files")?
        {
            args.push(file.as_str().context("invalid Python test file")?);
        }
        if collect_only {
            args.push("--collect-only");
        }
        let environment = [
            ("HOME", home.as_os_str()),
            ("TMPDIR", tmp.as_os_str()),
            ("TMP", tmp.as_os_str()),
            ("TEMP", tmp.as_os_str()),
            ("PYTEST_DISABLE_PLUGIN_AUTOLOAD", std::ffi::OsStr::new("1")),
            ("PYTEST_ADDOPTS", std::ffi::OsStr::new("")),
            ("PYTEST_PLUGINS", std::ffi::OsStr::new("")),
        ];
        let mut output = Box::pin(process::run_with_file_limit(
            executable
                .to_str()
                .context("Python executable is not UTF-8")?,
            &args,
            &directory,
            timeout,
            capture_limit,
            cancellation,
            &environment,
            4 * 1024 * 1024,
        ))
        .await?;
        elapsed += output.elapsed;
        #[cfg(test)]
        timings.finish("pytest_process");
        let validated = async {
            let (nodes, outcome) = read_evidence(
                &output,
                &evidence,
                nonce,
                copied_text,
                collect_only,
                &serde_json::from_value::<Vec<String>>(original["conftests"].clone())?,
            )?;
            if let Some(expected) = expected {
                expected.validate_inventory(&nodes)?;
            }
            let collected_files: std::collections::BTreeSet<_> = nodes
                .iter()
                .filter_map(|node| node.split_once("::").map(|(file, _)| file))
                .collect();
            let expected_files: std::collections::BTreeSet<_> = original["tests"]
                .as_array()
                .context("Python scope omitted test files")?
                .iter()
                .map(|file| file.as_str().context("invalid Python test file"))
                .collect::<Result<_>>()?;
            ensure!(
                collected_files == expected_files,
                "Python native collection does not cover the audited test files"
            );
            if let Some(expected) = &inventory {
                ensure!(
                    expected == &nodes,
                    "Python collection changed between discovery and execution"
                );
            }
            ensure!(
                std::fs::read(&driver)? == DRIVER.as_bytes()
                    && std::fs::read(&config)? == b"[pytest]\n",
                "Python observer/config changed during execution"
            );
            ensure!(
                original == helper(&executable, SCOPE, &[copied_text], cancellation).await?,
                "Python workspace inputs changed during execution"
            );
            #[cfg(test)]
            timings.finish("evidence_and_scope_copy");
            let current_identity = ExecutionScope::capture(
                helper(&executable, RUNTIME, &[], cancellation).await?,
                &original,
                timeout,
                capture_limit,
            )?;
            execution_identity.validate_rerun(&current_identity)?;
            #[cfg(test)]
            timings.finish("runtime_after");
            ensure!(
                original == helper(&executable, SCOPE, &[root_text], cancellation).await?,
                "Python original inputs changed during execution"
            );
            #[cfg(test)]
            timings.finish("scope_original_after");
            Ok::<_, anyhow::Error>((nodes, outcome))
        }
        .await;
        if collect_only
            && !collection_only
            && let Ok((nodes, crate::report::Outcome::Missed)) = &validated
        {
            inventory = Some(nodes.clone());
            continue;
        }
        let nodes = validated
            .as_ref()
            .map(|(nodes, _)| nodes.clone())
            .unwrap_or_default();
        if !nodes.is_empty() {
            execution_identity.bind(nodes.clone())?;
        }
        let evidence = validated
            .map(|(_, outcome)| outcome)
            .map_err(|e| format!("{e:#}"));
        let mut rendered = tempfile::NamedTempFile::new()?;
        crate::command::write_lossy(&mut output.stdout.reader()?, &mut rendered)?;
        writeln!(rendered)?;
        crate::command::write_lossy(&mut output.stderr.reader()?, &mut rendered)?;
        let text = crate::capture::Capture::utf8(rendered)?;
        output.elapsed = elapsed;
        #[cfg(test)]
        timings.finish("diagnostics");
        return Ok(Execution {
            #[cfg(test)]
            timings: timings.values,
            command: crate::command::CommandOutput {
                raw: output,
                text,
                evidence: crate::command::OutcomeEvidence::Validated(evidence),
                tool: "Python",
            },
            identity: execution_identity,
            project: original,
            nodes,
        });
    }
    anyhow::bail!("Python execution omitted its test command")
}
