use super::*;

#[test]
#[ignore = "runs real Cargo call removal, serial/cached comparisons and stronger-test reruns"]
fn rust_calls_preserve_scores_identity_and_stronger_reruns() {
    let fixture = Fixture::from_fixture("rust-calls");
    fs::write(
        fixture.root.join("mutie.toml"),
        "[rust]\noperators=['call_remove']\n",
    )
    .unwrap();
    let original = files(&fixture.root);
    let listing = fixture.run(&["--lang", "rust", "--list"]);
    assert!(listing.status.success(), "{listing:?}");
    assert!(String::from_utf8_lossy(&listing.stdout).contains("Found 3 mutations"));
    let expected =
        serde_json::json!({"killed":1,"missed":1,"timeout":0,"build_failed":1,"errors":0});
    let serial = fixture.run(&["--lang", "rust", "--jobs", "1"]);
    assert!(serial.status.success(), "{serial:?}");
    let (serial_dir, _, _, serial_outcomes) = report(&serial);
    assert_eq!(serial_outcomes["summary"], expected);
    let serial_saved = files(&serial_dir);
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
    assert_eq!(outcomes["summary"], expected);
    assert_eq!(outcomes["mutations"][2]["outcome"], "build_failed");
    assert_eq!(run["score"]["percentage"], 50.0);
    for (a, b) in serial_outcomes["mutations"]
        .as_array()
        .unwrap()
        .iter()
        .zip(outcomes["mutations"].as_array().unwrap())
    {
        assert_eq!(a["outcome"], b["outcome"]);
    }
    for mutation in inventory["mutations"].as_array().unwrap() {
        assert_eq!(mutation["operator"], "call_remove");
        assert_eq!(mutation["replacement"], "{}");
        assert_eq!(mutation["file"], "src/lib.rs");
    }
    assert_eq!(files(&fixture.root), original);
    let saved = files(&directory);
    let inventory_path = directory.join("mutants.json");
    let inventory_bytes = fs::read(&inventory_path).unwrap();
    for (key, value) in [
        ("replacement", ";"),
        ("replacement", "{panic!()}"),
        ("original", "f()?;"),
        ("original", "f(); g();"),
        ("file", "src/lib.js"),
    ] {
        let mut corrupted = inventory.clone();
        corrupted["mutations"][1][key] = serde_json::json!(value);
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
            String::from_utf8_lossy(&rejected.stderr).contains("saved Rust call mutation"),
            "{rejected:?}"
        );
    }
    fs::write(inventory_path, inventory_bytes).unwrap();
    fs::write(fixture.root.join("tests/stronger.rs"), "use mutie_rust_calls::*;\n#[test] fn included_case() { assert_eq!(weak(true),vec![9]); }\n").unwrap();
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
        serde_json::json!([{"id":1,"source_id":2}])
    );
    assert_eq!(
        rerun_outcomes["summary"],
        serde_json::json!({"killed":1,"missed":0,"timeout":0,"build_failed":0,"errors":0})
    );
    assert_eq!(files(&fixture.root), stronger);
    assert_eq!(files(&directory), saved);
    assert_eq!(files(&serial_dir), serial_saved);
    let source = fixture.root.join("src/lib.rs");
    let bytes = fs::read(&source).unwrap();
    fs::write(&source, [bytes.as_slice(), b"\n// source drift\n"].concat()).unwrap();
    let stale = fixture.run(&[
        "--lang",
        "rust",
        "--rerun-missed",
        directory.to_str().unwrap(),
        "--list",
    ]);
    assert!(!stale.status.success(), "{stale:?}");
    fs::write(source, bytes).unwrap();
    assert_eq!(files(&fixture.root), stronger);
    assert_eq!(files(&directory), saved);
}
