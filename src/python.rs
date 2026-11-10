//! Narrow Python admission. Project code is never imported during discovery.
mod identity;
mod installation;
mod number;
#[cfg(test)]
mod profile;
pub(crate) mod project;
mod protocol;
mod string;
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

const EVIDENCE_BYTES: u64 = 16 * 1024 * 1024;

const RUNTIME: &str = include_str!("python/runtime.py");
const SCOPE: &str = concat!(
    include_str!("python/installed_scope.py"),
    "\n",
    include_str!("python/scope.py")
);
const DRIVER: &str = include_str!("python/driver.py");
const INSTALL: &str = include_str!("python/install.py");

fn read_evidence(
    output: &process::CommandOutput,
    path: &Path,
    nonce: &str,
    root: &str,
    collect_only: bool,
    conftests: &[String],
    installation: Option<&installation::Receipt>,
) -> Result<(
    Vec<String>,
    protocol::CandidateOutcome,
    Vec<String>,
    protocol::Selection,
)> {
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
        .take(EVIDENCE_BYTES + 1)
        .read_to_end(&mut bytes)
        .context("reading bounded Python evidence")?;
    protocol::inspect_selection(
        &bytes,
        output.pid,
        nonce,
        root,
        collect_only,
        output
            .status
            .code()
            .context("Python terminated without an exit code")?,
        protocol::Inputs {
            conftests,
            installation,
        },
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
    include: &[String],
    exclude: &[String],
    operators: &[crate::mutation::Operator],
    json: bool,
    cancellation: &Cancellation,
) -> Result<()> {
    // Resolve relative launchers before changing cwd, but preserve venv symlinks.
    let executable = std::path::absolute(executable)?;
    let root = root.canonicalize().context("resolving Python project")?;
    let identity = helper(&executable, RUNTIME, &[], cancellation).await?;
    let project = project::Project::load(&root, &executable, cancellation).await?;
    let scope = project.value()?;
    let mut found = project.discover(&crate::mutation::Scope::new(include, exclude)?)?;
    found.retain(|m| operators.is_empty() || operators.contains(&m.operator));
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
                "mutations":found.iter().enumerate().map(|(i,m)| crate::mutation::preview_json(i+1,m)).collect::<Vec<_>>(),
            }))?
        );
    } else {
        for (index, mutation) in found.iter().enumerate() {
            println!(
                "{}: {} {}",
                index + 1,
                mutation.operator,
                mutation.description()
            );
        }
        println!("{} Python mutations", found.len());
    }
    Ok(())
}

