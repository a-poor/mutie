#![cfg(unix)]

use std::{
    collections::HashSet,
    fs,
    os::unix::{fs::PermissionsExt, process::CommandExt},
    path::{Path, PathBuf},
    process::{Child, Command, Output, Stdio},
    thread,
    time::{Duration, Instant},
};

use serde_json::Value;

struct Fixture {
    _temp: tempfile::TempDir,
    root: PathBuf,
    bin: PathBuf,
    state: PathBuf,
    workspaces: PathBuf,
    go_temporary: PathBuf,
    go_cache: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let temp = tempfile::tempdir_in(std::env::temp_dir().canonicalize().unwrap()).unwrap();
        let root = temp.path().join("module");
        let bin = temp.path().join("bin");
        let state = temp.path().join("state");
        let workspaces = temp.path().join("workspaces");
        let go_temporary = temp.path().join("go-tmp");
        let go_cache = temp.path().join("go-cache");
        for directory in [&root, &bin, &state, &workspaces, &go_temporary, &go_cache] {
            fs::create_dir(directory).unwrap();
        }
        fs::create_dir(workspaces.join("mutie-test-existing")).unwrap();
        fs::write(
            workspaces.join("mutie-test-existing/keep"),
            "unrelated test file",
        )
        .unwrap();
        fs::create_dir(go_temporary.join("go-build-existing")).unwrap();
        fs::write(go_temporary.join("go-build-existing/keep"), "unrelated").unwrap();
        fs::write(go_cache.join("keep"), "cached build").unwrap();
        fs::write(root.join("go.mod"), "module example.com/cancellation\n").unwrap();
        for name in ["a", "b", "c", "z"] {
            fs::write(
                root.join(format!("{name}.go")),
                format!("package p\nvar {name} = true\n"),
            )
            .unwrap();
        }
        let go = bin.join("go");
        fs::write(
            &go,
            r#"#!/bin/sh
set -eu
# Deliberately leave build files even on success. Mutie must own their cleanup.
mkdir "$GOTMPDIR/go-build-$$"
printf 'build output\n' > "$GOTMPDIR/go-build-$$/object"
printf '%s\n' "$GOTMPDIR" > "$MUTIE_TEST_STATE/$$.gotmpdir"
# Test helpers may use any of these variables, or leave nested files/symlinks.
test "$TMPDIR" = "$TMP"
test "$TMPDIR" = "$TEMP"
test "$TMPDIR" != "$GOTMPDIR"
test ! -e "$TMPDIR/test-artifact"
printf 'test output\n' > "$TMPDIR/test-artifact"
mkdir "$TMPDIR/nested"
printf 'nested output\n' > "$TMPDIR/nested/artifact"
ln -s "$GOCACHE" "$TMPDIR/cache-link"
printf '%s\n' "$TMPDIR" > "$MUTIE_TEST_STATE/$$.tmpdir"
printf 'cache reused\n' > "$GOCACHE/reused"
hang() {
    label="$1"
    printf 'active %s\n' "$label"
    printf 'diagnostic %s\n' "$label" >&2
    printf '%s\n' "$$" > "$MUTIE_TEST_STATE/$label.pid"
    printf '%s\n' "$PWD" > "$MUTIE_TEST_STATE/$label.workspace"
    (sleep 30; touch "$MUTIE_TEST_STATE/leaked") &
    printf '%s\n' "$!" > "$MUTIE_TEST_STATE/$label.child.pid"
    touch "$MUTIE_TEST_STATE/$label.ready"
    if [ "$MUTIE_TEST_MODE" = isolation ] && [ "$label" = b ]; then
        while [ ! -f "$MUTIE_TEST_STATE/release-b" ]; do sleep 0.01; done
        exit 0
    fi
    if { [ "$MUTIE_TEST_MODE" = tool_error ] || [ "$MUTIE_TEST_MODE" = capture_error ]; } && [ "$label" = c ]; then
        while [ ! -f "$MUTIE_TEST_STATE/fail" ]; do sleep 0.01; done
        if [ "$MUTIE_TEST_MODE" = capture_error ]; then
            while :; do printf 'stdout diagnostic\n'; printf 'stderr diagnostic\n' >&2; done
        fi
        printf 'go tool failed unexpectedly\n' >&2
        exit 2
    fi
    wait
}
if [ "$1" = list ]; then
    if [ "$MUTIE_TEST_MODE" = discovery ]; then hang discovery; fi
    printf '{"Dir":"%s","GoFiles":["z.go","c.go","a.go","b.go"]}\n' "$PWD"
    exit 0
fi
if grep -q false a.go; then
    printf '{"Action":"output","Output":"completed a\n"}\n'
    exit 0
fi
for name in b c; do
    if grep -q false "$name.go"; then hang "$name"; fi
done
if grep -q false z.go; then touch "$MUTIE_TEST_STATE/unexpected"; fi
if [ "$MUTIE_TEST_MODE" = baseline ]; then hang baseline; fi
printf '{"Action":"output","Output":"PASS\n"}\n'
"#,
        )
        .unwrap();
        fs::set_permissions(&go, fs::Permissions::from_mode(0o755)).unwrap();
        Self {
            _temp: temp,
            root,
            bin,
            state,
            workspaces,
            go_temporary,
            go_cache,
        }
    }

    fn start(&self, mode: &str) -> Running {
        self.start_with_args(
            mode,
            &[
                "--jobs",
                "2",
                "--timeout",
                "60",
                "--fail-on-missed",
                "--min-score",
                "100",
            ],
        )
    }

    fn command(&self, mode: &str) -> Command {
        let mut paths = vec![self.bin.clone()];
        paths.extend(std::env::split_paths(
            &std::env::var_os("PATH").unwrap_or_default(),
        ));
        let mut command = Command::new(env!("CARGO_BIN_EXE_mutie"));
        command
            .args(["--max-capture-bytes", "4096"])
            .env("PATH", std::env::join_paths(paths).unwrap())
            .env("TMPDIR", &self.workspaces)
            .env("TMP", &self.go_temporary)
            .env("TEMP", &self.go_temporary)
            .env("GOTMPDIR", &self.go_temporary)
            .env("GOCACHE", &self.go_cache)
            .env("MUTIE_TEST_STATE", &self.state)
            .env("MUTIE_TEST_MODE", mode)
            .current_dir(self._temp.path());
        command
    }

    fn start_with_args(&self, mode: &str, args: &[&str]) -> Running {
        let stdout = self.state.join("stdout");
        let stderr = self.state.join("stderr");
        let child = self
            .command(mode)
            .arg(&self.root)
            .args(args)
            .stdin(Stdio::null())
            .stdout(fs::File::create(&stdout).unwrap())
            .stderr(fs::File::create(&stderr).unwrap())
            .process_group(0)
            .spawn()
            .unwrap();
        Running {
            child,
            state: self.state.clone(),
            stdout,
            stderr,
        }
    }

    fn ready(&self, name: &str) {
        wait_until(|| self.state.join(format!("{name}.ready")).exists());
    }

    fn report(&self) -> PathBuf {
        let directories: Vec<_> = fs::read_dir(self.root.join(".mutie"))
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect();
        assert_eq!(directories.len(), 1);
        directories[0].clone()
    }

    fn assert_cleanup(&self) {
        self.assert_test_temporary_cleanup();
        assert!(
            !self.state.join("unexpected").exists(),
            "scheduled pending mutation after cancellation"
        );
        assert!(!self.state.join("leaked").exists());
        assert_eq!(fs::read_dir(&self.go_temporary).unwrap().count(), 1);
        assert_eq!(
            fs::read_to_string(self.go_temporary.join("go-build-existing/keep")).unwrap(),
            "unrelated"
        );
        assert_eq!(
            fs::read_to_string(self.go_cache.join("keep")).unwrap(),
            "cached build"
        );
        assert_eq!(
            fs::read_to_string(self.go_cache.join("reused")).unwrap(),
            "cache reused\n"
        );
        let mut build_directories = HashSet::new();
        let mut test_directories = HashSet::new();
        for name in ["a", "b", "c", "z"] {
            assert_eq!(
                fs::read_to_string(self.root.join(format!("{name}.go"))).unwrap(),
                format!("package p\nvar {name} = true\n")
            );
        }
        for entry in fs::read_dir(&self.state).unwrap() {
            let path = entry.unwrap().path();
            if path
                .extension()
                .is_some_and(|extension| extension == "tmpdir")
            {
                let directory = PathBuf::from(fs::read_to_string(path).unwrap().trim());
                assert_eq!(directory.parent(), Some(self.workspaces.as_path()));
                assert!(
                    directory
                        .file_name()
                        .unwrap()
                        .to_str()
                        .unwrap()
                        .starts_with("mutie-test-")
                );
                assert!(!directory.exists(), "Go test temporary directory leaked");
                assert!(
                    test_directories.insert(directory),
                    "Go commands shared a test directory"
                );
                continue;
            }
            if path
                .extension()
                .is_some_and(|extension| extension == "gotmpdir")
            {
                let directory = PathBuf::from(fs::read_to_string(path).unwrap().trim());
                assert!(directory.is_absolute());
                assert!(
                    directory
                        .file_name()
                        .unwrap()
                        .to_str()
                        .unwrap()
                        .starts_with("mutie-go-")
                );
                assert!(!directory.exists(), "Go temporary build directory leaked");
                assert!(
                    build_directories.insert(directory),
                    "Go commands shared a build directory"
                );
                continue;
            }
            if path.extension().is_some_and(|extension| extension == "pid") {
                let pid: i32 = fs::read_to_string(path).unwrap().trim().parse().unwrap();
                wait_until(|| !process_running(pid));
            }
        }
        assert!(!build_directories.is_empty());
        assert_eq!(test_directories.len(), build_directories.len());
        assert!(test_directories.is_disjoint(&build_directories));
    }

    fn assert_test_temporary_cleanup(&self) {
        assert_eq!(
            fs::read_dir(&self.workspaces).unwrap().count(),
            1,
            "temporary workspace, capture, or test files leaked"
        );
        assert_eq!(
            fs::read_to_string(self.workspaces.join("mutie-test-existing/keep")).unwrap(),
            "unrelated test file"
        );
    }

    fn test_temporary(&self, label: &str) -> PathBuf {
        let pid = fs::read_to_string(self.state.join(format!("{label}.pid"))).unwrap();
        PathBuf::from(
            fs::read_to_string(self.state.join(format!("{}.tmpdir", pid.trim())))
                .unwrap()
                .trim(),
        )
    }
}

