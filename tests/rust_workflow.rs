//! Opt-in Cargo executions stay out of the default suite to avoid recursively
//! spawning Rust mutation runs when Mutie tests its own source.
#![cfg(unix)]

use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    process::{Child, Command, Output},
    thread,
    time::{Duration, Instant},
};

use serde_json::Value;

#[path = "support/rust_boundaries.rs"]
mod boundaries;
#[path = "support/rust_cache.rs"]
mod cache;
#[path = "support/rust_compilation.rs"]
mod compilation;
#[path = "support/rust_dismissals.rs"]
mod dismissals;
#[path = "support/rust_reruns.rs"]
mod reruns;

struct Fixture {
    temp: tempfile::TempDir,
    root: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        Self::from_fixture("rust")
    }

    fn from_fixture(name: &str) -> Self {
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
            &Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures")
                .join(name),
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
            .args(["--ui", "plain"])
            .args(args)
            .env("CARGO_TARGET_DIR", self.temp.path().join("caller-target"))
            .env(
                "CARGO_BUILD_BUILD_DIR",
                self.temp.path().join("caller-build"),
            )
            .env("TMPDIR", self.temp.path().join("owned"));
        if !args.contains(&"--baseline-timeout") {
            command.args(["--baseline-timeout", "60"]);
        }
        if !args.contains(&"--clear-max-capture-bytes") && !args.contains(&"--max-capture-bytes") {
            command.args(["--max-capture-bytes", "1048576"]);
        }
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
                fs::read_dir(&directory).unwrap().count(),
                1,
                "Cargo wrote into caller artifacts: {directory:?}"
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
    let rerun = fixture.run(&["--rerun-missed", directory.to_str().unwrap(), "--list"]);
    assert!(rerun.status.success(), "{rerun:?}");
    let preview = String::from_utf8_lossy(&rerun.stdout);
    assert!(preview.contains("Found 1 mutations"), "{preview}");
    assert!(preview.contains("source [000003]"), "{preview}");
    assert!(!preview.contains("Running baseline"));
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
    let stronger = fixture.run(&[
        "--rerun-missed",
        directory.to_str().unwrap(),
        "--id",
        "3,1,3",
        "--operator",
        "boolean_flip",
        "--jobs",
        "2",
        "--min-score",
        "100",
    ]);
    assert!(stronger.status.success(), "{stronger:?}");
    let (rerun_dir, rerun, selected, results) = report(&stronger);
    assert_ne!(rerun_dir, directory);
    assert_eq!(rerun["planned_mutations"], 1);
    assert_eq!(rerun["score"]["scope"], "rerun");
    assert_eq!(rerun["score_policy"]["status"], "passed");
    assert_eq!(rerun["rerun"]["selection"], "missed");
    assert_eq!(rerun["rerun"]["source_checkpoint"], run["checkpoint"]);
    assert_eq!(rerun["rerun"]["source_ids"], serde_json::json!([1, 3]));
    assert_eq!(
        rerun["rerun"]["mutations"],
        serde_json::json!([{"id":1,"source_id":3}])
    );
    assert_eq!(selected["mutations"][0]["id"], 1);
    assert_eq!(
        selected["mutations"][0]["byte_start"],
        inventory["mutations"][2]["byte_start"]
    );
    assert_eq!(results["baseline"]["outcome"], "passed");
    assert_eq!(results["summary"]["killed"], 1);
    let log = results["mutations"][0]["logs"]["text"].as_str().unwrap();
    assert!(
        fs::read_to_string(rerun_dir.join(log))
            .unwrap()
            .contains("unchecked_flag_smoke_test")
    );
    let chained = fixture.run(&[
        "--rerun",
        rerun_dir.to_str().unwrap(),
        "--id",
        "1",
        "--list",
    ]);
    assert!(chained.status.success(), "{chained:?}");
    assert!(String::from_utf8_lossy(&chained.stdout).contains("Found 1 mutations"));
    assert_eq!(
        fs::read(directory.join("outcomes.json")).unwrap(),
        original_report
    );
    assert_eq!(
        fs::read(fixture.root.join("src/flags.rs")).unwrap(),
        original
    );

    let empty = fixture.run(&["--rerun-missed", rerun_dir.to_str().unwrap()]);
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

// Capture the entire input/report tree, including unexpected new files.
fn files(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn visit(root: &Path, directory: &Path, result: &mut BTreeMap<PathBuf, Vec<u8>>) {
        for entry in fs::read_dir(directory).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                visit(root, &path, result);
            } else {
                result.insert(
                    path.strip_prefix(root).unwrap().into(),
                    fs::read(path).unwrap(),
                );
            }
        }
    }
    let mut result = BTreeMap::new();
    visit(root, root, &mut result);
    result
}

