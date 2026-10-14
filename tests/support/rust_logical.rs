use super::*;

#[test]
#[ignore = "runs real Cargo logical swaps and a stronger-test rerun; opt-in to avoid recursive self-testing"]
fn rust_logical_swaps_preserve_grouping_and_find_missing_mixed_inputs() {
    let fixture = Fixture::from_fixture("rust-logical");
    fs::write(
        fixture.root.join("mutie.toml"),
        "operators = [\"logical_swap\"]\n",
    )
    .unwrap();
    let original = files(&fixture.root);
    let listing = fixture.run(&["--list"]);
    assert!(listing.status.success(), "{listing:?}");
    assert!(String::from_utf8_lossy(&listing.stdout).contains("Found 8 mutations"));
    let output = fixture.run(&[
        "--jobs",
        "2",
        "--rust-build-cache",
        "worker",
        "--min-score",
        "100",
    ]);
    assert_eq!(output.status.code(), Some(3), "{output:?}");
    let (directory, run, inventory, outcomes) = report(&output);
    assert_eq!(run["status"], "completed");
    assert_eq!(run["score"]["percentage"], 87.5);
    assert_eq!(
        outcomes["summary"],
        serde_json::json!({"killed":7,"missed":1,"timeout":0,"build_failed":0,"errors":0})
    );
    for m in inventory["mutations"].as_array().unwrap() {
        assert_eq!(m["file"], "src/logic.rs");
        assert_eq!(m["operator"], "logical_swap");
        assert!(directory.join(m["diff"].as_str().unwrap()).is_file());
    }
    assert_eq!(outcomes["mutations"][2]["outcome"], "missed");
    assert_eq!(files(&fixture.root), original);
    let saved = files(&directory);
    let preview = fixture.run(&[
        "--rerun-missed",
        directory.to_str().unwrap(),
        "--operator",
        "logical_swap",
        "--list",
    ]);
    assert!(preview.status.success(), "{preview:?}");
    assert!(String::from_utf8_lossy(&preview.stdout).contains("Found 1 mutations"));
    let tests = fixture.root.join("tests/cli.rs");
    let text = fs::read_to_string(&tests).unwrap();
    fs::write(&tests, format!("{text}\n#[test]\nfn mixed_inputs_are_accepted() {{ check(\"weak\", false, true, false, \"true:\"); }}\n")).unwrap();
    let stronger = files(&fixture.root);
    let rerun = fixture.run(&[
        "--rerun-missed",
        directory.to_str().unwrap(),
        "--operator",
        "logical_swap",
        "--min-score",
        "100",
    ]);
    assert!(rerun.status.success(), "{rerun:?}");
    let (rerun_dir, rerun_run, _, rerun_outcomes) = report(&rerun);
    assert_eq!(
        rerun_run["rerun"]["mutations"],
        serde_json::json!([{"id":1,"source_id":3}])
    );
    assert_eq!(
        rerun_outcomes["summary"],
        serde_json::json!({"killed":1,"missed":0,"timeout":0,"build_failed":0,"errors":0})
    );
    let log = fs::read_to_string(rerun_dir.join("logs/000001.log")).unwrap();
    assert!(
        log.contains("mixed_inputs_are_accepted ... FAILED"),
        "{log}"
    );
    assert_eq!(files(&fixture.root), stronger);
    assert_eq!(files(&directory), saved);
    let validation = Command::new(env!("CARGO_BIN_EXE_mutie"))
        .arg("prune")
        .arg(fixture.temp.path().join("reports"))
        .args(["--keep-last", "0"])
        .output()
        .unwrap();
    assert!(validation.status.success(), "{validation:?}");
}
