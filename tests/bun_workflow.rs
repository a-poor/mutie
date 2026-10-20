//! Opt-in real Bun scored workflows; never recursively run during self-mutation.
#![cfg(unix)]
use serde_json::Value;
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};
fn fixture(syntax: &str) -> (tempfile::TempDir, PathBuf) {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("project with spaces");
    fs::create_dir_all(root.join("src")).unwrap();
    fs::create_dir(root.join("test")).unwrap();
    let name = if syntax == "js" {
        "bun-js"
    } else {
        "bun-typescript"
    };
    for file in [
        "package.json".to_owned(),
        format!("src/flags.{syntax}"),
        format!("test/flags.test.{syntax}"),
    ] {
        fs::copy(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures")
                .join(name)
                .join(&file),
            root.join(file),
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
            "bun",
            "--ui",
            "plain",
            "--timeout",
            "5",
            "--baseline-timeout",
            "10",
            "--output-dir",
        ])
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
#[test]
fn unsupported_bun_options_fail_before_launching_runtime() {
    let (_temp, root) = fixture("js");
    for args in [
        vec!["--jobs", "2"],
        vec!["--test-scope", "affected"],
        vec!["--clear-max-capture-bytes"],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_mutie"))
            .arg(&root)
            .args(["--lang", "bun", "--list"])
            .args(args)
            .env("PATH", "")
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("Bun"),
            "{output:?}"
        );
    }
}
#[test]
fn excluded_report_directories_cannot_hide_bun_tests() {
    let (_temp, root) = fixture("js");
    let reports = root.join("test/reports");
    fs::create_dir(&reports).unwrap();
    fs::write(
        reports.join("hidden.test.js"),
        "import {test} from 'bun:test'; test('hidden',()=>{});",
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_mutie"))
        .arg(&root)
        .args(["--lang", "bun", "--output-dir"])
        .arg(&reports)
        .env("PATH", "")
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("report exclusions cannot hide executable inputs"),
        "{output:?}"
    );
    assert!(reports.join("hidden.test.js").exists());
}

#[test]
#[ignore = "requires pinned Bun 1.3.11 on PATH; real JS/TS scored runs and focused stronger-test reruns"]
fn scored_js_ts_reports_reruns_and_preservation() {
    for syntax in ["js", "ts"] {
        let (temp, root) = fixture(syntax);
        let reports = temp.path().join("reports");
        let source = root.join(format!("src/flags.{syntax}"));
        let before = fs::read(&source).unwrap();
        let output = run(
            &root,
            &reports,
            &["--fail-on-missed", "--max-log-bytes", "32"],
        );
        assert_eq!(output.status.code(), Some(3), "{output:?}");
        let (directory, saved, inventory, outcomes) = report(&output);
        assert_eq!(saved["status"], "completed");
        assert_eq!(saved["bun_scope"]["runtime_version"], "1.3.11");
        assert_eq!(saved["score"]["killed"], 1);
        assert_eq!(saved["score"]["missed"], 1);
        assert_eq!(inventory["mutations"].as_array().unwrap().len(), 2);
        assert_eq!(outcomes["summary"]["errors"], 0);
        let original_report = fs::read(directory.join("outcomes.json")).unwrap();
        let original_run = fs::read(directory.join("run.json")).unwrap();
        let mut corrupt = saved.clone();
        corrupt["bun_scope"] = Value::Null;
        fs::write(
            directory.join("run.json"),
            serde_json::to_vec(&corrupt).unwrap(),
        )
        .unwrap();
        let browse = Command::new(env!("CARGO_BIN_EXE_mutie"))
            .arg("report")
            .arg(&directory)
            .output()
            .unwrap();
        assert!(
            String::from_utf8_lossy(&browse.stderr).contains("lacks runner identity"),
            "{browse:?}"
        );
        fs::write(directory.join("run.json"), &original_run).unwrap();
        // New test files are permitted in focused reruns; each new run validates
        // its complete fresh inventory before baseline and every mutation.
        fs::write(root.join(format!("test/stronger.test.{syntax}")),format!("import {{test,expect}} from 'bun:test'; import {{fallback}} from '../src/flags.{syntax}'; test('empty boundary',()=>{{expect(fallback(' ')).toBe(false)}});")).unwrap();
        let output = run(
            &root,
            &reports,
            &[
                "--rerun-missed",
                directory.to_str().unwrap(),
                "--min-score",
                "100",
            ],
        );
        assert!(output.status.success(), "{output:?}");
        let (_, rerun, selected, stronger) = report(&output);
        assert_eq!(selected["mutations"].as_array().unwrap().len(), 1);
        assert_eq!(stronger["summary"]["killed"], 1);
        assert_eq!(rerun["score"]["scope"], "rerun");
        assert_eq!(
            rerun["bun_scope"]["test_files"].as_array().unwrap().len(),
            2
        );
        assert_eq!(fs::read(&source).unwrap(), before);
        assert_eq!(
            fs::read(directory.join("outcomes.json")).unwrap(),
            original_report
        );
        fs::write(root.join("tsconfig.json"), "{}").unwrap();
        let output = run(
            &root,
            &reports,
            &["--rerun-missed", directory.to_str().unwrap()],
        );
        assert!(!output.status.success());
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("Bun execution identity changed"),
            "{output:?}"
        );
        assert_eq!(fs::read(directory.join("run.json")).unwrap(), original_run);
    }
}
#[test]
#[ignore = "requires pinned Bun; mutation-triggered cleanup must invalidate the score"]
fn mutation_cleanup_errors_are_not_kills() {
    let (temp, root) = fixture("js");
    fs::write(root.join("test/flags.test.js"),"import {test,expect,afterEach} from 'bun:test'; import {enabled} from '../src/flags.js'; afterEach(()=>{if(!enabled())throw new Error('cleanup')}); test('enabled',()=>expect(enabled()).toBe(true));").unwrap();
    let output = run(&root, &temp.path().join("reports"), &[]);
    assert!(!output.status.success(), "{output:?}");
    let (_, saved, _, outcomes) = report(&output);
    assert_eq!(saved["status"], "failed");
    assert!(saved["score"]["percentage"].is_null());
    assert!(!saved["score"]["unavailable_reason"].is_null());
    assert_eq!(outcomes["summary"]["errors"], 1);
    assert_eq!(outcomes["summary"]["killed"], 0);
}
