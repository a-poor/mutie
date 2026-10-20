//! Scoped Bun execution with isolated inputs and attributed outcome evidence.
mod config;
mod identity;
#[cfg(test)]
mod profile;
pub(crate) mod project;
mod protocol;

use crate::{
    cancellation::Cancellation,
    command::{CommandOutput, OutcomeEvidence},
    process,
    report::Outcome,
    workspace::Workspace,
};
use anyhow::{Context, Result, ensure};
pub(crate) use identity::ExecutionIdentity as ExecutionScope;
use std::{
    fs,
    io::{Read, Write},
    num::NonZeroUsize,
    path::Path,
    time::Duration,
};
const EVIDENCE_LIMIT: u64 = 8 * 1024 * 1024;

fn fingerprint(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    format!("sha256:{:x}", Sha256::digest(bytes))
}
fn utf8(path: &Path) -> Result<&str> {
    path.to_str().context("Bun requires UTF-8 paths")
}
fn read_evidence(path: &Path) -> Result<String> {
    ensure!(
        fs::symlink_metadata(path)?.file_type().is_file(),
        "Bun evidence must be a regular file"
    );
    let mut text = String::new();
    fs::File::open(path)?
        .take(EVIDENCE_LIMIT + 1)
        .read_to_string(&mut text)?;
    ensure!(
        text.len() as u64 <= EVIDENCE_LIMIT,
        "oversized Bun evidence"
    );
    Ok(text)
}

pub fn list(
    root: &Path,
    include: &[String],
    exclude: &[String],
    json: bool,
    cancellation: &Cancellation,
) -> Result<()> {
    let project = project::Project::load(root, cancellation)?;
    let found = crate::javascript::discover_bun(
        &project.root,
        &crate::mutation::Scope::new(include, exclude)?,
        cancellation,
    )?;
    // Validation and discovery read the same original inputs; fail if they changed.
    ensure!(
        project.files == project::Project::load(&project.root, cancellation)?.files,
        "Bun inputs changed during discovery"
    );
    if json {
        let mutations: Vec<_> = found
            .mutations
            .iter()
            .enumerate()
            .map(|(i, m)| {
                serde_json::json!({
                    "id":i+1, "operator":"boolean_flip", "file":m.file.path,
                    "source_fingerprint":m.file.fingerprint, "line":m.line, "column":m.column,
                    "byte_start":m.range.start, "byte_end":m.range.end,
                    "original":m.original(), "replacement":m.replacement,
                })
            })
            .collect();
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "kind":"bun-discovery-preview", "runtime_checked":false,
                "input_fingerprint":fingerprint(&serde_json::to_vec(&project.files)?),
                "test_files":project.tests, "mutations":mutations, "exclusions":found.exclusions,
            }))?
        );
    } else {
        println!("Bun discovery preview only; no runtime or tests executed.");
        for exclusion in found.exclusions {
            println!("Excluded {exclusion}");
        }
        for (i, mutation) in found.mutations.iter().enumerate() {
            println!("{}: boolean_flip {}", i + 1, mutation.description());
        }
        println!("{} mutations", found.mutations.len());
    }
    Ok(())
}

struct Execution {
    command: CommandOutput,
    identity: identity::ExecutionIdentity,
    input_fingerprint: String,
    #[cfg(test)]
    timings: Vec<(&'static str, f64)>,
}
impl std::ops::Deref for Execution {
    type Target = CommandOutput;
    fn deref(&self) -> &Self::Target {
        &self.command
    }
}

pub async fn check(
    root: &Path,
    timeout: Duration,
    capture_limit: NonZeroUsize,
    json: bool,
    cancellation: &Cancellation,
) -> Result<()> {
    let output = execute(root, "bun", timeout, capture_limit, cancellation).await?;
    let outcome = output.classify().with_context(|| {
        format!(
            "Bun baseline failed:\n{}",
            output.text.text().unwrap_or_default()
        )
    })?;
    ensure!(
        outcome == Outcome::Missed,
        "Bun baseline did not pass ({outcome:?}):\n{}",
        output.text.text()?
    );
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "kind":"bun-baseline-check", "outcome":"passed", "identity":output.identity,
            "input_fingerprint":output.input_fingerprint,
                "command_seconds":output.raw.elapsed.as_secs_f64(),
            }))?
        );
    } else {
        println!(
            "Bun baseline passed ({:.2}s command time); no mutations or saved report.",
            output.raw.elapsed.as_secs_f64()
        );
    }
    Ok(())
}

