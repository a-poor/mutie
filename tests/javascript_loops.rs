//! Opt-in scored loops controls for each independently supported JS/TS runner.
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
    assert!(output.status.success(), "{output:?}");
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
        fs::write(root.join("package.json"), serde_json::to_vec(&serde_json::json!({"name":"loops-controls", "type":"module", "scripts":{"test":script}})).unwrap()).unwrap();
    }
    fs::create_dir_all(root.join("src")).unwrap();
    fs::create_dir_all(root.join("test")).unwrap();
    let param = if syntax == "ts" { "a: any" } else { "a" };
    let source = format!(
        "export function choose({param}) {{ let n=0; while(a.pop()) {{ n++; }} return n; }}\nexport function weak({param}) {{ let n=0; for(let i=0; i<a.length; i++) {{ n++; }} return n; }}\nexport const enabled = true;\n"
    );
    let source_path = root.join(format!("src/loops.{syntax}"));
    fs::write(&source_path, &source).unwrap();
    let import = match runner {
        "node" => "import {test} from 'node:test'; import assert from 'node:assert/strict';",
        "bun" => "import {test} from 'bun:test'; import assert from 'node:assert/strict';",
        _ => {
            "import {test, expect} from 'vitest'; const assert = {equal: (a,b) => expect(a).toBe(b)};"
        }
    };
    let test = format!(
        "{import}\nimport * as c from '../src/loops.{syntax}';\ntest('loop values', () => {{ assert.equal(c.choose([1]), 1); assert.equal(c.choose([]), 0); assert.equal(c.weak([]),0); assert.equal(c.enabled,true); }});\n"
    );
    let test_path = root.join(format!("test/loops.test.{syntax}"));
    fs::write(&test_path, &test).unwrap();
    let reports = temp.path().join("reports");
    let (directory, saved, inventory, outcomes) =
        read(&run(&root, &reports, runner, &["--operator", "loop_skip"]));
    assert_eq!(saved["status"], "completed");
    assert_eq!(inventory["mutations"].as_array().unwrap().len(), 2);
    assert_eq!(outcomes["summary"]["killed"], 1);
    assert_eq!(outcomes["summary"]["missed"], 1);
    assert_eq!(outcomes["summary"]["errors"], 0);
    let original_run = fs::read(directory.join("run.json")).unwrap();
    let original_outcomes = fs::read(directory.join("outcomes.json")).unwrap();
    let original_inventory = fs::read(directory.join("mutants.json")).unwrap();
    let (filtered, _, selected, _) =
        read(&run(&root, &reports, runner, &["--operator", "loop_skip"]));
    assert_eq!(selected["mutations"].as_array().unwrap().len(), 2);
    assert!(filtered.exists());
    fs::write(
        &test_path,
        format!("{test}\ntest('nonempty loop', () => {{ assert.equal(c.weak([1]),1); }});\n"),
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
    // An ungrouped or arbitrary expression must not enter saved reruns.
    let mut corrupt = inventory;
    corrupt["mutations"][0]["replacement"] = "(true)".into();
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
    fs::write(directory.join("mutants.json"), &original_inventory).unwrap();
    let with_hook = test
        .replace("{test}", "{test, beforeEach}")
        .replace("{test, expect}", "{test, expect, beforeEach}");
    fs::write(&test_path, format!("{with_hook}\nbeforeEach(() => {{ if (c.weak([1]) === 0) throw new Error('fixture setup'); }});\n")).unwrap();
    // The same surviving skipped loop now triggers setup failure: it must remain an
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
    assert_eq!(fs::read(&source_path).unwrap(), source.as_bytes());
}
#[test]
#[ignore = "requires pinned Node"]
fn node_javascript_loops() {
    workflow("node", "js");
}
#[test]
#[ignore = "requires pinned Node"]
fn node_typescript_loops() {
    workflow("node", "ts");
}
#[test]
#[ignore = "requires pinned Bun"]
fn bun_javascript_loops() {
    workflow("bun", "js");
}
#[test]
#[ignore = "requires pinned Bun"]
fn bun_typescript_loops() {
    workflow("bun", "ts");
}
#[test]
#[ignore = "requires pinned Node and MUTIE_VITEST_FIXTURE"]
fn vitest_javascript_loops() {
    workflow("vitest", "js");
}
#[test]
#[ignore = "requires pinned Node and MUTIE_VITEST_FIXTURE"]
fn vitest_typescript_loops() {
    workflow("vitest", "ts");
}
