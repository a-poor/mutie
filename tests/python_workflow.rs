use std::{fs, path::Path, process::Command};

fn python() -> String {
    std::env::var("MUTIE_PYTHON").expect("set MUTIE_PYTHON to the pinned venv launcher")
}

fn fixture() -> tempfile::TempDir {
    let directory = tempfile::tempdir().unwrap();
    for name in [
        "pyproject.toml",
        "sample/__init__.py",
        "tests/test_sample.py",
    ] {
        let destination = directory.path().join(name);
        fs::create_dir_all(destination.parent().unwrap()).unwrap();
        fs::copy(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/python")
                .join(name),
            destination,
        )
        .unwrap();
    }
    directory
}

#[test]
fn unsupported_scored_options_fail_before_runtime_launch() {
    let root = fixture();
    for args in [
        vec!["--jobs", "2"],
        vec!["--test-scope", "affected"],
        vec!["--clear-max-capture-bytes"],
        vec!["--operator", "logical_swap"],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_mutie"))
            .arg(root.path())
            .args(["--lang", "python", "--python", "/missing-python", "--list"])
            .args(args)
            .output()
            .unwrap();
        assert!(!output.status.success(), "{output:?}");
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("Python"),
            "{output:?}"
        );
        assert!(
            !String::from_utf8_lossy(&output.stderr).contains("could not start"),
            "{output:?}"
        );
    }
}

fn run(command: &str, root: &Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_mutie"))
        .args([command, "--python", &python(), "--json"])
        .arg(root)
        .args(args)
        // Inherited options must not suppress or alter the audited test suite.
        .env("PYTEST_ADDOPTS", "--ignore=tests")
        .env("PYTEST_PLUGINS", "missing_plugin")
        .output()
        .unwrap()
}

#[test]
#[ignore = "requires MUTIE_PYTHON with pinned CPython/pytest"]
fn discovery_and_isolated_baseline_preserve_inputs() {
    let root = fixture();
    let source = fs::read(root.path().join("sample/__init__.py")).unwrap();
    let tests = fs::read(root.path().join("tests/test_sample.py")).unwrap();
    for command in ["python-list", "python-check"] {
        let output = run(command, root.path(), &[]);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(value["scope"]["mutations"].as_array().unwrap().len(), 2);
        if command == "python-check" {
            assert_eq!(value["tests"].as_array().unwrap().len(), 2);
        }
        assert_eq!(
            source,
            fs::read(root.path().join("sample/__init__.py")).unwrap()
        );
        assert_eq!(
            tests,
            fs::read(root.path().join("tests/test_sample.py")).unwrap()
        );
        assert!(!root.path().join(".mutie").exists());
        assert!(!root.path().join("sample/__pycache__").exists());
        assert!(!root.path().join(".pytest_cache").exists());
    }
}

#[test]
#[ignore = "requires MUTIE_PYTHON with pinned CPython/pytest"]
fn scope_failure_precedes_project_imports() {
    let root = fixture();
    let marker = root.path().join("imported");
    fs::write(
        root.path().join("sample/__init__.py"),
        format!(
            "open({:?}, 'w').write('imported')\nvalue=True\n",
            marker.to_str().unwrap()
        ),
    )
    .unwrap();
    let output = run("python-check", root.path(), &[]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("unsupported Python runtime access"));
    assert!(!marker.exists());
}

#[test]
#[ignore = "requires MUTIE_PYTHON with pinned CPython/pytest"]
fn baseline_rejects_assertion_runtime_skip_and_timeout() {
    for body in [
        "assert False",
        "raise ValueError('runtime')",
        "pytest.skip('unsupported')",
        "while True: pass",
    ] {
        let root = fixture();
        fs::write(
            root.path().join("tests/test_sample.py"),
            format!("import pytest\ndef test_sample():\n    {body}\n"),
        )
        .unwrap();
        let output = run("python-check", root.path(), &["--timeout", "2"]);
        assert!(!output.status.success(), "{body}");
        assert!(!root.path().join(".mutie").exists());
    }
}

#[test]
#[ignore = "requires MUTIE_PYTHON with pinned CPython/pytest"]
fn stale_bytecode_does_not_mask_changed_source() {
    let root = fixture();
    let source = root.path().join("sample/__init__.py");
    fs::write(
        &source,
        "def enabled():\n    return True \ndef optional():\n    return False\n",
    )
    .unwrap();
    let output = Command::new(python()).args(["-I", "-S", "-c",
        "import os, pathlib, py_compile, sys; p=pathlib.Path(sys.argv[1]); s=p.stat(); py_compile.compile(str(p), doraise=True); p.write_bytes(p.read_bytes().replace(b'True ', b'False')); os.utime(p, ns=(s.st_atime_ns, s.st_mtime_ns))"])
        .arg(&source).output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let native = Command::new(python()).args(["-I", "-S", "-B", "-c",
        "import sys; sys.path.insert(0, sys.argv[1]); import sample; assert sample.enabled() is True"])
        .arg(root.path()).output().unwrap();
    assert!(
        native.status.success(),
        "negative control must read stale bytecode"
    );
    let changed = fs::read(&source).unwrap();
    let output = run("python-check", root.path(), &[]);
    assert!(
        !output.status.success(),
        "fresh cache must expose the changed source"
    );
    assert!(String::from_utf8_lossy(&output.stderr).contains("AssertionError"));
    assert_eq!(changed, fs::read(&source).unwrap());
}

