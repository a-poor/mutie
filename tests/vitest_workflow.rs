//! Real pinned Vitest installations are opt-in; never recurse in self-mutation tests.
#![cfg(unix)]
use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};

#[path = "support/vitest_dismissals.rs"]
mod review;

fn run(root: &Path, reports: &Path, args: &[&str]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_mutie"));
    command.arg(root).args([
        "--lang",
        "vitest",
        "--ui",
        "plain",
        "--timeout",
        "20",
        "--baseline-timeout",
        "30",
    ]);
    if !args.contains(&"--list") {
        command.arg("--output-dir").arg(reports);
    }
    command.args(args).output().unwrap()
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
#[test]
#[ignore = "requires MUTIE_VITEST_FIXTURE with pinned installed dependencies and Node 24.22.0"]
fn scored_runs_saved_evidence_stronger_tests_and_zero_exit_failure() {
    let prepared =
        PathBuf::from(std::env::var_os("MUTIE_VITEST_FIXTURE").expect("set MUTIE_VITEST_FIXTURE"));
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("project with spaces");
    assert!(
        Command::new("cp")
            .arg("-R")
            .arg(&prepared)
            .arg(&root)
            .status()
            .unwrap()
            .success()
    );
    let reports = temp.path().join("reports");
    let before: Vec<_> = [
        "src/flags.js",
        "src/flags.ts",
        "pnpm-lock.yaml",
        "package.json",
    ]
    .into_iter()
    .map(|p| (p, fs::read(root.join(p)).unwrap()))
    .collect();
    let output = run(&root, &reports, &["--max-log-bytes", "32"]);
    assert!(output.status.success(), "{output:?}");
    let (directory, saved, inventory, outcomes) = report(&output);
    assert_eq!(saved["status"], "completed");
    assert_eq!(saved["score"]["killed"], 4);
    assert_eq!(saved["score"]["missed"], 2);
    assert!((saved["score"]["percentage"].as_f64().unwrap() - 200.0 / 3.0).abs() < 0.01);
    assert_eq!(saved["effective_config"]["lang"], "vitest");
    assert_eq!(saved["vitest_scope"]["node"], "24.22.0");
    assert_eq!(saved["vitest_scope"]["vitest"], "4.1.9");
    assert_eq!(saved["vitest_scope"]["config"]["test"]["retry"], 0);
    assert!(
        saved["vitest_scope"]["reporter_fingerprint"]
            .as_str()
            .unwrap()
            .starts_with("sha256:")
    );
    assert_eq!(inventory["mutations"].as_array().unwrap().len(), 6);
    assert_eq!(outcomes["summary"]["killed"], 4);
    assert_eq!(outcomes["summary"]["missed"], 2);
    assert_eq!(outcomes["summary"]["errors"], 0);
    assert_eq!(outcomes["baseline"]["outcome"], "passed");
    for mutation in outcomes["mutations"].as_array().unwrap() {
        assert!(
            directory
                .join(mutation["logs"]["text"].as_str().unwrap())
                .is_file()
        );
        assert!(
            mutation["logs"]["stdout"]
                .as_str()
                .unwrap()
                .ends_with(".log")
        );
    }
    for (path, bytes) in &before {
        assert_eq!(fs::read(root.join(path)).unwrap(), *bytes);
    }
    assert!(!root.join(".mutie").exists());
    let browse = || {
        Command::new(env!("CARGO_BIN_EXE_mutie"))
            .arg("report")
            .arg(&directory)
            .output()
            .unwrap()
    };
    // Loading happens before terminal setup: this proves real reports pass the
    // shared reader without requiring a terminal or reopening the input project.
    let browser = browse();
    assert!(
        String::from_utf8_lossy(&browser.stderr).contains("requires interactive terminal"),
        "{browser:?}"
    );
    let run_bytes = fs::read(directory.join("run.json")).unwrap();
    let mut corrupted = saved.clone();
    corrupted["vitest_scope"]["manifest_fingerprint"] = "bad".into();
    fs::write(
        directory.join("run.json"),
        serde_json::to_vec(&corrupted).unwrap(),
    )
    .unwrap();
    assert!(String::from_utf8_lossy(&browse().stderr).contains("invalid saved Vitest fingerprint"));
    fs::write(directory.join("run.json"), &run_bytes).unwrap();
    let original_report = fs::read(directory.join("outcomes.json")).unwrap();
    let test = root.join("test/flags.js.test.js");
    let original = fs::read_to_string(&test).unwrap();
    fs::write(
        &test,
        format!("{original}\ntest('stronger',()=>expect(fallback(false)).toBe(false));"),
    )
    .unwrap();
    let output = run(&root, &reports, &["--include", "src/flags.js"]);
    assert!(output.status.success(), "{output:?}");
    let (_, _, _, strengthened) = report(&output);
    assert_eq!(strengthened["summary"]["killed"], 3);
    assert_eq!(strengthened["summary"]["missed"], 0);
    assert_eq!(
        fs::read(directory.join("outcomes.json")).unwrap(),
        original_report
    );
    let rerun_args = ["--rerun-missed", directory.to_str().unwrap()];
    let output = run(&root, &reports, &rerun_args);
    assert!(output.status.success(), "{output:?}");
    let (_, rerun, selected, results) = report(&output);
    assert_eq!(selected["mutations"].as_array().unwrap().len(), 2);
    assert_eq!(results["summary"]["killed"], 1);
    assert_eq!(results["summary"]["missed"], 1);
    assert_eq!(rerun["score"]["scope"], "rerun");
    assert_eq!(
        rerun["rerun"]["mutations"],
        serde_json::json!([{"id":1,"source_id":3},{"id":2,"source_id":6}])
    );
    let ts_test = root.join("test/flags.ts.test.ts");
    let ts_original = fs::read_to_string(&ts_test).unwrap();
    fs::write(
        &ts_test,
        format!("{ts_original}\ntest('stronger',()=>expect(fallback(false)).toBe(false));"),
    )
    .unwrap();
    let output = run(
        &root,
        &reports,
        &["--rerun-missed", directory.to_str().unwrap(), "--id", "6"],
    );
    assert!(output.status.success(), "{output:?}");
    let (_, rerun, selected, results) = report(&output);
    assert_eq!(selected["mutations"].as_array().unwrap().len(), 1);
    assert_eq!(results["summary"]["killed"], 1);
    assert_eq!(results["summary"]["missed"], 0);
    assert_eq!(rerun["rerun"]["mutations"][0]["source_id"], 6);
    assert_eq!(
        fs::read(directory.join("outcomes.json")).unwrap(),
        original_report
    );
    assert_eq!(fs::read(directory.join("run.json")).unwrap(), run_bytes);
    for file in ["package.json", "pnpm-lock.yaml", "tsconfig.json"] {
        let path = root.join(file);
        let original = fs::read(&path).ok();
        let modified = match &original {
            Some(bytes) => {
                let mut bytes = bytes.clone();
                bytes.push(b'\n');
                bytes
            }
            None => b"{}".to_vec(),
        };
        fs::write(&path, modified).unwrap();
        let output = run(&root, &reports, &rerun_args);
        assert!(!output.status.success(), "{file}: {output:?}");
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("identity changed"),
            "{file}: {output:?}"
        );
        let (_, failed, _, results) = report(&output);
        assert_eq!(failed["status"], "failed");
        assert!(results["baseline"].is_null());
        match original {
            Some(bytes) => fs::write(&path, bytes).unwrap(),
            None => fs::remove_file(&path).unwrap(),
        }
    }
    // Source edits invalidate saved IDs even if a boolean remains at the same location.
    let source = root.join("src/flags.js");
    let original = fs::read_to_string(&source).unwrap();
    fs::write(&source, format!("{original}\n// edited source\n")).unwrap();
    let output = run(
        &root,
        &reports,
        &[
            "--list",
            "--rerun",
            directory.to_str().unwrap(),
            "--id",
            "3",
        ],
    );
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("source fingerprint changed"),
        "{output:?}"
    );
    fs::write(source, original).unwrap();
    let output = run(
        &root,
        &reports,
        &[
            "--list",
            "--rerun-missed",
            directory.to_str().unwrap(),
            "--id",
            "1",
        ],
    );
    assert!(output.status.success(), "{output:?}");
    assert!(String::from_utf8_lossy(&output.stdout).contains("Found 0 mutations"));
    fs::write(&test, "import {test,expect} from 'vitest'; import {enabled} from '../src/flags.js'; test('early mutation exit',()=>{if (!enabled()) process.exit(0); expect(enabled()).toBe(true);});").unwrap();
    let output = run(&root, &reports, &["--include", "src/flags.js"]);
    assert!(!output.status.success());
    let (_, failed, _, failed_outcomes) = report(&output);
    assert_eq!(failed["status"], "failed");
    assert_eq!(failed_outcomes["summary"]["errors"], 1);
    assert_eq!(failed_outcomes["mutations"][0]["outcome"], "error");
    // Successful process exit without a completed runner is an infrastructure failure.
    fs::write(
        root.join("node_modules/vitest/vitest.mjs"),
        "console.log('early exit diagnostic'); process.exit(0);",
    )
    .unwrap();
    let output = run(&root, &reports, &[]);
    assert!(!output.status.success());
    assert!(!String::from_utf8_lossy(&output.stdout).contains("Baseline passed"));
    let (directory, saved, inventory, outcomes) = report(&output);
    assert_eq!(saved["status"], "failed");
    assert_eq!(outcomes["baseline"]["outcome"], "failed");
    assert!(inventory["mutations"].as_array().unwrap().is_empty());
    let log = directory.join(outcomes["baseline"]["logs"]["text"].as_str().unwrap());
    assert!(
        fs::read_to_string(log)
            .unwrap()
            .contains("early exit diagnostic")
    );
}

#[test]
fn unsupported_vitest_options_fail_before_execution() {
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join("package.json"), "{}").unwrap();
    for (args, message) in [
        (vec!["--jobs", "2"], "only --jobs 1"),
        (vec!["--test-scope", "affected"], "only full test scope"),
        (
            vec!["--rust-build-cache", "worker"],
            "only supported for Rust",
        ),
    ] {
        let output = run(root.path(), &root.path().join("reports"), &args);
        assert!(!output.status.success());
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(message),
            "{output:?}"
        );
        assert!(!root.path().join("reports").exists());
    }
    for action in ["list", "check"] {
        let output = Command::new(env!("CARGO_BIN_EXE_mutie"))
            .args(["dismiss", "--lang", "vitest", "--module"])
            .arg(root.path())
            .arg(action)
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
    }
}