struct InterruptibleRun {
    child: Child,
    state: PathBuf,
    stdout: PathBuf,
    stderr: PathBuf,
}

impl InterruptibleRun {
    fn start(fixture: &Fixture, args: &[&str]) -> Self {
        Self::start_command(fixture, fixture.command(args))
    }

    fn start_command(fixture: &Fixture, mut command: Command) -> Self {
        let state = fixture.temp.path().join("state");
        fs::create_dir(&state).unwrap();
        let stdout = fixture.temp.path().join("stdout");
        let stderr = fixture.temp.path().join("stderr");
        let child = command
            .env("MUTIE_RUST_TEST_STATE", &state)
            .stdout(fs::File::create(&stdout).unwrap())
            .stderr(fs::File::create(&stderr).unwrap())
            .spawn()
            .unwrap();
        Self {
            child,
            state,
            stdout,
            stderr,
        }
    }

    fn diagnostics(&self) -> String {
        let mut text = format!(
            "stdout:\n{}\nstderr:\n{}",
            fs::read_to_string(&self.stdout).unwrap_or_default(),
            fs::read_to_string(&self.stderr).unwrap_or_default(),
        );
        // A fixture helper failure can look like an ordinary killed mutation.
        // Retain its command diagnostics before the temporary report is removed.
        let reports = self.state.parent().unwrap().join("reports");
        for run in fs::read_dir(reports).into_iter().flatten().flatten() {
            for log in fs::read_dir(run.path().join("logs"))
                .into_iter()
                .flatten()
                .flatten()
            {
                let path = log.path();
                if path.extension().is_some_and(|extension| extension == "log")
                    && let Ok(bytes) = fs::read(&path)
                {
                    let tail = &bytes[bytes.len().saturating_sub(16 * 1024)..];
                    text.push_str(&format!(
                        "\n{} (tail):\n{}",
                        path.display(),
                        String::from_utf8_lossy(tail)
                    ));
                }
            }
        }
        text
    }

    fn ready(&mut self, labels: &[&str]) {
        let deadline = Instant::now() + Duration::from_secs(90);
        while !labels
            .iter()
            .all(|label| self.state.join(format!("{label}.ready")).exists())
        {
            assert!(
                self.child.try_wait().unwrap().is_none() && Instant::now() < deadline,
                "Cargo did not reach checkpoints {labels:?}: {}",
                self.diagnostics()
            );
            thread::sleep(Duration::from_millis(20));
        }
    }

    fn interrupt(&mut self, signal: i32) -> Output {
        // SAFETY: signal only the child CLI owned by this test.
        assert_eq!(unsafe { libc::kill(self.child.id() as i32, signal) }, 0);
        self.finish(Duration::from_secs(15))
    }

    fn finish(&mut self, limit: Duration) -> Output {
        let deadline = Instant::now() + limit;
        let status = loop {
            if let Some(status) = self.child.try_wait().unwrap() {
                break status;
            }
            assert!(
                Instant::now() < deadline,
                "run did not finish: {}",
                self.diagnostics()
            );
            thread::sleep(Duration::from_millis(20));
        };
        Output {
            status,
            stdout: fs::read(&self.stdout).unwrap(),
            stderr: fs::read(&self.stderr).unwrap(),
        }
    }
}