#[test]
#[ignore = "requires MUTIE_PYTHON with pinned CPython/pytest"]
fn static_scope_regressions() {
    let output = Command::new(python())
        .args(["-I", "-S", "-B"])
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/support/python_scope.py"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn scored(root: &Path, reports: &Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_mutie"))
        .arg(root)
        .args([
            "--lang",
            "python",
            "--python",
            &python(),
            "--ui",
            "plain",
            "--timeout",
            "5",
            "--baseline-timeout",
            "15",
            "--output-dir",
        ])
        .arg(reports)
        .args(args)
        .output()
        .unwrap()
}

fn saved(
    output: &std::process::Output,
) -> (std::path::PathBuf, serde_json::Value, serde_json::Value) {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let directory = std::path::PathBuf::from(
        stdout
            .lines()
            .find_map(|line| line.strip_prefix("Saving report to "))
            .unwrap_or_else(|| panic!("{output:?}")),
    );
    let read = |name| serde_json::from_slice(&fs::read(directory.join(name)).unwrap()).unwrap();
    (directory.clone(), read("run.json"), read("outcomes.json"))
}

#[test]
#[ignore = "requires MUTIE_PYTHON; real scored reports, stronger tests and identity-checked reruns"]
fn scored_reports_and_stronger_test_reruns() {
    let root = fixture();
    let reports = tempfile::tempdir().unwrap();
    let source = fs::read(root.path().join("sample/__init__.py")).unwrap();
    let first = scored(
        root.path(),
        reports.path(),
        &["--fail-on-missed", "--max-log-bytes", "32"],
    );
    assert_eq!(first.status.code(), Some(3), "{first:?}");
    let (directory, run, outcomes) = saved(&first);
    assert_eq!(run["status"], "completed");
    assert_eq!(run["score"]["killed"], 1);
    assert_eq!(run["score"]["missed"], 1);
    assert_eq!(outcomes["summary"]["errors"], 0);
    assert_eq!(
        run["python_scope"]["test_nodes"].as_array().unwrap().len(),
        2
    );
    let original_report = fs::read(directory.join("outcomes.json")).unwrap();
    let original_run = fs::read(directory.join("run.json")).unwrap();
    let browse = || {
        Command::new(env!("CARGO_BIN_EXE_mutie"))
            .arg("report")
            .arg(&directory)
            .output()
            .unwrap()
    };
    let accepted = browse();
    assert!(
        String::from_utf8_lossy(&accepted.stderr).contains("requires interactive terminal"),
        "{accepted:?}"
    );
    let mut corrupt = run.clone();
    corrupt["python_scope"]["test_nodes"] = serde_json::json!([]);
    fs::write(
        directory.join("run.json"),
        serde_json::to_vec(&corrupt).unwrap(),
    )
    .unwrap();
    let rejected = browse();
    assert!(
        String::from_utf8_lossy(&rejected.stderr).contains("pristine test inventory"),
        "{rejected:?}"
    );
    fs::write(directory.join("run.json"), &original_run).unwrap();
    let dismiss = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_mutie"))
            .args(["dismiss", "--python", &python(), "--module"])
            .arg(root.path())
            .args(["--lang", "python"])
            .args(args)
            .output()
            .unwrap()
    };
    for args in [
        vec![
            "add",
            directory.to_str().unwrap(),
            "--id",
            "2",
            "--reason",
            "accepted fixture gap",
        ],
        vec!["update", "1", "--note", "reviewed survivor"],
        vec!["list"],
        vec!["check"],
    ] {
        let output = dismiss(&args);
        assert!(output.status.success(), "{output:?}");
    }
    let reviewed = scored(
        root.path(),
        reports.path(),
        &["--fail-on-missed", "--min-score", "100"],
    );
    assert_eq!(reviewed.status.code(), Some(3), "{reviewed:?}");
    let (_, reviewed_run, reviewed_outcomes) = saved(&reviewed);
    assert_eq!(reviewed_run["policy"]["status"], "passed");
    assert_eq!(reviewed_run["policy"]["dismissed_missed"], 1);
    assert_eq!(reviewed_run["score_policy"]["status"], "failed");
    assert_eq!(reviewed_run["score"]["killed"], 1);
    assert_eq!(reviewed_run["score"]["missed"], 1);
    assert_eq!(reviewed_outcomes["summary"], outcomes["summary"]);
    let removed = dismiss(&["remove", "1"]);
    assert!(removed.status.success(), "{removed:?}");
    fs::write(
        root.path().join("tests/test_stronger.py"),
        "from sample import optional\ndef test_optional():\n    assert optional() is False\n",
    )
    .unwrap();
    let rerun = scored(
        root.path(),
        reports.path(),
        &[
            "--rerun-missed",
            directory.to_str().unwrap(),
            "--fail-on-missed",
        ],
    );
    assert!(rerun.status.success(), "{rerun:?}");
    let (_, run, outcomes) = saved(&rerun);
    assert_eq!(run["score"]["killed"], 1);
    assert_eq!(run["score"]["missed"], 0);
    assert_eq!(
        run["python_scope"]["test_nodes"].as_array().unwrap().len(),
        3
    );
    assert_eq!(outcomes["summary"]["errors"], 0);
    assert_eq!(
        fs::read(root.path().join("sample/__init__.py")).unwrap(),
        source
    );
    assert_eq!(
        fs::read(directory.join("outcomes.json")).unwrap(),
        original_report
    );
    assert_eq!(fs::read(directory.join("run.json")).unwrap(), original_run);
    let manifest = root.path().join("pyproject.toml");
    fs::write(
        &manifest,
        fs::read_to_string(&manifest).unwrap() + "\n# changed configuration\n",
    )
    .unwrap();
    let rejected = scored(
        root.path(),
        reports.path(),
        &["--rerun-missed", directory.to_str().unwrap()],
    );
    assert!(!rejected.status.success(), "{rejected:?}");
    assert!(
        String::from_utf8_lossy(&rejected.stderr).contains("Python execution identity changed"),
        "{rejected:?}"
    );
}

