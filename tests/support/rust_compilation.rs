use super::*;

#[path = "rust_failures.rs"]
mod failures;

struct CompilationFixture {
    fixture: Fixture,
    bin: PathBuf,
    compiler: PathBuf,
}

impl CompilationFixture {
    fn new() -> Self {
        let fixture = Fixture::new();
        let sysroot = Command::new("rustc")
            .args(["--print", "sysroot"])
            .output()
            .unwrap();
        assert!(sysroot.status.success(), "{sysroot:?}");
        let compiler =
            PathBuf::from(String::from_utf8(sysroot.stdout).unwrap().trim()).join("bin/rustc");
        let bin = fixture.temp.path().join("bin");
        fs::create_dir(&bin).unwrap();
        let output = Command::new(&compiler)
            .args([
                "--edition=2024",
                "--crate-name",
                "compiler_gate",
                "-D",
                "warnings",
            ])
            .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/support/compiler_gate.rs"))
            .arg("-o")
            .arg(bin.join("cargo"))
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
        fs::hard_link(bin.join("cargo"), bin.join("rustc")).unwrap();
        Self {
            fixture,
            bin,
            compiler,
        }
    }

    fn start(&self, case: &str, args: &[&str]) -> InterruptibleRun {
        let mut command = self.fixture.command(args);
        let mut paths = vec![self.bin.clone()];
        for (index, path) in std::env::split_paths(&std::env::var_os("PATH").unwrap()).enumerate() {
            if (case == "startup" || case.ends_with(":spawn")) && path.join("cargo").exists() {
                // Keep linker/helper tools available while removing fallback
                // Cargo binaries. Some platforms skip unusable PATH entries.
                let mirror = self.fixture.temp.path().join(format!("path-{index}"));
                fs::create_dir(&mirror).unwrap();
                for entry in fs::read_dir(&path).unwrap() {
                    let entry = entry.unwrap();
                    if entry.file_name() != "cargo" {
                        std::os::unix::fs::symlink(entry.path(), mirror.join(entry.file_name()))
                            .unwrap();
                    }
                }
                paths.push(mirror);
            } else {
                paths.push(path);
            }
        }
        command
            .env("PATH", std::env::join_paths(paths).unwrap())
            .env("MUTIE_TEST_REAL_CARGO", env!("CARGO"))
            .env("MUTIE_TEST_REAL_RUSTC", &self.compiler)
            .env("MUTIE_TEST_COMPILE_CASE", case);
        InterruptibleRun::start_command(&self.fixture, command)
    }

    fn check_cleanup(&self, child: &InterruptibleRun, inputs: &BTreeMap<PathBuf, Vec<u8>>) {
        let pids: Vec<i32> = fs::read_dir(&child.state)
            .unwrap()
            .filter_map(|entry| {
                let path = entry.unwrap().path();
                (path.extension()? == "pid")
                    .then(|| fs::read_to_string(path).unwrap().trim().parse().unwrap())
            })
            .collect();
        assert!(!pids.is_empty());
        let deadline = Instant::now() + Duration::from_secs(10);
        while pids.iter().any(|pid| process_running(*pid)) {
            assert!(
                Instant::now() < deadline,
                "build/test processes leaked: {pids:?}"
            );
            thread::sleep(Duration::from_millis(20));
        }
        for entry in fs::read_dir(&child.state).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().is_some_and(|extension| {
                matches!(extension.to_str(), Some("workspace" | "target" | "tmpdir"))
            }) {
                let owned = PathBuf::from(fs::read_to_string(path).unwrap());
                assert!(
                    owned.starts_with(self.fixture.temp.path().join("owned")),
                    "{owned:?}"
                );
                assert!(!owned.exists(), "build artifacts leaked: {owned:?}");
            }
        }
        self.fixture.preserved();
        assert_eq!(&files(&self.fixture.root), inputs);
    }

    fn check_report(&self, directory: &Path) {
        let saved = files(directory);
        let output = Command::new(env!("CARGO_BIN_EXE_mutie"))
            .arg("prune")
            .arg(self.fixture.temp.path().join("reports"))
            .args(["--keep-last", "0"])
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
        assert!(
            String::from_utf8_lossy(&output.stdout).contains("Selected runs: 1"),
            "{output:?}"
        );
        assert_eq!(files(directory), saved);
    }
}