impl Drop for InterruptibleRun {
    fn drop(&mut self) {
        let running = self.child.try_wait().ok().flatten().is_none();
        if running || thread::panicking() {
            if running {
                // Give Mutie a chance to clean up even if the assertion failed
                // before our helpers recorded Cargo's process groups.
                // SAFETY: this is the child CLI owned by this test.
                unsafe {
                    libc::kill(self.child.id() as i32, libc::SIGTERM);
                }
                let deadline = Instant::now() + Duration::from_secs(5);
                while self.child.try_wait().ok().flatten().is_none() && Instant::now() < deadline {
                    thread::sleep(Duration::from_millis(20));
                }
            }
            let _ = self.child.kill();
            let _ = self.child.wait();
            // Cargo owns separate process groups. Stop helpers on assertion
            // failure too, without relying on their long fallback sleep.
            for entry in fs::read_dir(&self.state).into_iter().flatten().flatten() {
                let path = entry.path();
                if path
                    .extension()
                    .is_some_and(|extension| extension == "group")
                    && let Ok(text) = fs::read_to_string(path)
                    && let Ok(group) = text.trim().parse::<i32>()
                    && group > 1
                    // Never kill our own harness if process isolation regresses.
                    && group != unsafe { libc::getpgrp() }
                {
                    // SAFETY: groups recorded by this fixture's active children.
                    unsafe {
                        libc::kill(-group, libc::SIGKILL);
                    }
                }
            }
        }
    }
}

fn process_running(pid: i32) -> bool {
    // A killed grandchild may briefly be a zombie until its adopter reaps it.
    #[cfg(target_os = "linux")]
    if let Ok(status) = fs::read_to_string(format!("/proc/{pid}/status"))
        && status
            .lines()
            .any(|line| line.starts_with("State:") && line.contains('Z'))
    {
        return false;
    }
    // SAFETY: signal 0 only checks this test's recorded process IDs.
    unsafe { libc::kill(pid, 0) == 0 }
}

