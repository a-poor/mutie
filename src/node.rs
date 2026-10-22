//! Pinned, dependency-free ESM JavaScript / native TypeScript node:test adapter.
mod discovery;
pub mod project;
mod protocol;
mod typescript;
use crate::{cancellation::Cancellation, command, process, workspace::Workspace};
use anyhow::{Context, Result, ensure};
use sha2::{Digest, Sha256};
use std::{
    fs,
    num::NonZeroUsize,
    path::{Path, PathBuf},
    time::Duration,
};
const HOOK_HASH: &str = "6329a64a8825bf217d60526f7a53fd89af6fc5107ad9f15d41b825b19a587fa2";
pub fn fingerprint(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}
pub fn valid_fingerprint(value: &str) -> bool {
    value
        .strip_prefix("sha256:")
        .is_some_and(|s| s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit()))
}
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutionScope {
    pub package: String,
    pub node: String,
    pub node_executable: PathBuf,
    pub manifest_fingerprint: String,
    pub recipe_fingerprint: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub typescript: Option<typescript::TypeScriptScope>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub default_discovery: bool,
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub development_fingerprints: std::collections::BTreeMap<String, String>,
}
fn recipe(typescript: bool, default_discovery: bool) -> String {
    if default_discovery {
        return fingerprint(
            [
                recipe(typescript, false).as_bytes(),
                discovery::PATTERN.as_bytes(),
                discovery::HASHES.join(":").as_bytes(),
                b"native-default-selection-audited-test-root",
            ]
            .concat()
            .as_slice(),
        );
    }
    let mode: &[u8] = if typescript {
        b"node-test-serial-process-native-strip-types-explicit-js-or-ts-test-glob"
    } else {
        b"node-test-serial-process-test/*.test.js"
    };
    fingerprint(
        [
            include_bytes!("node/observer.mjs").as_slice(),
            include_bytes!("node/reporter.mjs").as_slice(),
            HOOK_HASH.as_bytes(),
            mode,
        ]
        .concat()
        .as_slice(),
    )
}
impl ExecutionScope {
    pub fn validate_record(&self) -> Result<()> {
        ensure!(
            !self.package.is_empty()
                && self.node == "24.22.0"
                && self.node_executable.is_absolute(),
            "invalid saved Node identity"
        );
        ensure!(
            self.recipe_fingerprint == recipe(self.typescript.is_some(), self.default_discovery),
            "unsupported saved Node runner recipe"
        );
        ensure!(
            valid_fingerprint(&self.manifest_fingerprint),
            "invalid saved Node manifest fingerprint"
        );
        ensure!(
            self.development_fingerprints
                .iter()
                .all(|(key, value)| crate::dependency_tree::identity_key(key)
                    && valid_fingerprint(value)),
            "invalid Node development installation identity"
        );
        if let Some(scope) = &self.typescript {
            scope.validate_record()?;
            ensure!(
                (scope.test_pattern == typescript::TestPattern::Default) == self.default_discovery,
                "conflicting saved Node discovery mode"
            );
        }
        Ok(())
    }
    pub fn test_command(&self) -> Vec<&'static str> {
        if self.default_discovery {
            return vec!["node", "--test", "--test-concurrency=1"];
        }
        vec![
            "node",
            "--test",
            "--test-concurrency=1",
            self.typescript
                .as_ref()
                .map_or("test/*.test.js", |scope| scope.test_pattern.glob()),
        ]
    }
    pub fn validate_rerun(&self, current: &Self) -> Result<()> {
        self.validate_record()?;
        current.validate_record()?;
        ensure!(
            self == current,
            "Node package or runner identity changed; start a fresh scan; no mutations were run"
        );
        Ok(())
    }
}
async fn scope_with(
    root: &Path,
    program: &str,
    cancellation: &Cancellation,
) -> Result<ExecutionScope> {
    let project = project::Project::load(root, cancellation)?;
    let mut expression = if project.default_discovery {
        discovery::expression()
    } else {
        "const files=undefined,discoveryHashes=undefined;".into()
    };
    expression.push_str("console.log(JSON.stringify({files,discoveryHashes,version:process.version,executable:process.execPath,hash:require('node:crypto').createHash('sha256').update(process.binding('natives')['internal/test_runner/test']).digest('hex')}))");
    let out = process::run_with_env(
        program,
        &["--eval", &expression],
        root,
        Duration::from_secs(10),
        NonZeroUsize::new(if project.default_discovery {
            4 * 1024 * 1024
        } else {
            4096
        }),
        cancellation,
        &[("NODE_OPTIONS", "".as_ref()), ("NODE_PATH", "".as_ref())],
    )
    .await?;
    cancellation.check()?;
    ensure!(
        !out.timed_out && !out.interrupted && out.capture_error.is_none() && out.status.success(),
        "could not identify Node test runtime"
    );
    let value: serde_json::Value = serde_json::from_str(&out.stdout.text()?)?;
    ensure!(
        value["version"] == "v24.22.0" && value["hash"] == HOOK_HASH,
        "Node runner requires audited Node 24.22.0 test implementation"
    );
    if project.default_discovery {
        discovery::validate_native(&value, &project.tests)?;
    }
    let scope = ExecutionScope {
        package: project.name,
        node: "24.22.0".into(),
        node_executable: value["executable"]
            .as_str()
            .context("missing Node executable")?
            .into(),
        manifest_fingerprint: project.manifest,
        recipe_fingerprint: recipe(project.typescript.is_some(), project.default_discovery),
        default_discovery: project.default_discovery,
        typescript: project.typescript,
        development_fingerprints: project.development_fingerprints,
    };
    scope.validate_record()?;
    Ok(scope)
}
pub async fn execution_scope(root: &Path, cancellation: &Cancellation) -> Result<ExecutionScope> {
    scope_with(root, "node", cancellation).await
}
#[allow(clippy::too_many_arguments)]
pub async fn test(
    root: &Path,
    limit: Duration,
    log_limit: Option<NonZeroUsize>,
    capture_limit: Option<NonZeroUsize>,
    expected: &ExecutionScope,
    cancellation: &Cancellation,
) -> Result<command::CommandOutput> {
    expected.validate_record()?;
    let actual = scope_with(
        root,
        expected
            .node_executable
            .to_str()
            .context("Node path is not UTF-8")?,
        cancellation,
    )
    .await?;
    ensure!(
        &actual == expected,
        "Node package or runner identity changed during the run"
    );
    let source = root.to_owned();
    let cancel = cancellation.clone();
    let workspace = tokio::task::spawn_blocking(move || Workspace::create(&source, &[], &cancel))
        .await
        .context("Node snapshot task panicked")??;
    let root = workspace.path();
    let project = project::Project::load(&root, cancellation)?;
    ensure!(
        project.manifest == expected.manifest_fingerprint
            && project.typescript == expected.typescript
            && project.default_discovery == expected.default_discovery
            && project.development_fingerprints == expected.development_fingerprints,
        "Node manifest or native TypeScript configuration changed during snapshotting"
    );
    if project.default_discovery {
        let copied = scope_with(
            &root,
            expected.node_executable.to_str().unwrap(),
            cancellation,
        )
        .await?;
        ensure!(
            &copied == expected,
            "Node identity changed during snapshotting"
        );
    }
    let owned = tempfile::Builder::new()
        .prefix("mutie-node-command-")
        .tempdir()?;
    let owned_path = owned.path().canonicalize()?;
    ensure!(
        !owned_path.starts_with(&project.root),
        "Node temporary root must be outside project"
    );
    let reporter = owned_path.join("reporter.mjs");
    let observer = owned_path.join("observer.mjs");
    let evidence = owned_path.join("completion.json");
    let temp = owned_path.join("tmp");
    fs::create_dir(&temp)?;
    fs::write(
        &reporter,
        format!(
            "const MUTIE_EVIDENCE={};\n{}",
            serde_json::to_string(&evidence)?,
            include_str!("node/reporter.mjs")
        ),
    )?;
    fs::write(
        &observer,
        format!(
            "const MUTIE_OWNED={};\nconst MUTIE_HOOK_HASH={};\n{}",
            serde_json::to_string(&owned_path)?,
            serde_json::to_string(HOOK_HASH)?,
            include_str!("node/observer.mjs")
        ),
    )?;
    let mut args = vec![
        "--import".to_owned(),
        observer.to_string_lossy().into_owned(),
        "--test".into(),
        "--test-concurrency=1".into(),
        format!("--test-reporter={}", reporter.display()),
    ];
    if !project.default_discovery {
        args.extend(project.tests.iter().cloned());
    }
    let raw = process::run_with_env(
        expected.node_executable.to_str().unwrap(),
        &args.iter().map(String::as_str).collect::<Vec<_>>(),
        &root,
        limit,
        capture_limit,
        cancellation,
        &[
            ("NODE_OPTIONS", "".as_ref()),
            ("NODE_PATH", "".as_ref()),
            ("TMPDIR", temp.as_os_str()),
            ("TMP", temp.as_os_str()),
            ("TEMP", temp.as_os_str()),
        ],
    )
    .await?;
    let result = protocol::classify(
        &evidence,
        &owned_path,
        &root,
        &project.tests,
        raw.status.code(),
    )
    .map_err(|e| format!("{e:#}"));
    let mut rendered = tempfile::NamedTempFile::new()?;
    command::write_lossy(&mut raw.stdout.reader()?, &mut rendered)?;
    command::write_lossy(&mut raw.stderr.reader()?, &mut rendered)?;
    let mut text = crate::capture::Capture::utf8(rendered)?;
    text.set_limit(log_limit);
    Ok(command::CommandOutput {
        raw,
        text,
        evidence: command::OutcomeEvidence::Validated(result),
        tool: "Node test runner",
    })
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use crate::report::Outcome;
    #[test]
    fn preserves_legacy_javascript_identity_and_separates_native_typescript() {
        let old = serde_json::json!({
            "package":"fixture", "node":"24.22.0", "node_executable":"/node",
            "manifest_fingerprint":fingerprint(b"manifest"),
            "recipe_fingerprint":"sha256:e354cef7579504dfc39c4ebf64699b796407459d29f955ca5abc0f51cf7cbe56"
        });
        let javascript: ExecutionScope = serde_json::from_value(old.clone()).unwrap();
        javascript.validate_record().unwrap();
        assert_eq!(serde_json::to_value(&javascript).unwrap(), old);
        let mut typescript = javascript.clone();
        typescript.typescript = Some(typescript::TypeScriptScope {
            test_pattern: typescript::TestPattern::TypeScript,
            tsconfig_fingerprint: None,
        });
        assert!(typescript.validate_record().is_err());
        typescript.recipe_fingerprint = recipe(true, false);
        typescript.validate_record().unwrap();
        assert!(javascript.validate_rerun(&typescript).is_err());
        let mut native = typescript.clone();
        native.default_discovery = true;
        native.recipe_fingerprint = recipe(true, true);
        assert!(native.validate_record().is_err());
        native.typescript.as_mut().unwrap().test_pattern = typescript::TestPattern::Default;
        native.validate_record().unwrap();
        assert!(typescript.validate_rerun(&native).is_err());
        // The already saved explicit TS recipe remains readable, as does JS.
        assert_eq!(
            typescript.recipe_fingerprint,
            "sha256:ad3c7bccc8ef0781e8384c7b6d21c31e4a0e7de8f41a8c30ea40924428d82e59"
        );
        typescript.typescript.as_mut().unwrap().tsconfig_fingerprint = Some("invalid".into());
        assert!(typescript.validate_record().is_err());
    }
    #[tokio::test]
    #[ignore = "requires audited Node 24.22.0 on PATH; real process cleanup"]
    async fn timeout_cancellation_capture_and_owned_files() -> Result<()> {
        cleanup(false, false).await
    }
    #[tokio::test]
    #[ignore = "requires audited Node 24.22.0 on PATH; real process cleanup"]
    async fn typescript_timeout_cancellation_capture_and_owned_files() -> Result<()> {
        cleanup(true, false).await
    }
    #[tokio::test]
    #[ignore = "requires audited Node 24.22.0 on PATH; real process cleanup"]
    async fn default_discovery_timeout_cancellation_capture_and_owned_files() -> Result<()> {
        cleanup(true, true).await
    }
    async fn cleanup(typescript: bool, default_discovery: bool) -> Result<()> {
        let source = Path::new(env!("CARGO_MANIFEST_DIR")).join(if typescript {
            "tests/fixtures/node-typescript"
        } else {
            "tests/fixtures/node"
        });
        let extension = if typescript { "ts" } else { "js" };
        let source_file = format!("src/flags.{extension}");
        let work = Workspace::create(&source, &[], &Cancellation::default())?;
        let root = work.path();
        if default_discovery {
            let manifest = root.join("package.json");
            let mut value: serde_json::Value = serde_json::from_slice(&fs::read(&manifest)?)?;
            value["scripts"]["test"] = "node --test".into();
            fs::write(manifest, serde_json::to_vec(&value)?)?;
        }
        let original = fs::read(root.join(&source_file))?;
        for mode in ["timeout", "cancel", "capture"] {
            let control = tempfile::tempdir()?;
            let checkpoint = control.path().join("checkpoint.json");
            let js_path = serde_json::to_string(&checkpoint)?;
            let noise = if mode == "capture" {
                "process.stdout.write('x'.repeat(65536));"
            } else {
                ""
            };
            fs::write(
                root.join(format!("test/probe.test.{extension}")),
                format!(
                    r#"
import {{test}} from 'node:test';
import {{writeFileSync}} from 'node:fs';
import {{spawn}} from 'node:child_process';
import {{join}} from 'node:path';
test('owned files', async () => {{
  const child=spawn(process.execPath,['-e','setInterval(()=>{{}},1000)'],{{stdio:'ignore'}});
  writeFileSync('src/owned-marker.txt','owned');
  writeFileSync(join(process.env.TMPDIR,'leftover'),'owned');
  writeFileSync({js_path},JSON.stringify({{pid:child.pid,root:process.cwd(),temp:process.env.TMPDIR}}));
  {noise}
  await new Promise(()=>{{}});
}});
"#
                ),
            )?;
            let cancellation = Cancellation::default();
            let scope = execution_scope(&root, &cancellation).await?;
            let observe = async {
                let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
                while !checkpoint.exists() && tokio::time::Instant::now() < deadline {
                    tokio::time::sleep(Duration::from_millis(20)).await;
                }
                ensure!(checkpoint.exists(), "{mode} test did not start");
                if mode == "cancel" {
                    cancellation.cancel();
                }
                Ok::<(), anyhow::Error>(())
            };
            let limit = Duration::from_secs(if mode == "timeout" { 3 } else { 20 });
            let cap = NonZeroUsize::new(if mode == "capture" {
                8192
            } else {
                4 * 1024 * 1024
            });
            let (result, observed) = tokio::join!(
                test(
                    &root,
                    limit,
                    NonZeroUsize::new(32),
                    cap,
                    &scope,
                    &cancellation
                ),
                observe
            );
            observed?;
            let result = result?;
            match mode {
                "timeout" => assert_eq!(result.classify()?, Outcome::Timeout),
                "cancel" => {
                    assert!(result.raw.interrupted);
                    assert!(result.classify().is_err());
                }
                _ => {
                    assert!(result.raw.capture_error.is_some());
                    assert!(result.classify().is_err());
                }
            }
            let checkpoint: serde_json::Value = serde_json::from_slice(&fs::read(checkpoint)?)?;
            assert!(!Path::new(checkpoint["root"].as_str().unwrap()).exists());
            assert!(!Path::new(checkpoint["temp"].as_str().unwrap()).exists());
            let pid = checkpoint["pid"].as_i64().context("missing child pid")? as i32;
            let deadline = tokio::time::Instant::now() + Duration::from_secs(3);
            while unsafe { libc::kill(pid, 0) } == 0 && tokio::time::Instant::now() < deadline {
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
            assert_ne!(
                unsafe { libc::kill(pid, 0) },
                0,
                "descendant remains after {mode}"
            );
            assert!(!root.join("src/owned-marker.txt").exists());
            assert_eq!(fs::read(root.join(&source_file))?, original);
        }
        Ok(())
    }
}
