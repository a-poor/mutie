//! A fresh project/dependency copy and process per command; no artifact reuse.
use super::{
    NodeTool,
    project::Project,
    protocol::{self, Evidence},
};
use crate::{
    cancellation::Cancellation, capture::Capture, mutation::Mutation, process, workspace::Workspace,
};
use anyhow::{Context, Result, ensure};
use std::{fs, io::Write, num::NonZeroUsize, path::Path, time::Duration};

pub struct Execution {
    pub raw: process::CommandOutput,
    pub diagnostics: Capture,
    pub evidence: Result<Evidence>,
    #[cfg(test)]
    timings: Vec<(&'static str, f64)>,
}
// Test-only phase observations: no CLI flags, report schema or runner identity changes.
#[cfg(test)]
struct PhaseTimings {
    last: std::time::Instant,
    values: Vec<(&'static str, f64)>,
}
#[cfg(test)]
impl PhaseTimings {
    fn new() -> Self {
        Self {
            last: std::time::Instant::now(),
            values: Vec::new(),
        }
    }
    fn finish(&mut self, name: &'static str) {
        let now = std::time::Instant::now();
        self.values
            .push((name, now.duration_since(self.last).as_secs_f64()));
        self.last = now;
    }
}

impl Execution {
    pub fn into_command(
        mut self,
        log_limit: Option<NonZeroUsize>,
    ) -> crate::command::CommandOutput {
        // Validate the complete completion record before retaining diagnostics.
        // Process interruption, capture failure and timeout take precedence in
        // the shared classifier even if completion was never written.
        let evidence = self
            .evidence
            .and_then(|evidence| evidence.classify(self.raw.status.code()))
            .map_err(|error| format!("{error:#}"));
        self.raw.stdout.set_limit(log_limit);
        self.raw.stderr.set_limit(log_limit);
        self.diagnostics.set_limit(log_limit);
        crate::command::CommandOutput {
            raw: self.raw,
            text: self.diagnostics,
            evidence: crate::command::OutcomeEvidence::Validated(evidence),
            tool: "Vitest",
        }
    }
}

pub(super) async fn run(
    project: &Project,
    node: &NodeTool,
    mutation: Option<Mutation>,
    limit: Duration,
    capture_limit: Option<NonZeroUsize>,
    cancellation: &Cancellation,
) -> Result<Execution> {
    #[cfg(test)]
    let mut timings = PhaseTimings::new();
    let source = project.root.clone();
    let copying = cancellation.clone();
    let workspace = tokio::task::spawn_blocking(move || {
        let excluded = [
            ".mutie",
            ".vitest",
            "node_modules/.vite",
            "node_modules/.vite-temp",
            "node_modules/.cache",
        ]
        .map(|p| source.join(p));
        Workspace::create(&source, &excluded, &copying)
    })
    .await
    .context("Vitest workspace preparation panicked")??;
    let root = workspace.path();
    #[cfg(test)]
    timings.finish("adapter_copy");
    let captured = Project::load(&root, cancellation)?;
    ensure!(
        captured.installation_fingerprint == project.installation_fingerprint
            && captured.package_manager == project.package_manager
            && captured.manifest_fingerprint == project.manifest_fingerprint
            && captured.lockfile_fingerprint == project.lockfile_fingerprint
            && captured.project_config_fingerprints == project.project_config_fingerprints,
        "Vitest package or configuration changed during snapshotting"
    );
    #[cfg(test)]
    timings.finish("copied_project_validation");
    super::lifecycle::instrument(&root)?;
    if let Some(mutation) = mutation {
        mutation.apply(&root)?;
    }
    // Separate from the project so test discovery cannot collect these helpers.
    let owned = tempfile::Builder::new()
        .prefix("mutie-vitest-command-")
        .tempdir()?;
    let owned_path = owned.path().canonicalize()?;
    ensure!(
        !owned_path.starts_with(&project.root),
        "Vitest temporary root must be outside the source project"
    );
    let evidence = owned_path.join("completion.json");
    let reporter = owned_path.join("reporter.mjs");
    let config = owned_path.join("config.mjs");
    let temp = owned_path.join("tmp");
    fs::create_dir(&temp)?;
    let constants = format!(
        "const MUTIE_ROOT={};\nconst MUTIE_EVIDENCE={};\nconst MUTIE_EVIDENCE_LIMIT={};\n",
        serde_json::to_string(&root)?,
        serde_json::to_string(&evidence)?,
        protocol::EVIDENCE_LIMIT
    );
    fs::write(&reporter, constants + include_str!("reporter.mjs"))?;
    let settings = settings(&root, &owned_path.join("cache"), &reporter);
    fs::write(
        &config,
        format!("export default {};\n", serde_json::to_string(&settings)?),
    )?;
    let cli = root.join("node_modules/vitest/vitest.mjs");
    let args = [
        utf8(&cli)?,
        "run",
        "--config",
        utf8(&config)?,
        "--configLoader",
        "native",
    ];
    #[cfg(test)]
    timings.finish("instrumentation_and_command_setup");
    let raw = process::run_with_env(
        utf8(&node.executable)?,
        &args,
        &root,
        limit,
        capture_limit,
        cancellation,
        &[
            ("NODE_OPTIONS", "".as_ref()),
            ("NODE_PATH", "".as_ref()),
            ("CI", "true".as_ref()),
            ("TMPDIR", temp.as_os_str()),
            ("TMP", temp.as_os_str()),
            ("TEMP", temp.as_os_str()),
        ],
    )
    .await?;
    #[cfg(test)]
    timings.finish("process_runner");
    // Read before cleanup, independently of diagnostic truncation. A killed or
    // timed-out child can leave no evidence; classification gives process facts priority.
    let evidence = protocol::read(&evidence);
    let mut rendered = tempfile::NamedTempFile::new()?;
    crate::command::write_lossy(&mut raw.stdout.reader()?, &mut rendered)?;
    writeln!(rendered)?;
    crate::command::write_lossy(&mut raw.stderr.reader()?, &mut rendered)?;
    let mut diagnostics = Capture::utf8(rendered)?;
    diagnostics.set_limit(NonZeroUsize::new(64 * 1024));
    #[cfg(test)]
    timings.finish("evidence_and_diagnostics");
    // The shared process runner has drained output and shut down the process group.
    drop(owned);
    drop(workspace);
    #[cfg(test)]
    timings.finish("cleanup");
    Ok(Execution {
        raw,
        diagnostics,
        evidence,
        #[cfg(test)]
        timings: timings.values,
    })
}
fn utf8(path: &Path) -> Result<&str> {
    path.to_str()
        .context("Vitest requires UTF-8 tool and temporary paths")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        mutation::Scope,
        report::Outcome,
        vitest::{discovery, resolve_node},
    };
    fn prepared() -> Result<std::path::PathBuf> {
        std::env::var_os("MUTIE_VITEST_FIXTURE")
            .map(std::path::PathBuf::from)
            .context("set MUTIE_VITEST_FIXTURE to the prepared pinned fixture")
    }
    async fn execute(
        root: &Path,
        node: &NodeTool,
        mutation: Option<Mutation>,
    ) -> Result<crate::command::CommandOutput> {
        Ok(run(
            &Project::load(root, &Cancellation::default())?,
            node,
            mutation,
            Duration::from_secs(20),
            NonZeroUsize::new(4 * 1024 * 1024),
            &Cancellation::default(),
        )
        .await?
        .into_command(NonZeroUsize::new(64 * 1024)))
    }
    /// Measures the actual adapter path, without recursively running Mutie mutations.
    /// Keep opt-in: the external project must have a passing pinned Vitest baseline.
    #[tokio::test]
    #[ignore = "requires MUTIE_VITEST_PROFILE_PROJECT; prints three baseline phase profiles"]
    async fn real_feedback_phase_profile() -> Result<()> {
        let source = std::env::var_os("MUTIE_VITEST_PROFILE_PROJECT")
            .map(std::path::PathBuf::from)
            .context("set MUTIE_VITEST_PROFILE_PROJECT")?;
        let cancellation = Cancellation::default();
        let expected = crate::vitest::execution_scope(&source, &cancellation).await?;
        for repeat in 1..=3 {
            let total = std::time::Instant::now();
            let mut outer = PhaseTimings::new();
            let workspace = Workspace::create(&source, &[], &cancellation)?;
            outer.finish("outer_workspace_copy");
            let root = workspace.path();
            let actual = crate::vitest::execution_scope(&root, &cancellation).await?;
            ensure!(
                actual == expected,
                "profile copy execution identity changed"
            );
            outer.finish("execution_identity");
            let project = Project::load(&root, &cancellation)?;
            outer.finish("project_reload");
            let node = NodeTool {
                executable: actual.node_executable,
            };
            let output = run(
                &project,
                &node,
                None,
                Duration::from_secs(120),
                NonZeroUsize::new(4 * 1024 * 1024),
                &cancellation,
            )
            .await?;
            let phases = output.timings.clone();
            let command_seconds = output.raw.elapsed.as_secs_f64();
            let counts = output
                .evidence
                .as_ref()
                .map_err(|e| anyhow::anyhow!("{e:#}"))?
                .counts();
            ensure!(
                output.into_command(None).classify()? == Outcome::Missed,
                "profile baseline must pass"
            );
            outer.finish("adapter_total");
            drop(workspace);
            outer.finish("outer_workspace_cleanup");
            println!(
                "vitest-profile {}",
                serde_json::json!({
                    "repeat": repeat, "total_seconds": total.elapsed().as_secs_f64(),
                    "outer_phases": outer.values, "adapter_phases": phases,
                    "command_seconds": command_seconds, "counts": counts
                })
            );
        }
        ensure!(
            crate::vitest::execution_scope(&source, &cancellation).await? == expected,
            "source execution identity changed during profiling"
        );
        Ok(())
    }

