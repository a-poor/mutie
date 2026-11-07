#![cfg(unix)]

#[path = "support/invocation.rs"]
mod invocation;
#[allow(unused_imports)]
use invocation::{Child, Command};

use serde_json::{Value, json};
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::Output,
};

struct Fixture {
    temp: tempfile::TempDir,
    root: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let temp = tempfile::tempdir_in(std::env::temp_dir().canonicalize().unwrap()).unwrap();
        let root = temp.path().join("project");
        fs::create_dir(&root).unwrap();
        fs::create_dir(temp.path().join("bin")).unwrap();
        fs::write(root.join("go.mod"), "module example.com/test\n").unwrap();
        for name in ["a", "b", "c"] {
            fs::write(
                root.join(format!("{name}.go")),
                format!("package p\nvar {name} = true\n"),
            )
            .unwrap();
        }
        let go = temp.path().join("bin/go");
        fs::write(&go, r#"#!/bin/sh
printf '%s\n' "$*" >> "$CALLS"
if [ -e archived/poison.go ]; then
    printf 'source report leaked into workspace\n' >&2
    exit 1
fi
if [ "$1" = list ]; then
    if [ -f new.go ]; then
        printf '{"Dir":"%s","GoFiles":["a.go","b.go","c.go","new.go"]}\n' "$PWD"
    else
        printf '{"Dir":"%s","GoFiles":["a.go","b.go","c.go"]}\n' "$PWD"
    fi
    exit 0
fi
if [ -f broken_test.go ] || grep -q 'a = false' a.go || { [ -f stronger_test.go ] && grep -q 'b = false' b.go; }; then
    printf '{"Action":"fail","Package":"p","Test":"TestBehavior"}\n'
    exit 1
fi
printf '{"Action":"pass","Package":"p"}\n'
"#).unwrap();
        fs::set_permissions(go, fs::Permissions::from_mode(0o755)).unwrap();
        Self { temp, root }
    }

    fn run(&self, args: &[&str]) -> Output {
        let mut paths = vec![self.temp.path().join("bin")];
        paths.extend(std::env::split_paths(
            &std::env::var_os("PATH").unwrap_or_default(),
        ));
        Command::new(env!("CARGO_BIN_EXE_mutie"))
            .current_dir(self.temp.path())
            .arg(&self.root)
            .args(["--ui", "plain", "--jobs", "2"])
            .args(args)
            .env("PATH", std::env::join_paths(paths).unwrap())
            .env("CALLS", self.temp.path().join("calls"))
            .output()
            .unwrap()
    }

    fn first_run(&self) -> PathBuf {
        let output = self.run(&[]);
        success(&output);
        report_path(&output)
    }

    fn with_operators() -> Self {
        let fixture = Self::new();
        fs::write(fixture.root.join("c.go"), "package p\nvar c = 1 + 2\n").unwrap();
        fixture
    }

    fn reset_calls(&self) {
        fs::write(self.temp.path().join("calls"), "").unwrap();
    }

    fn calls(&self) -> String {
        fs::read_to_string(self.temp.path().join("calls")).unwrap()
    }
}