pub async fn execution_scope(
    root: &Path,
    timeout: Duration,
    capture_limit: NonZeroUsize,
    cancellation: &Cancellation,
) -> Result<ExecutionScope> {
    let project = project::Project::load(root, cancellation)?;
    let owned = tempfile::tempdir()?;
    let config = owned.path().join("bunfig.toml");
    fs::write(&config, "[test]\n")?;
    let output = process::run_with_env("bun", &["--no-env-file", "--no-install", &format!("--config={}",utf8(&config)?), "-e", "console.log(JSON.stringify({version:Bun.version,revision:Bun.revision,executable:process.execPath}))"], owned.path(), Duration::from_secs(10), NonZeroUsize::new(4096), cancellation, &[("HOME",owned.path().as_os_str()),("XDG_CONFIG_HOME",owned.path().as_os_str()),("BUN_OPTIONS","".as_ref()),("NODE_OPTIONS","".as_ref()),("NODE_PATH","".as_ref()),("BUN_INSPECT_CONNECT_TO","".as_ref())]).await?;
    cancellation.check()?;
    ensure!(
        output.status.success() && !output.timed_out && output.capture_error.is_none(),
        "could not identify Bun"
    );
    let value: serde_json::Value = serde_json::from_str(&output.stdout.text()?)?;
    ensure!(
        value["version"] == "1.3.11"
            && value["revision"] == "af24e281ebacd6ac77c0f14b4206599cf4ae1c9f",
        "Bun requires pinned 1.3.11+af24e281e"
    );
    let executable = Path::new(
        value["executable"]
            .as_str()
            .context("missing Bun executable")?,
    );
    let scope = ExecutionScope::capture(&project, executable, timeout, capture_limit)?;
    scope.validate_record()?;
    Ok(scope)
}

pub async fn test(
    root: &Path,
    timeout: Duration,
    log_limit: Option<NonZeroUsize>,
    capture_limit: Option<NonZeroUsize>,
    expected: &ExecutionScope,
    cancellation: &Cancellation,
) -> Result<CommandOutput> {
    expected.validate_record()?;
    let mut result = execute_expected(
        root,
        utf8(&expected.executable)?,
        timeout,
        capture_limit.context("Bun requires a raw capture limit")?,
        cancellation,
        Some(expected),
    )
    .await?;
    result.command.raw.stdout.set_limit(log_limit);
    result.command.raw.stderr.set_limit(log_limit);
    result.command.text.set_limit(log_limit);
    Ok(result.command)
}

async fn execute(
    root: &Path,
    program: &str,
    timeout: Duration,
    capture_limit: NonZeroUsize,
    cancellation: &Cancellation,
) -> Result<Execution> {
    execute_expected(root, program, timeout, capture_limit, cancellation, None).await
}

