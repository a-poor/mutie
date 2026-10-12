//! Opt-in Cargo executions stay out of the default suite to avoid recursively
//! spawning Rust mutation runs when Mutie tests its own source.
#![cfg(unix)]

use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    thread,
    time::{Duration, Instant},
};

use serde_json::Value;

struct Fixture {
    temp: tempfile::TempDir,
    root: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        fn copy(from: &Path, to: &Path) {
            fs::create_dir_all(to).unwrap();
            for entry in fs::read_dir(from).unwrap() {
                let entry = entry.unwrap();
                if matches!(entry.file_name().to_str(), Some("target" | ".mutie")) {
                    continue;
                }
                let target = to.join(entry.file_name());
                if entry.file_type().unwrap().is_dir() {
                    copy(&entry.path(), &target);
                } else {
                    fs::copy(entry.path(), target).unwrap();
                }
            }
        }
        let temp = tempfile::tempdir_in(std::env::temp_dir().canonicalize().unwrap()).unwrap();
        let root = temp.path().join("project with spaces");
        copy(
            &Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/rust"),
            &root,
        );
        for directory in [
            root.join("target"),
            temp.path().join("caller-target"),
            temp.path().join("caller-build"),
            temp.path().join("owned"),
        ] {
            fs::create_dir_all(directory).unwrap();
        }
        for directory in [
            root.join("target"),
            temp.path().join("caller-target"),
            temp.path().join("caller-build"),
        ] {
            fs::write(directory.join("preserve"), "caller artifact").unwrap();
        }
        Self { temp, root }
    }

    fn command(&self, args: &[&str]) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_mutie"));
        command
            .arg(&self.root)
            .args([
                "--ui",
                "plain",
                "--baseline-timeout",
                "60",
                "--max-capture-bytes",
                "1048576",
            ])
            .args(args)
            .env("CARGO_TARGET_DIR", self.temp.path().join("caller-target"))
            .env(
                "CARGO_BUILD_BUILD_DIR",
                self.temp.path().join("caller-build"),
            )
            .env("TMPDIR", self.temp.path().join("owned"));
        if !args.contains(&"--timeout") {
            command.args(["--timeout", "30"]);
        }
        if !args.contains(&"--list") {
            command
                .arg("--output-dir")
                .arg(self.temp.path().join("reports"));
        }
        command
    }

    fn run(&self, args: &[&str]) -> Output {
        let output = self.command(args).output().unwrap();
        self.preserved();
        output
    }

    fn preserved(&self) {
        for directory in [
            self.root.join("target"),
            self.temp.path().join("caller-target"),
            self.temp.path().join("caller-build"),
        ] {
            assert_eq!(
                fs::read_to_string(directory.join("preserve")).unwrap(),
                "caller artifact"
            );
            assert_eq!(
                fs::read_dir(directory).unwrap().count(),
                1,
                "Cargo wrote into caller artifacts"
            );
        }
        assert_eq!(
            fs::read_dir(self.temp.path().join("owned"))
                .unwrap()
                .count(),
            0,
            "owned temporary artifacts leaked"
        );
    }
}

fn report(output: &Output) -> (PathBuf, Value, Value, Value) {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let directory = PathBuf::from(
        stdout
            .lines()
            .find_map(|line| line.strip_prefix("Saving report to "))
            .unwrap_or_else(|| panic!("{output:?}")),
    );
    let read = |name| serde_json::from_slice(&fs::read(directory.join(name)).unwrap()).unwrap();
    (
        directory.clone(),
        read("run.json"),
        read("mutants.json"),
        read("outcomes.json"),
    )
}

