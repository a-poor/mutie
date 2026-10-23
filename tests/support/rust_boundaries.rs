use super::*;

#[test]
#[ignore = "runs real Cargo boundary mutations and a stronger-test rerun; opt-in to avoid recursive self-testing"]
fn rust_comparison_boundaries_find_missing_equality_and_rerun_with_stronger_tests() {
    let fixture = Fixture::from_fixture("rust-boundaries");
    let original = files(&fixture.root);
    let listing = fixture.run(&["--list", "--operator", "comparison_boundary"]);
    assert!(listing.status.success(), "{listing:?}");
    assert!(String::from_utf8_lossy(&listing.stdout).contains("Found 5 mutations"));
    let filtered = fixture.run(&["--list", "--operator", "boolean_flip"]);
    assert!(filtered.status.success(), "{filtered:?}");
    assert!(String::from_utf8_lossy(&filtered.stdout).contains("Found 0 mutations"));
    assert!(!fixture.temp.path().join("reports").exists());

    let output = fixture.run(&[
        "--operator",
        "comparison_boundary",
        "--jobs",
        "2",
        "--min-score",
        "100",
    ]);
    assert_eq!(output.status.code(), Some(3), "{output:?}");
    let (directory, run, inventory, outcomes) = report(&output);
    assert_eq!(run["status"], "completed");
    assert_eq!(run["score"]["percentage"], 80.0);
    assert_eq!(outcomes["summary"]["killed"], 4);
    assert_eq!(outcomes["summary"]["missed"], 1);
    assert_eq!(outcomes["summary"]["errors"], 0);
    assert_eq!(outcomes["summary"]["build_failed"], 0);
    assert_eq!(outcomes["summary"]["timeout"], 0);
    let mutations = inventory["mutations"].as_array().unwrap();
    assert_eq!(mutations.len(), 5);
    let expected = [
        ("<", "<="),
        ("<=", "<"),
        (">", ">="),
        (">=", ">"),
        ("<", "<="),
    ];
    for (mutation, (from, to)) in mutations.iter().zip(expected) {
        assert_eq!(mutation["file"], "src/comparisons.rs");
        assert_eq!(mutation["operator"], "comparison_boundary");
        assert_eq!(mutation["original"], from);
        assert_eq!(mutation["replacement"], to);
        let diff = fs::read_to_string(directory.join(mutation["diff"].as_str().unwrap())).unwrap();
        assert!(diff.contains(&format!(" {from} ")) && diff.contains(&format!(" {to} ")));
    }
    assert_eq!(outcomes["mutations"][3]["outcome"], "missed");
    let log = fs::read_to_string(directory.join("logs/000004.log")).unwrap();
    assert!(log.contains("test result: ok."));
    let summary = fs::read_to_string(directory.join("summary.txt")).unwrap();
    assert!(summary.contains("comparison_boundary") && summary.contains("80.00%"));
    assert_eq!(files(&fixture.root), original);
    let saved = files(&directory);

    let preview = fixture.run(&[
        "--rerun-missed",
        directory.to_str().unwrap(),
        "--operator",
        "comparison_boundary",
        "--list",
    ]);
    assert!(preview.status.success(), "{preview:?}");
    assert!(String::from_utf8_lossy(&preview.stdout).contains("Found 1 mutations"));
    let tests = fixture.root.join("tests/cli.rs");
    let source = fs::read_to_string(&tests).unwrap();
    fs::write(&tests, format!("{source}\n#[test]\nfn equality_is_included() {{\n    check(\"at-least\", \"10\", \"10\", \"true\\n\");\n}}\n")).unwrap();
    let strengthened = files(&fixture.root);
    let rerun = fixture.run(&[
        "--rerun-missed",
        directory.to_str().unwrap(),
        "--operator",
        "comparison_boundary",
        "--fail-on-missed",
        "--min-score",
        "100",
    ]);
    assert!(rerun.status.success(), "{rerun:?}");
    let (rerun_dir, rerun_run, rerun_inventory, rerun_outcomes) = report(&rerun);
    assert_eq!(
        rerun_run["rerun"]["mutations"],
        serde_json::json!([{"id":1,"source_id":4}])
    );
    assert_eq!(
        rerun_inventory["mutations"][0]["operator"],
        "comparison_boundary"
    );
    assert_eq!(rerun_outcomes["summary"]["killed"], 1);
    assert_eq!(rerun_outcomes["summary"]["missed"], 0);
    assert_eq!(rerun_run["score"]["percentage"], 100.0);
    assert_eq!(rerun_run["score_policy"]["status"], "passed");
    let failure = fs::read_to_string(rerun_dir.join("logs/000001.log")).unwrap();
    assert!(
        failure.contains("equality_is_included ... FAILED"),
        "{failure}"
    );
    assert_eq!(files(&fixture.root), strengthened);
    assert_eq!(files(&directory), saved);
    let validation = Command::new(env!("CARGO_BIN_EXE_mutie"))
        .arg("prune")
        .arg(fixture.temp.path().join("reports"))
        .args(["--keep-last", "0"])
        .output()
        .unwrap();
    assert!(validation.status.success(), "{validation:?}");
    assert!(String::from_utf8_lossy(&validation.stdout).contains("Selected runs: 2"));
}