#[test]
fn focused_ids_preview_and_execute_once_with_source_mappings_and_filters() {
    let fixture = Fixture::with_operators();
    let original = fixture.first_run();
    let original_files: Vec<_> = ["run.json", "mutants.json", "outcomes.json", "summary.txt"]
        .into_iter()
        .map(|name| (name, fs::read(original.join(name)).unwrap()))
        .collect();
    let sources: Vec<_> = ["a.go", "b.go", "c.go"]
        .into_iter()
        .map(|name| (name, fs::read(fixture.root.join(name)).unwrap()))
        .collect();
    let args = [
        "--rerun",
        original.to_str().unwrap(),
        "--id",
        "3,1",
        "--id",
        "3",
        "--operator",
        "boolean_flip,arithmetic_replace",
        "--operator",
        "boolean_flip",
    ];
    fixture.reset_calls();
    let preview = success(&fixture.run(&[args.as_slice(), &["--list"]].concat()));
    assert!(
        preview.contains("[000001] (source [000001]) boolean_flip a.go:"),
        "{preview}"
    );
    assert!(
        preview.contains("[000002] (source [000003]) arithmetic_replace c.go:"),
        "{preview}"
    );
    assert!(!preview.contains("b.go:") && !preview.contains("Saving report"));
    assert_eq!(fixture.calls(), "list -json ./...\n");
    assert_eq!(
        fs::read_dir(fixture.root.join(".mutie")).unwrap().count(),
        1
    );

    fixture.reset_calls();
    let output = fixture.run(&args);
    assert!(success(&output).contains("2 mutations: 1 killed, 1 missed"));
    let current = report_path(&output);
    assert!(
        fixture
            .calls()
            .starts_with("test -json -count=1 -timeout=0 ./...\nlist -json ./...\n")
    );
    assert_eq!(
        fixture
            .calls()
            .lines()
            .filter(|line| line.starts_with("test "))
            .count(),
        3
    );
    let metadata = read(&current, "run.json");
    let request = json!({"source_run": original, "selection": "all", "source_ids": [1,3],
        "operators": ["arithmetic_replace", "boolean_flip"]});
    assert_eq!(metadata["effective_config"]["rerun"], request);
    for field in ["source_run", "selection", "source_ids", "operators"] {
        assert_eq!(metadata["rerun"][field], request[field]);
    }
    assert_eq!(
        metadata["rerun"]["source_checkpoint"],
        read(&original, "run.json")["checkpoint"]
    );
    assert_eq!(
        metadata["rerun"]["mutations"],
        json!([{"id":1,"source_id":1},{"id":2,"source_id":3}])
    );
    assert_eq!(
        read(&current, "outcomes.json")["baseline"]["outcome"],
        "passed"
    );
    let inventory = read(&current, "mutants.json");
    assert_eq!(inventory["mutations"].as_array().unwrap().len(), 2);
    assert_eq!(inventory["mutations"][1]["file"], "c.go");
    let summary = fs::read_to_string(current.join("summary.txt")).unwrap();
    assert!(summary.contains("source IDs: 1, 3"));
    assert!(summary.contains("operators: arithmetic_replace, boolean_flip"));
    assert!(summary.contains("[000002] <- source [000003]"));
    for (name, bytes) in original_files {
        assert_eq!(fs::read(original.join(name)).unwrap(), bytes);
    }
    for (name, bytes) in sources {
        assert_eq!(fs::read(fixture.root.join(name)).unwrap(), bytes);
    }
    let repeated = fixture.run(&["--rerun", current.to_str().unwrap(), "--id", "2"]);
    assert!(success(&repeated).contains("1 mutations: 0 killed, 1 missed"));
    assert_eq!(
        read(&report_path(&repeated), "run.json")["rerun"]["mutations"],
        json!([{"id":1,"source_id":2}])
    );
}

#[test]
fn operator_id_and_missed_filters_intersect_and_empty_selections_are_explicit() {
    let fixture = Fixture::with_operators();
    let original = fixture.first_run();
    for (mode, filters, files) in [
        ("--rerun", vec![], vec!["a.go", "b.go", "c.go"]),
        (
            "--rerun",
            vec!["--operator", "boolean_flip"],
            vec!["a.go", "b.go"],
        ),
        (
            "--rerun",
            vec!["--operator", "arithmetic_replace"],
            vec!["c.go"],
        ),
        (
            "--rerun-missed",
            vec!["--operator", "boolean_flip"],
            vec!["b.go"],
        ),
        (
            "--rerun-missed",
            vec!["--id", "1,2", "--operator", "boolean_flip"],
            vec!["b.go"],
        ),
        ("--rerun-missed", vec!["--id", "1"], vec![]),
        ("--rerun", vec!["--operator", "logical_swap"], vec![]),
    ] {
        fixture.reset_calls();
        let mut args = vec![mode, original.to_str().unwrap(), "--list"];
        args.extend(filters);
        let text = success(&fixture.run(&args));
        assert!(
            text.contains(&format!("Found {} mutations", files.len())),
            "{text}"
        );
        for name in ["a.go", "b.go", "c.go"] {
            assert_eq!(
                text.contains(&format!("{name}:")),
                files.contains(&name),
                "{text}"
            );
        }
        if files.is_empty() {
            assert!(text.contains("No saved mutations matched the rerun selection"));
        }
        assert_eq!(fixture.calls(), "list -json ./...\n");
    }
    assert_eq!(
        fs::read_dir(fixture.root.join(".mutie")).unwrap().count(),
        1
    );
    fixture.reset_calls();
    let output = fixture.run(&[
        "--rerun",
        original.to_str().unwrap(),
        "--id",
        "1",
        "--operator",
        "arithmetic_replace",
    ]);
    assert!(success(&output).contains("No saved mutations matched the rerun selection"));
    let current = report_path(&output);
    assert_eq!(read(&current, "run.json")["status"], "completed");
    assert_eq!(read(&current, "run.json")["rerun"]["mutations"], json!([]));
    assert_eq!(read(&current, "mutants.json")["mutations"], json!([]));
    assert_eq!(
        read(&current, "outcomes.json")["baseline"]["outcome"],
        "passed"
    );
    assert_eq!(fixture.calls().lines().count(), 2);
}