#[test]
#[ignore = "requires MUTIE_PYTHON; candidate collection must match pristine test IDs"]
fn mutation_cannot_change_parametrized_inventory() {
    let root = fixture();
    let reports = tempfile::tempdir().unwrap();
    fs::write(root.path().join("tests/test_sample.py"),
        "import pytest\nfrom sample import enabled\n@pytest.mark.parametrize('value', [1, 2] if enabled() else [1])\ndef test_value(value):\n    assert value > 0\n").unwrap();
    let output = scored(root.path(), reports.path(), &[]);
    assert!(!output.status.success(), "{output:?}");
    let (directory, run, outcomes) = saved(&output);
    assert_eq!(run["status"], "failed");
    assert_eq!(outcomes["summary"]["errors"], 1);
    assert_eq!(run["score"]["killed"], 0);
    let text = fs::read_to_string(directory.join("outcomes.json")).unwrap();
    assert!(
        text.contains("candidate collection differs from pristine baseline"),
        "{text}"
    );
}

#[test]
#[ignore = "requires MUTIE_PYTHON; pristine collection failures retain bounded diagnostics"]
fn failed_pristine_collection_is_saved() {
    let root = fixture();
    let reports = tempfile::tempdir().unwrap();
    fs::write(
        root.path().join("tests/test_sample.py"),
        "assert False, 'collection failure'\ndef test_body(): pass\n",
    )
    .unwrap();
    let output = scored(root.path(), reports.path(), &["--max-log-bytes", "64"]);
    assert!(!output.status.success(), "{output:?}");
    let (directory, run, outcomes) = saved(&output);
    assert_eq!(run["status"], "failed");
    assert_eq!(outcomes["discovery"]["outcome"], "failed");
    assert!(outcomes["baseline"].is_null());
    let logs = &outcomes["discovery"]["logs"];
    for stream in ["stdout", "stderr", "text"] {
        let bytes = fs::read(directory.join(logs[stream].as_str().unwrap())).unwrap();
        let retention = &logs["retention"][stream];
        let retained =
            retention["head_bytes"].as_u64().unwrap() + retention["tail_bytes"].as_u64().unwrap();
        assert!(retained <= 64);
        // Saved files include the shared human-readable truncation notice.
        assert!(bytes.len() <= 64 + 192);
    }
    assert!(
        logs["retention"]["stdout"]["omitted_bytes"]
            .as_u64()
            .unwrap()
            > 0
    );
}