    /// Acceptance boundary for worker-side failure-phase evidence.
    #[tokio::test]
    #[ignore = "requires prepared Vitest fixture; lifecycle failure regressions"]
    async fn real_lifecycle_failure_evidence() -> Result<()> {
        let input = prepared()?;
        let cancellation = Cancellation::default();
        let work = Workspace::create(&input, &[], &cancellation)?;
        let root = work.path();
        let node = resolve_node(&root, &cancellation).await?;
        assert_eq!(
            execute(&root, &node, None).await?.classify()?,
            Outcome::Missed
        );
        for (name, expected, code) in [
            (
                "body-assertion",
                "killed",
                "test('body',()=>expect(true).toBe(false));",
            ),
            (
                "successful-before",
                "killed",
                "beforeEach(()=>{}); test('body',()=>expect(true).toBe(false));",
            ),
            (
                "before-failure",
                "error",
                "beforeEach(()=>expect(true).toBe(false)); test('body',()=>{});",
            ),
            (
                "after-failure",
                "error",
                "afterEach(()=>expect(true).toBe(false)); test('body',()=>{});",
            ),
            (
                "returned-cleanup",
                "error",
                "beforeEach(()=>()=>expect(true).toBe(false)); test('body',()=>{});",
            ),
            (
                "before-all",
                "error",
                "beforeAll(()=>expect(true).toBe(false)); test('body',()=>{});",
            ),
            (
                "after-all",
                "error",
                "afterAll(()=>expect(true).toBe(false)); test('body',()=>{});",
            ),
            (
                "around-setup",
                "error",
                "aroundEach(async run=>{expect(true).toBe(false); await run();}); test('body',()=>{});",
            ),
            (
                "around-teardown",
                "error",
                "aroundEach(async run=>{await run(); expect(true).toBe(false);}); test('body',()=>{});",
            ),
            (
                "around-all",
                "error",
                "aroundAll(async run=>{await run(); expect(true).toBe(false);}); test('body',()=>{});",
            ),
            (
                "finished-callback",
                "error",
                "test('body',()=>{onTestFinished(()=>expect(true).toBe(false));});",
            ),
            (
                "failed-callback",
                "error",
                "test('body',()=>{onTestFailed(()=>expect(true).toBe(false)); expect(true).toBe(false);});",
            ),
            (
                "passing-fixture",
                "missed",
                "const extended=test.extend({value:async({},use)=>{await use(1);}}); extended('body',({value})=>expect(value).toBe(1));",
            ),
            (
                "passing-around",
                "missed",
                "aroundEach(async run=>{await run();}); test('body',()=>{});",
            ),
            (
                "passing-finished",
                "missed",
                "test('body',()=>{onTestFinished(()=>{});});",
            ),
            (
                "inverted-lifecycle",
                "error",
                "test.fails('body',()=>{onTestFinished(()=>expect(true).toBe(false));});",
            ),
            (
                "fixture-body",
                "killed",
                "const extended=test.extend({value:async({},use)=>{await use(1);}}); extended('body',({value})=>expect(value).toBe(2));",
            ),
            (
                "around-body",
                "killed",
                "aroundEach(async run=>{await run();}); test('body',()=>expect(true).toBe(false));",
            ),
            (
                "finished-body",
                "killed",
                "test('body',()=>{onTestFinished(()=>{}); expect(true).toBe(false);});",
            ),
            (
                "failed-body",
                "killed",
                "test('body',()=>{onTestFailed(()=>{}); expect(true).toBe(false);});",
            ),
            (
                "mixed-cleanup",
                "error",
                "test('body',()=>{onTestFinished(()=>expect(true).toBe(false)); expect(true).toBe(false);});",
            ),
            (
                "fixture-mixed",
                "error",
                "const extended=test.extend({value:async({},use)=>{await use(1); expect(true).toBe(false);}}); extended('body',({value})=>expect(value).toBe(2));",
            ),
            (
                "after-body",
                "killed",
                "afterEach(()=>{}); test('body',()=>expect(true).toBe(false));",
            ),
            (
                "returned-cleanup-body",
                "killed",
                "beforeEach(()=>()=>{}); test('body',()=>expect(true).toBe(false));",
            ),
            (
                "concurrent-body",
                "killed",
                "test.concurrent('fail',()=>{onTestFinished(()=>{}); expect(true).toBe(false);}); test.concurrent('pass',()=>{});",
            ),
            (
                "assertion-count",
                "error",
                "test('body',()=>{expect.assertions(1);});",
            ),
            (
                "before-async-assertion",
                "error",
                "beforeEach(()=>{expect(Promise.resolve(true)).resolves.toBe(false);}); test('body',()=>{});",
            ),
            (
                "body-async-assertion",
                "killed",
                "test('body',()=>{expect(Promise.resolve(true)).resolves.toBe(false);});",
            ),
            (
                "fixture-async-assertion",
                "error",
                "const extended=test.extend({value:async({},use)=>{expect(Promise.resolve(true)).resolves.toBe(false); await use(1);}}); extended('body',({value})=>expect(value).toBe(1));",
            ),
            (
                "fixture-setup",
                "error",
                "const extended=test.extend({value:async({},use)=>{expect(true).toBe(false); await use(1);}}); extended('body',({value})=>expect(value).toBe(1));",
            ),
            (
                "fixture-teardown",
                "error",
                "const extended=test.extend({value:async({},use)=>{await use(1); expect(true).toBe(false);}}); extended('body',({value})=>expect(value).toBe(1));",
            ),
            (
                "auto-fixture",
                "error",
                "const extended=test.extend({value:[async({},use)=>{await use(1); expect(true).toBe(false);},{auto:true}]}); extended('body',()=>{});",
            ),
            (
                "concurrent-finished",
                "error",
                "test.concurrent('cleanup',()=>{onTestFinished(()=>expect(true).toBe(false));}); test.concurrent('pass',async()=>{await new Promise(r=>setTimeout(r,10));});",
            ),
        ] {
            fs::write(
                root.join("test/lifecycle.test.js"),
                format!(
                    "import {{test,expect,beforeEach,afterEach,beforeAll,afterAll,aroundEach,aroundAll,onTestFinished,onTestFailed}} from 'vitest';\n{code}\n"
                ),
            )?;
            let execution = run(
                &Project::load(&root, &cancellation)?,
                &node,
                None,
                Duration::from_secs(20),
                NonZeroUsize::new(4 * 1024 * 1024),
                &cancellation,
            )
            .await?;
            let evidence = execution
                .evidence
                .as_ref()
                .map_err(|error| anyhow::anyhow!("{name}: {error:#}"))?;
            let classified = evidence.classify(execution.raw.status.code());
            println!("{name}: expected={expected}, observed={classified:?}");
            match expected {
                "killed" => assert_eq!(classified?, Outcome::Killed, "{name}"),
                "missed" => assert_eq!(classified?, Outcome::Missed, "{name}"),
                "error" => assert!(classified.is_err(), "{name}: {evidence:?}"),
                _ => unreachable!(),
            }
        }
        Ok(())
    }