#[test]
fn focused_reruns_reject_invalid_selectors_and_preserve_validation_and_failure_metadata() {
    let fixture = Fixture::new();
    let original = fixture.first_run();
    for args in [
        vec![
            "--rerun",
            original.to_str().unwrap(),
            "--id",
            "99",
            "--operator",
            "logical_swap",
        ],
        vec!["--rerun-missed", original.to_str().unwrap(), "--id", "1,99"],
        vec!["--rerun", original.to_str().unwrap(), "--operator", "typo"],
        vec!["--id", "1"],
    ] {
        fixture.reset_calls();
        let output = fixture.run(&args);
        assert!(!output.status.success(), "{args:?}");
        assert!(fixture.calls().is_empty());
        assert_eq!(
            fs::read_dir(fixture.root.join(".mutie")).unwrap().count(),
            1
        );
    }
    let args = [
        "--rerun",
        original.to_str().unwrap(),
        "--id",
        "1",
        "--operator",
        "boolean_flip",
    ];
    fs::write(fixture.root.join("broken_test.go"), "package p\n").unwrap();
    fixture.reset_calls();
    let output = fixture.run(&args);
    assert!(!output.status.success());
    assert_eq!(fixture.calls(), "test -json -count=1 -timeout=0 ./...\n");
    let metadata = read(&report_path(&output), "run.json");
    assert_eq!(metadata["status"], "failed");
    assert_eq!(
        metadata["effective_config"]["rerun"],
        json!({"source_run": original,
        "selection": "all", "source_ids": [1], "operators": ["boolean_flip"]})
    );
    fs::remove_file(fixture.root.join("broken_test.go")).unwrap();

    // Unselected files can change; selected files still require exact identity.
    fs::write(
        fixture.root.join("c.go"),
        "// changed\npackage p\nvar c = true\n",
    )
    .unwrap();
    let listing = [args.as_slice(), &["--list"]].concat();
    success(&fixture.run(&listing));
    fs::write(
        fixture.root.join("a.go"),
        "// changed\npackage p\nvar a = true\n",
    )
    .unwrap();
    for excluded in [false, true] {
        fixture.reset_calls();
        let mut args = args.to_vec();
        if excluded {
            args.extend(["--exclude", "a.go"]);
        }
        let output = fixture.run(&args);
        assert!(!output.status.success());
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(if excluded {
                "missing"
            } else {
                "stale"
            })
        );
        assert_eq!(fixture.calls().lines().count(), 2);
        assert_eq!(
            read(&report_path(&output), "outcomes.json")["mutations"],
            json!([])
        );
    }
}