struct Running {
    child: Child,
    state: PathBuf,
    stdout: PathBuf,
    stderr: PathBuf,
}

impl Running {
    fn signal(&self, signal: i32) {
        // SAFETY: signal only the child CLI owned by this test.
        assert_eq!(unsafe { libc::kill(self.child.id() as i32, signal) }, 0);
    }

    fn finish(&mut self) -> Output {
        let deadline = Instant::now() + Duration::from_secs(15);
        let status = loop {
            if let Some(status) = self.child.try_wait().unwrap() {
                break status;
            }
            assert!(
                Instant::now() < deadline,
                "CLI did not finish; stdout: {}\nstderr: {}",
                fs::read_to_string(&self.stdout).unwrap_or_default(),
                fs::read_to_string(&self.stderr).unwrap_or_default()
            );
            thread::sleep(Duration::from_millis(10));
        };
        Output {
            status,
            stdout: fs::read(&self.stdout).unwrap(),
            stderr: fs::read(&self.stderr).unwrap(),
        }
    }
}

impl Drop for Running {
    fn drop(&mut self) {
        // Cleanup also on assertion failure; Go children have separate groups.
        if self.child.try_wait().ok().flatten().is_none() {
            let _ = self.child.kill();
            let _ = self.child.wait();
            for entry in fs::read_dir(&self.state).into_iter().flatten().flatten() {
                let path = entry.path();
                if path.extension().is_some_and(|extension| extension == "pid")
                    && let Ok(text) = fs::read_to_string(path)
                    && let Ok(pid) = text.trim().parse::<i32>()
                {
                    // SAFETY: these PIDs were recorded by this test's children.
                    unsafe {
                        libc::kill(-pid, libc::SIGKILL);
                    }
                }
            }
        }
    }
}

