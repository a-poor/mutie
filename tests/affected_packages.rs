#[path = "support/invocation.rs"]
mod invocation;
#[allow(unused_imports)]
use invocation::{Child, Command};

use std::{
    fs,
    path::{Path, PathBuf},
    process::Output,
};

use serde_json::Value;

fn report(output: &Output) -> (PathBuf, Value, Value, Value) {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let directory = PathBuf::from(
        stdout
            .lines()
            .find_map(|s| s.strip_prefix("Saving report to "))
            .unwrap_or_else(|| panic!("{output:?}")),
    );
    let read = |name| serde_json::from_slice(&fs::read(directory.join(name)).unwrap()).unwrap();
    let metadata = read("run.json");
    let inventory = read("mutants.json");
    let outcomes = read("outcomes.json");
    (directory, metadata, inventory, outcomes)
}

fn write(root: &Path, path: &str, contents: &str) {
    let path = root.join(path);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, contents).unwrap();
}

#[test]
#[ignore = "requires Go 1.24+ on PATH; runs real multi-package builds"]
fn real_go_selection_covers_test_imports_and_is_recomputed_for_reruns() {
    let temp = tempfile::tempdir_in(std::env::temp_dir().canonicalize().unwrap()).unwrap();
    let root = temp.path().join("module");
    write(&root, "go.mod", "module example.com/affected\n\ngo 1.24\n");
    write(
        &root,
        "leaf/leaf.go",
        "package leaf\nfunc Ready() bool { return true }\n",
    );
    write(
        &root,
        "leaf/leaf_test.go",
        "package leaf\nimport \"testing\"\nfunc TestSmoke(t *testing.T) { t.Log(Ready()) }\n",
    );
    write(
        &root,
        "mid/mid.go",
        "package mid\nimport \"example.com/affected/leaf\"\nfunc Ready() bool { return leaf.Ready() }\n",
    );
    write(
        &root,
        "top/top.go",
        "package top\nimport \"example.com/affected/mid\"\nfunc Ready() bool { return mid.Ready() }\n",
    );
    write(
        &root,
        "top/top_test.go",
        "package top\nimport \"testing\"\nfunc TestTransitive(t *testing.T) { if !Ready() { t.Fatal(\"transitive mutation\") } }\n",
    );
    write(&root, "inside/check.go", "package inside\n");
    write(
        &root,
        "inside/check_test.go",
        "package inside\nimport (\"testing\"; \"example.com/affected/leaf\")\nfunc TestInternal(t *testing.T) { if !leaf.Ready() { t.Fatal(\"internal test import\") } }\n",
    );
    // testdata is excluded by ./..., but dependencies through it still matter.
    write(
        &root,
        "testdata/helper/helper.go",
        "package helper\nimport \"example.com/affected/leaf\"\nfunc Ready() bool { return leaf.Ready() }\n",
    );
    write(&root, "outside/check.go", "package outside\n");
    write(
        &root,
        "outside/check_test.go",
        "package outside_test\nimport (\"testing\"; \"example.com/affected/testdata/helper\")\nfunc TestExternal(t *testing.T) { if !helper.Ready() { t.Fatal(\"external test import\") } }\n",
    );
    write(&root, "unrelated/unrelated.go", "package unrelated\n");
    write(
        &root,
        "unrelated/unrelated_test.go",
        "package unrelated\nimport \"testing\"\nfunc TestUnrelated(t *testing.T) { t.Log(\"unrelated ran\") }\n",
    );
    let builds = temp.path().join("builds");
    fs::create_dir(&builds).unwrap();
    let run = |args: &[&str]| {
        let output = Command::new(env!("CARGO_BIN_EXE_mutie"))
            .arg(&root)
            .args(["--include", "leaf/leaf.go", "--timeout", "20"])
            .args(args)
            .env("GOTMPDIR", &builds)
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
        assert_eq!(fs::read_dir(&builds).unwrap().count(), 0);
        output
    };
    let (_, full_metadata, full_inventory, full_outcomes) = report(&run(&[]));
    assert_eq!(full_metadata["effective_config"]["test_scope"], "full");
    assert_eq!(
        full_inventory["mutations"][0]["test_packages"],
        serde_json::json!(["./..."])
    );
    let (directory, metadata, inventory, outcomes) =
        report(&run(&["--test-scope", "affected", "--jobs", "2"]));
    assert_eq!(outcomes["summary"], full_outcomes["summary"]);
    assert_eq!(outcomes["summary"]["killed"], 1);
    assert_eq!(inventory["mutations"].as_array().unwrap().len(), 1);
    let selected = serde_json::json!([
        "example.com/affected/inside",
        "example.com/affected/leaf",
        "example.com/affected/mid",
        "example.com/affected/outside",
        "example.com/affected/top"
    ]);
    assert_eq!(inventory["mutations"][0]["test_packages"], selected);
    assert_eq!(
        metadata["baseline_command"],
        serde_json::json!(["go", "test", "-json", "-count=1", "-timeout=0", "./..."])
    );
    assert_eq!(
        metadata["mutation_command_prefix"],
        serde_json::json!(["go", "test", "-json", "-count=1", "-timeout=0"])
    );
    assert_eq!(
        metadata["discovery_command"],
        serde_json::json!(["go", "list", "-deps", "-test", "-json", "./..."])
    );
    let baseline = fs::read_to_string(
        directory.join(outcomes["baseline"]["logs"]["stdout"].as_str().unwrap()),
    )
    .unwrap();
    let mutation = fs::read_to_string(
        directory.join(outcomes["mutations"][0]["logs"]["stdout"].as_str().unwrap()),
    )
    .unwrap();
    assert!(baseline.contains("unrelated ran"));
    assert!(!mutation.contains("example.com/affected/unrelated"));
    for message in [
        "transitive mutation",
        "internal test import",
        "external test import",
    ] {
        assert!(mutation.contains(message), "{mutation}");
    }
    let original_inventory = fs::read(directory.join("mutants.json")).unwrap();
    write(
        &root,
        "late/late_test.go",
        "package late\nimport (\"testing\"; \"example.com/affected/leaf\")\nfunc TestLate(t *testing.T) { if !leaf.Ready() { t.Fatal(\"new dependent\") } }\n",
    );
    let (_, _, rerun_inventory, rerun_outcomes) = report(&run(&[
        "--test-scope",
        "affected",
        "--rerun",
        directory.to_str().unwrap(),
        "--id",
        "1",
    ]));
    assert!(
        rerun_inventory["mutations"][0]["test_packages"]
            .as_array()
            .unwrap()
            .contains(&serde_json::json!("example.com/affected/late"))
    );
    assert_eq!(rerun_outcomes["baseline"]["outcome"], "passed");
    assert_eq!(rerun_outcomes["summary"]["killed"], 1);
    assert_eq!(
        fs::read(directory.join("mutants.json")).unwrap(),
        original_inventory
    );
    assert_eq!(
        fs::read_to_string(root.join("leaf/leaf.go")).unwrap(),
        "package leaf\nfunc Ready() bool { return true }\n"
    );
    let listing = run(&["--test-scope", "affected", "--list"]);
    let listing = String::from_utf8_lossy(&listing.stdout);
    assert!(listing.contains("Tests: ") && listing.contains("example.com/affected/late"));
    assert!(!listing.contains("Running baseline") && !listing.contains("Saving report"));
}