fn success(output: &Output) -> String {
    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn report_path(output: &Output) -> PathBuf {
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .find_map(|line| line.strip_prefix("Saving report to "))
        .unwrap()
        .into()
}

fn read(directory: &Path, name: &str) -> Value {
    serde_json::from_slice(&fs::read(directory.join(name)).unwrap()).unwrap()
}

#[test]
fn improved_tests_rerun_only_missed_with_fresh_results_and_provenance() {
    let fixture = Fixture::new();
    let original = fixture.first_run();
    assert_eq!(read(&original, "outcomes.json")["summary"]["missed"], 2);
    let original_files: Vec<_> = ["run.json", "mutants.json", "outcomes.json", "summary.txt"]
        .into_iter()
        .map(|name| (name, fs::read(original.join(name)).unwrap()))
        .collect();
    fs::write(
        fixture.root.join("stronger_test.go"),
        "package p\n// new test\n",
    )
    .unwrap();
    fs::write(
        fixture.root.join("new.go"),
        "package p\nvar newCode = true\n",
    )
    .unwrap();
    fixture.reset_calls();
    let output = fixture.run(&["--rerun-missed", original.to_str().unwrap()]);
    assert!(success(&output).contains("2 mutations: 1 killed, 1 missed"));
    let current = report_path(&output);
    assert_ne!(original, current);
    assert!(
        fixture
            .calls()
            .starts_with("test -json -count=1 -timeout=0 ./...\nlist -json ./...\n")
    );
    assert_eq!(
        fixture
            .calls()
            .lines()
            .filter(|s| s.starts_with("test "))
            .count(),
        3
    );
    let metadata = read(&current, "run.json");
    assert_eq!(metadata["rerun"]["source_run"], json!(original));
    assert_eq!(
        metadata["rerun"]["source_checkpoint"],
        read(&original, "run.json")["checkpoint"]
    );
    assert_eq!(metadata["rerun"]["selection"], "missed");
    assert_eq!(
        metadata["rerun"]["mutations"],
        json!([{"id":1,"source_id":2},{"id":2,"source_id":3}])
    );
    let inventory = read(&current, "mutants.json");
    assert_eq!(inventory["mutations"].as_array().unwrap().len(), 2);
    assert_eq!(inventory["mutations"][0]["file"], "b.go");
    assert_eq!(inventory["mutations"][1]["file"], "c.go");
    assert_eq!(
        read(&current, "outcomes.json")["baseline"]["outcome"],
        "passed"
    );
    for (name, bytes) in original_files {
        assert_eq!(fs::read(original.join(name)).unwrap(), bytes);
    }
    for name in ["a", "b", "c"] {
        assert_eq!(
            fs::read_to_string(fixture.root.join(format!("{name}.go"))).unwrap(),
            format!("package p\nvar {name} = true\n")
        );
    }
    // Reruns can themselves be the source of a later rerun.
    let repeated = fixture.run(&["--rerun-missed", current.to_str().unwrap()]);
    assert!(success(&repeated).contains("1 mutations: 0 killed, 1 missed"));
    assert_eq!(
        read(&report_path(&repeated), "run.json")["rerun"]["mutations"],
        json!([{"id":1,"source_id":2}])
    );
}

#[test]
fn list_previews_moved_reports_without_tests_or_new_artifacts() {
    let fixture = Fixture::new();
    let original = fixture.first_run();
    let archived = fixture.root.join("archived");
    fs::rename(original, &archived).unwrap();
    fs::write(archived.join("poison.go"), "invalid Go").unwrap();
    fixture.reset_calls();
    let output = fixture.run(&["--rerun-missed", "project/archived", "--list"]);
    let text = success(&output);
    assert!(text.contains("Found 2 mutations"));
    assert!(text.contains("b.go:") && text.contains("c.go:"));
    assert!(!text.contains("a.go:") && !text.contains("Saving report"));
    assert_eq!(fixture.calls(), "list -json ./...\n");
    assert_eq!(
        fs::read_dir(fixture.root.join(".mutie")).unwrap().count(),
        0
    );
    fixture.reset_calls();
    success(&fixture.run(&["--rerun-missed", "project/archived"]));
    // Never let a new output directory alter the source run.
    fixture.reset_calls();
    let output = fixture.run(&[
        "--rerun-missed",
        "project/archived",
        "--output-dir",
        "project/archived/new",
    ]);
    assert!(!output.status.success());
    assert!(fixture.calls().is_empty());
    assert!(!archived.join("new").exists());
}

#[test]
fn baseline_failure_and_unmatched_targets_never_start_mutation_workers() {
    let fixture = Fixture::new();
    let original = fixture.first_run();
    fixture.reset_calls();
    fs::write(fixture.root.join("broken_test.go"), "package p\n").unwrap();
    let output = fixture.run(&["--rerun-missed", original.to_str().unwrap()]);
    assert!(!output.status.success());
    assert_eq!(fixture.calls(), "test -json -count=1 -timeout=0 ./...\n");
    assert_eq!(read(&report_path(&output), "run.json")["status"], "failed");
    assert_eq!(
        read(&report_path(&output), "mutants.json")["complete"],
        false
    );
    fs::remove_file(fixture.root.join("broken_test.go")).unwrap();
    for stale in [false, true] {
        fixture.reset_calls();
        let mut args = vec!["--rerun-missed", original.to_str().unwrap()];
        if stale {
            fs::write(
                fixture.root.join("b.go"),
                "// changed\npackage p\nvar b = true\n",
            )
            .unwrap();
        } else {
            args.extend(["--exclude", "c.go"]);
        }
        let output = fixture.run(&args);
        assert!(!output.status.success());
        let error = String::from_utf8_lossy(&output.stderr);
        assert!(
            error.contains(if stale { "stale" } else { "missing" }),
            "{error}"
        );
        assert_eq!(fixture.calls().lines().count(), 2);
        assert_eq!(read(&report_path(&output), "run.json")["status"], "failed");
        assert_eq!(
            read(&report_path(&output), "outcomes.json")["mutations"],
            json!([])
        );
    }
}

#[test]
fn invalid_report_fails_before_go_or_report_creation_and_empty_selection_is_explicit() {
    let fixture = Fixture::new();
    // This source run has no missed mutations.
    let output = fixture.run(&["--include", "a.go"]);
    success(&output);
    let original = report_path(&output);
    fixture.reset_calls();
    let output = fixture.run(&["--rerun-missed", original.to_str().unwrap()]);
    assert!(success(&output).contains("No saved mutations matched the rerun selection"));
    let current = report_path(&output);
    assert_eq!(read(&current, "mutants.json")["mutations"], json!([]));
    assert_eq!(read(&current, "run.json")["status"], "completed");
    assert_eq!(fixture.calls().lines().count(), 2);

    let mut metadata = read(&original, "run.json");
    metadata["status"] = json!("running");
    fs::write(
        original.join("run.json"),
        serde_json::to_vec(&metadata).unwrap(),
    )
    .unwrap();
    fixture.reset_calls();
    let output = fixture.run(&["--rerun-missed", original.to_str().unwrap()]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("not finalized"));
    assert!(fixture.calls().is_empty());
    assert_eq!(
        fs::read_dir(fixture.root.join(".mutie")).unwrap().count(),
        2
    );
}