    #[tokio::test]
    #[ignore = "requires prepared Vitest fixture; scoped fixtures and nested lifecycle"]
    async fn real_scoped_fixtures_and_nested_lifecycle() -> Result<()> {
        let input = prepared()?;
        let cancellation = Cancellation::default();
        let work = Workspace::create(&input, &[], &cancellation)?;
        let root = work.path();
        let node = resolve_node(&root, &cancellation).await?;
        let mut cases = Vec::new();
        for scope in ["file", "worker"] {
            for (name, expected, setup, body, cleanup) in [
                ("pass", "missed", "", "expect(value).toBe(1);", ""),
                ("body", "killed", "", "expect(value).toBe(2);", ""),
                ("setup", "error", "expect(true).toBe(false);", "", ""),
                (
                    "cleanup",
                    "error",
                    "",
                    "expect(value).toBe(1);",
                    "expect(true).toBe(false);",
                ),
                (
                    "mixed",
                    "error",
                    "",
                    "expect(value).toBe(2);",
                    "expect(true).toBe(false);",
                ),
            ] {
                cases.push((format!("{scope}-{name}"), expected, format!(
                    "const extended=test.extend({{value:[async({{}},use)=>{{{setup} await use(1); {cleanup}}},{{scope:'{scope}'}}]}}); extended('body',({{value}})=>{{{body}}});"
                )));
            }
        }
        for (name, expected, code) in [
            (
                "nested-around-body",
                "killed",
                "aroundEach(async run=>{await run();}); describe('inner',()=>{aroundEach(async run=>{await run();}); test('body',()=>expect(true).toBe(false));});",
            ),
            (
                "nested-around-cleanup",
                "error",
                "aroundEach(async run=>{await run(); expect(true).toBe(false);}); describe('inner',()=>{aroundEach(async run=>{await run();}); test('body',()=>{});});",
            ),
            (
                "nested-around-mixed",
                "error",
                "aroundEach(async run=>{await run();}); describe('inner',()=>{aroundEach(async run=>{await run(); expect(true).toBe(false);}); test('body',()=>expect(true).toBe(false));});",
            ),
            (
                "nested-suite-body",
                "killed",
                "aroundAll(async run=>{await run();}); describe('inner',()=>{aroundAll(async run=>{await run();}); test('body',()=>expect(true).toBe(false));});",
            ),
            (
                "nested-suite-cleanup",
                "error",
                "describe('inner',()=>{aroundAll(async run=>{await run(); expect(true).toBe(false);}); test('body',()=>{});});",
            ),
            (
                "nested-before-cleanup",
                "error",
                "beforeEach(()=>()=>expect(true).toBe(false)); describe('inner',()=>{beforeEach(()=>()=>{}); test('body',()=>{});});",
            ),
            (
                "concurrent-mixed",
                "error",
                "test.concurrent('body',({expect})=>expect(true).toBe(false)); test.concurrent('cleanup',({expect,onTestFinished})=>{onTestFinished(()=>expect(true).toBe(false));});",
            ),
            (
                "file-concurrent-body",
                "killed",
                "const extended=test.extend({value:[async({},use)=>{await use(1);},{scope:'file'}]}); extended.concurrent('body',({value,expect})=>expect(value).toBe(2)); extended.concurrent('pass',({value,expect})=>expect(value).toBe(1));",
            ),
        ] {
            cases.push((name.to_owned(), expected, code.to_owned()));
        }
        for (name, expected, code) in cases {
            fs::write(
                root.join("test/scoped.test.js"),
                format!(
                    "import {{test,expect,describe,beforeEach,aroundEach,aroundAll}} from 'vitest';\n{code}\n"
                ),
            )?;
            let execution = run(
                &Project::load(&root, &cancellation)?,
                &node,
                None,
                Duration::from_secs(20),
                NonZeroUsize::new(4 * 1024 * 1024),
                &cancellation,
            )
            .await?;
            let evidence = execution
                .evidence
                .as_ref()
                .map_err(|error| anyhow::anyhow!("{name}: {error:#}"))?;
            let classified = evidence.classify(execution.raw.status.code());
            println!("{name}: expected={expected}, observed={classified:?}");
            match expected {
                "killed" => assert_eq!(classified?, Outcome::Killed, "{name}"),
                "missed" => assert_eq!(classified?, Outcome::Missed, "{name}"),
                "error" => assert!(classified.is_err(), "{name}: {evidence:?}"),
                _ => unreachable!(),
            }
        }
        Ok(())
    }

