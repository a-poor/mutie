//! Opt-in scored string controls for each independently supported JS/TS runner.
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
    let mut command = Command::new(env!("CARGO_BIN_EXE_mutie"));
    if !args.contains(&"--timeout") {
        command.args(["--timeout", "20"]);
    }
    command
        .arg(root)
        .args([
            "--lang",
            runner,
            "--ui",
            "plain",
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
        fs::write(root.join("package.json"), serde_json::to_vec(&serde_json::json!({"name":"strings-controls", "type":"module", "scripts":{"test":script}})).unwrap()).unwrap();
    }
    fs::create_dir_all(root.join("src")).unwrap();
    fs::create_dir_all(root.join("test")).unwrap();
    let source = "export function checked(){return 'checked';}\nexport function weak(){return '';}\nexport const unit='\\u0000';\nexport const keys={'hidden':true};\n";
    let source_path = root.join(format!("src/strings.{syntax}"));
    fs::write(&source_path, source).unwrap();
    let import = match runner {
        "node" => "import {test} from 'node:test'; import assert from 'node:assert/strict';",
        "bun" => "import {test} from 'bun:test'; import assert from 'node:assert/strict';",
        _ => "import {test, expect} from 'vitest'; const assert={equal:(a,b)=>expect(a).toBe(b)};",
    };
    let test = format!(
        "{import}\nimport * as c from '../src/strings.{syntax}';\ntest('values',()=>{{assert.equal(c.checked(),'checked'); assert.equal(typeof c.weak(),'string'); assert.equal(c.unit.length,1);}});\n"
    );
    let test_path = root.join(format!("test/strings.test.{syntax}"));
    fs::write(&test_path, &test).unwrap();
    let reports = temp.path().join("reports");
    let output = run(
        &root,
        &reports,
        runner,
        &["--operator", "string_replace", "--min-score", "100"],
    );
    assert_eq!(output.status.code(), Some(3), "{output:?}");
    let (directory, saved, inventory, outcomes) = read(&output);
    assert_eq!(saved["status"], "completed");
    assert_eq!(saved["score"]["killed"], 2);
    assert_eq!(saved["score"]["missed"], 1);
    assert!((saved["score"]["percentage"].as_f64().unwrap() - 200.0 / 3.0).abs() < 0.01);
    assert_eq!(saved["score_policy"]["status"], "failed");
    assert_eq!(outcomes["summary"]["killed"], 2);
    assert_eq!(outcomes["summary"]["missed"], 1);
    assert_eq!(outcomes["summary"]["errors"], 0);
    assert_eq!(inventory["mutations"].as_array().unwrap().len(), 3);
    assert_eq!(inventory["mutations"][1]["replacement"], "(\"x\")");
    let original_files: [Vec<u8>; 3] = ["run.json", "mutants.json", "outcomes.json"]
        .map(|name| fs::read(directory.join(name)).unwrap());
    let review = Command::new(env!("CARGO_BIN_EXE_mutie"))
        .args(["dismiss", "--lang", runner, "--module"])
        .arg(&root)
        .args([
            "add",
            directory.to_str().unwrap(),
            "--id",
            "2",
            "--reason",
            "pending string value test",
        ])
        .output()
        .unwrap();
    assert!(review.status.success(), "{review:?}");
    fs::write(
        &test_path,
        format!("{test}\ntest('empty value',()=>{{assert.equal(c.weak(),'')}});\n"),
    )
    .unwrap();
    let (_, _, focused, result) = read(&run(
        &root,
        &reports,
        runner,
        &["--rerun-missed", directory.to_str().unwrap()],
    ));
    assert_eq!(focused["mutations"].as_array().unwrap().len(), 1);
    assert_eq!(focused["dismissals"]["matched"][0]["mutation_id"], 1);
    assert_eq!(result["summary"]["killed"], 1);
    assert_eq!(result["summary"]["missed"], 0);
    let corrupt = temp.path().join("corrupt-report");
    assert!(
        Command::new("cp")
            .arg("-R")
            .arg(&directory)
            .arg(&corrupt)
            .status()
            .unwrap()
            .success()
    );
    for (key, value) in [
        ("replacement", "(\"\")"),
        ("replacement", "\"x\""),
        ("operator", "arithmetic_replace"),
    ] {
        let mut bad = inventory.clone();
        bad["mutations"][1][key] = value.into();
        fs::write(
            corrupt.join("mutants.json"),
            serde_json::to_vec(&bad).unwrap(),
        )
        .unwrap();
        let out = run(
            &root,
            &reports,
            runner,
            &["--rerun-missed", corrupt.to_str().unwrap(), "--dry-run"],
        );
        assert!(!out.status.success(), "{out:?}");
    }
    // An excluded property key has a valid string recipe in isolation. Only
    // fresh full-source discovery can establish that its context is unsupported.
    let key = "'hidden'";
    let start = source.find(key).unwrap();
    let before = &source[..start];
    let line = before.bytes().filter(|b| *b == b'\n').count() + 1;
    let column = before.rsplit('\n').next().unwrap().len() + 1;
    let mut bad = inventory.clone();
    let m = &mut bad["mutations"][1];
    m["original"] = key.into();
    m["replacement"] = "(\"\")".into();
    m["byte_start"] = start.into();
    m["byte_end"] = (start + key.len()).into();
    m["line"] = line.into();
    m["end_line"] = line.into();
    m["column"] = column.into();
    m["end_column"] = (column + key.len()).into();
    fs::write(
        corrupt.join("mutants.json"),
        serde_json::to_vec(&bad).unwrap(),
    )
    .unwrap();
    let out = run(
        &root,
        &reports,
        runner,
        &["--rerun-missed", corrupt.to_str().unwrap(), "--dry-run"],
    );
    assert!(!out.status.success(), "{out:?}");
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("no longer matches a discovered mutation"),
        "{out:?}"
    );
    fs::write(&source_path, format!("{source}// drift\n")).unwrap();
    let stale = run(
        &root,
        &reports,
        runner,
        &["--rerun-missed", directory.to_str().unwrap(), "--dry-run"],
    );
    assert!(
        String::from_utf8_lossy(&stale.stderr).contains("source fingerprint changed"),
        "{stale:?}"
    );
    fs::write(&source_path, source).unwrap();
    // A literal mutation can break setup: keep that failure out of killed counts.
    let hooks = match runner {
        "node" => "import {beforeEach} from 'node:test';",
        "bun" => "import {beforeEach} from 'bun:test';",
        _ => "import {beforeEach} from 'vitest';",
    };
    fs::write(&test_path,format!("{test}\n{hooks}\nbeforeEach(()=>{{if(c.checked()!=='checked')throw new Error('setup failure')}});\n")).unwrap();
    let failed = run(
        &root,
        &reports,
        runner,
        &["--operator", "string_replace", "--min-score", "100"],
    );
    assert_eq!(failed.status.code(), Some(1), "{failed:?}");
    let stdout = String::from_utf8_lossy(&failed.stdout);
    let failed_dir = PathBuf::from(
        stdout
            .lines()
            .find_map(|s| s.strip_prefix("Saving report to "))
            .unwrap(),
    );
    let failed_out: Value =
        serde_json::from_slice(&fs::read(failed_dir.join("outcomes.json")).unwrap()).unwrap();
    let failed_run: Value =
        serde_json::from_slice(&fs::read(failed_dir.join("run.json")).unwrap()).unwrap();
    assert_eq!(failed_out["summary"]["errors"], 1);
    assert_eq!(failed_out["summary"]["killed"], 0);
    assert!(failed_run["score"]["percentage"].is_null());
    fs::write(&test_path,format!("{import}\nimport * as c from '../src/strings.{syntax}';\ntest('bounded hang',()=>{{while(c.checked()===''){{}}}});\n")).unwrap();
    let (_, timed_run, _, timed) = read(&run(
        &root,
        &reports,
        runner,
        &[
            "--rerun",
            directory.to_str().unwrap(),
            "--id",
            "1",
            "--timeout",
            "3",
        ],
    ));
    assert_eq!(timed["summary"]["timeout"], 1);
    assert_eq!(timed["summary"]["killed"], 0);
    assert_eq!(timed_run["score"]["timeout"], 1);
    assert_eq!(
        timed_run["score"]["unavailable_reason"],
        "no_scored_mutations"
    );
    assert!(timed_run["score"]["percentage"].is_null());
    assert_eq!(fs::read_to_string(&source_path).unwrap(), source);
    for (name, bytes) in ["run.json", "mutants.json", "outcomes.json"]
        .iter()
        .zip(original_files)
    {
        assert_eq!(fs::read(directory.join(name)).unwrap(), bytes);
    }
}

#[test]
#[ignore = "requires pinned Node"]
fn node_javascript_strings() {
    workflow("node", "js");
}
#[test]
#[ignore = "requires pinned Node"]
fn node_typescript_strings() {
    workflow("node", "ts");
}
#[test]
#[ignore = "requires pinned Bun"]
fn bun_javascript_strings() {
    workflow("bun", "js");
}
#[test]
#[ignore = "requires pinned Bun"]
fn bun_typescript_strings() {
    workflow("bun", "ts");
}
#[test]
#[ignore = "requires prepared pinned Vitest fixture"]
fn vitest_javascript_strings() {
    workflow("vitest", "js");
}
#[test]
#[ignore = "requires prepared pinned Vitest fixture"]
fn vitest_typescript_strings() {
    workflow("vitest", "ts");
}