struct Execution {
    #[cfg(test)]
    timings: Vec<(&'static str, f64)>,
    command: crate::command::CommandOutput,
    identity: ExecutionScope,
    #[cfg(test)]
    project: Value,
    #[cfg(test)]
    nodes: Vec<String>,
}

#[cfg(test)]
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
    // Explicit xfail calls require a pristine executed baseline so a candidate
    // cannot introduce or remove a skipped body while retaining a passing score.
    let collection_only = collection_only
        && !original["expected_failure_calls"]
            .as_bool()
            .unwrap_or(false);
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
        // Match scope.py: review metadata is not a project input. Omit it from
        // execution too, so tests cannot observe metadata ignored by identity.
        let excluded = [
            source.join("mutie.dismissals.toml"),
            source.join(".mutie.dismissals.lock"),
        ];
        crate::workspace::Workspace::create(&source, &excluded, &cancel)
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
    let installed = original.get("installation").is_some();
    let config = if installed {
        copied.join("pyproject.toml")
    } else {
        directory.join("pytest.ini")
    };
    let home = directory.join("home");
    let tmp = directory.join("tmp");
    std::fs::write(&driver, DRIVER)?;
    if !installed {
        std::fs::write(&config, "[pytest]\n")?;
    }
    let config_bytes = std::fs::read(&config)?;
    std::fs::create_dir(&home)?;
    std::fs::create_dir(&tmp)?;
    let installation = directory.join("installation");
    let receipt_path = installation.join("receipt.json");
    let scope_path = directory.join("scope.json");
    let mut command_executable = executable.clone();
    let mut installed_receipt: Option<installation::Receipt> = None;
    if installed {
        let receipt = helper(
            &executable,
            INSTALL,
            &[
                copied_text,
                installation
                    .to_str()
                    .context("Python installation path is not UTF-8")?,
                original["package"].as_str().context("missing package")?,
                original["source_root"]
                    .as_str()
                    .context("missing source root")?,
            ],
            cancellation,
        )
        .await?;
        command_executable = receipt["executable"]
            .as_str()
            .context("missing installed interpreter")?
            .into();
        installed_receipt = Some(serde_json::from_value(receipt)?);
        std::fs::write(&scope_path, serde_json::to_vec(&original)?)?;
    }
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
            if !installed {
                args.push(file.as_str().context("invalid Python test file")?);
            }
        }
        if installed {
            args.push("tests");
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
            (
                "MUTIE_PYTHON_INSTALLATION",
                if installed {
                    receipt_path.as_os_str()
                } else {
                    std::ffi::OsStr::new("")
                },
            ),
            (
                "MUTIE_PYTHON_SCOPE",
                if installed {
                    scope_path.as_os_str()
                } else {
                    std::ffi::OsStr::new("")
                },
            ),
        ];
        if let Some(receipt) = &installed_receipt {
            receipt.validate(&installation, &executable)?;
        }
        let mut output = Box::pin(process::run_with_file_limit(
            command_executable
                .to_str()
                .context("Python executable is not UTF-8")?,
            &args,
            &directory,
            timeout,
            capture_limit,
            cancellation,
            &environment,
            EVIDENCE_BYTES,
        ))
        .await?;
        elapsed += output.elapsed;
        #[cfg(test)]
        timings.finish("pytest_process");
        let validated = async {
            if let Some(receipt) = &installed_receipt {
                receipt.validate(&installation, &executable)?;
            }
            let (nodes, outcome, expected_failures, selection) = read_evidence(
                &output,
                &evidence,
                nonce,
                copied_text,
                collect_only,
                &serde_json::from_value::<Vec<String>>(original["conftests"].clone())?,
                installed_receipt.as_ref(),
            )?;
            if let Some(expected) = expected {
                expected.validate_inventory(&nodes)?;
                expected.validate_selection(&selection, collect_only)?;
                if !collect_only {
                    expected.validate_expected_failures(&expected_failures)?;
                }
            }
            let mut collected_files: std::collections::BTreeSet<_> = nodes
                .iter()
                .filter_map(|node| node.split_once("::").map(|(file, _)| file))
                .collect();
            if let Some(excluded) = &selection.deselected {
                collected_files.extend(excluded.files.iter().map(String::as_str));
            }
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
                    expected == &(nodes.clone(), selection.deselected.clone()),
                    "Python collection changed between discovery and execution"
                );
            }
            ensure!(
                std::fs::read(&driver)? == DRIVER.as_bytes()
                    && std::fs::read(&config)? == config_bytes,
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
            Ok::<_, anyhow::Error>((nodes, outcome, expected_failures, selection))
        }
        .await;
        if collect_only
            && !collection_only
            && let Ok((
                nodes,
                protocol::CandidateOutcome::Scored(crate::report::Outcome::Missed),
                _,
                selection,
            )) = &validated
        {
            inventory = Some((nodes.clone(), selection.deselected.clone()));
            continue;
        }
        let nodes = validated
            .as_ref()
            .map(|(nodes, _, _, _)| nodes.clone())
            .unwrap_or_default();
        if !nodes.is_empty() {
            if let Ok((_, _, _, selection)) = &validated {
                execution_identity.bind_selection(selection.clone());
            }
            execution_identity.bind(nodes.clone())?;
            if let Ok((_, _, expected_failures, _)) = &validated {
                execution_identity.bind_expected_failures(expected_failures.clone())?;
            }
        }
        let evidence = match validated {
            Ok((_, protocol::CandidateOutcome::BodyError, _, _)) => {
                crate::command::OutcomeEvidence::IsolatedCandidateError(
                    "Python run contains an unsupported phase failure".into(),
                )
            }
            Ok((_, protocol::CandidateOutcome::Scored(outcome), _, _)) => {
                crate::command::OutcomeEvidence::Validated(Ok(outcome))
            }
            Err(error) => crate::command::OutcomeEvidence::Validated(Err(format!("{error:#}"))),
        };
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
                evidence,
                tool: "Python",
            },
            identity: execution_identity,
            #[cfg(test)]
            project: original,
            #[cfg(test)]
            nodes,
        });
    }
    anyhow::bail!("Python execution omitted its test command")
}