#[test]
#[ignore = "runs real Cargo mutations and interrupts active tests; opt-in to avoid recursive self-testing"]
fn rust_mutation_cancellation_preserves_partial_results_and_cleans_processes() {
    // Exercise both capture paths and signals: direct files for one worker,
    // drained pipes for two workers. No sleep decides when to interrupt.
    for (jobs, signal, bounded_capture, active) in [
        ("1", libc::SIGTERM, false, &["checked"][..]),
        ("2", libc::SIGINT, true, &["checked", "unchecked"][..]),
    ] {
        let fixture = Fixture::new();
        let flags = fixture.root.join("src/flags.rs");
        fs::write(
            &flags,
            format!(
                "{}\npub fn pending_flag() -> bool {{ true }}\n",
                fs::read_to_string(&flags).unwrap()
            ),
        )
        .unwrap();
        let main = fixture.root.join("src/main.rs");
        fs::write(
            &main,
            fs::read_to_string(&main).unwrap().replace(
                "Some(\"unit\") =>",
                "Some(\"pending\") => flags::pending_flag(),\n        Some(\"unit\") =>",
            ),
        )
        .unwrap();
        fs::write(
            fixture.root.join("tests/cli.rs"),
            r##"use std::{fs, path::PathBuf, process::Command};

unsafe extern "C" { fn getpgrp() -> i32; }

fn check(argument: &str, expected: &[u8]) {
    let output = Command::new(env!("CARGO_BIN_EXE_mutie-rust-fixture"))
        .arg(argument).output().unwrap();
    assert!(output.status.success());
    if output.stdout == expected { return; }
    let state = PathBuf::from(std::env::var_os("MUTIE_RUST_TEST_STATE").unwrap());
    if argument == "pending" {
        fs::write(state.join("unexpected"), b"pending mutation executed").unwrap();
        panic!("pending mutation must never run");
    }
    fs::write(state.join(format!("{argument}.test.pid")), std::process::id().to_string()).unwrap();
    // SAFETY: getpgrp has no arguments and only reads our own process group.
    fs::write(state.join(format!("{argument}.group")), unsafe { getpgrp() }.to_string()).unwrap();
    // Shell output bypasses libtest's in-memory capture, so interrupted tests
    // leave diagnostics. The helper and its descendant share Cargo's group.
    let status = Command::new("sh").args(["-c", r#"
set -eu
label="$1"
state="$MUTIE_RUST_TEST_STATE"
printf '%s\n' "$$" > "$state/$label.shell.pid"
sleep 300 &
printf '%s\n' "$!" > "$state/$label.descendant.pid"
printf '%s\n' "$PWD" > "$state/$label.workspace"
printf '%s\n' "$CARGO_TARGET_DIR" > "$state/$label.target"
test "$TMPDIR" = "$TMP"
test "$TMPDIR" = "$TEMP"
printf '%s\n' "$TMPDIR" > "$state/$label.tmpdir"
mkdir "$TMPDIR/nested"
printf 'owned test artifact\n' > "$TMPDIR/nested/artifact"
printf 'active %s\n' "$label"
printf 'diagnostic %s\n' "$label" >&2
touch "$state/$label.ready"
wait
"#, "mutie-cancellation-helper", argument]).status().unwrap();
    panic!("blocking helper unexpectedly exited: {status}");
}

#[test]
fn checked_flag() { check("checked", b"false\n"); }
#[test]
fn unchecked_flag() { check("unchecked", b"true\n"); }
#[test]
fn pending_flag() { check("pending", b"true\n"); }
"##,
        )
        .unwrap();
        let inputs = files(&fixture.root);
        let mut args = vec![
            "--jobs",
            jobs,
            "--timeout",
            "120",
            "--fail-on-missed",
            "--min-score",
            "100",
        ];
        if !bounded_capture {
            args.push("--clear-max-capture-bytes");
        }
        let mut child = InterruptibleRun::start(&fixture, &args);
        child.ready(active);
        let directories: Vec<_> = fs::read_dir(fixture.temp.path().join("reports"))
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect();
        assert_eq!(directories.len(), 1);
        let directory = &directories[0];
        let read = |name| -> Value {
            serde_json::from_slice(&fs::read(directory.join(name)).unwrap()).unwrap()
        };
        let inventory = fs::read(directory.join("mutants.json")).unwrap();
        assert_eq!(
            read("mutants.json")["mutations"].as_array().unwrap().len(),
            4
        );
        assert_eq!(read("mutants.json")["complete"], true);
        let before = read("outcomes.json");
        assert_eq!(before["baseline"]["outcome"], "passed");
        assert_eq!(before["mutations"][0]["state"], "completed");
        assert_eq!(before["mutations"][0]["outcome"], "killed");
        for index in 1..4 {
            assert_eq!(
                before["mutations"][index]["state"],
                if index <= active.len() {
                    "running"
                } else {
                    "pending"
                }
            );
        }
        let completed_logs: Vec<_> = ["text", "stdout", "stderr"]
            .map(|stream| {
                let path = directory.join(before["mutations"][0]["logs"][stream].as_str().unwrap());
                let bytes = fs::read(&path).unwrap();
                (path, bytes)
            })
            .into();
        let pids: Vec<i32> = fs::read_dir(&child.state)
            .unwrap()
            .filter_map(|entry| {
                let path = entry.unwrap().path();
                matches!(path.extension()?.to_str()?, "pid" | "group")
                    .then(|| fs::read_to_string(path).unwrap().trim().parse().unwrap())
            })
            .collect();
        assert_eq!(pids.len(), active.len() * 4);
        assert!(pids.iter().all(|pid| process_running(*pid)));

        let output = child.interrupt(signal);
        assert_eq!(output.status.code(), Some(1), "{output:?}");
        assert!(String::from_utf8_lossy(&output.stderr).contains("interrupted"));
        let (reported_directory, run, _, outcomes) = report(&output);
        assert_eq!(&reported_directory, directory);
        assert_eq!(run["status"], "interrupted");
        assert_eq!(outcomes["status"], "interrupted");
        assert_eq!(run["checkpoint"], outcomes["checkpoint"]);
        assert_eq!(run["policy"]["status"], "not_evaluated");
        assert_eq!(run["score_policy"]["status"], "not_evaluated");
        assert!(run["score"]["percentage"].is_null());
        assert_eq!(outcomes["baseline"], before["baseline"]);
        assert_eq!(outcomes["mutations"][0], before["mutations"][0]);
        for (path, bytes) in completed_logs {
            assert_eq!(fs::read(path).unwrap(), bytes);
        }
        for (index, label) in active.iter().enumerate() {
            let execution = &outcomes["mutations"][index + 1];
            assert_eq!(execution["state"], "interrupted");
            assert!(execution["outcome"].is_null());
            assert!(execution["duration_seconds"].as_f64().unwrap() > 0.0);
            for (stream, diagnostic) in [
                ("stdout", "active"),
                ("stderr", "diagnostic"),
                ("text", "active"),
            ] {
                let log =
                    fs::read_to_string(directory.join(execution["logs"][stream].as_str().unwrap()))
                        .unwrap();
                assert!(log.contains(&format!("{diagnostic} {label}")), "{log}");
                assert!(
                    log.contains("--- Cargo test execution ---"),
                    "compilation/execution logs not merged: {log}"
                );
            }
        }
        for index in active.len() + 1..4 {
            assert_eq!(outcomes["mutations"][index], before["mutations"][index]);
            assert!(outcomes["mutations"][index]["logs"].is_null());
        }
        assert_eq!(
            outcomes["summary"],
            serde_json::json!({"killed":1,"missed":0,"timeout":0,"build_failed":0,"errors":0})
        );
        assert_eq!(fs::read(directory.join("mutants.json")).unwrap(), inventory);
        let summary = fs::read_to_string(directory.join("summary.txt")).unwrap();
        for expected in [
            "Reported: 1/4".to_owned(),
            format!("Interrupted: {}", active.len()),
            format!("Pending: {}", 3 - active.len()),
        ] {
            assert!(summary.contains(&expected), "{summary}");
        }
        assert!(!child.state.join("unexpected").exists());
        let deadline = Instant::now() + Duration::from_secs(10);
        while pids.iter().any(|pid| process_running(*pid)) {
            assert!(
                Instant::now() < deadline,
                "Cargo/test/helper processes leaked: {pids:?}"
            );
            thread::sleep(Duration::from_millis(20));
        }
        for label in active {
            for kind in ["workspace", "target", "tmpdir"] {
                let path = PathBuf::from(
                    fs::read_to_string(child.state.join(format!("{label}.{kind}")))
                        .unwrap()
                        .trim(),
                );
                assert!(
                    path.starts_with(fixture.temp.path().join("owned")),
                    "{path:?}"
                );
                assert!(!path.exists(), "owned {kind} leaked: {path:?}");
            }
        }
        fixture.preserved();
        assert_eq!(files(&fixture.root), inputs);

        // Retention uses the same strict reader as the browser, and validates
        // every saved log/diff path. Preview must accept this partial run.
        let saved = files(directory);
        let preview = Command::new(env!("CARGO_BIN_EXE_mutie"))
            .arg("prune")
            .arg(fixture.temp.path().join("reports"))
            .args(["--keep-last", "0"])
            .output()
            .unwrap();
        assert!(preview.status.success(), "{preview:?}");
        assert!(
            String::from_utf8_lossy(&preview.stdout).contains("Selected runs: 1"),
            "{preview:?}"
        );
        assert_eq!(files(directory), saved);
    }
}