    #[tokio::test]
    #[ignore = "requires prepared Vitest fixture and Node 24.22.0; real isolated runner matrix"]
    async fn real_fixture_outcomes_and_failure_evidence() -> Result<()> {
        let input = prepared()?;
        let cancellation = Cancellation::default();
        let work = Workspace::create(&input, &[], &cancellation)?;
        let root = work.path();
        let node = resolve_node(&root, &cancellation).await?;
        let baseline = execute(&root, &node, None).await?;
        assert_eq!(baseline.classify()?, Outcome::Missed);
        // A valid-looking installed manifest is not proof that the CLI completed.
        let cli = root.join("node_modules/vitest/vitest.mjs");
        let original_cli = fs::read(&cli)?;
        for startup in ["process.exit(0);", "throw new Error('startup probe');"] {
            fs::write(&cli, startup)?;
            let incomplete = execute(&root, &node, None).await?;
            assert!(incomplete.classify().is_err());
        }
        fs::write(&cli, original_cli)?;
        let found = discovery::discover(&root, &Scope::new(&[], &[])?, &cancellation)?;
        assert_eq!(found.mutations.len(), 6);
        for (index, mutation) in found.mutations.into_iter().enumerate() {
            let expected = if index % 3 == 2 {
                Outcome::Missed
            } else {
                Outcome::Killed
            };
            let result = execute(&root, &node, Some(mutation.clone())).await?;
            assert_eq!(result.classify()?, expected, "mutation {index}");
            if expected == Outcome::Missed {
                let extension = if index < 3 { "js" } else { "ts" };
                let test = root.join(format!("test/flags.{extension}.test.{extension}"));
                let source = fs::read_to_string(&test)?;
                fs::write(
                    &test,
                    format!(
                        "{source}\ntest('stronger', () => expect(fallback(false)).toBe(false));\n"
                    ),
                )?;
                assert_eq!(
                    execute(&root, &node, Some(mutation)).await?.classify()?,
                    Outcome::Killed
                );
                fs::write(&test, source)?;
            }
        }
        let original = fs::read_to_string(root.join("src/flags.ts"))?;
        fs::write(root.join("src/flags.ts"), "export function invalid( {\n")?;
        let syntax = execute(&root, &node, None).await?;
        assert_eq!(
            syntax.classify().with_context(|| format!(
                "syntax evidence: {:?}\n{}",
                syntax.evidence,
                syntax.text.text().unwrap_or_default()
            ))?,
            Outcome::BuildFailed
        );
        fs::write(
            root.join("test/mixed.test.js"),
            "import {test,expect} from 'vitest'; test('mixed',()=>expect(true).toBe(false));",
        )?;
        assert!(execute(&root, &node, None).await?.classify().is_err());
        fs::remove_file(root.join("test/mixed.test.js"))?;
        fs::write(root.join("src/flags.ts"), original)?;
        for (name, code) in [
            ("import", "throw new Error('import-probe');"),
            (
                "before",
                "beforeEach(() => { throw new Error('hook-probe'); }); test('body',()=>expect(true).toBe(true));",
            ),
            (
                "after",
                "afterEach(() => { throw new Error('hook-probe'); }); test('body',()=>expect(true).toBe(true));",
            ),
            (
                "hook-cleanup",
                "beforeEach(() => () => { throw new Error('cleanup-probe'); }); test('body',()=>expect(true).toBe(true));",
            ),
            (
                "unhandled",
                "test('unhandled',async()=>{ Promise.reject(new Error('unhandled-probe')); await new Promise(r=>setTimeout(r,30)); });",
            ),
            ("early-exit", "process.exit(0);"),
            (
                "retry",
                "test('retry', {retry: 1}, () => expect(true).toBe(true));",
            ),
            (
                "per-test-timeout",
                "test('timeout override', {timeout: 10}, () => expect(true).toBe(true));",
            ),
        ] {
            fs::write(
                root.join("test/probe.test.js"),
                format!("import {{test,expect,beforeEach,afterEach}} from 'vitest';\n{code}\n"),
            )?;
            let result = execute(&root, &node, None).await?;
            assert!(result.classify().is_err(), "{name}: {:?}", result.evidence);
            fs::remove_file(root.join("test/probe.test.js"))?;
        }
        fs::write(
            root.join("test/noise.test.js"),
            "import {test} from 'vitest'; test('noise',()=>{ console.log('x'.repeat(200000)); process.stdout.write('{not completion JSON}'); });",
        )?;
        let noisy = execute(&root, &node, None).await?;
        assert_eq!(noisy.classify()?, Outcome::Missed);
        assert!(noisy.text.retention()?.omitted_bytes > 0);
        fs::remove_file(root.join("test/noise.test.js"))?;
        for extension in ["js", "ts"] {
            let path = root.join(format!("test/flags.{extension}.test.{extension}"));
            fs::write(
                &path,
                fs::read_to_string(&path)?.replace("test(", "test.skip("),
            )?;
        }
        assert!(execute(&root, &node, None).await?.classify().is_err());
        work.reset(&input, &cancellation)?;
        fs::remove_dir_all(root.join("test"))?;
        assert!(execute(&root, &node, None).await?.classify().is_err());
        Ok(())
    }
}

#[cfg(all(test, unix))]
mod reliability_tests {
    use super::*;
    use crate::{report::Outcome, vitest::resolve_node};
    #[tokio::test]
    #[ignore = "requires prepared Vitest fixture and Node 24.22.0; real cleanup and output probes"]
    async fn real_timeout_cancellation_capture_and_isolation() -> Result<()> {
        let source = std::env::var_os("MUTIE_VITEST_FIXTURE")
            .map(std::path::PathBuf::from)
            .context("set MUTIE_VITEST_FIXTURE")?;
        let work = Workspace::create(&source, &[], &Cancellation::default())?;
        let root = work.path();
        let node = resolve_node(&root, &Cancellation::default()).await?;
        let original = fs::read(root.join("src/flags.js"))?;
        for mode in ["timeout", "cancel", "capture"] {
            let control = tempfile::tempdir()?;
            let checkpoint = control.path().join("checkpoint.json");
            let checkpoint_js = serde_json::to_string(&checkpoint)?;
            let finish = if mode == "capture" {
                "process.stdout.write('x'.repeat(65536));"
            } else {
                ""
            };
            fs::write(
                root.join("test/probe.test.js"),
                format!(
                    r#"
import {{test}} from 'vitest';
import {{writeFileSync}} from 'node:fs';
import {{spawn}} from 'node:child_process';
import {{join}} from 'node:path';
test('owned files and descendant', async () => {{
  const child=spawn(process.execPath,['-e','setInterval(()=>{{}},1000)'],{{stdio:'ignore'}});
  writeFileSync('src/owned-marker.txt','owned');
  writeFileSync('node_modules/owned-marker.txt','owned');
  writeFileSync(join(process.env.TMPDIR,'leftover'),'owned');
  writeFileSync({checkpoint_js}, JSON.stringify({{pid:child.pid,root:process.cwd(),temp:process.env.TMPDIR}}));
  {finish}
  await new Promise(()=>{{}});
}}, 60000);
"#
                ),
            )?;
            let project = Project::load(&root, &Cancellation::default())?;
            let cancellation = Cancellation::default();
            let observe = async {
                let deadline = tokio::time::Instant::now() + Duration::from_secs(20);
                while !checkpoint.exists() && tokio::time::Instant::now() < deadline {
                    tokio::time::sleep(Duration::from_millis(20)).await;
                }
                ensure!(checkpoint.exists(), "{mode} test did not start");
                if mode == "cancel" {
                    cancellation.cancel();
                }
                Ok::<(), anyhow::Error>(())
            };
            let limit = if mode == "timeout" {
                Duration::from_secs(4)
            } else {
                Duration::from_secs(20)
            };
            let cap = NonZeroUsize::new(if mode == "capture" {
                8192
            } else {
                4 * 1024 * 1024
            })
            .unwrap();
            let (result, observed) = tokio::join!(
                run(&project, &node, None, limit, Some(cap), &cancellation),
                observe
            );
            observed?;
            let result = result?.into_command(None);
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
            let pid = checkpoint["pid"].as_i64().context("child pid missing")? as i32;
            let deadline = tokio::time::Instant::now() + Duration::from_secs(3);
            while unsafe { libc::kill(pid, 0) } == 0 && tokio::time::Instant::now() < deadline {
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
            assert_ne!(
                unsafe { libc::kill(pid, 0) },
                0,
                "descendant {pid} remains after {mode}"
            );
            assert!(!root.join("src/owned-marker.txt").exists());
            assert!(!root.join("node_modules/owned-marker.txt").exists());
            assert_eq!(fs::read(root.join("src/flags.js"))?, original);
        }
        Ok(())
    }
}

pub(super) fn settings(root: &Path, cache: &Path, reporter: &Path) -> serde_json::Value {
    serde_json::json!({
        "root": root, "envDir": false, "cacheDir": cache,
        "test": {"reporters": [reporter], "environment":"node", "pool":"forks",
            "maxWorkers":1, "fileParallelism":false, "isolate":true, "watch":false,
            "passWithNoTests":false, "allowOnly":false, "retry":0, "bail":0,
            "coverage":{"enabled":false}, "update":false, "testTimeout":0, "hookTimeout":0}
    })
}
