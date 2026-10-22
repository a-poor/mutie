use super::*;

#[test]
#[ignore = "runs real Cargo arithmetic mutations and stronger-test reruns"]
fn rust_arithmetic_scores_panics_separately_from_build_failures_and_reruns() {
    let fixture = Fixture::from_fixture("rust-arithmetic");
    fs::write(
        fixture.root.join("mutie.toml"),
        "operators = [\"arithmetic_replace\"]\n",
    )
    .unwrap();
    let original = files(&fixture.root);
    let listing = fixture.run(&["--lang", "rust", "--list"]);
    assert!(listing.status.success(), "{listing:?}");
    assert!(String::from_utf8_lossy(&listing.stdout).contains("Found 10 mutations"));
    let output = fixture.run(&[
        "--lang",
        "rust",
        "--jobs",
        "2",
        "--rust-build-cache",
        "worker",
    ]);
    assert!(output.status.success(), "{output:?}");
    let (directory, run, inventory, outcomes) = report(&output);
    assert_eq!(run["status"], "completed");
    assert_eq!(
        outcomes["summary"],
        serde_json::json!({"killed":7,"missed":2,"timeout":0,"build_failed":1,"errors":0}),
        "{output:?}"
    );
    assert_eq!(outcomes["mutations"][7]["outcome"], "killed");
    assert_eq!(outcomes["mutations"][8]["outcome"], "killed");
    assert_eq!(outcomes["mutations"][9]["outcome"], "build_failed");
    for m in inventory["mutations"].as_array().unwrap() {
        assert_eq!(m["operator"], "arithmetic_replace");
        assert_eq!(m["file"], "src/lib.rs");
        assert_eq!(m["original"].as_str().unwrap().len(), 1);
    }
    assert_eq!(files(&fixture.root), original);
    let saved = files(&directory);
    // Invalid arithmetic recipes are rejected by the saved-report reader before rerun.
    let inventory_path = directory.join("mutants.json");
    let inventory_bytes = fs::read(&inventory_path).unwrap();
    let mut corrupted = inventory.clone();
    corrupted["mutations"][0]["replacement"] = serde_json::json!("*");
    fs::write(
        &inventory_path,
        serde_json::to_vec_pretty(&corrupted).unwrap(),
    )
    .unwrap();
    let rejected = fixture.run(&[
        "--lang",
        "rust",
        "--rerun-missed",
        directory.to_str().unwrap(),
        "--list",
    ]);
    assert!(!rejected.status.success());
    assert!(
        String::from_utf8_lossy(&rejected.stderr).contains("saved Rust arithmetic mutation"),
        "{rejected:?}"
    );
    fs::write(inventory_path, inventory_bytes).unwrap();
    fs::write(fixture.root.join("tests/stronger.rs"), "use mutie_rust_arithmetic::*;\n#[test] fn nonzero_operands() { assert_eq!(weak_add(7,2),9); assert_eq!(weak_remainder(7,2),1); }\n").unwrap();
    let stronger = files(&fixture.root);
    let rerun = fixture.run(&[
        "--lang",
        "rust",
        "--rerun-missed",
        directory.to_str().unwrap(),
        "--rust-build-cache",
        "worker",
        "--min-score",
        "100",
    ]);
    assert!(rerun.status.success(), "{rerun:?}");
    let (_, rerun_run, _, rerun_outcomes) = report(&rerun);
    assert_eq!(
        rerun_run["rerun"]["mutations"],
        serde_json::json!([{"id":1,"source_id":6},{"id":2,"source_id":7}])
    );
    assert_eq!(
        rerun_outcomes["summary"],
        serde_json::json!({"killed":2,"missed":0,"timeout":0,"build_failed":0,"errors":0})
    );
    assert_eq!(files(&fixture.root), stronger);
    assert_eq!(files(&directory), saved);
}
