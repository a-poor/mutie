use super::*;

const CAPTURE_LIMIT: u64 = 65_536;

fn raw_bytes(logs: &Value) -> u64 {
    logs["retention"]["stdout"]["total_bytes"].as_u64().unwrap()
        + logs["retention"]["stderr"]["total_bytes"].as_u64().unwrap()
}

fn check_logs(directory: &Path, logs: &Value) {
    for stream in ["text", "stdout", "stderr"] {
        assert!(directory.join(logs[stream].as_str().unwrap()).is_file());
        let retention = &logs["retention"][stream];
        assert!(
            retention["head_bytes"].as_u64().unwrap() + retention["tail_bytes"].as_u64().unwrap()
                <= 64
        );
    }
}

#[test]
#[ignore = "injects Cargo tool/protocol faults around real builds/tests; opt-in to avoid recursive self-testing"]
fn cargo_faults_preserve_partial_results_and_stop_parallel_workers() {
    for (phase, fault, diagnostic) in [
        (
            "clean",
            "exit",
            "Cargo package cleanup failed before compilation",
        ),
        (
            "clean",
            "signal",
            "Cargo package cleanup failed before compilation",
        ),
        (
            "clean",
            "spawn",
            "starting Cargo compilation after cleanup: could not start cargo",
        ),
        ("clean", "overflow-mixed", "--max-capture-bytes"),
        (
            "build",
            "missing-build",
            "no successful build-finished event",
        ),
        (
            "tests",
            "missing-build",
            "without build and standard test-harness completion evidence",
        ),
        (
            "tests",
            "missing-tests",
            "without build and standard test-harness completion evidence",
        ),
        (
            "tests",
            "exit",
            "command failed without a test or build failure",
        ),
        (
            "tests",
            "signal",
            "command failed without a test or build failure",
        ),
        (
            "build",
            "spawn",
            "starting Cargo tests after compilation: could not start cargo",
        ),
        ("build", "overflow-err", "--max-capture-bytes"),
        ("tests", "overflow-out", "--max-capture-bytes"),
        ("tests", "overflow-mixed", "--max-capture-bytes"),
    ] {
        let fixture = CompilationFixture::new();
        let flags = fixture.fixture.root.join("src/flags.rs");
        fs::write(
            &flags,
            format!(
                "{}\npub fn pending_flag() -> bool {{ true }}\n",
                fs::read_to_string(&flags).unwrap()
            ),
        )
        .unwrap();
        let inputs = files(&fixture.fixture.root);
        let mut child = fixture.start(
            &format!("fault:c:{phase}:{fault}"),
            &[
                "--jobs",
                "2",
                "--rust-build-cache",
                if phase == "clean" { "worker" } else { "fresh" },
                "--timeout",
                "120",
                "--max-capture-bytes",
                "65536",
                "--max-log-bytes",
                "64",
                "--fail-on-missed",
                "--min-score",
                "100",
            ],
        );
        child.ready(&["b", "fault"]);
        let reports: Vec<_> = fs::read_dir(fixture.fixture.temp.path().join("reports"))
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect();
        assert_eq!(reports.len(), 1);
        let before = files(&reports[0]);
        let previous: Value = serde_json::from_slice(&before[Path::new("outcomes.json")]).unwrap();
        assert_eq!(previous["mutations"][0]["outcome"], "killed");
        for index in [1, 2] {
            assert_eq!(previous["mutations"][index]["state"], "running");
        }
        assert_eq!(previous["mutations"][3]["state"], "pending");
        fs::write(child.state.join("release-fault"), "fail").unwrap();
        let output = child.finish(Duration::from_secs(20));
        assert_eq!(output.status.code(), Some(1), "{phase}/{fault}: {output:?}");
        let (directory, run, inventory, outcomes) = report(&output);
        assert_eq!(run["status"], "failed", "{phase}/{fault}: {run}");
        assert_eq!(run["checkpoint"], outcomes["checkpoint"]);
        assert!(inventory["complete"].as_bool().unwrap());
        check_unevaluated(&run);
        assert_eq!(outcomes["baseline"], previous["baseline"]);
        assert_eq!(outcomes["mutations"][0], previous["mutations"][0]);
        assert_eq!(outcomes["mutations"][1]["state"], "interrupted");
        assert!(outcomes["mutations"][1]["outcome"].is_null());
        assert_eq!(outcomes["mutations"][3], previous["mutations"][3]);
        assert!(!child.state.join("d.build.started").exists());
        let failed = &outcomes["mutations"][2];
        assert_eq!(failed["state"], "completed");
        assert_eq!(failed["outcome"], "error", "{phase}/{fault}: {failed}");
        assert!(
            failed["error"].as_str().unwrap().contains(diagnostic),
            "{phase}/{fault}: {failed}"
        );
        if fault == "exit" {
            assert_eq!(failed["exit_code"], 17);
        } else if fault == "signal" {
            assert!(failed["exit_code"].is_null());
        }
        check_logs(&directory, &failed["logs"]);
        check_logs(&directory, &outcomes["mutations"][1]["logs"]);
        if phase == "build" || phase == "clean" {
            assert!(!child.state.join("c.tests.started").exists());
        }
        if phase == "clean" {
            assert!(!child.state.join("c.build.started").exists());
        }
        if fault.starts_with("overflow") {
            let bytes = raw_bytes(&failed["logs"]);
            if phase == "build" || phase == "clean" {
                assert_eq!(bytes, CAPTURE_LIMIT);
            } else {
                // Separate limits for compile and execution, plus the two
                // separators Mutie adds while merging stdout/stderr logs.
                let separators = 2 * b"\n--- Cargo test execution ---\n".len() as u64;
                assert!(
                    bytes > CAPTURE_LIMIT && bytes <= CAPTURE_LIMIT * 2 + separators,
                    "{bytes}"
                );
            }
        }
        assert_eq!(
            outcomes["summary"],
            serde_json::json!({"killed":1,"missed":0,"timeout":0,"build_failed":0,"errors":1})
        );
        for (path, bytes) in &before {
            if path == Path::new("mutants.json")
                || path.to_str().unwrap().starts_with("logs/000001")
            {
                assert_eq!(&fs::read(directory.join(path)).unwrap(), bytes);
            }
        }
        fixture.check_cleanup(&child, &inputs);
        fixture.check_report(&directory);
    }
}

