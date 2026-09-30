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
    Command::new(env!("CARGO_BIN_EXE_mutie"))
        .arg(root)
        .args(args)
        .output()
        .expect("could not start mutie")
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
    let temp = tempfile::tempdir().unwrap();
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

    // Multiple mutations on one worker also prove that test side effects reset.
    let custom = root.join("artifacts/reports");
    fs::create_dir_all(&custom).unwrap();
    // This would break the baseline if reports were copied into workspaces.
    fs::write(custom.join("poison.go"), "this is not Go").unwrap();
    let scoped_output = run(
        &root,
        &[
            "--include",
            "*.go",
            "--exclude",
            "ready.go",
            "--timeout",
            "10",
            "--output-dir",
            custom.to_str().unwrap(),
        ],
    );
    let scoped = stdout(&scoped_output);
    assert!(
        scoped.contains("3 mutations: 1 killed, 1 missed, 0 timed out, 1 failed to compile"),
        "{scoped}"
    );
    let (custom_directory, _, _, custom_report) = saved_report(&scoped_output);
    assert!(custom_directory.starts_with(&custom));
    assert_eq!(custom_report["status"], "completed");
    assert_eq!(custom_report["mutations"].as_array().unwrap().len(), 3);
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

#[cfg(unix)]
#[test]
fn saves_partial_results_when_a_later_command_fails() {
    use std::os::unix::fs::PermissionsExt;

    let temp = tempfile::tempdir().unwrap();
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
