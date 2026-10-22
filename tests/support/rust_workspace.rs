use super::*;

#[test]
#[ignore = "runs real Cargo workspace mutations, cross-member tests and cache/rerun controls"]
fn full_workspace_tests_catch_selected_member_changes_and_preserve_identity() {
    let fixture = Fixture::from_fixture("rust-workspace");
    let source = fs::read(fixture.root.join("crates/core/src/lib.rs")).unwrap();
    let manifest = fs::read(fixture.root.join("Cargo.toml")).unwrap();
    let lock = fs::read(fixture.root.join("Cargo.lock")).unwrap();
    let mut original = None;
    for mode in ["fresh", "worker"] {
        let output = fixture.run(&[
            "--rust-package",
            "workspace-core",
            "--rust-build-cache",
            mode,
        ]);
        assert!(output.status.success(), "{output:?}");
        let (directory, run, inventory, outcomes) = report(&output);
        assert_eq!(run["status"], "completed");
        assert_eq!(run["score"]["scope"], "filtered");
        assert_eq!(
            run["rust_scope"]["workspace"]["selected"],
            serde_json::json!(["workspace-core"])
        );
        assert!(
            run["baseline_command"]
                .as_array()
                .unwrap()
                .contains(&serde_json::json!("--workspace"))
        );
        assert_eq!(inventory["mutations"].as_array().unwrap().len(), 3);
        assert_eq!(
            outcomes["summary"],
            serde_json::json!({"killed":1,"missed":2,"timeout":0,"build_failed":0,"errors":0})
        );
        for outcome in outcomes["mutations"].as_array().unwrap() {
            let stdout =
                fs::read_to_string(directory.join(outcome["logs"]["stdout"].as_str().unwrap()))
                    .unwrap();
            let compiled: Vec<Value> = stdout
                .split("--- Cargo test execution ---")
                .next()
                .unwrap()
                .lines()
                .filter_map(|line| serde_json::from_str::<Value>(line).ok())
                .filter(|event| event["reason"] == "compiler-artifact" && event["fresh"] == false)
                .collect();
            for package in ["workspace_core", "workspace_consumer"] {
                assert!(
                    compiled
                        .iter()
                        .any(|event| event["target"]["name"] == package),
                    "{mode}: did not rebuild {package}: {stdout}"
                );
            }
        }
        assert_eq!(
            fs::read(fixture.root.join("crates/core/src/lib.rs")).unwrap(),
            source
        );
        assert_eq!(fs::read(fixture.root.join("Cargo.toml")).unwrap(), manifest);
        assert_eq!(fs::read(fixture.root.join("Cargo.lock")).unwrap(), lock);
        original = Some((directory, outcomes));
    }
    let (directory, original_outcomes) = original.unwrap();
    let saved = fs::read(directory.join("outcomes.json")).unwrap();
    let stronger = Fixture::from_fixture("rust-workspace");
    let test = stronger.root.join("crates/consumer/tests/cross_member.rs");
    fs::write(
        &test,
        format!(
            "{}\n#[test]\nfn fallback_boundary() {{ assert!(workspace_core::fallback(false)); }}\n",
            fs::read_to_string(&test).unwrap()
        ),
    )
    .unwrap();
    let output = stronger.run(&[
        "--rust-package",
        "workspace-core",
        "--rerun-missed",
        directory.to_str().unwrap(),
        "--rust-build-cache",
        "worker",
    ]);
    assert!(output.status.success(), "{output:?}");
    let (_, run, _, outcomes) = report(&output);
    assert_eq!(
        outcomes["summary"],
        serde_json::json!({"killed":2,"missed":0,"timeout":0,"build_failed":0,"errors":0})
    );
    assert_eq!(run["score"]["scope"], "rerun");
    assert_eq!(fs::read(directory.join("outcomes.json")).unwrap(), saved);
    assert_eq!(original_outcomes["summary"]["missed"], 2);
    let changed = stronger.root.join("crates/consumer/Cargo.toml");
    fs::write(
        &changed,
        format!(
            "{}\n# changed member manifest\n",
            fs::read_to_string(&changed).unwrap()
        ),
    )
    .unwrap();
    let drift = stronger.run(&[
        "--rust-package",
        "workspace-core",
        "--rerun-missed",
        directory.to_str().unwrap(),
    ]);
    assert!(!drift.status.success());
    assert!(
        String::from_utf8_lossy(&drift.stderr)
            .contains("workspace membership, selection or manifests changed"),
        "{drift:?}"
    );
    assert_eq!(fs::read(directory.join("outcomes.json")).unwrap(), saved);
}