fn wait_until(mut predicate: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(15);
    while !predicate() {
        assert!(
            Instant::now() < deadline,
            "timed out waiting for test process"
        );
        thread::sleep(Duration::from_millis(10));
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
    // SAFETY: signal 0 only checks whether this test's process still exists.
    unsafe { libc::kill(pid, 0) == 0 }
}

fn read(directory: &Path, name: &str) -> Value {
    serde_json::from_slice(&fs::read(directory.join(name)).unwrap()).unwrap()
}

#[test]
fn interrupt_parallel_run_retains_inventory_results_and_active_diagnostics() {
    let fixture = Fixture::new();
    let mut child = fixture.start("mutations");
    fixture.ready("b");
    fixture.ready("c");
    let directory = fixture.report();
    let inventory_bytes = fs::read(directory.join("mutants.json")).unwrap();
    let inventory = read(&directory, "mutants.json");
    assert_eq!(inventory["complete"], true);
    assert_eq!(inventory["mutations"].as_array().unwrap().len(), 4);
    for mutation in inventory["mutations"].as_array().unwrap() {
        assert!(directory.join(mutation["diff"].as_str().unwrap()).is_file());
    }
    let before = read(&directory, "outcomes.json");
    assert_eq!(before["mutations"][0]["state"], "completed");
    assert_eq!(before["mutations"][1]["state"], "running");
    assert_eq!(before["mutations"][2]["state"], "running");
    assert_eq!(before["mutations"][3]["state"], "pending");
    child.signal(libc::SIGINT);
    let output = child.finish();
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stderr).contains("interrupted"));
    let metadata = read(&directory, "run.json");
    let outcomes = read(&directory, "outcomes.json");
    assert_eq!(metadata["status"], "interrupted");
    assert_eq!(metadata["policy"]["status"], "not_evaluated");
    assert_eq!(metadata["score_policy"]["status"], "not_evaluated");
    assert!(metadata["score"]["percentage"].is_null());
    assert_eq!(metadata["checkpoint"], outcomes["checkpoint"]);
    let workspaces = &metadata["workspace_metrics"];
    assert_eq!(
        workspaces["temporary_root"],
        fixture.workspaces.to_str().unwrap()
    );
    for (phase, count) in [
        ("snapshot", 1),
        ("baseline_setup", 1),
        ("discovery_reset", 1),
        ("worker_setup", 2),
        ("worker_reset", 1),
    ] {
        assert_eq!(workspaces[phase]["operations"], count);
        assert_eq!(workspaces[phase]["completed"], count);
        assert_eq!(workspaces[phase]["failed"], 0);
        assert_eq!(workspaces[phase]["interrupted"], 0);
    }
    assert_eq!(outcomes["mutations"][0]["outcome"], "missed");
    assert_eq!(outcomes["mutations"][3]["state"], "pending");
    for (index, name) in [(1, "b"), (2, "c")] {
        let mutation = &outcomes["mutations"][index];
        assert_eq!(mutation["state"], "interrupted");
        assert!(mutation["outcome"].is_null());
        assert!(
            fs::read_to_string(directory.join(mutation["logs"]["stdout"].as_str().unwrap()))
                .unwrap()
                .contains(&format!("active {name}"))
        );
        assert!(
            fs::read_to_string(directory.join(mutation["logs"]["stderr"].as_str().unwrap()))
                .unwrap()
                .contains(&format!("diagnostic {name}"))
        );
    }
    assert_eq!(
        outcomes["summary"],
        serde_json::json!({"killed":0,"missed":1,"timeout":0,"build_failed":0,"errors":0})
    );
    assert_eq!(
        fs::read(directory.join("mutants.json")).unwrap(),
        inventory_bytes
    );
    assert_eq!(
        fs::read_to_string(directory.join("missed.txt"))
            .unwrap()
            .lines()
            .count(),
        1
    );
    fixture.assert_cleanup();
}