#[test]
#[ignore = "runs real Cargo baseline/mutations and stronger-test probes; opt-in to avoid recursive self-testing"]
fn rust_fixture_runs_lists_filters_and_saves_results_without_touching_inputs() {
    let fixture = Fixture::new();
    let original = fs::read(fixture.root.join("src/flags.rs")).unwrap();
    let listing = fixture.run(&["--list"]);
    assert!(listing.status.success(), "{listing:?}");
    let text = String::from_utf8_lossy(&listing.stdout);
    assert!(text.contains("Found 3 mutations"), "{text}");
    assert!(!text.contains("Running baseline"));
    assert!(!fixture.temp.path().join("reports").exists());
    let filtered = fixture.run(&["--list", "--include", "src/main.rs"]);
    assert!(filtered.status.success(), "{filtered:?}");
    assert!(String::from_utf8_lossy(&filtered.stdout).contains("Found 0 mutations"));

    let output = fixture.run(&["--jobs", "2", "--min-score", "100", "--max-log-bytes", "32"]);
    assert_eq!(output.status.code(), Some(3), "{output:?}");
    let (directory, run, inventory, outcomes) = report(&output);
    assert_eq!(run["status"], "completed");
    assert_eq!(run["effective_config"]["lang"], "rust");
    assert_eq!(run["rust_scope"]["package"], "mutie-rust-fixture");
    assert!(
        run["rust_scope"]["rustc"]
            .as_str()
            .unwrap()
            .contains("rustc")
    );
    assert!(
        run["rust_scope"]["cargo"]
            .as_str()
            .unwrap()
            .contains("cargo")
    );
    assert!(
        run["rust_scope"]["discovery_exclusions"]
            .as_array()
            .unwrap()
            .len()
            >= 3
    );
    assert_eq!(
        run["compilation_command"]
            .as_array()
            .unwrap()
            .last()
            .unwrap(),
        "--no-run"
    );
    assert_eq!(inventory["complete"], true);
    assert_eq!(outcomes["summary"]["killed"], 2);
    assert_eq!(outcomes["summary"]["missed"], 1);
    assert_eq!(outcomes["summary"]["errors"], 0);
    assert_eq!(run["score_policy"]["status"], "failed");
    for mutation in inventory["mutations"].as_array().unwrap() {
        assert_eq!(mutation["file"], "src/flags.rs");
        assert_eq!(mutation["operator"], "boolean_flip");
        assert_eq!(mutation["test_packages"], serde_json::json!(["--tests"]));
    }
    assert!(
        outcomes["mutations"][0]["logs"]["retention"]["stdout"]["omitted_bytes"]
            .as_u64()
            .unwrap()
            > 0
    );
    assert!(
        fs::read_to_string(directory.join("summary.txt"))
            .unwrap()
            .contains("boolean_flip")
    );
    let original_report = fs::read(directory.join("outcomes.json")).unwrap();
    let preview = Command::new(env!("CARGO_BIN_EXE_mutie"))
        .args([
            "prune",
            fixture.temp.path().join("reports").to_str().unwrap(),
            "--keep-last",
            "0",
        ])
        .output()
        .unwrap();
    assert!(preview.status.success(), "{preview:?}");
    assert!(
        String::from_utf8_lossy(&preview.stdout).contains("Selected runs: 1"),
        "{preview:?}"
    );
    let rerun = fixture.run(&["--rerun", directory.to_str().unwrap()]);
    assert!(!rerun.status.success());
    assert!(
        String::from_utf8_lossy(&rerun.stderr).contains("Rust focused reruns are not implemented")
    );
    assert_eq!(
        fs::read(directory.join("outcomes.json")).unwrap(),
        original_report
    );

    let repeated = fixture.run(&["--jobs", "1"]);
    assert!(repeated.status.success(), "{repeated:?}");
    assert_eq!(report(&repeated).3["summary"], outcomes["summary"]);
    let tests = fixture.root.join("tests/cli.rs");
    let source = fs::read_to_string(&tests).unwrap();
    fs::write(
        &tests,
        source.replace(
            "    let output = run(\"unchecked\");",
            "    let output = run(\"unchecked\");\n    assert_eq!(output.stdout, b\"true\\n\");",
        ),
    )
    .unwrap();
    let stronger = fixture.run(&["--operator", "boolean_flip", "--jobs", "2"]);
    assert!(stronger.status.success(), "{stronger:?}");
    assert_eq!(report(&stronger).3["summary"]["killed"], 3);
    assert_eq!(
        fs::read(fixture.root.join("src/flags.rs")).unwrap(),
        original
    );

    let empty = fixture.run(&["--include", "src/main.rs"]);
    assert!(empty.status.success(), "{empty:?}");
    assert_eq!(report(&empty).1["planned_mutations"], 0);
    assert_eq!(report(&empty).3["baseline"]["outcome"], "passed");
}