#[test]
#[ignore = "injects Cargo discovery/baseline failures and overflow; opt-in to avoid recursive self-testing"]
fn cargo_faults_stop_discovery_and_baseline_without_mutations() {
    for (phase, fault, diagnostic) in [
        ("metadata", "malformed-metadata", "decoding Cargo metadata"),
        ("metadata", "overflow-out", "--max-capture-bytes"),
        (
            "build",
            "malformed-build",
            "no successful build-finished event",
        ),
        ("tests", "signal", "baseline evidence invalid"),
        ("build", "overflow-mixed", "--max-capture-bytes"),
        ("tests", "overflow-err", "--max-capture-bytes"),
    ] {
        let fixture = CompilationFixture::new();
        let inputs = files(&fixture.fixture.root);
        let mut child = fixture.start(
            &format!("fault:baseline:{phase}:{fault}"),
            &[
                "--max-capture-bytes",
                "65536",
                "--max-log-bytes",
                "64",
                "--fail-on-missed",
                "--min-score",
                "100",
            ],
        );
        let output = child.finish(Duration::from_secs(90));
        assert_eq!(output.status.code(), Some(1), "{phase}/{fault}: {output:?}");
        let (directory, run, inventory, outcomes) = report(&output);
        assert_eq!(run["status"], "failed");
        assert!(
            run["error"].as_str().unwrap().contains(diagnostic),
            "{phase}/{fault}: {run}"
        );
        check_unevaluated(&run);
        assert_eq!(inventory["complete"], false);
        assert!(outcomes["mutations"].as_array().unwrap().is_empty());
        let command = &outcomes[if phase == "metadata" {
            "discovery"
        } else {
            "baseline"
        }];
        check_logs(&directory, &command["logs"]);
        if fault == "signal" {
            assert!(command["exit_code"].is_null());
            assert!(
                run["error"]
                    .as_str()
                    .unwrap()
                    .contains("without a test or build failure")
            );
        }
        if fault.starts_with("overflow") {
            assert_eq!(command["outcome"], "failed");
            assert!(
                command["capture_error"]
                    .as_str()
                    .unwrap()
                    .contains("--max-capture-bytes")
            );
            let bytes = raw_bytes(&command["logs"]);
            if phase == "tests" {
                assert!(
                    bytes > CAPTURE_LIMIT
                        && bytes
                            <= CAPTURE_LIMIT * 2
                                + 2 * b"\n--- Cargo test execution ---\n".len() as u64
                );
            } else {
                assert_eq!(bytes, CAPTURE_LIMIT);
            }
        }
        if phase != "tests" {
            assert!(!child.state.join("baseline.tests.started").exists());
        }
        assert!(!child.state.join("a.build.started").exists());
        fixture.check_cleanup(&child, &inputs);
        fixture.check_report(&directory);
    }
}

#[test]
#[ignore = "checks Rust Cargo startup failures; opt-in with other Cargo workflows"]
fn cargo_startup_failure_preserves_inputs_and_saves_the_error() {
    let fixture = CompilationFixture::new();
    let inputs = files(&fixture.fixture.root);
    let broken = fixture.bin.join("broken-cargo");
    std::os::unix::fs::symlink("cargo", &broken).unwrap();
    fs::rename(broken, fixture.bin.join("cargo")).unwrap();
    let mut child = fixture.start("startup", &["--fail-on-missed", "--min-score", "100"]);
    let output = child.finish(Duration::from_secs(15));
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let (directory, run, inventory, outcomes) = report(&output);
    assert_eq!(run["status"], "failed");
    assert!(
        run["error"]
            .as_str()
            .unwrap()
            .contains("could not start cargo")
    );
    check_unevaluated(&run);
    assert_eq!(inventory["complete"], false);
    assert!(outcomes["mutations"].as_array().unwrap().is_empty());
    assert!(outcomes["discovery"].is_null());
    fixture.fixture.preserved();
    assert_eq!(files(&fixture.fixture.root), inputs);
    fixture.check_report(&directory);
}

#[test]
#[ignore = "runs real Cargo metadata with overflowing listing output; opt-in with other Cargo workflows"]
fn cargo_listing_overflow_does_not_publish_partial_inventory() {
    let fixture = CompilationFixture::new();
    let inputs = files(&fixture.fixture.root);
    let mut child = fixture.start(
        "fault:baseline:metadata:overflow-out",
        &[
            "--list",
            "--max-capture-bytes",
            "65536",
            "--max-log-bytes",
            "1",
        ],
    );
    let output = child.finish(Duration::from_secs(30));
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("--max-capture-bytes"),
        "{output:?}"
    );
    assert!(!String::from_utf8_lossy(&output.stdout).contains("Found "));
    assert!(!fixture.fixture.temp.path().join("reports").exists());
    assert!(!child.state.join("baseline.build.started").exists());
    fixture.check_cleanup(&child, &inputs);
}