#[cfg(unix)]
#[test]
fn affected_execution_and_failed_graphs_preserve_the_run_contract() {
    use std::os::unix::fs::PermissionsExt;
    let temp = tempfile::tempdir_in(std::env::temp_dir().canonicalize().unwrap()).unwrap();
    let root = temp.path().join("module");
    write(&root, "go.mod", "module example.com/selection\n");
    write(&root, "leaf/leaf.go", "package leaf\nvar Ready = true\n");
    write(&root, "mutie.toml", "test_scope = 'affected'\n");
    let go = temp.path().join("bin/go");
    write(
        temp.path(),
        "bin/go",
        r#"#!/bin/sh
set -eu
printf '%s\n' "$*" >> "$CALLS"
if [ "$1" = list ]; then
    if [ "$MODE" = bad_graph ]; then
        printf '{"ImportPath":"example.com/selection/leaf","Dir":"%s/leaf","GoFiles":["leaf.go"],"Match":["./..."],"Imports":["missing"]}\n' "$PWD"
    elif [ "$MODE" = failed_discovery ]; then
        printf 'graph failure\n' >&2
        exit 2
    else
        printf '{"ImportPath":"example.com/selection/leaf","Dir":"%s/leaf","GoFiles":["leaf.go"],"Match":["./..."]}\n' "$PWD"
    fi
else
    printf '{"Action":"pass","Package":"example.com/selection/leaf"}\n'
fi
"#,
    );
    fs::set_permissions(&go, fs::Permissions::from_mode(0o755)).unwrap();
    let run = |mode: &str, args: &[&str]| {
        let calls = temp.path().join(format!("calls-{mode}"));
        let output = Command::new(env!("CARGO_BIN_EXE_mutie"))
            .arg(&root)
            .args(args)
            .env("PATH", go.parent().unwrap())
            .env("CALLS", &calls)
            .env("MODE", mode)
            .output()
            .unwrap();
        (output, fs::read_to_string(calls).unwrap())
    };
    let (output, calls) = run("success", &[]);
    assert!(output.status.success(), "{output:?}");
    assert_eq!(
        calls,
        "test -json -count=1 -timeout=0 ./...\nlist -deps -test -json ./...\ntest -json -count=1 -timeout=0 example.com/selection/leaf\n"
    );
    let (_, _, inventory, outcomes) = report(&output);
    assert_eq!(
        inventory["mutations"][0]["test_packages"],
        serde_json::json!(["example.com/selection/leaf"])
    );
    assert_eq!(outcomes["summary"]["missed"], 1);
    let (output, calls) = run("override", &["--test-scope", "full"]);
    assert!(output.status.success(), "{output:?}");
    assert_eq!(
        calls,
        "test -json -count=1 -timeout=0 ./...\nlist -json ./...\ntest -json -count=1 -timeout=0 ./...\n"
    );
    for mode in ["bad_graph", "failed_discovery"] {
        let (output, calls) = run(mode, &[]);
        assert!(!output.status.success());
        assert_eq!(calls.lines().count(), 2);
        let (directory, metadata, inventory, outcomes) = report(&output);
        assert_eq!(metadata["status"], "failed");
        assert_eq!(inventory["complete"], false);
        assert!(outcomes["mutations"].as_array().unwrap().is_empty());
        assert!(directory.join("logs/discovery.stdout.jsonl").exists());
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(if mode == "bad_graph" {
                "graph is missing"
            } else {
                "graph failure"
            })
        );
    }
}