fn check_build_logs(directory: &Path, logs: &Value, label: &str) {
    let log = fs::read_to_string(directory.join(logs["text"].as_str().unwrap())).unwrap();
    assert!(
        log.contains(&format!("Cargo build checkpoint {label}")),
        "{log}"
    );
    assert!(
        log.contains(&format!("compiler checkpoint {label}")),
        "{log}"
    );
    assert!(
        !log.contains("--- Cargo test execution ---"),
        "tests launched after stopped compilation: {log}"
    );
}

fn check_unevaluated(run: &Value) {
    assert_eq!(run["policy"]["status"], "not_evaluated");
    assert_eq!(run["score_policy"]["status"], "not_evaluated");
    assert!(run["score"]["percentage"].is_null());
}

#[test]
#[ignore = "runs real Cargo/rustc and interrupts compilation; opt-in to avoid recursive self-testing"]
fn compilation_cancellation_preserves_baseline_and_parallel_partial_results() {
    for (case, signal, active, mode) in [
        ("baseline", libc::SIGTERM, &["baseline"][..], "fresh"),
        ("mutations", libc::SIGINT, &["b", "c"][..], "fresh"),
        ("mutations", libc::SIGTERM, &["b", "c"][..], "worker"),
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
            case,
            &[
                "--jobs",
                "2",
                "--rust-build-cache",
                mode,
                "--timeout",
                "120",
                "--fail-on-missed",
                "--min-score",
                "100",
            ],
        );
        child.ready(active);
        for label in active {
            for kind in ["build", "proxy", "compiler", "descendant"] {
                let pid = fs::read_to_string(child.state.join(format!("{label}.{kind}.pid")))
                    .unwrap()
                    .trim()
                    .parse()
                    .unwrap();
                assert!(
                    process_running(pid),
                    "{label} {kind} exited before interruption"
                );
            }
            assert!(!child.state.join(format!("{label}.tests.started")).exists());
        }
        let reports: Vec<_> = fs::read_dir(fixture.fixture.temp.path().join("reports"))
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect();
        assert_eq!(reports.len(), 1);
        let before = files(&reports[0]);
        let previous: Value = serde_json::from_slice(&before[Path::new("outcomes.json")]).unwrap();
        if case == "mutations" {
            assert_eq!(previous["mutations"][0]["outcome"], "killed");
            assert_eq!(previous["mutations"][1]["state"], "running");
            assert_eq!(previous["mutations"][2]["state"], "running");
            assert_eq!(previous["mutations"][3]["state"], "pending");
        }
        let output = child.interrupt(signal);
        assert_eq!(output.status.code(), Some(1), "{output:?}");
        let (directory, run, inventory, outcomes) = report(&output);
        assert_eq!(run["status"], "interrupted");
        assert_eq!(run["checkpoint"], outcomes["checkpoint"]);
        check_unevaluated(&run);
        if case == "baseline" {
            assert_eq!(inventory["complete"], false);
            assert_eq!(outcomes["baseline"]["outcome"], "interrupted");
            assert!(outcomes["mutations"].as_array().unwrap().is_empty());
            check_build_logs(&directory, &outcomes["baseline"]["logs"], "baseline");
        } else {
            assert_eq!(inventory["complete"], true);
            assert_eq!(outcomes["baseline"], previous["baseline"]);
            assert_eq!(outcomes["mutations"][0], previous["mutations"][0]);
            assert_eq!(outcomes["mutations"][3], previous["mutations"][3]);
            for (index, label) in [(1, "b"), (2, "c")] {
                assert_eq!(outcomes["mutations"][index]["state"], "interrupted");
                assert!(outcomes["mutations"][index]["outcome"].is_null());
                check_build_logs(&directory, &outcomes["mutations"][index]["logs"], label);
            }
            assert_eq!(
                outcomes["summary"],
                serde_json::json!({"killed":1,"missed":0,"timeout":0,"build_failed":0,"errors":0})
            );
            for (path, bytes) in &before {
                if path == Path::new("mutants.json")
                    || path.to_str().unwrap().starts_with("logs/000001")
                {
                    assert_eq!(&fs::read(directory.join(path)).unwrap(), bytes);
                }
            }
            assert!(!child.state.join("d.build.started").exists());
        }
        for label in active {
            assert!(!child.state.join(format!("{label}.tests.started")).exists());
        }
        fixture.check_cleanup(&child, &inputs);
        fixture.check_report(&directory);
    }
}

