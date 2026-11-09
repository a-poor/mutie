use super::*;

fn write_json(path: &Path, value: &Value) {
    fs::write(path, serde_json::to_vec_pretty(value).unwrap()).unwrap();
}

fn failed_before_baseline(output: &Output, expected: &str) {
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(
        String::from_utf8_lossy(&output.stderr).contains(expected),
        "{output:?}"
    );
    let (_, run, inventory, outcomes) = report(output);
    assert_eq!(run["status"], "failed");
    assert!(outcomes["baseline"].is_null());
    assert_eq!(inventory["complete"], false);
    assert!(outcomes["mutations"].as_array().unwrap().is_empty());
}

#[test]
#[ignore = "runs real Cargo reruns and scope checks; opt-in to avoid recursive self-testing"]
fn rust_reruns_validate_scope_source_partial_runs_and_moved_checkouts() {
    let mut fixture = Fixture::new();
    let first = fixture.run(&[]);
    assert!(first.status.success(), "{first:?}");
    let (directory, run, inventory, outcomes) = report(&first);
    let saved: BTreeMap<_, _> = ["run.json", "mutants.json", "outcomes.json"]
        .into_iter()
        .map(|name| (name, fs::read(directory.join(name)).unwrap()))
        .collect();
    let source = directory.to_str().unwrap();
    let flags = fixture.root.join("src/flags.rs");
    let original = fs::read(&flags).unwrap();

    // Selection cannot silently skip a stale, missing or excluded target.
    let mut changed = original.clone();
    changed.extend_from_slice(b"\n// source edit\n");
    fs::write(&flags, &changed).unwrap();
    let stale = fixture.run(&["--rerun", source, "--id", "3", "--list"]);
    assert!(!stale.status.success());
    assert!(
        String::from_utf8_lossy(&stale.stderr).contains("fingerprint changed"),
        "{stale:?}"
    );
    assert_eq!(fs::read(&flags).unwrap(), changed);
    fs::write(&flags, &original).unwrap();
    let excluded = fixture.run(&[
        "--rerun",
        source,
        "--id",
        "3",
        "--exclude",
        "src/flags.rs",
        "--list",
    ]);
    assert!(!excluded.status.success());
    assert!(
        String::from_utf8_lossy(&excluded.stderr).contains("missing from current discovery"),
        "{excluded:?}"
    );

    // Manifest and lock identity are checked before any baseline runs, even if
    // the edit is only a comment and Cargo accepts it.
    for (file, expected) in [
        ("Cargo.toml", "Cargo.toml fingerprint"),
        ("Cargo.lock", "Cargo.lock fingerprint"),
    ] {
        let path = fixture.root.join(file);
        let contents = fs::read_to_string(&path).unwrap();
        fs::write(&path, format!("{contents}\n# changed identity\n")).unwrap();
        let output = fixture.run(&["--rerun", source, "--id", "3"]);
        failed_before_baseline(&output, expected);
        fs::write(&path, contents).unwrap();
    }

    let altered = fixture.temp.path().join("altered report");
    fs::create_dir(&altered).unwrap();
    write_json(&altered.join("mutants.json"), &inventory);
    write_json(&altered.join("outcomes.json"), &outcomes);
    // Reports written before target inventories normalize to the same identity
    // and can still execute a focused rerun when the strict evidence is present.
    let mut legacy = run.clone();
    let scope = legacy["rust_scope"].as_object_mut().unwrap();
    let targets = scope.remove("targets").unwrap();
    scope.insert("binary".into(), targets[0]["name"].clone());
    scope.insert("source".into(), targets[0]["source"].clone());
    scope.insert("edition".into(), targets[0]["edition"].clone());
    write_json(&altered.join("run.json"), &legacy);
    let legacy_rerun = fixture.run(&["--rerun", altered.to_str().unwrap(), "--id", "3"]);
    assert!(legacy_rerun.status.success(), "{legacy_rerun:?}");
    let (_, rewritten, _, rerun_outcomes) = report(&legacy_rerun);
    assert_eq!(
        rewritten["rust_scope"]["targets"],
        run["rust_scope"]["targets"]
    );
    assert_eq!(rerun_outcomes["mutations"].as_array().unwrap().len(), 1);
    fixture.preserved();
    for field in ["rustc", "cargo"] {
        let mut changed = run.clone();
        changed["rust_scope"][field] = "different toolchain".into();
        write_json(&altered.join("run.json"), &changed);
        let output = fixture.run(&["--rerun", altered.to_str().unwrap(), "--id", "3"]);
        failed_before_baseline(&output, "toolchain changed");
    }

    // Old Rust reports remain readable, but lack the manifest evidence required
    // for reruns. Language mismatches and unknown IDs fail before starting tools
    // or creating a report, including when the final filter would be empty.
    for (case, expected) in [
        ("legacy", "lacks manifest"),
        ("go", "does not match"),
        ("id", "unknown source mutation ID"),
    ] {
        let mut changed = run.clone();
        if case == "legacy" {
            changed["rust_scope"]
                .as_object_mut()
                .unwrap()
                .remove("manifest_fingerprint");
        } else if case == "go" {
            changed["effective_config"]["lang"] = "go".into();
            changed.as_object_mut().unwrap().remove("rust_scope");
        }
        write_json(&altered.join("run.json"), &changed);
        let output = fixture
            .command(&[
                "--rerun-missed",
                altered.to_str().unwrap(),
                "--id",
                if case == "id" { "999" } else { "1" },
            ])
            .env("PATH", "")
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(expected),
            "{case}: {output:?}"
        );
        assert!(!String::from_utf8_lossy(&output.stdout).contains("Saving report"));
        fixture.preserved();
    }
    fs::write(fixture.root.join("go.mod"), "module example.com/other\n").unwrap();
    let config = fixture.root.join("mutie.toml");
    let original_config = fs::read(&config).unwrap();
    fs::write(&config, [original_config.as_slice(), b"\n[go]\n"].concat()).unwrap();
    let mismatch = std::process::Command::new(env!("CARGO_BIN_EXE_mutie"))
        .arg("--path")
        .arg(&fixture.root)
        .args(["rerun", source, "--lang", "go"])
        .env("PATH", "")
        .output()
        .unwrap();
    assert!(
        String::from_utf8_lossy(&mismatch.stderr).contains("does not match"),
        "{mismatch:?}"
    );
    fs::write(&config, original_config).unwrap();
    fs::remove_file(fixture.root.join("go.mod")).unwrap();

    // Simulate a finalized partial report. General selection retries unfinished
    // work; missed-only selection ignores both pending and interrupted work.
    let mut partial_run = run.clone();
    partial_run["status"] = "interrupted".into();
    partial_run.as_object_mut().unwrap().remove("score");
    partial_run.as_object_mut().unwrap().remove("score_policy");
    let mut partial_outcomes = outcomes.clone();
    partial_outcomes["status"] = "interrupted".into();
    for (index, state) in [(0, "pending"), (1, "interrupted")] {
        partial_outcomes["mutations"][index]["state"] = state.into();
        partial_outcomes["mutations"][index]["outcome"] = Value::Null;
    }
    write_json(&altered.join("run.json"), &partial_run);
    write_json(&altered.join("outcomes.json"), &partial_outcomes);
    for (mode, count) in [("--rerun", 3), ("--rerun-missed", 1)] {
        let output = fixture.run(&[mode, altered.to_str().unwrap(), "--list"]);
        assert!(output.status.success(), "{output:?}");
        assert!(
            String::from_utf8_lossy(&output.stdout).contains(&format!("Found {count} mutations"))
        );
    }
    let retry = fixture.run(&[
        "--rerun",
        altered.to_str().unwrap(),
        "--id",
        "2,1",
        "--jobs",
        "2",
    ]);
    assert!(retry.status.success(), "{retry:?}");
    let (_, retried, _, results) = report(&retry);
    assert_eq!(retried["rerun"]["source_status"], "interrupted");
    assert_eq!(
        retried["rerun"]["mutations"],
        serde_json::json!([{"id":1,"source_id":1},{"id":2,"source_id":2}])
    );
    assert_eq!(results["summary"]["killed"], 2);

    // A new failing integration test must block mutation execution.
    let broken = fixture.root.join("tests/broken.rs");
    fs::write(
        &broken,
        "#[test] fn broken() { panic!(\"new failing test\"); }\n",
    )
    .unwrap();
    let failed = fixture.run(&["--rerun-missed", source]);
    assert!(!failed.status.success());
    let (_, _, failed_inventory, failed_results) = report(&failed);
    assert_eq!(failed_inventory["complete"], false);
    assert_eq!(failed_results["baseline"]["outcome"], "failed");
    assert!(failed_results["mutations"].as_array().unwrap().is_empty());
    fs::remove_file(broken).unwrap();

    let moved = fixture.temp.path().join("moved checkout");
    fs::rename(&fixture.root, &moved).unwrap();
    fixture.root = moved;
    let moved_run = fixture.run(&["--rerun-missed", source]);
    assert!(moved_run.status.success(), "{moved_run:?}");
    assert_eq!(report(&moved_run).3["summary"]["missed"], 1);
    for (name, bytes) in saved {
        assert_eq!(
            fs::read(directory.join(name)).unwrap(),
            bytes,
            "source report {name} changed"
        );
    }
    assert_eq!(
        fs::read(fixture.root.join("src/flags.rs")).unwrap(),
        original
    );
}