#[test]
#[ignore = "requires MUTIE_PYTHON; scored resource failures must not count as kills"]
fn scored_resource_limits_preserve_score_boundaries() {
    for (name, body) in [
        ("timeout", "        while True: pass\n"),
        (
            "capture",
            "        with capsys.disabled():\n            print('x' * 65536, flush=True)\n",
        ),
        (
            "file",
            "        (tmp_path / 'large').write_bytes(b'x' * (5 * 1024 * 1024))\n",
        ),
    ] {
        let root = fixture();
        let reports = tempfile::tempdir().unwrap();
        let source = root.path().join("sample/__init__.py");
        fs::write(&source, "def enabled():\n    return True\n").unwrap();
        let before = fs::read(&source).unwrap();
        fs::write(
            root.path().join("tests/test_sample.py"),
            format!("from sample import enabled\ndef test_body(capsys, tmp_path):\n    if not enabled():\n{body}"),
        ).unwrap();
        let args = if name == "capture" {
            vec!["--max-capture-bytes", "8192", "--min-score", "100"]
        } else {
            vec!["--min-score", "100"]
        };
        let output = scored(root.path(), reports.path(), &args);
        let (_, run, outcomes) = saved(&output);
        assert_eq!(
            outcomes["baseline"]["outcome"], "passed",
            "{name}: {output:?}"
        );
        assert_eq!(outcomes["summary"]["killed"], 0, "{name}: {outcomes}");
        assert!(run["score"]["percentage"].is_null(), "{name}: {run}");
        if name == "timeout" {
            assert_eq!(run["status"], "completed");
            assert_eq!(outcomes["summary"]["timeout"], 1);
        } else {
            assert_eq!(run["status"], "failed");
            assert_eq!(outcomes["summary"]["errors"], 1);
        }
        assert!(!output.status.success(), "{name}: {output:?}");
        assert_eq!(fs::read(source).unwrap(), before);
    }
}

#[cfg(unix)]
#[test]
#[ignore = "requires MUTIE_PYTHON; interrupt an observed running mutant and clean owned workspaces"]
fn scored_cancellation_cleans_owned_workspaces() {
    use std::{
        process::Stdio,
        thread,
        time::{Duration, Instant},
    };
    fn has_checkpoint(path: &Path) -> bool {
        let Ok(entries) = fs::read_dir(path) else {
            return false;
        };
        entries.filter_map(Result::ok).any(|entry| {
            let Ok(kind) = entry.file_type() else {
                return false;
            };
            (kind.is_file() && entry.file_name() == "mutant-ready")
                || (kind.is_dir() && has_checkpoint(&entry.path()))
        })
    }
    let root = fixture();
    let reports = tempfile::tempdir().unwrap();
    let scratch = tempfile::tempdir().unwrap();
    let source = root.path().join("sample/__init__.py");
    fs::write(&source, "def enabled():\n    return True\n").unwrap();
    let before = fs::read(&source).unwrap();
    fs::write(root.path().join("tests/test_sample.py"),
        "from sample import enabled\ndef test_body(tmp_path):\n    if not enabled():\n        (tmp_path / 'mutant-ready').write_text('ready')\n        while True: pass\n").unwrap();
    let stdout = reports.path().join("stdout");
    let stderr = reports.path().join("stderr");
    let mut child = Command::new(env!("CARGO_BIN_EXE_mutie"))
        .arg(root.path())
        .args([
            "--lang",
            "python",
            "--python",
            &python(),
            "--ui",
            "plain",
            "--timeout",
            "30",
            "--baseline-timeout",
            "15",
            "--output-dir",
        ])
        .arg(reports.path())
        .env("TMPDIR", scratch.path())
        .stdout(Stdio::from(fs::File::create(&stdout).unwrap()))
        .stderr(Stdio::from(fs::File::create(&stderr).unwrap()))
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(60);
    while !has_checkpoint(scratch.path()) {
        if let Some(status) = child.try_wait().unwrap() {
            panic!(
                "exited before mutant checkpoint: {status}: {}",
                fs::read_to_string(&stderr).unwrap()
            );
        }
        if Instant::now() > deadline {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("mutant checkpoint never appeared");
        }
        thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(unsafe { libc::kill(child.id() as i32, libc::SIGINT) }, 0);
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if Instant::now() > deadline {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("Python cancellation did not finish");
        }
        thread::sleep(Duration::from_millis(20));
    };
    let output = std::process::Output {
        status,
        stdout: fs::read(stdout).unwrap(),
        stderr: fs::read(stderr).unwrap(),
    };
    let (_, run, outcomes) = saved(&output);
    assert_eq!(run["status"], "interrupted", "{output:?}");
    assert_eq!(outcomes["baseline"]["outcome"], "passed");
    assert_eq!(outcomes["mutations"][0]["state"], "interrupted");
    assert_eq!(outcomes["summary"]["killed"], 0);
    assert!(run["score"]["percentage"].is_null());
    assert_eq!(fs::read(source).unwrap(), before);
    assert_eq!(
        fs::read_dir(scratch.path()).unwrap().count(),
        0,
        "owned temporary files leaked"
    );
}