#[test]
fn interruption_during_baseline_and_discovery_preserves_command_output() {
    for (phase, signal, scope) in [
        ("baseline", libc::SIGINT, "full"),
        ("discovery", libc::SIGTERM, "full"),
        ("discovery", libc::SIGTERM, "affected"),
    ] {
        let fixture = Fixture::new();
        let mut child = fixture.start_with_args(
            phase,
            &[
                "--test-scope",
                scope,
                "--fail-on-missed",
                "--min-score",
                "100",
            ],
        );
        fixture.ready(phase);
        child.signal(signal);
        let output = child.finish();
        assert_eq!(output.status.code(), Some(1));
        let directory = fixture.report();
        let metadata = read(&directory, "run.json");
        let outcomes = read(&directory, "outcomes.json");
        assert_eq!(metadata["status"], "interrupted");
        assert_eq!(metadata["policy"]["status"], "not_evaluated");
        assert_eq!(metadata["score_policy"]["status"], "not_evaluated");
        assert!(metadata["score"]["percentage"].is_null());
        assert_eq!(metadata["phase"], phase);
        assert_eq!(metadata["workspace_metrics"]["snapshot"]["completed"], 1);
        assert_eq!(
            metadata["workspace_metrics"]["baseline_setup"]["completed"],
            1
        );
        assert_eq!(
            metadata["workspace_metrics"]["discovery_reset"]["completed"],
            if phase == "discovery" { 1 } else { 0 }
        );
        assert_eq!(
            metadata["workspace_metrics"]["worker_setup"]["operations"],
            0
        );
        assert_eq!(
            metadata["workspace_metrics"]["worker_reset"]["operations"],
            0
        );
        assert_eq!(outcomes[phase]["outcome"], "interrupted");
        assert!(outcomes["mutations"].as_array().unwrap().is_empty());
        assert_eq!(read(&directory, "mutants.json")["complete"], false);
        assert!(
            fs::read_to_string(directory.join(outcomes[phase]["logs"]["text"].as_str().unwrap()))
                .unwrap()
                .contains(&format!("active {phase}"))
        );
        fixture.assert_cleanup();
    }
}