#[test]
#[ignore = "runs real Cargo/rustc compilation timeouts; opt-in to avoid recursive self-testing"]
fn compilation_timeouts_stop_before_tests_and_preserve_diagnostics() {
    for case in ["baseline", "mutations"] {
        let fixture = CompilationFixture::new();
        let inputs = files(&fixture.fixture.root);
        let args = if case == "baseline" {
            vec![
                "--baseline-timeout",
                "5",
                "--fail-on-missed",
                "--min-score",
                "100",
            ]
        } else {
            vec![
                "--jobs",
                "2",
                "--timeout",
                "10",
                "--clear-max-capture-bytes",
            ]
        };
        let mut child = fixture.start(case, &args);
        child.ready(if case == "baseline" {
            &["baseline"]
        } else {
            &["b", "c"]
        });
        let output = child.finish(Duration::from_secs(45));
        let (directory, run, inventory, outcomes) = report(&output);
        assert_eq!(run["checkpoint"], outcomes["checkpoint"]);
        if case == "baseline" {
            assert_eq!(output.status.code(), Some(1), "{output:?}");
            assert_eq!(run["status"], "failed");
            assert_eq!(inventory["complete"], false);
            assert_eq!(outcomes["baseline"]["outcome"], "timeout");
            assert!(outcomes["mutations"].as_array().unwrap().is_empty());
            check_unevaluated(&run);
            check_build_logs(&directory, &outcomes["baseline"]["logs"], "baseline");
            assert!(!child.state.join("baseline.tests.started").exists());
        } else {
            assert!(output.status.success(), "{output:?}");
            assert_eq!(run["status"], "completed");
            assert_eq!(outcomes["baseline"]["outcome"], "passed");
            assert_eq!(
                outcomes["summary"],
                serde_json::json!({"killed":1,"missed":0,"timeout":2,"build_failed":0,"errors":0})
            );
            for (index, label) in [(1, "b"), (2, "c")] {
                assert_eq!(outcomes["mutations"][index]["state"], "completed");
                assert_eq!(outcomes["mutations"][index]["outcome"], "timeout");
                check_build_logs(&directory, &outcomes["mutations"][index]["logs"], label);
                assert!(!child.state.join(format!("{label}.tests.started")).exists());
            }
        }
        fixture.check_cleanup(&child, &inputs);
        fixture.check_report(&directory);
    }
}

