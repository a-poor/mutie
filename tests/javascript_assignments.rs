//! Opt-in scored assignments controls for each independently supported JS/TS runner.
#![cfg(unix)]
#[path = "support/invocation.rs"]
mod invocation;
#[allow(unused_imports)]
use invocation::{Child, Command};

use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
    process::Output,
};

fn run(root: &Path, reports: &Path, runner: &str, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_mutie"))
        .arg(root)
        .args([
            "--lang",
            runner,
            "--ui",
            "plain",
            "--timeout",
            "20",
            "--baseline-timeout",
            "30",
            "--output-dir",
        ])
        .arg(reports)
        .args(args)
        .output()
        .unwrap()
}
fn read(output: &Output) -> (PathBuf, Value, Value, Value) {
    assert!(
        output.status.success() || output.status.code() == Some(3),
        "{output:?}"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let directory = PathBuf::from(
        stdout
            .lines()
            .find_map(|s| s.strip_prefix("Saving report to "))
            .unwrap(),
    );
    let json = |name| serde_json::from_slice(&fs::read(directory.join(name)).unwrap()).unwrap();
    (
        directory.clone(),
        json("run.json"),
        json("mutants.json"),
        json("outcomes.json"),
    )
}
fn workflow(runner: &str, syntax: &str) {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("project with spaces");
    if runner == "vitest" {
        let prepared = std::env::var_os("MUTIE_VITEST_FIXTURE").expect("set MUTIE_VITEST_FIXTURE");
        assert!(
            Command::new("cp")
                .arg("-R")
                .arg(prepared)
                .arg(&root)
                .status()
                .unwrap()
                .success()
        );
        for name in ["src", "test", "tests"] {
            if root.join(name).exists() {
                fs::remove_dir_all(root.join(name)).unwrap();
            }
        }
    } else {
        fs::create_dir(&root).unwrap();
        let script = if runner == "node" {
            format!("node --test test/*.test.{syntax}")
        } else {
            "bun test".into()
        };
        fs::write(root.join("package.json"), serde_json::to_vec(&serde_json::json!({"name":"assignments-controls", "type":"module", "scripts":{"test":script}})).unwrap()).unwrap();
    }
    fs::create_dir_all(root.join("src")).unwrap();
    fs::create_dir_all(root.join("test")).unwrap();
    let param = if syntax == "ts" { "a: any" } else { "a" };
    let source = format!(
        "export function choose({param}) {{ a.value = 1; return a.value; }}\nexport function weak({param}) {{ a['value'] = 1; return a; }}\nexport const enabled = true;\nexport class Initialized {{ constructor() {{ this.initial = 7; }} }}\n"
    );
    let source_path = root.join(format!("src/assignments.{syntax}"));
    fs::write(&source_path, &source).unwrap();
    let import = match runner {
        "node" => "import {test} from 'node:test'; import assert from 'node:assert/strict';",
        "bun" => "import {test} from 'bun:test'; import assert from 'node:assert/strict';",
        _ => {
            "import {test, expect} from 'vitest'; const assert = {equal: (a,b) => expect(a).toBe(b)};"
        }
    };
    let test = format!(
        "{import}\nimport * as c from '../src/assignments.{syntax}';\ntest('assignment effects', () => {{ assert.equal(c.choose({{value:0}}), 1); assert.equal(typeof c.weak({{value:0}}),'object'); assert.equal(c.enabled,true); }});\n"
    );
    let test_path = root.join(format!("test/assignments.test.{syntax}"));
    fs::write(&test_path, &test).unwrap();
    let reports = temp.path().join("reports");
    let (directory, saved, inventory, outcomes) = read(&run(
        &root,
        &reports,
        runner,
        &["--operator", "assignment_remove"],
    ));
    assert_eq!(saved["status"], "completed");
    assert_eq!(saved["score"]["percentage"], 50.0);
    assert_eq!(inventory["mutations"].as_array().unwrap().len(), 2);
    assert_eq!(outcomes["summary"]["killed"], 1);
    assert_eq!(outcomes["summary"]["missed"], 1);
    assert_eq!(outcomes["summary"]["errors"], 0);
    let original_run = fs::read(directory.join("run.json")).unwrap();
    let original_outcomes = fs::read(directory.join("outcomes.json")).unwrap();
    let original_inventory = fs::read(directory.join("mutants.json")).unwrap();
    let policy = run(
        &root,
        &reports,
        runner,
        &["--operator", "assignment_remove", "--min-score", "100"],
    );
    assert_eq!(policy.status.code(), Some(3), "{policy:?}");
    let (filtered, policy_run, selected, _) = read(&policy);
    assert_eq!(policy_run["score"]["percentage"], 50.0);
    assert_eq!(selected["mutations"].as_array().unwrap().len(), 2);
    assert!(filtered.exists());
    fs::write(
        &test_path,
        format!(
            "{test}\ntest('stored value', () => {{ assert.equal(c.weak({{value:0}}).value,1); }});\n"
        ),
    )
    .unwrap();
    let (_, _, focused, result) = read(&run(
        &root,
        &reports,
        runner,
        &["--rerun-missed", directory.to_str().unwrap()],
    ));
    assert_eq!(focused["mutations"].as_array().unwrap().len(), 1);
    assert_eq!(result["summary"]["killed"], 1);
    assert_eq!(fs::read(&source_path).unwrap(), source.as_bytes());
    assert_eq!(fs::read(directory.join("run.json")).unwrap(), original_run);
    assert_eq!(
        fs::read(directory.join("outcomes.json")).unwrap(),
        original_outcomes
    );
    assert_eq!(
        fs::read(directory.join("mutants.json")).unwrap(),
        original_inventory
    );
    // Reject a changed recipe or a store mislabeled as a call, despite both
    // statement operators using the same semicolon replacement.
    for (key, value) in [("replacement", "(true)"), ("operator", "call_remove")] {
        let mut corrupt = inventory.clone();
        corrupt["mutations"][0][key] = value.into();
        fs::write(
            directory.join("mutants.json"),
            serde_json::to_vec(&corrupt).unwrap(),
        )
        .unwrap();
        let output = run(
            &root,
            &reports,
            runner,
            &["--rerun", directory.to_str().unwrap()],
        );
        assert!(!output.status.success());
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("outside the supported scope"),
            "{output:?}"
        );
    }
    // Isolated recipe parsing cannot know the surrounding constructor. Fresh
    // discovery must reject even a real store with correct source coordinates.
    let mut corrupt = inventory.clone();
    let text = "this.initial = 7;";
    let start = source.find(text).unwrap();
    let before = &source[..start];
    let line = before.bytes().filter(|b| *b == b'\n').count() + 1;
    let column = before.rsplit('\n').next().unwrap().len() + 1;
    let mutation = &mut corrupt["mutations"][0];
    mutation["original"] = text.into();
    mutation["byte_start"] = start.into();
    mutation["byte_end"] = (start + text.len()).into();
    mutation["line"] = line.into();
    mutation["end_line"] = line.into();
    mutation["column"] = column.into();
    mutation["end_column"] = (column + text.len()).into();
    fs::write(
        directory.join("mutants.json"),
        serde_json::to_vec(&corrupt).unwrap(),
    )
    .unwrap();
    let output = run(
        &root,
        &reports,
        runner,
        &["--rerun", directory.to_str().unwrap(), "--id", "1"],
    );
    assert!(!output.status.success(), "{output:?}");
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("no longer matches a discovered mutation"),
        "{output:?}"
    );
    fs::write(directory.join("mutants.json"), &original_inventory).unwrap();
    let with_hook = test
        .replace("{test}", "{test, beforeEach}")
        .replace("{test, expect}", "{test, expect, beforeEach}");
    fs::write(&test_path, format!("{with_hook}\nbeforeEach(() => {{ if (c.weak({{value:0}}).value === 0) throw new Error('fixture setup'); }});\n")).unwrap();
    // The same surviving removed store now triggers setup failure: it must remain an
    // error, even though the stronger body assertion established a genuine kill.
    let output = run(
        &root,
        &reports,
        runner,
        &["--rerun-missed", directory.to_str().unwrap(), "--id", "2"],
    );
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let failed = PathBuf::from(
        stdout
            .lines()
            .find_map(|s| s.strip_prefix("Saving report to "))
            .unwrap(),
    );
    let outcomes: Value =
        serde_json::from_slice(&fs::read(failed.join("outcomes.json")).unwrap()).unwrap();
    assert_eq!(outcomes["summary"]["errors"], 1);
    assert_eq!(outcomes["summary"]["killed"], 0);
    let failed_run: Value =
        serde_json::from_slice(&fs::read(failed.join("run.json")).unwrap()).unwrap();
    assert_eq!(failed_run["status"], "failed");
    assert!(failed_run["score"]["percentage"].is_null());
    fs::write(&source_path, format!("{source}// changed source bytes\n")).unwrap();
    let stale = run(
        &root,
        &reports,
        runner,
        &["--rerun-missed", directory.to_str().unwrap()],
    );
    assert_eq!(stale.status.code(), Some(1), "{stale:?}");
    assert!(
        String::from_utf8_lossy(&stale.stderr).contains("source fingerprint changed"),
        "{stale:?}"
    );
    fs::write(&source_path, &source).unwrap();
    assert_eq!(fs::read(&source_path).unwrap(), source.as_bytes());
    assert_eq!(fs::read(directory.join("run.json")).unwrap(), original_run);
    assert_eq!(
        fs::read(directory.join("outcomes.json")).unwrap(),
        original_outcomes
    );
    assert_eq!(
        fs::read(directory.join("mutants.json")).unwrap(),
        original_inventory
    );
}
#[test]
#[ignore = "requires pinned Node"]
fn node_javascript_assignments() {
    workflow("node", "js");
}
#[test]
#[ignore = "requires pinned Node"]
fn node_typescript_assignments() {
    workflow("node", "ts");
}
#[test]
#[ignore = "requires pinned Bun"]
fn bun_javascript_assignments() {
    workflow("bun", "js");
}
#[test]
#[ignore = "requires pinned Bun"]
fn bun_typescript_assignments() {
    workflow("bun", "ts");
}
#[test]
#[ignore = "requires pinned Node and MUTIE_VITEST_FIXTURE"]
fn vitest_javascript_assignments() {
    workflow("vitest", "js");
}
#[test]
#[ignore = "requires pinned Node and MUTIE_VITEST_FIXTURE"]
fn vitest_typescript_assignments() {
    workflow("vitest", "ts");
}
