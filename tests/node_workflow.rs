//! Opt-in native Node runs, excluded from recursive self-mutation tests.
#![cfg(unix)]
use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};
fn fixture() -> (tempfile::TempDir, PathBuf) {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("project with spaces");
    fs::create_dir_all(root.join("src")).unwrap();
    fs::create_dir_all(root.join("test")).unwrap();
    for name in ["package.json", "src/flags.js", "test/flags.test.js"] {
        fs::copy(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/node")
                .join(name),
            root.join(name),
        )
        .unwrap();
    }
    (temp, root)
}
fn run(root: &Path, reports: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_mutie"))
        .arg(root)
        .args([
            "--lang",
            "node",
            "--ui",
            "plain",
            "--timeout",
            "5",
            "--baseline-timeout",
            "10",
        ])
        .arg("--output-dir")
        .arg(reports)
        .args(args)
        .output()
        .unwrap()
}
fn report(output: &Output) -> (PathBuf, Value, Value, Value) {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let path = PathBuf::from(
        stdout
            .lines()
            .find_map(|s| s.strip_prefix("Saving report to "))
            .unwrap_or_else(|| panic!("{output:?}")),
    );
    let read = |name| serde_json::from_slice(&fs::read(path.join(name)).unwrap()).unwrap();
    (
        path.clone(),
        read("run.json"),
        read("mutants.json"),
        read("outcomes.json"),
    )
}
fn dismiss(root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_mutie"))
        .args(["dismiss", "--module"])
        .arg(root)
        .args(["--lang", "node"])
        .args(args)
        .env("PATH", "")
        .output()
        .unwrap()
}
fn ok(output: Output) -> String {
    assert!(output.status.success(), "{output:?}");
    String::from_utf8(output.stdout).unwrap()
}
#[test]
#[ignore = "requires audited Node 24.22.0 on PATH"]
fn reports_reruns_review_policies_and_preservation() {
    let (temp, root) = fixture();
    let reports = temp.path().join("reports");
    let source = fs::read(root.join("src/flags.js")).unwrap();
    let manifest = fs::read(root.join("package.json")).unwrap();
    let output = run(
        &root,
        &reports,
        &["--fail-on-missed", "--max-log-bytes", "32"],
    );
    assert_eq!(output.status.code(), Some(3), "{output:?}");
    let (directory, saved, inventory, outcomes) = report(&output);
    assert_eq!(saved["status"], "completed");
    assert_eq!(saved["node_scope"]["node"], "24.22.0");
    assert_eq!(saved["score"]["killed"], 2);
    assert_eq!(saved["score"]["missed"], 1);
    assert_eq!(inventory["mutations"].as_array().unwrap().len(), 3);
    assert_eq!(outcomes["summary"]["errors"], 0);
    let original_report = fs::read(directory.join("outcomes.json")).unwrap();
    let browse = || {
        Command::new(env!("CARGO_BIN_EXE_mutie"))
            .arg("report")
            .arg(&directory)
            .output()
            .unwrap()
    };
    assert!(String::from_utf8_lossy(&browse().stderr).contains("requires interactive terminal"));
    let run_bytes = fs::read(directory.join("run.json")).unwrap();
    let mut corrupt = saved.clone();
    corrupt["node_scope"]["recipe_fingerprint"] = "bad".into();
    fs::write(
        directory.join("run.json"),
        serde_json::to_vec(&corrupt).unwrap(),
    )
    .unwrap();
    assert!(
        String::from_utf8_lossy(&browse().stderr).contains("unsupported saved Node runner recipe")
    );
    fs::write(directory.join("run.json"), &run_bytes).unwrap();
    ok(dismiss(
        &root,
        &[
            "add",
            directory.to_str().unwrap(),
            "--id",
            "3",
            "--reason",
            "reviewed gap",
        ],
    ));
    assert!(ok(dismiss(&root, &["check"])).contains("1 matched; 0 require review"));
    ok(dismiss(
        &root,
        &["update", "1", "--note", "fallback boundary"],
    ));
    assert!(ok(dismiss(&root, &["list"])).contains("fallback boundary"));
    let output = run(&root, &reports, &["--fail-on-missed", "--min-score", "100"]);
    assert_eq!(output.status.code(), Some(3), "{output:?}");
    let (_, reviewed, _, review_outcomes) = report(&output);
    assert_eq!(reviewed["policy"]["status"], "passed");
    assert_eq!(reviewed["score_policy"]["status"], "failed");
    assert_eq!(review_outcomes["summary"], outcomes["summary"]);
    ok(dismiss(&root, &["remove", "1"]));
    let test = root.join("test/flags.test.js");
    let old = fs::read_to_string(&test).unwrap();
    fs::write(
        &test,
        format!("{old}\ntest('stronger',()=>assert.equal(fallback(false),true));\n"),
    )
    .unwrap();
    let output = run(
        &root,
        &reports,
        &["--rerun-missed", directory.to_str().unwrap()],
    );
    assert!(output.status.success(), "{output:?}");
    let (_, rerun, selected, stronger) = report(&output);
    assert_eq!(selected["mutations"].as_array().unwrap().len(), 1);
    assert_eq!(stronger["summary"]["killed"], 1);
    assert_eq!(stronger["summary"]["missed"], 0);
    assert_eq!(rerun["score"]["scope"], "rerun");
    assert_eq!(
        fs::read(directory.join("outcomes.json")).unwrap(),
        original_report
    );
    assert_eq!(fs::read(root.join("src/flags.js")).unwrap(), source);
    assert_eq!(fs::read(root.join("package.json")).unwrap(), manifest);
    assert!(!root.join(".mutie").exists());
    // Test edits are allowed; execution manifest edits require a fresh scan.
    fs::write(
        root.join("package.json"),
        format!("{}\n", String::from_utf8(manifest).unwrap()),
    )
    .unwrap();
    let output = run(
        &root,
        &reports,
        &["--rerun-missed", directory.to_str().unwrap()],
    );
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("identity changed"),
        "{output:?}"
    );
}
#[test]
#[ignore = "requires audited Node 24.22.0 on PATH"]
fn mutation_triggered_lifecycle_errors_do_not_raise_scores() {
    let (temp, root) = fixture();
    let reports = temp.path().join("reports");
    fs::write(
        root.join("src/flags.js"),
        "export const enabled = () => true;\n",
    )
    .unwrap();
    let header = "import { test, before, after, beforeEach, afterEach } from 'node:test';\nimport assert from 'node:assert/strict';\nimport { enabled } from '../src/flags.js';\n";
    let cases = [
        ("body", "test('body',()=>assert.ok(enabled()));", "killed"),
        (
            "before",
            "before(()=>assert.ok(enabled())); test('body',()=>{});",
            "errors",
        ),
        (
            "after",
            "after(()=>assert.ok(enabled())); test('body',()=>{});",
            "errors",
        ),
        (
            "beforeEach",
            "beforeEach(()=>assert.ok(enabled())); test('body',()=>{});",
            "errors",
        ),
        (
            "afterEach",
            "afterEach(()=>assert.ok(enabled())); test('body',()=>{});",
            "errors",
        ),
        (
            "mixed",
            "afterEach(()=>assert.ok(enabled())); test('body',()=>assert.ok(enabled()));",
            "errors",
        ),
        (
            "async cleanup",
            "afterEach(async()=>{await Promise.resolve(); assert.ok(enabled())}); test('body',()=>assert.ok(enabled()));",
            "errors",
        ),
        (
            "callback cleanup",
            "afterEach((_t,done)=>done(enabled()?null:new Error('cleanup'))); test('body',()=>assert.ok(enabled()));",
            "errors",
        ),
        (
            "successful cleanup",
            "afterEach((_t,done)=>done()); test('body',()=>assert.ok(enabled()));",
            "killed",
        ),
        (
            "nested mixed",
            "test('parent',async t=>{t.afterEach(()=>assert.ok(enabled())); await t.test('child',()=>assert.ok(enabled()))});",
            "errors",
        ),
        (
            "nested body",
            "test('parent',async t=>{t.afterEach(()=>{}); await t.test('child',()=>assert.ok(enabled()))});",
            "killed",
        ),
        (
            "concurrent mixed",
            "test('parent',{concurrency:true},async t=>{t.afterEach(()=>assert.ok(enabled())); await Promise.all([t.test('child',()=>assert.ok(enabled())),t.test('other',()=>{})])});",
            "errors",
        ),
        (
            "plan failure",
            "test('body',t=>{t.plan(enabled()?1:2); t.assert.ok(true)});",
            "errors",
        ),
        (
            "generic error",
            "test('body',()=>{if(!enabled()) throw new TypeError('body')});",
            "errors",
        ),
        (
            "zero exit",
            "if(!enabled()) process.exit(0); test('body',()=>{});",
            "errors",
        ),
        (
            "cleanup exit",
            "after(()=>{if(!enabled()) process.exit(0)}); test('body',()=>{});",
            "errors",
        ),
        ("empty", "if(enabled()) test('body',()=>{});", "errors"),
        (
            "all skipped",
            "test('body',{skip:!enabled()},()=>{});",
            "errors",
        ),
        (
            "late error",
            "test('body',()=>{if(!enabled()) setImmediate(()=>{throw new Error('late')})});",
            "errors",
        ),
        (
            "import error",
            "if(!enabled()) throw new Error('import'); test('body',()=>{});",
            "errors",
        ),
    ];
    for (name, body, outcome) in cases {
        fs::write(root.join("test/flags.test.js"), format!("{header}{body}\n")).unwrap();
        let output = run(
            &root,
            &reports,
            &["--max-log-bytes", "16", "--min-score", "0"],
        );
        let (_, saved, _, result) = report(&output);
        assert_eq!(
            saved["status"],
            if outcome == "errors" {
                "failed"
            } else {
                "completed"
            },
            "{name}: {output:?}"
        );
        assert_eq!(result["summary"][outcome], 1, "{name}: {result}");
        assert_eq!(
            output.status.code(),
            Some(if outcome == "errors" { 1 } else { 0 }),
            "{name}: {output:?}"
        );
        assert_eq!(
            saved["score"]["killed"],
            if outcome == "killed" { 1 } else { 0 },
            "{name}"
        );
    }
}

#[test]
fn rejects_unsupported_scope_before_invoking_node() {
    let (temp, root) = fixture();
    let check = |args: &[&str], expected: &str| {
        let out = Command::new(env!("CARGO_BIN_EXE_mutie"))
            .arg(&root)
            .args(["--lang", "node", "--list", "--ui", "plain"])
            .args(args)
            .env("PATH", "")
            .output()
            .unwrap();
        assert!(!out.status.success());
        assert!(
            String::from_utf8_lossy(&out.stderr).contains(expected),
            "{out:?}"
        );
    };
    check(&["--jobs", "2"], "Node currently supports only --jobs 1");
    check(
        &["--test-scope", "affected"],
        "Node supports only full test scope",
    );
    check(
        &["--operator", "logical_swap"],
        "Node supports only boolean_flip",
    );
    fs::write(temp.path().join("package.json"), r#"{"workspaces":["*"]}"#).unwrap();
    check(&[], "enclosing workspace is unsupported");
    fs::remove_file(temp.path().join("package.json")).unwrap();
    fs::write(root.join("src/unsupported.ts"), "export const a=true;").unwrap();
    check(&[], "supports JavaScript .js only");
}