async fn execute_expected(
    root: &Path,
    program: &str,
    timeout: Duration,
    capture_limit: NonZeroUsize,
    cancellation: &Cancellation,
    expected: Option<&ExecutionScope>,
) -> Result<Execution> {
    #[cfg(test)]
    let mut timings = profile::PhaseTimings::new();
    let project = project::Project::load(root, cancellation)?;
    #[cfg(test)]
    timings.finish("original_validation");
    let source = project.root.clone();
    let copying = cancellation.clone();
    let workspace = tokio::task::spawn_blocking(move || Workspace::create(&source, &[], &copying))
        .await
        .context("Bun snapshot preparation panicked")??;
    let root = workspace.path();
    #[cfg(test)]
    timings.finish("isolated_copy");
    let captured = project::Project::load(&root, cancellation)?;
    ensure!(
        captured.files == project.files && captured.tests == project.tests,
        "Bun input changed during snapshotting"
    );
    #[cfg(test)]
    timings.finish("copied_validation");
    let owned = tempfile::Builder::new()
        .prefix("mutie-bun-command-")
        .tempdir()?;
    let owned_path = owned.path().canonicalize()?;
    ensure!(
        !owned_path.starts_with(&project.root),
        "Bun temporary root must be outside project"
    );
    let config = owned_path.join("bunfig.toml");
    fs::write(&config, "[test]\n")?;
    let home = owned_path.join("home");
    let temp = owned_path.join("tmp");
    fs::create_dir(&home)?;
    fs::create_dir(&temp)?;
    let config_arg = format!("--config={}", utf8(&config)?);
    let environment = [
        ("HOME", home.as_os_str()),
        ("XDG_CONFIG_HOME", home.as_os_str()),
        ("BUN_OPTIONS", "".as_ref()),
        ("NODE_OPTIONS", "".as_ref()),
        ("NODE_PATH", "".as_ref()),
        ("BUN_INSPECT_CONNECT_TO", "".as_ref()),
        ("BUN_RUNTIME_TRANSPILER_CACHE_PATH", "0".as_ref()),
        ("BUN_INSTALL_CACHE_DIR", temp.as_os_str()),
        ("TMPDIR", temp.as_os_str()),
        ("TMP", temp.as_os_str()),
        ("TEMP", temp.as_os_str()),
        ("NO_COLOR", "1".as_ref()),
        ("FORCE_COLOR", "0".as_ref()),
        ("CI", "true".as_ref()),
    ];
    #[cfg(test)]
    timings.finish("private_environment_setup");
    // Identify from a private empty directory before executing any project code.
    let identity = process::run_with_env(program, &["--no-env-file", "--no-install", &config_arg, "-e", "console.log(JSON.stringify({version:Bun.version,revision:Bun.revision,executable:process.execPath}))"], &owned_path, Duration::from_secs(10), NonZeroUsize::new(4096), cancellation, &environment).await?;
    cancellation.check()?;
    ensure!(
        identity.status.success()
            && !identity.timed_out
            && !identity.interrupted
            && identity.capture_error.is_none(),
        "could not identify Bun runtime"
    );
    let identity: serde_json::Value = serde_json::from_str(&identity.stdout.text()?)?;
    ensure!(
        identity["version"] == "1.3.11"
            && identity["revision"] == "af24e281ebacd6ac77c0f14b4206599cf4ae1c9f",
        "Bun requires pinned 1.3.11+af24e281e"
    );
    let executable = Path::new(
        identity["executable"]
            .as_str()
            .context("missing Bun executable")?,
    );
    ensure!(executable.is_absolute(), "Bun executable must be absolute");
    #[cfg(test)]
    timings.finish("runtime_identity_process");
    let execution_identity =
        identity::ExecutionIdentity::capture(&project, executable, timeout, capture_limit)?;
    if let Some(expected) = expected {
        expected.validate_rerun(&execution_identity)?;
        ensure!(
            expected.test_files == execution_identity.test_files,
            "Bun test inventory changed during the run"
        );
    }
    #[cfg(test)]
    timings.finish("identity_before");
    let observer = owned_path.join("observer.mjs");
    let events = owned_path.join("observer.jsonl");
    let junit = owned_path.join("junit.xml");
    fs::write(&observer, include_bytes!("bun/observer.mjs"))?;
    let mut environment = environment.to_vec();
    environment.push(("MUTIE_BUN_OBSERVER", events.as_os_str()));
    // OS file-size limits bound native JUnit and observer writes, including child
    // processes. This is a per-file bound, not a total workspace disk quota.
    #[cfg(test)]
    timings.finish("observer_setup");
    let raw = process::run_with_file_limit(
        utf8(executable)?,
        &[
            "test",
            "--max-concurrency=1",
            "--no-env-file",
            "--no-install",
            &config_arg,
            "--preload",
            utf8(&observer)?,
            "--reporter=junit",
            &format!("--reporter-outfile={}", utf8(&junit)?),
        ],
        &root,
        timeout,
        capture_limit,
        cancellation,
        &environment,
        EVIDENCE_LIMIT,
    )
    .await?;
    #[cfg(test)]
    timings.finish("test_process");
    let evidence = (|| -> Result<Outcome> {
        ensure!(
            !raw.interrupted && !raw.timed_out && raw.capture_error.is_none(),
            "Bun command did not complete normally"
        );
        ensure!(
            execution_identity
                == identity::ExecutionIdentity::capture(
                    &project,
                    executable,
                    timeout,
                    capture_limit
                )?,
            "Bun executable changed during command"
        );
        #[cfg(test)]
        timings.finish("identity_after");
        ensure!(
            fs::read(&observer)? == include_bytes!("bun/observer.mjs"),
            "Bun observer changed during command"
        );
        let mut stderr = Vec::new();
        raw.stderr.reader()?.read_to_end(&mut stderr)?;
        let tests: Vec<_> = project.tests.iter().map(String::as_str).collect();
        let completion = protocol::classify(protocol::Evidence {
            exit_code: raw.status.code(),
            outer_timeout: raw.timed_out,
            stderr: &stderr,
            junit: &read_evidence(&junit)?,
            observer: &read_evidence(&events)?,
            expected_files: &tests,
        })?;
        Ok(match completion {
            protocol::Completion::Passed => Outcome::Missed,
            protocol::Completion::BodyFailure => Outcome::Killed,
            protocol::Completion::Timeout => Outcome::Timeout,
        })
    })()
    .map_err(|e| format!("{e:#}"));
    #[cfg(test)]
    timings.finish("evidence_classification");
    let mut rendered = tempfile::NamedTempFile::new()?;
    crate::command::write_lossy(&mut raw.stdout.reader()?, &mut rendered)?;
    writeln!(rendered)?;
    crate::command::write_lossy(&mut raw.stderr.reader()?, &mut rendered)?;
    let mut text = crate::capture::Capture::utf8(rendered)?;
    text.set_limit(NonZeroUsize::new(64 * 1024));
    #[cfg(test)]
    timings.finish("diagnostics");
    // Shared runner drains captures and stops descendants before these drops.
    drop(owned);
    drop(workspace);
    #[cfg(test)]
    timings.finish("cleanup");
    Ok(Execution {
        command: CommandOutput {
            raw,
            text,
            evidence: OutcomeEvidence::Validated(evidence),
            tool: "Bun",
        },
        identity: execution_identity,
        input_fingerprint: fingerprint(&serde_json::to_vec(&project.files)?),
        #[cfg(test)]
        timings: timings.values,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(syntax: &str) -> tempfile::TempDir {
        let root = tempfile::tempdir().unwrap();
        for dir in ["src", "test"] {
            fs::create_dir(root.path().join(dir)).unwrap();
        }
        fs::write(
            root.path().join("package.json"),
            r#"{"name":"probe","type":"module","scripts":{"test":"bun test"}}"#,
        )
        .unwrap();
        fs::write(
            root.path().join(format!("src/flags.{syntax}")),
            "export const enabled = true;",
        )
        .unwrap();
        fs::write(root.path().join(format!("test/flags.test.{syntax}")), format!("import {{test,expect}} from 'bun:test'; import {{enabled}} from '../src/flags.{syntax}'; test('flag',()=>{{expect(enabled).toBe(true)}});")).unwrap();
        root
    }

    #[test]
    fn rejects_unsupported_packages_and_discovery_before_execution() {
        let root = fixture("js");
        let load = || project::Project::load(root.path(), &Cancellation::default());
        assert_eq!(load().unwrap().tests, ["test/flags.test.js"]);
        for (path, contents) in [
            ("bunfig.toml", "[test]\npreload=['evil.js']"),
            ("tsconfig.json", "{\"extends\":\"evil\"}"),
            (".env", "BUN_OPTIONS=evil"),
            ("outside.test.js", ""),
            ("test/flags.spec.js", ""),
            ("test/config/package.json", "{}"),
        ] {
            let path = root.path().join(path);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(&path, contents).unwrap();
            assert!(load().is_err(), "{}", path.display());
            fs::remove_file(path).unwrap();
        }
        for source in [
            "import('x');",
            "import x from 'dependency';",
            "import {test} from 'bun:test';",
            "require('fs');",
        ] {
            fs::write(root.path().join("src/flags.js"), source).unwrap();
            assert!(load().is_err(), "{source}");
        }
    }

    #[tokio::test]
    #[ignore = "requires pinned Bun 1.3.11 on PATH; runs isolated JS/TS outcome matrix"]
    async fn real_isolated_outcomes_and_input_preservation() -> Result<()> {
        for syntax in ["js", "ts"] {
            let root = fixture(syntax);
            let test = root.path().join(format!("test/flags.test.{syntax}"));
            for (source, expected) in [
                (
                    "test('body',()=>{console.log('MUTIE_PATHS:'+JSON.stringify([process.cwd(),process.env.HOME,process.env.TMPDIR]))});",
                    Some(Outcome::Missed),
                ),
                (
                    "test('body',()=>{throw new Error('body')});",
                    Some(Outcome::Killed),
                ),
                (
                    "afterEach(()=>{throw new Error('hook')}); test('body',()=>{throw new Error('body')});",
                    None,
                ),
                (
                    "test('body',()=>{Promise.reject(new Error('late')); throw new Error('body')});",
                    None,
                ),
                ("test('body',()=>{expect.assertions(1)});", None),
                ("test('body',()=>{process.exit(0)});", None),
            ] {
                fs::write(
                    &test,
                    format!("import {{test,expect,afterEach}} from 'bun:test'; {source}"),
                )?;
                let before = project::Project::load(root.path(), &Cancellation::default())?.files;
                let output = execute(
                    root.path(),
                    "bun",
                    Duration::from_secs(5),
                    NonZeroUsize::new(1024 * 1024).unwrap(),
                    &Cancellation::default(),
                )
                .await?;
                let mut removed_paths = 0;
                for line in output.text.text()?.lines() {
                    if let Some(paths) = line.strip_prefix("MUTIE_PATHS:") {
                        let paths: Vec<String> = serde_json::from_str(paths)?;
                        for path in paths {
                            removed_paths += 1;
                            assert!(
                                !Path::new(&path).exists(),
                                "temporary path was not removed: {path}"
                            );
                        }
                    }
                }
                if expected == Some(Outcome::Missed) {
                    assert_eq!(removed_paths, 3);
                }
                let actual = output.classify();
                assert_eq!(
                    actual.as_ref().ok().copied(),
                    expected,
                    "{syntax}: {source}: {actual:?}\n{}",
                    output.text.text()?
                );
                assert_eq!(
                    before,
                    project::Project::load(root.path(), &Cancellation::default())?.files
                );
            }
        }
        Ok(())
    }

    #[tokio::test]
    #[ignore = "requires pinned Bun; checks output/file bounds, timeout, cancellation and descendant shutdown"]
    async fn real_limits_and_descendant_cleanup() -> Result<()> {
        let root = fixture("js");
        let test = root.path().join("test/flags.test.js");
        for source in [
            "test('noise',()=>{console.log('x'.repeat(200000));});",
            "test('x'.repeat(9*1024*1024),()=>{});",
            "test('file',async()=>{await Bun.write('large.bin',new Uint8Array(9*1024*1024));});",
        ] {
            fs::write(&test, format!("import {{test}} from 'bun:test'; {source}"))?;
            let output = execute(
                root.path(),
                "bun",
                Duration::from_secs(5),
                NonZeroUsize::new(4096).unwrap(),
                &Cancellation::default(),
            )
            .await?;
            assert!(output.classify().is_err(), "{}", output.text.text()?);
        }
        let marker = tempfile::tempdir()?;
        for cancel in [false, true] {
            let path = marker
                .path()
                .join(if cancel { "cancel" } else { "timeout" });
            let ready = path.with_extension("ready");
            let child = format!(
                "setTimeout(()=>require('fs').writeFileSync({},'leaked'),1500)",
                serde_json::to_string(&path)?
            );
            fs::write(
                &test,
                format!(
                    "import {{test}} from 'bun:test'; import {{spawn}} from 'node:child_process'; test('pending',async()=>{{spawn(process.execPath,['-e',{}],{{stdio:'ignore'}}); await Bun.write({},'ready'); return new Promise(()=>{{}})}});",
                    serde_json::to_string(&child)?,
                    serde_json::to_string(&ready)?
                ),
            )?;
            let cancellation = Cancellation::default();
            let stopping = cancellation.clone();
            let waiting = ready.clone();
            let stop = tokio::spawn(async move {
                if cancel {
                    let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
                    while !waiting.exists() {
                        assert!(
                            tokio::time::Instant::now() < deadline,
                            "test never spawned its child"
                        );
                        tokio::time::sleep(Duration::from_millis(20)).await;
                    }
                    stopping.cancel();
                }
            });
            let result = execute(
                root.path(),
                "bun",
                if cancel {
                    Duration::from_secs(5)
                } else {
                    Duration::from_millis(250)
                },
                NonZeroUsize::new(4096).unwrap(),
                &cancellation,
            )
            .await;
            stop.await?;
            assert!(ready.exists(), "test did not run before shutdown");
            if cancel {
                match result {
                    Ok(output) => assert!(output.classify().is_err()),
                    Err(error) => assert!(
                        error
                            .downcast_ref::<crate::cancellation::Interrupted>()
                            .is_some(),
                        "{error:#}"
                    ),
                }
            } else {
                assert_eq!(result?.classify()?, Outcome::Timeout);
            }
            tokio::time::sleep(Duration::from_millis(1700)).await;
            assert!(!path.exists(), "descendant outlived command shutdown");
        }
        Ok(())
    }
    #[test]
    fn accepts_scoped_config_but_keeps_ancillary_files_out_of_imports_and_discovery() -> Result<()>
    {
        let root = fixture("ts");
        let load = || project::Project::load(root.path(), &Cancellation::default());
        let config = root.path().join("tsconfig.json");
        fs::write(
            &config,
            r#"{"compilerOptions":{"module":"NodeNext","moduleResolution":"NodeNext","target":"ES2022","moduleDetection":"force","lib":["ESNext","DOM"],"strict":true,"noEmit":true},"include":["src","tests","examples"]}"#,
        )?;
        fs::create_dir(root.path().join("scripts"))?;
        fs::write(
            root.path().join("scripts/release.ts"),
            "import 'uninstalled-release-tool';",
        )?;
        let before = load()?.files;
        fs::write(&config, r#"{"compilerOptions":{"strict":false}}"#)?;
        assert_ne!(before, load()?.files);
        for path in [
            "scripts/hidden.test.ts",
            "scripts/hidden.spec.ts",
            "src/tsconfig.json",
            "jsconfig.json",
        ] {
            let path = root.path().join(path);
            fs::write(&path, "{}")?;
            assert!(load().is_err(), "{}", path.display());
            fs::remove_file(path)?;
        }
        fs::write(
            root.path().join("src/flags.ts"),
            "import '../scripts/release.ts';",
        )?;
        assert!(load().is_err());
        Ok(())
    }

    #[tokio::test]
    #[ignore = "requires pinned Bun and unchanged MUTIE_BUN_COMPOSER; real baseline/finding controls"]
    async fn real_composer_baseline_and_stronger_finding() -> Result<()> {
        let source = std::path::PathBuf::from(
            std::env::var("MUTIE_BUN_COMPOSER").context("set MUTIE_BUN_COMPOSER")?,
        );
        let cancellation = Cancellation::default();
        let before = project::Project::load(&source, &cancellation)?.files;
        let recorded: serde_json::Value = serde_json::from_str(include_str!(
            "../benchmarks/bun-observer-composer-results.json"
        ))?;
        let raw_hashes: std::collections::BTreeMap<_, _> = before
            .iter()
            .map(|(k, v)| (k.clone(), v.trim_start_matches("sha256:").to_owned()))
            .collect();
        ensure!(
            serde_json::to_value(&raw_hashes)? == recorded["original_file_sha256"],
            "Composer does not match the unchanged audited revision"
        );
        let copy = Workspace::create(&source, &[], &cancellation)?;
        let root = copy.path();
        let path = root.join("src/utils.ts");
        let original = fs::read_to_string(&path)?;
        ensure!(
            original.matches("configurable: true,").count() == 1,
            "candidate changed"
        );
        let mut records = Vec::new();
        for (mode, stronger, candidate, expected, count) in [
            ("baseline", false, false, Outcome::Missed, "272 pass"),
            ("candidate", false, true, Outcome::Missed, "272 pass"),
            (
                "stronger_baseline",
                true,
                false,
                Outcome::Missed,
                "273 pass",
            ),
            (
                "stronger_candidate",
                true,
                true,
                Outcome::Killed,
                "272 pass",
            ),
        ] {
            fs::write(
                &path,
                if candidate {
                    original.replace("configurable: true,", "configurable: false,")
                } else {
                    original.clone()
                },
            )?;
            if stronger {
                fs::write(
                    root.join("tests/mutie-stronger.test.ts"),
                    include_bytes!("../benchmarks/fixtures/bun-audit/composer-stronger.test.ts"),
                )?;
            }
            let inputs = project::Project::load(&root, &cancellation)?.files;
            let output = execute(
                &root,
                "bun",
                Duration::from_secs(15),
                NonZeroUsize::new(4 * 1024 * 1024).unwrap(),
                &cancellation,
            )
            .await?;
            let text = output.text.text()?;
            assert_eq!(output.classify()?, expected, "{mode}: {text}");
            assert!(text.contains(count), "{mode}: {text}");
            if expected == Outcome::Killed {
                assert!(
                    text.contains("1 fail") && text.contains("middleware can be named again"),
                    "{text}"
                );
            }
            assert_eq!(inputs, project::Project::load(&root, &cancellation)?.files);
            records.push(serde_json::json!({"mode":mode,"outcome":format!("{expected:?}"),"command_seconds":output.raw.elapsed.as_secs_f64(),"passed":if mode=="stronger_baseline"{273}else{272},"failed":if candidate&&stronger{1}else{0},"diagnostics_sha256":fingerprint(text.as_bytes())}));
        }
        assert_eq!(
            before,
            project::Project::load(&source, &cancellation)?.files
        );
        println!(
            "BUN_COMPOSER_ACCEPTANCE={}",
            serde_json::json!({"original_file_sha256":raw_hashes,"observer_sha256":fingerprint(include_bytes!("bun/observer.mjs")),"runs":records,"inputs_preserved":true})
        );
        Ok(())
    }
}