#[test]
fn tool_error_drains_parallel_workers_and_preserves_the_failure_reason() {
    for (mode, diagnostic) in [
        ("tool_error", "go tool failed unexpectedly"),
        ("capture_error", "--max-capture-bytes"),
    ] {
        let fixture = Fixture::new();
        let mut child = fixture.start(mode);
        fixture.ready("b");
        fixture.ready("c");
        fs::write(fixture.state.join("fail"), "").unwrap();
        let output = child.finish();
        assert_eq!(output.status.code(), Some(1));
        assert!(String::from_utf8_lossy(&output.stderr).contains(diagnostic));
        let directory = fixture.report();
        let outcomes = read(&directory, "outcomes.json");
        assert_eq!(outcomes["status"], "failed");
        assert_eq!(outcomes["mutations"][0]["outcome"], "missed");
        assert_eq!(outcomes["mutations"][1]["state"], "interrupted");
        assert_eq!(outcomes["mutations"][2]["outcome"], "error");
        assert_eq!(outcomes["mutations"][3]["state"], "pending");
        assert!(
            fs::read_to_string(directory.join("logs/000002.log"))
                .unwrap()
                .contains("active b")
        );
        assert_eq!(outcomes["summary"]["errors"], 1);
        assert_eq!(outcomes["summary"]["missed"], 1);
        let metadata = read(&directory, "run.json");
        assert_eq!(metadata["policy"]["status"], "not_evaluated");
        assert_eq!(metadata["score_policy"]["status"], "not_evaluated");
        assert!(metadata["score"]["percentage"].is_null());
        assert_eq!(
            metadata["workspace_metrics"]["worker_setup"]["completed"],
            2
        );
        assert_eq!(
            metadata["workspace_metrics"]["worker_reset"]["completed"],
            1
        );
        fixture.assert_cleanup();
    }
}