#[test]
#[ignore = "runs real Cargo with a controlled build delay and hanging test; opt-in to avoid recursive self-testing"]
fn compilation_and_tests_share_one_deadline() {
    let fixture = CompilationFixture::new();
    let tests = fixture.fixture.root.join("tests/cli.rs");
    fs::write(
        &tests,
        fs::read_to_string(&tests).unwrap().replace(
            "    assert_eq!(output.stdout, b\"false\\n\");",
            r#"    if output.stdout == b"true\n" {
        let state = std::path::PathBuf::from(std::env::var_os("MUTIE_RUST_TEST_STATE").unwrap());
        std::fs::write(state.join("hanging-test.pid"), std::process::id().to_string()).unwrap();
        std::fs::write(state.join("hanging-test.ready"), b"test started").unwrap();
        loop { std::thread::sleep(std::time::Duration::from_millis(20)); }
    }
    assert_eq!(output.stdout, b"false\n");"#,
        ),
    )
    .unwrap();
    let inputs = files(&fixture.fixture.root);
    let mut child = fixture.start("deadline", &["--timeout", "20"]);
    // Real compilation finishes before the gate opens. Holding the build phase
    // spends eight seconds of its budget independently of compiler performance.
    child.ready(&["b"]);
    assert!(!child.state.join("b.tests.started").exists());
    thread::sleep(Duration::from_secs(8));
    fs::write(child.state.join("release-build"), "release").unwrap();
    child.ready(&["hanging-test"]);
    let output = child.finish(Duration::from_secs(40));
    assert!(output.status.success(), "{output:?}");
    let (directory, run, _, outcomes) = report(&output);
    assert_eq!(run["status"], "completed");
    assert_eq!(
        outcomes["summary"],
        serde_json::json!({"killed":1,"missed":1,"timeout":1,"build_failed":0,"errors":0})
    );
    let mutation = &outcomes["mutations"][1];
    assert_eq!(mutation["outcome"], "timeout");
    let elapsed = mutation["duration_seconds"].as_f64().unwrap();
    assert!(
        (20.0..23.0).contains(&elapsed),
        "compile/test deadline was reset or expired early: {elapsed}s"
    );
    let log =
        fs::read_to_string(directory.join(mutation["logs"]["stdout"].as_str().unwrap())).unwrap();
    for marker in [
        "Cargo build checkpoint b",
        "--- Cargo test execution ---",
        "Cargo tests checkpoint b",
    ] {
        assert!(log.contains(marker), "{log}");
    }
    fixture.check_cleanup(&child, &inputs);
    fixture.check_report(&directory);
}

#[test]
#[ignore = "runs real Cargo worker-cache cleanup deadlines and cancellation; opt-in to avoid recursive self-testing"]
fn worker_cache_cleanup_timeout_and_cancellation_preserve_completed_results() {
    for interrupt in [false, true] {
        let fixture = CompilationFixture::new();
        let inputs = files(&fixture.fixture.root);
        let mut child = fixture.start(
            "clean-wait",
            &[
                "--rust-build-cache",
                "worker",
                "--jobs",
                "1",
                "--timeout",
                "8",
            ],
        );
        child.ready(&["b"]);
        assert!(!child.state.join("b.build.started").exists());
        let output = if interrupt {
            child.interrupt(libc::SIGINT)
        } else {
            child.finish(Duration::from_secs(30))
        };
        assert_eq!(output.status.success(), !interrupt, "{output:?}");
        let (directory, run, _, outcomes) = report(&output);
        assert_eq!(outcomes["mutations"][0]["outcome"], "killed");
        assert!(!child.state.join("b.tests.started").exists());
        assert!(!child.state.join("b.build.started").exists());
        let stopped = &outcomes["mutations"][1];
        let log = fs::read_to_string(directory.join(stopped["logs"]["stdout"].as_str().unwrap()))
            .unwrap();
        assert!(log.contains("Cargo clean checkpoint b"), "{log}");
        if interrupt {
            assert_eq!(run["status"], "interrupted");
            assert_eq!(stopped["state"], "interrupted");
            assert_eq!(outcomes["mutations"][2]["state"], "pending");
            assert!(!child.state.join("c.clean.started").exists());
        } else {
            assert_eq!(run["status"], "completed");
            assert_eq!(stopped["outcome"], "timeout");
            let elapsed = stopped["duration_seconds"].as_f64().unwrap();
            assert!((8.0..11.0).contains(&elapsed), "{elapsed}");
            // The same worker must clean and rebuild successfully after the
            // timed-out cleanup, restoring the previous killed mutation too.
            assert_eq!(outcomes["mutations"][2]["outcome"], "missed");
        }
        fixture.check_cleanup(&child, &inputs);
        fixture.check_report(&directory);
    }
}