#[test]
#[ignore = "runs real Cargo failures, a mutation timeout and cancellation; opt-in to avoid recursive self-testing"]
fn rust_failures_timeout_and_cancellation_preserve_reports_and_cleanup() {
    let fixture = Fixture::new();
    let file = fixture.root.join("src/flags.rs");
    let original = fs::read_to_string(&file).unwrap();
    for invalid in [
        "pub fn unit_flag() -> bool { 123 }",
        "pub fn unit_flag() -> bool { false }",
    ] {
        fs::write(
            &file,
            original.replace("pub fn unit_flag() -> bool {\n    true\n}", invalid),
        )
        .unwrap();
        let output = fixture.run(&[]);
        assert!(!output.status.success(), "{output:?}");
        let (_, run, inventory, outcomes) = report(&output);
        assert_eq!(run["status"], "failed");
        assert_eq!(inventory["complete"], false);
        assert_eq!(outcomes["baseline"]["outcome"], "failed");
        assert!(outcomes["mutations"].as_array().unwrap().is_empty());
    }
    fs::write(&file, original.replace("pub fn unit_flag() -> bool {", "pub fn unit_flag() -> bool {\n    while false { std::thread::sleep(std::time::Duration::from_millis(20)); }")).unwrap();
    let output = fixture.run(&["--timeout", "15"]);
    assert!(output.status.success(), "{output:?}");
    let (_, run, _, outcomes) = report(&output);
    assert_eq!(run["status"], "completed");
    assert_eq!(outcomes["summary"]["timeout"], 1);
    assert_eq!(outcomes["summary"]["killed"], 2);
    assert_eq!(outcomes["summary"]["missed"], 1);

    let marker = fixture.temp.path().join("test-started");
    fs::write(&file, original.replace("pub fn unit_flag() -> bool {", "pub fn unit_flag() -> bool {\n    if let Some(marker) = std::env::var_os(\"MUTIE_RUST_TEST_MARKER\") {\n        std::fs::write(marker, b\"started\").unwrap();\n        loop { std::thread::sleep(std::time::Duration::from_millis(20)); }\n    }")).unwrap();
    let mut command = fixture.command(&[]);
    command
        .env("MUTIE_RUST_TEST_MARKER", &marker)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    let mut child = command.spawn().unwrap();
    let started = Instant::now();
    while !marker.exists() {
        if started.elapsed() > Duration::from_secs(45) {
            let _ = child.kill();
            panic!("Cargo baseline did not reach its test");
        }
        assert!(
            child.try_wait().unwrap().is_none(),
            "Mutie exited before the test marker"
        );
        thread::sleep(Duration::from_millis(20));
    }
    unsafe {
        libc::kill(child.id() as i32, libc::SIGTERM);
    }
    let output = child.wait_with_output().unwrap();
    assert!(!output.status.success());
    let (directory, run, inventory, outcomes) = report(&output);
    assert_eq!(run["status"], "interrupted");
    assert_eq!(inventory["complete"], false);
    assert_eq!(outcomes["baseline"]["outcome"], "interrupted");
    assert!(
        directory
            .join(outcomes["baseline"]["logs"]["stdout"].as_str().unwrap())
            .is_file()
    );
    fixture.preserved();
}