#[test]
fn timeouts_remove_go_build_and_test_temporary_files_and_retain_diagnostics() {
    let fixture = Fixture::new();
    let mut child = fixture.start_with_args(
        "mutations",
        &["--jobs", "2", "--timeout", "1", "--exclude", "z.go"],
    );
    let output = child.finish();
    assert!(output.status.success(), "{output:?}");
    let directory = fixture.report();
    let outcomes = read(&directory, "outcomes.json");
    assert_eq!(outcomes["status"], "completed");
    assert_eq!(outcomes["summary"]["missed"], 1);
    assert_eq!(outcomes["summary"]["timeout"], 2);
    for (index, name) in [(1, "b"), (2, "c")] {
        let mutation = &outcomes["mutations"][index];
        assert_eq!(mutation["outcome"], "timeout");
        assert!(
            fs::read_to_string(directory.join(mutation["logs"]["stdout"].as_str().unwrap()))
                .unwrap()
                .contains(&format!("active {name}"))
        );
    }
    fixture.assert_cleanup();
}

#[test]
fn go_temporary_directories_respect_parent_placement_for_listing() {
    for placement in ["unset", "empty", "absolute", "relative"] {
        let fixture = Fixture::new();
        let mut command = fixture.command("discovery_success");
        let parent = match placement {
            "unset" => {
                command.env_remove("GOTMPDIR");
                &fixture.workspaces
            }
            "empty" => {
                command.env("GOTMPDIR", "");
                &fixture.workspaces
            }
            "relative" => {
                command.env("GOTMPDIR", "go-tmp");
                command.env("TMPDIR", "workspaces");
                &fixture.go_temporary
            }
            _ => &fixture.go_temporary,
        };
        let output = command.arg(&fixture.root).arg("--list").output().unwrap();
        assert!(output.status.success(), "{placement}: {output:?}");
        for entry in fs::read_dir(&fixture.state).unwrap() {
            let path = entry.unwrap().path();
            if path
                .extension()
                .is_some_and(|extension| extension == "gotmpdir")
            {
                let directory = PathBuf::from(fs::read_to_string(path).unwrap().trim());
                assert_eq!(directory.parent(), Some(parent.as_path()));
            }
        }
        fixture.assert_cleanup();
    }
}

#[test]
fn go_startup_errors_do_not_leave_temporary_directories() {
    for invalid_parent in [false, true] {
        let fixture = Fixture::new();
        // An executable whose interpreter is missing fails during spawn, after
        // Mutie has created both private temporary directories.
        fs::write(fixture.bin.join("go"), "#!/nonexistent/mutie-test-shell\n").unwrap();
        let mut command = fixture.command("startup_error");
        command
            .arg(&fixture.root)
            .arg("--list")
            .env("PATH", &fixture.bin);
        if invalid_parent {
            command.env("GOTMPDIR", fixture.go_temporary.join("missing"));
        }
        let output = command.output().unwrap();
        assert!(!output.status.success());
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains(if invalid_parent {
                "resolving Go temporary build directory"
            } else {
                "could not start go"
            }),
            "{stderr}"
        );
        fixture.assert_test_temporary_cleanup();
        assert_eq!(fs::read_dir(&fixture.go_temporary).unwrap().count(), 1);
        assert_eq!(
            fs::read_to_string(fixture.go_temporary.join("go-build-existing/keep")).unwrap(),
            "unrelated"
        );
        assert_eq!(fs::read_dir(&fixture.go_cache).unwrap().count(), 1);
    }
}

#[test]
fn completing_one_worker_preserves_another_workers_test_temporary_files() {
    let fixture = Fixture::new();
    let mut child = fixture.start_with_args(
        "isolation",
        &["--jobs", "2", "--timeout", "60", "--exclude", "z.go"],
    );
    fixture.ready("b");
    fixture.ready("c");
    let b = fixture.test_temporary("b");
    let c = fixture.test_temporary("c");
    assert_ne!(b, c);
    for directory in [&b, &c] {
        assert_eq!(
            fs::read_to_string(directory.join("test-artifact")).unwrap(),
            "test output\n"
        );
    }
    fs::write(fixture.state.join("release-b"), "").unwrap();
    wait_until(|| !b.exists());
    assert_eq!(
        fs::read_to_string(c.join("test-artifact")).unwrap(),
        "test output\n"
    );
    child.signal(libc::SIGTERM);
    let output = child.finish();
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    fixture.assert_cleanup();
}
