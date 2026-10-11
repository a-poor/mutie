use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};

fn copy_tree(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        if entry.file_name() == ".mutie" {
            continue;
        }
        let destination = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&entry.path(), &destination);
        } else {
            fs::copy(entry.path(), destination).unwrap();
        }
    }
}

fn run(root: &Path, args: &[&str]) -> Output {
    with_build_cleanup(
        Command::new(env!("CARGO_BIN_EXE_mutie"))
            .arg(root)
            .args(args),
    )
}

fn dismiss(root: &Path, args: &[&str]) -> Output {
    with_build_cleanup(
        Command::new(env!("CARGO_BIN_EXE_mutie"))
            .args(["dismiss", "--module"])
            .arg(root)
            .args(args),
    )
}

fn with_build_cleanup(command: &mut Command) -> Output {
    let builds = tempfile::tempdir().unwrap();
    let output = command
        .env("GOTMPDIR", builds.path())
        .output()
        .expect("could not start mutie");
    assert!(
        fs::read_dir(builds.path()).unwrap().next().is_none(),
        "Go temporary builds leaked: {output:?}"
    );
    output
}

fn stdout(output: &Output) -> String {
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn saved_report(
    output: &Output,
) -> (
    PathBuf,
    serde_json::Value,
    serde_json::Value,
    serde_json::Value,
) {
    let text = String::from_utf8_lossy(&output.stdout);
    let directory = text
        .lines()
        .find_map(|line| line.strip_prefix("Saving report to "))
        .unwrap_or_else(|| panic!("missing report path: {text}"));
    let directory = PathBuf::from(directory);
    let read = |name| serde_json::from_slice(&fs::read(directory.join(name)).unwrap()).unwrap();
    let run = read("run.json");
    let inventory = read("mutants.json");
    let outcomes = read("outcomes.json");
    (directory, run, inventory, outcomes)
}

#[test]
#[ignore = "requires Go 1.24+ on PATH; runs real builds and a deliberate timeout"]
fn go_workflow() {
    let temp = tempfile::tempdir_in(std::env::temp_dir().canonicalize().unwrap()).unwrap();
    let root = temp.path().join("module");
    copy_tree(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/go"),
        &root,
    );

    let listing = stdout(&run(&root, &["--list"]));
    assert!(listing.contains("Found 4 mutations"), "{listing}");
    assert!(!listing.contains("Running baseline"));
    assert!(!listing.contains("disabled.go:"));
    assert!(!listing.contains("generated.go:"));
    assert!(!listing.contains("nested.go:"));
    assert!(!listing.contains("fixture_test.go:"));
    assert!(!root.join(".mutie").exists());

    let output = run(&root, &["--jobs", "2", "--timeout", "5"]);
    let results = stdout(&output);
    assert!(
        results.contains("4 mutations: 1 killed, 1 missed, 1 timed out, 1 failed to compile"),
        "{results}"
    );
    let (first_directory, metadata, inventory, report) = saved_report(&output);
    assert_eq!(report["schema_version"], 1);
    assert_eq!(metadata["checkpoint"], report["checkpoint"]);
    assert_eq!(inventory["complete"], true);
    assert_eq!(report["status"], "completed");
    assert_eq!(metadata["planned_mutations"], 4);
    assert_eq!(report["summary"]["killed"], 1);
    assert_eq!(report["summary"]["missed"], 1);
    assert_eq!(report["summary"]["timeout"], 1);
    assert_eq!(report["summary"]["build_failed"], 1);
    assert_eq!(report["summary"]["errors"], 0);
    assert_eq!(report["baseline"]["outcome"], "passed");
    let affected = run(
        &root,
        &[
            "--jobs",
            "2",
            "--timeout",
            "5",
            "--test-scope",
            "affected",
            "--fail-on-missed",
            "--max-log-bytes",
            "128",
        ],
    );
    assert_eq!(affected.status.code(), Some(3), "{affected:?}");
    let (_, affected_metadata, affected_inventory, affected_outcomes) = saved_report(&affected);
    assert_eq!(affected_metadata["status"], "completed");
    assert_eq!(affected_metadata["effective_config"]["max_log_bytes"], 128);
    assert_eq!(affected_metadata["policy"]["status"], "failed");
    assert_eq!(affected_metadata["policy"]["undismissed_missed"], 1);
    assert_eq!(affected_outcomes["summary"], report["summary"]);
    assert_eq!(affected_inventory["mutations"].as_array().unwrap().len(), 4);
    for mutation in affected_inventory["mutations"].as_array().unwrap() {
        assert_eq!(
            mutation["test_packages"],
            serde_json::json!(["example.com/mutie-fixture"])
        );
    }
    for (phase, operations) in [
        ("snapshot", 1),
        ("baseline_setup", 1),
        ("discovery_reset", 1),
        ("worker_setup", 2),
        ("worker_reset", 2),
    ] {
        let measured = &metadata["workspace_metrics"][phase];
        assert_eq!(measured["operations"], operations);
        assert_eq!(measured["completed"], operations);
        assert_eq!(measured["failed"], 0);
        assert_eq!(measured["interrupted"], 0);
        assert!(measured["duration_seconds"].as_f64().unwrap() > 0.0);
        // The real filesystem chooses the copy method; both are supported.
        assert!(
            measured["reflink_files"].as_u64().unwrap()
                + measured["copied_files"].as_u64().unwrap()
                > 0
        );
    }
    assert!(results.contains("Workspaces: snapshot "));
    assert!(results.contains("Copies: "));
    let mutations = report["mutations"].as_array().unwrap();
    for (index, mutation) in mutations.iter().enumerate() {
        assert_eq!(mutation["id"], index + 1);
        assert!(mutation["duration_seconds"].as_f64().unwrap() > 0.0);
        assert!(
            first_directory
                .join(inventory["mutations"][index]["diff"].as_str().unwrap())
                .is_file()
        );
        for kind in ["text", "stdout", "stderr"] {
            assert!(
                first_directory
                    .join(mutation["logs"][kind].as_str().unwrap())
                    .is_file()
            );
        }
    }
    let killed_log =
        fs::read_to_string(first_directory.join(mutations[0]["logs"]["text"].as_str().unwrap()))
            .unwrap();
    assert!(killed_log.contains("wrong sum"));
    let build_log =
        fs::read_to_string(first_directory.join(mutations[1]["logs"]["text"].as_str().unwrap()))
            .unwrap();
    assert!(build_log.contains("invalid operation"));
    let summary = fs::read_to_string(first_directory.join("summary.txt")).unwrap();
    assert!(summary.contains("Missed mutations:"));
    assert!(summary.contains("missed.go:4:"));
    let first_json = fs::read(first_directory.join("run.json")).unwrap();

    stdout(&dismiss(
        &root,
        &[
            "add",
            first_directory.to_str().unwrap(),
            "--id",
            "3",
            "--reason",
            "accepted test gap",
            "--note",
            "Exercise persistent review metadata.",
        ],
    ));
    assert!(stdout(&dismiss(&root, &["check"])).contains("1 matched; 0 require review"));
    let dismissal_bytes = fs::read(root.join("mutie.dismissals.toml")).unwrap();

    // Select a killed and a missed arithmetic mutation by their source IDs.
    // The dismissed missed mutation still executes; the build failure and timeout do not.
    let focused = run(
        &root,
        &[
            "--rerun",
            first_directory.to_str().unwrap(),
            "--id",
            "3,1",
            "--operator",
            "arithmetic_replace",
            "--jobs",
            "2",
            "--timeout",
            "5",
        ],
    );
    assert!(
        stdout(&focused)
            .contains("2 mutations: 1 killed, 1 missed, 0 timed out, 0 failed to compile")
    );
    let (_, focused_metadata, focused_inventory, focused_outcomes) = saved_report(&focused);
    assert_eq!(focused_inventory["mutations"].as_array().unwrap().len(), 2);
    assert_eq!(
        focused_inventory["dismissals"]["matched"][0]["mutation_id"],
        2
    );
    assert_eq!(
        focused_inventory["dismissals"]["matched"][0]["dismissal"]["reason"],
        "accepted test gap"
    );
    assert_eq!(focused_outcomes["baseline"]["outcome"], "passed");
    assert_eq!(
        focused_metadata["rerun"]["mutations"],
        serde_json::json!([
            {"id": 1, "source_id": 1}, {"id": 2, "source_id": 3}
        ])
    );
    let preview = stdout(&run(
        &root,
        &[
            "--rerun-missed",
            first_directory.to_str().unwrap(),
            "--operator",
            "arithmetic_replace",
            "--list",
        ],
    ));
    assert!(preview.contains("Found 1 mutations") && preview.contains("(source [000003])"));

    // Multiple mutations on one worker also prove that test side effects reset.
    let custom = root.join("artifacts/reports");
    fs::create_dir_all(&custom).unwrap();
    // This would break the baseline if reports were copied into workspaces.
    fs::write(custom.join("poison.go"), "this is not Go").unwrap();
    fs::write(root.join("mutie.toml"), "include = ['*.go']\nexclude = ['ready.go']\ntimeout = 10\noutput_dir = 'artifacts/reports'\n").unwrap();
    let scoped_output = run(&root, &[]);
    let scoped = stdout(&scoped_output);
    assert!(
        scoped.contains("3 mutations: 1 killed, 1 missed, 0 timed out, 1 failed to compile"),
        "{scoped}"
    );
    let (custom_directory, custom_metadata, _, custom_report) = saved_report(&scoped_output);
    assert!(custom_directory.starts_with(&custom));
    assert_eq!(custom_report["status"], "completed");
    assert_eq!(custom_report["mutations"].as_array().unwrap().len(), 3);
    assert_eq!(
        custom_metadata["effective_config"]["exclude"],
        serde_json::json!(["ready.go"])
    );
    assert_eq!(
        custom_metadata["effective_config"]["output_dir"],
        custom.to_str().unwrap()
    );
    assert_eq!(
        fs::read(first_directory.join("run.json")).unwrap(),
        first_json
    );
    assert_eq!(
        fs::read_to_string(custom.join("poison.go")).unwrap(),
        "this is not Go"
    );
    assert!(!root.join("test-artifact").exists());
    assert_eq!(
        fs::read_to_string(root.join("testdata/state.txt")).unwrap(),
        "pristine\n"
    );
    assert_eq!(
        fs::read(root.join("add.go")).unwrap(),
        fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/go/add.go")).unwrap()
    );

    let empty = run(
        &root,
        &[
            "--exclude",
            "**/*.go",
            "--output-dir",
            custom.to_str().unwrap(),
        ],
    );
    stdout(&empty);
    let (_, empty_metadata, empty_inventory, empty_report) = saved_report(&empty);
    assert_eq!(empty_report["status"], "completed");
    assert_eq!(empty_metadata["planned_mutations"], 0);
    assert_eq!(empty_metadata["workers"], 0);
    assert!(empty_report["mutations"].as_array().unwrap().is_empty());
    assert_eq!(empty_inventory["complete"], true);
    assert!(empty_inventory["mutations"].as_array().unwrap().is_empty());

    // Adding a test changes the missed mutation's outcome without changing its
    // source identity. A focused rerun still gets a baseline and clean workspaces.
    fs::write(root.join("difference_test.go"), "package fixture\nimport \"testing\"\nfunc TestDifference(t *testing.T) { if Difference(5, 2) != 3 { t.Fatal(\"wrong difference\") } }\n").unwrap();
    assert!(stdout(&dismiss(&root, &["check"])).contains("1 matched; 0 require review"));
    let rerun = run(
        &root,
        &["--rerun-missed", first_directory.to_str().unwrap()],
    );
    assert!(stdout(&rerun).contains("1 mutations: 1 killed, 0 missed"));
    let (rerun_directory, rerun_metadata, rerun_inventory, rerun_outcomes) = saved_report(&rerun);
    assert_eq!(rerun_inventory["mutations"][0]["file"], "missed.go");
    assert_eq!(
        rerun_inventory["dismissals"]["matched"][0]["mutation_id"],
        1
    );
    assert_eq!(rerun_outcomes["baseline"]["outcome"], "passed");
    assert_eq!(rerun_outcomes["mutations"].as_array().unwrap().len(), 1);
    assert_eq!(
        rerun_metadata["rerun"]["mutations"],
        serde_json::json!([{"id": 1, "source_id": 3}])
    );
    assert!(
        fs::read_to_string(rerun_directory.join("logs/000001.log"))
            .unwrap()
            .contains("wrong difference")
    );
    assert_eq!(
        fs::read(first_directory.join("run.json")).unwrap(),
        first_json
    );
    assert_eq!(
        fs::read(root.join("mutie.dismissals.toml")).unwrap(),
        dismissal_bytes
    );
    assert!(!root.join("test-artifact").exists());
    assert_eq!(
        fs::read_to_string(root.join("testdata/state.txt")).unwrap(),
        "pristine\n"
    );

    fs::write(root.join("fail_test.go"), "package fixture\nimport \"testing\"\nfunc TestFail(t *testing.T) { t.Fatal(\"baseline fails\") }\n").unwrap();
    // Keep using the custom output root so it remains excluded from snapshots.
    let failed = run(&root, &["--output-dir", custom.to_str().unwrap()]);
    assert!(!failed.status.success());
    assert!(
        String::from_utf8_lossy(&failed.stderr)
            .contains("baseline tests failed; no mutations were run")
    );
    assert!(!String::from_utf8_lossy(&failed.stdout).contains("Running 1 worker"));
    assert!(!root.join("test-artifact").exists());
    let (failed_directory, _, failed_inventory, failed_report) = saved_report(&failed);
    assert_eq!(failed_report["status"], "failed");
    assert_eq!(failed_inventory["complete"], false);
    assert_eq!(failed_report["baseline"]["outcome"], "failed");
    assert!(failed_report["mutations"].as_array().unwrap().is_empty());
    assert!(
        fs::read_to_string(failed_directory.join("logs/baseline.log"))
            .unwrap()
            .contains("baseline fails")
    );

    fs::write(root.join("fail_test.go"), "package fixture\nimport (\"testing\"; \"time\")\nfunc TestHang(t *testing.T) { time.Sleep(time.Hour) }\n").unwrap();
    let timed_out = run(
        &root,
        &[
            "--baseline-timeout",
            "1",
            "--output-dir",
            custom.to_str().unwrap(),
        ],
    );
    assert!(!timed_out.status.success());
    assert!(String::from_utf8_lossy(&timed_out.stderr).contains("baseline timed out"));
    let (_, _, _, timeout_report) = saved_report(&timed_out);
    assert_eq!(timeout_report["status"], "failed");
    assert_eq!(timeout_report["baseline"]["outcome"], "timeout");
}

#[test]
#[ignore = "requires Go 1.24+ on PATH; builds call-removal mutants and a focused rerun"]
fn call_removal_reports_real_outcomes_and_a_stronger_test_kills_a_survivor() {
    let root = tempfile::tempdir_in(std::env::temp_dir().canonicalize().unwrap()).unwrap();
    let root = root.path();
    fs::write(root.join("go.mod"), "module example.com/calls\n\ngo 1.24\n").unwrap();
    let source = concat!(
        "package calls\n\nvar State int\n\nfunc set(value int) { State = value }\n\n",
        "func Checked() {\n\tset(\n\t\t1,\n\t)\n}\n\n",
        "func Unchecked() { set(2) }\n\n",
        "func Noisy() {\n\tx := 3\n\tset(x)\n}\n\n",
        "func Parenthesized() { (set(4)) }\n"
    );
    fs::write(root.join("calls.go"), source).unwrap();
    fs::write(
        root.join("calls_test.go"),
        r#"package calls

import "testing"

func TestCalls(t *testing.T) {
    State = 0
    Checked()
    if State != 1 { t.Fatal("missing checked effect") }
    Unchecked()
    Noisy()
    Parenthesized()
    if State != 4 { t.Fatal("missing parenthesized effect") }
}
"#,
    )
    .unwrap();
    let preview = stdout(&run(root, &["--operator", "call_remove", "--list"]));
    assert!(preview.contains("Found 4 mutations"), "{preview}");
    assert!(!root.join(".mutie").exists());
    let output = run(
        root,
        &["--operator", "call_remove", "--jobs", "2", "--timeout", "5"],
    );
    stdout(&output);
    let (directory, metadata, inventory, outcomes) = saved_report(&output);
    assert_eq!(metadata["status"], "completed");
    assert_eq!(outcomes["summary"]["killed"], 2);
    assert_eq!(outcomes["summary"]["missed"], 1);
    assert_eq!(outcomes["summary"]["build_failed"], 1);
    assert_eq!(outcomes["summary"]["errors"], 0);
    assert_eq!(inventory["mutations"][0]["original"], "set(\n\t\t1,\n\t)");
    assert_eq!(inventory["mutations"][3]["original"], "(set(4))");
    for m in inventory["mutations"].as_array().unwrap() {
        assert_eq!(m["operator"], "call_remove");
        assert_eq!(m["replacement"], "");
    }
    let saved = fs::read(directory.join("run.json")).unwrap();
    stdout(&dismiss(
        root,
        &[
            "add",
            directory.to_str().unwrap(),
            "--id",
            "2",
            "--reason",
            "test review",
        ],
    ));
    fs::write(
        root.join("stronger_test.go"),
        r#"package calls

import "testing"

func TestUncheckedEffect(t *testing.T) {
    State = 0
    Unchecked()
    if State != 2 { t.Fatal("missing previously unchecked effect") }
}
"#,
    )
    .unwrap();
    let rerun = run(
        root,
        &[
            "--rerun-missed",
            directory.to_str().unwrap(),
            "--operator",
            "call_remove",
            "--test-scope",
            "affected",
            "--timeout",
            "5",
        ],
    );
    stdout(&rerun);
    let (_, metadata, inventory, outcomes) = saved_report(&rerun);
    assert_eq!(metadata["rerun"]["mutations"][0]["source_id"], 2);
    assert_eq!(inventory["mutations"].as_array().unwrap().len(), 1);
    assert_eq!(inventory["dismissals"]["matched"][0]["mutation_id"], 1);
    assert_eq!(outcomes["summary"]["killed"], 1);
    assert_eq!(outcomes["summary"]["missed"], 0);
    assert_eq!(fs::read(directory.join("run.json")).unwrap(), saved);
    assert_eq!(fs::read_to_string(root.join("calls.go")).unwrap(), source);
}

#[cfg(unix)]
#[test]
fn saves_partial_results_when_a_later_command_fails() {
    use std::os::unix::fs::PermissionsExt;

    let temp = tempfile::tempdir_in(std::env::temp_dir().canonicalize().unwrap()).unwrap();
    let root = temp.path().join("module");
    let bin = temp.path().join("bin");
    fs::create_dir(&root).unwrap();
    fs::create_dir(&bin).unwrap();
    fs::write(root.join("go.mod"), "module example.com/test\n").unwrap();
    fs::write(root.join("code.go"), "package p\nvar ready = true\n").unwrap();
    fs::write(root.join("a.go"), "package p\nvar a = true\n").unwrap();
    fs::write(root.join("z.go"), "package p\nvar z = true\n").unwrap();
    let go = bin.join("go");
    fs::write(
        &go,
        r#"#!/bin/sh
if [ "$1" = list ]; then
    printf '{"Dir":"%s","GoFiles":["a.go","code.go","z.go"]}\n' "$PWD"
    exit 0
fi
if grep -q 'true' code.go; then
    printf '{"Action":"output","Output":"PASS\\n"}\n'
    exit 0
fi
printf 'go tool failed unexpectedly\n' >&2
exit 1
"#,
    )
    .unwrap();
    fs::set_permissions(go, fs::Permissions::from_mode(0o755)).unwrap();
    let mut paths = vec![bin];
    paths.extend(std::env::split_paths(
        &std::env::var_os("PATH").unwrap_or_default(),
    ));
    let output = Command::new(env!("CARGO_BIN_EXE_mutie"))
        .arg(&root)
        .env("PATH", std::env::join_paths(paths).unwrap())
        .output()
        .unwrap();
    assert!(!output.status.success());
    let (directory, metadata, inventory, report) = saved_report(&output);
    assert_eq!(report["status"], "failed");
    assert_eq!(metadata["planned_mutations"], 3);
    assert_eq!(inventory["mutations"].as_array().unwrap().len(), 3);
    assert_eq!(report["mutations"].as_array().unwrap().len(), 3);
    assert_eq!(report["mutations"][2]["state"], "pending");
    assert!(report["mutations"][2]["outcome"].is_null());
    assert!(directory.join("diff/000003.diff").is_file());
    assert_eq!(report["mutations"][0]["outcome"], "missed");
    assert_eq!(report["mutations"][1]["outcome"], "error");
    assert_eq!(report["summary"]["killed"], 0);
    assert_eq!(report["summary"]["missed"], 1);
    assert_eq!(report["summary"]["errors"], 1);
    assert!(
        fs::read_to_string(directory.join("logs/000002.stderr.log"))
            .unwrap()
            .contains("go tool failed unexpectedly")
    );
    assert!(
        fs::read_to_string(directory.join("diff/000002.diff"))
            .unwrap()
            .contains("+var ready = false")
    );
}
