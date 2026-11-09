#[path = "support/invocation.rs"]
mod invocation;
#[allow(unused_imports)]
use invocation::{Child, Command};

use std::{fs, path::Path};

#[path = "support/python_calls.rs"]
mod calls;

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
    for args in [vec!["--jobs", "2"], vec!["--test-scope", "affected"]] {
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
        if command == "python-check" {
            assert_eq!(value["passed"], true);
        } else {
            assert_eq!(value["mutations"].as_array().unwrap().len(), 2);
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
fn review_metadata_is_excluded_from_execution_but_nested_data_is_preserved() {
    let root = fixture();
    for name in ["mutie.dismissals.toml", ".mutie.dismissals.lock"] {
        fs::write(root.path().join(name), "owned review metadata").unwrap();
    }
    fs::create_dir(root.path().join("tests/data")).unwrap();
    fs::write(
        root.path().join("tests/data/mutie.dismissals.toml"),
        "ordinary fixture",
    )
    .unwrap();
    fs::write(
        root.path().join("tests/test_metadata.py"),
        "from pathlib import Path\ndef test_metadata():\n root = Path(__file__).parent.parent\n assert not (root / 'mutie.dismissals.toml').exists()\n assert not (root / '.mutie.dismissals.lock').exists()\n assert (root / 'tests/data/mutie.dismissals.toml').read_text() == 'ordinary fixture'\n",
    )
    .unwrap();
    let output = run("python-check", root.path(), &[]);
    assert!(output.status.success(), "{output:?}");
    for name in ["mutie.dismissals.toml", ".mutie.dismissals.lock"] {
        assert_eq!(
            fs::read_to_string(root.path().join(name)).unwrap(),
            "owned review metadata"
        );
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

#[test]
#[ignore = "requires MUTIE_PYTHON; runtime fingerprint format, drift and rejection controls"]
fn runtime_fingerprint_regressions() {
    let cache = tempfile::tempdir().unwrap();
    let output = Command::new(python())
        .args(["-I", "-S", "-B", "-X"])
        .arg(format!("pycache_prefix={}", cache.path().display()))
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/support/python_runtime.py"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn scored(root: &Path, reports: &Path, args: &[&str]) -> std::process::Output {
    scored_with_timeout(root, reports, args, "5")
}

fn scored_with_timeout(
    root: &Path,
    reports: &Path,
    args: &[&str],
    timeout: &str,
) -> std::process::Output {
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
            timeout,
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
    scored_review_contract(false, false, false, false);
}

#[test]
#[ignore = "requires MUTIE_PYTHON; native src layout scoring, review and reruns"]
fn src_scored_reports_and_stronger_test_reruns() {
    scored_review_contract(true, false, false, false);
}

#[test]
#[ignore = "requires MUTIE_PYTHON; root tests, inert packaging and ancillary files"]
fn root_scored_reports_and_stronger_test_reruns() {
    scored_review_contract(false, true, false, false);
}

#[test]
#[ignore = "requires MUTIE_PYTHON; preserve flat packages named src with subpackages"]
fn flat_package_named_src_preserves_native_execution() {
    let root = fixture();
    fs::rename(root.path().join("sample"), root.path().join("src")).unwrap();
    fs::create_dir(root.path().join("src/nested")).unwrap();
    fs::write(root.path().join("src/nested/__init__.py"), "").unwrap();
    let test = root.path().join("tests/test_sample.py");
    fs::write(
        &test,
        fs::read_to_string(&test).unwrap().replace("sample", "src") + "\nfrom src import nested\n",
    )
    .unwrap();
    let source = fs::read(root.path().join("src/__init__.py")).unwrap();
    let reports = tempfile::tempdir().unwrap();
    // This is a layout control, not a short-deadline probe. Give isolated
    // interpreter startup the same headroom as the fixture baseline.
    let output = scored_with_timeout(root.path(), reports.path(), &[], "15");
    assert!(output.status.success(), "{output:?}");
    let (_, run, outcomes) = saved(&output);
    assert_eq!(run["python_scope"]["package"], "src");
    assert!(run["python_scope"].get("source_root").is_none());
    assert_eq!(
        outcomes["summary"],
        serde_json::json!({
            "killed": 1, "missed": 1, "errors": 0, "timeout": 0, "build_failed": 0,
        }),
        "{outcomes}"
    );
    assert_eq!(
        fs::read(root.path().join("src/__init__.py")).unwrap(),
        source
    );
}

#[test]
#[ignore = "requires MUTIE_PYTHON; unittest scoring, review and focused reruns"]
fn unittest_scored_reports_and_stronger_test_reruns() {
    scored_review_contract(false, false, true, false);
}

#[test]
#[ignore = "requires MUTIE_PYTHON; conftest scoring, review and immutable fixture identity"]
fn conftest_scored_reports_and_stronger_test_reruns() {
    scored_review_contract(false, false, false, true);
}

fn scored_review_contract(src_layout: bool, root_tests: bool, unittest: bool, conftest: bool) {
    let root = fixture();
    if conftest {
        fs::write(root.path().join("conftest.py"), "import pytest\nfrom sample import enabled, optional\n@pytest.fixture\ndef enabled_value():\n yield enabled()\n@pytest.fixture(scope='session')\ndef optional_callable():\n return optional\n").unwrap();
        fs::write(root.path().join("tests/test_sample.py"), "def test_enabled(enabled_value): assert enabled_value is True\ndef test_optional_exists(optional_callable): assert callable(optional_callable)\n").unwrap();
    }
    if unittest {
        fs::write(root.path().join("tests/test_sample.py"), "import unittest\nfrom sample import enabled, optional\nclass TestSample(unittest.TestCase):\n def setUp(self): self.addCleanup(self.assertTrue, True)\n def test_enabled(self): self.assertTrue(enabled())\n def test_optional_exists(self): self.assertTrue(callable(optional))\n").unwrap();
    }
    if root_tests {
        fs::rename(
            root.path().join("tests/test_sample.py"),
            root.path().join("test_sample.py"),
        )
        .unwrap();
        fs::write(root.path().join("pyproject.toml"), "[project]\nname='sample'\ndynamic=['version']\n[project.optional-dependencies]\ntests=['pytest','pytest-cov','pytest-xdist']\n[tool.setuptools]\npackages=['sample']\n[tool.setuptools.dynamic]\nversion={attr='sample.__version__'}\n[tool.setuptools.package-data]\nsample=['py.typed']\n").unwrap();
        let source = root.path().join("sample/__init__.py");
        fs::write(
            &source,
            "__version__ = '1.0'\n".to_owned() + &fs::read_to_string(&source).unwrap(),
        )
        .unwrap();
        for name in ["doc/source/conf.py", ".circleci/build_docs.py"] {
            let path = root.path().join(name);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(
                path,
                "raise RuntimeError('ancillary code must never execute')\n",
            )
            .unwrap();
        }
    }
    let source_name = if src_layout {
        fs::create_dir(root.path().join("src")).unwrap();
        fs::rename(root.path().join("sample"), root.path().join("src/sample")).unwrap();
        fs::write(
            root.path().join("pyproject.toml"),
            r#"[project]
name = "sample"
dependencies = []
[tool.pytest.ini_options]
pythonpath = ["src"]
addopts = "-q"
"#,
        )
        .unwrap();
        "src/sample/__init__.py"
    } else {
        "sample/__init__.py"
    };
    let reports = tempfile::tempdir().unwrap();
    let source = fs::read(root.path().join(source_name)).unwrap();
    // This verifies reports/review/layout, not deadlines. Interpreter startup
    // can exceed five seconds under validation load; keep timeout probes separate.
    let first = scored_with_timeout(
        root.path(),
        reports.path(),
        &["--fail-on-missed", "--max-log-bytes", "32"],
        "15",
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
    assert_eq!(
        run["python_scope"]
            .get("source_root")
            .and_then(|v| v.as_str())
            .unwrap_or(""),
        if src_layout { "src" } else { "" }
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
    let mut wrong_root = run.clone();
    wrong_root["python_scope"]["source_root"] =
        serde_json::json!(if src_layout { "" } else { "src" });
    fs::write(
        directory.join("run.json"),
        serde_json::to_vec(&wrong_root).unwrap(),
    )
    .unwrap();
    let rejected = browse();
    assert!(
        String::from_utf8_lossy(&rejected.stderr).contains("outside supported source scope"),
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
    let reviewed = scored_with_timeout(
        root.path(),
        reports.path(),
        &["--fail-on-missed", "--min-score", "100"],
        "15",
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
        if unittest { "from unittest import TestCase\nfrom sample import optional\nclass TestStronger(TestCase):\n def test_optional(self): self.assertFalse(optional())\n" } else { "from sample import optional\ndef test_optional():\n    assert optional() is False\n" },
    )
    .unwrap();
    let rerun = scored_with_timeout(
        root.path(),
        reports.path(),
        &[
            "--rerun-missed",
            directory.to_str().unwrap(),
            "--fail-on-missed",
        ],
        "15",
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
    assert_eq!(fs::read(root.path().join(source_name)).unwrap(), source);
    assert_eq!(
        fs::read(directory.join("outcomes.json")).unwrap(),
        original_report
    );
    assert_eq!(fs::read(directory.join("run.json")).unwrap(), original_run);
    if conftest {
        let path = root.path().join("conftest.py");
        let before = fs::read_to_string(&path).unwrap();
        fs::write(
            &path,
            before.clone() + "\n# changed fixture configuration\n",
        )
        .unwrap();
        let rejected = scored_with_timeout(
            root.path(),
            reports.path(),
            &["--rerun-missed", directory.to_str().unwrap()],
            "15",
        );
        assert!(!rejected.status.success());
        assert!(
            String::from_utf8_lossy(&rejected.stderr).contains("Python execution identity changed")
        );
        fs::write(&path, before).unwrap();
    }
    let manifest = root.path().join("pyproject.toml");
    fs::write(
        &manifest,
        fs::read_to_string(&manifest).unwrap() + "\n# changed configuration\n",
    )
    .unwrap();
    let rejected = scored_with_timeout(
        root.path(),
        reports.path(),
        &["--rerun-missed", directory.to_str().unwrap()],
        "15",
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
#[ignore = "requires MUTIE_PYTHON; mixed unittest failures remain saved errors"]
fn unittest_body_and_cleanup_failure_cannot_be_scored_as_kill() {
    let root = fixture();
    let reports = tempfile::tempdir().unwrap();
    let original = fs::read(root.path().join("sample/__init__.py")).unwrap();
    fs::write(root.path().join("tests/test_sample.py"),
        "import unittest\nfrom sample import enabled\nclass TestBody(unittest.TestCase):\n def setUp(self): self.addCleanup(self.assertTrue, enabled())\n def test_body(self): self.assertTrue(enabled())\n").unwrap();
    let output = scored_with_timeout(root.path(), reports.path(), &[], "15");
    assert!(!output.status.success(), "{output:?}");
    let (_, run, outcomes) = saved(&output);
    assert_eq!(run["status"], "failed");
    assert_eq!(outcomes["summary"]["errors"], 1);
    assert_eq!(outcomes["summary"]["killed"], 0);
    assert_eq!(
        fs::read(root.path().join("sample/__init__.py")).unwrap(),
        original
    );
}

#[test]
#[ignore = "requires MUTIE_PYTHON; conftest cleanup dominates body failures"]
fn conftest_cleanup_failure_is_saved_as_error() {
    let root = fixture();
    let reports = tempfile::tempdir().unwrap();
    fs::write(root.path().join("tests/conftest.py"),
        "import pytest\nfrom sample import enabled\n@pytest.fixture(autouse=True)\ndef cleanup():\n yield\n assert enabled()\n").unwrap();
    let source = fs::read(root.path().join("sample/__init__.py")).unwrap();
    let output = scored_with_timeout(root.path(), reports.path(), &[], "15");
    assert!(!output.status.success(), "{output:?}");
    let (_, run, outcomes) = saved(&output);
    assert_eq!(run["status"], "failed");
    assert_eq!(outcomes["summary"]["errors"], 1);
    assert_eq!(outcomes["summary"]["killed"], 0);
    assert_eq!(
        fs::read(root.path().join("sample/__init__.py")).unwrap(),
        source
    );
}

#[test]
#[ignore = "requires MUTIE_PYTHON; no ancestor conftest imports and no report exclusions hiding fixtures"]
fn conftest_scope_and_parent_boundaries() {
    let container = tempfile::tempdir().unwrap();
    fs::write(
        container.path().join("conftest.py"),
        "raise RuntimeError('outside project')\n",
    )
    .unwrap();
    let source = fixture();
    let root = container.path().join("project");
    fs::create_dir(&root).unwrap();
    for name in [
        "pyproject.toml",
        "sample/__init__.py",
        "tests/test_sample.py",
    ] {
        fs::create_dir_all(root.join(name).parent().unwrap()).unwrap();
        fs::copy(source.path().join(name), root.join(name)).unwrap();
    }
    fs::write(
        root.join("conftest.py"),
        "import pytest\n@pytest.fixture(autouse=True)\ndef audit(): yield\n",
    )
    .unwrap();
    let passed = run("python-check", &root, &["--timeout", "15"]);
    assert!(passed.status.success(), "{passed:?}");
    // Keep the root test while a report-directory exclusion removes only conftest.
    fs::rename(
        root.join("tests/test_sample.py"),
        root.join("test_sample.py"),
    )
    .unwrap();
    fs::write(
        root.join("pyproject.toml"),
        "[project]\nname='sample'\nversion='0.1.0'\n",
    )
    .unwrap();
    fs::rename(root.join("conftest.py"), root.join("tests/conftest.py")).unwrap();
    let excluded = scored_with_timeout(&root, &root.join("tests"), &[], "15");
    assert!(!excluded.status.success(), "{excluded:?}");
    assert!(
        String::from_utf8_lossy(&excluded.stderr).contains("snapshot"),
        "{excluded:?}"
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
    cancellation_contract(false);
}

#[cfg(unix)]
#[test]
#[ignore = "requires MUTIE_PYTHON; cancellation after a continued error keeps interruption precedence"]
fn candidate_error_continuation_still_cancels() {
    cancellation_contract(true);
}

#[cfg(unix)]
fn cancellation_contract(continuation: bool) {
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
    fs::write(
        &source,
        if continuation {
            "def earlier():\n    return True\ndef enabled():\n    return True\n"
        } else {
            "def enabled():\n    return True\n"
        },
    )
    .unwrap();
    let before = fs::read(&source).unwrap();
    fs::write(root.path().join("tests/test_sample.py"),
        format!("{}from sample import enabled\ndef test_body(tmp_path):\n    if not enabled():\n        (tmp_path / 'mutant-ready').write_text('ready')\n        while True: pass\n",
            if continuation { "from sample import earlier\ndef test_earlier():\n    if not earlier(): raise ValueError('candidate')\n" } else { "" })).unwrap();
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
        .args(if continuation {
            vec!["--continue-on-error", "10"]
        } else {
            vec![]
        })
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
    assert_eq!(
        outcomes["mutations"][usize::from(continuation)]["state"],
        "interrupted"
    );
    assert_eq!(outcomes["summary"]["errors"], usize::from(continuation));
    assert_eq!(outcomes["summary"]["killed"], 0);
    assert!(run["score"]["percentage"].is_null());
    assert_eq!(fs::read(source).unwrap(), before);
    assert_eq!(
        fs::read_dir(scratch.path()).unwrap().count(),
        0,
        "owned temporary files leaked"
    );
}

#[test]
#[ignore = "requires MUTIE_PYTHON; imported conftest helpers retain lifecycle attribution"]
fn conftest_helpers_preserve_lifecycle_attribution() {
    for cleanup in [false, true] {
        let root = fixture();
        let reports = tempfile::tempdir().unwrap();
        fs::write(
            root.path().join("tests/__init__.py"),
            "\"\"\"Local helper imports.\"\"\"\n",
        )
        .unwrap();
        let tail = if cleanup { "\n check()" } else { "" };
        fs::write(root.path().join("tests/conftest.py"), format!(
            "import pytest\nfrom sample import enabled\ndef check(): assert enabled() is True\n@pytest.fixture(autouse=True)\ndef shared():\n yield{tail}\n"
        )).unwrap();
        fs::write(
            root.path().join("tests/test_sample.py"),
            "from .conftest import check\ndef test_enabled(): check()\n",
        )
        .unwrap();
        let source = fs::read(root.path().join("sample/__init__.py")).unwrap();
        let output = scored_with_timeout(root.path(), reports.path(), &[], "15");
        let (_, run, outcomes) = saved(&output);
        assert_eq!(output.status.success(), !cleanup, "{output:?}");
        assert_eq!(run["status"], if cleanup { "failed" } else { "completed" });
        assert_eq!(outcomes["summary"]["errors"], usize::from(cleanup));
        assert_eq!(outcomes["summary"]["killed"], usize::from(!cleanup));
        assert_eq!(
            fs::read(root.path().join("sample/__init__.py")).unwrap(),
            source
        );
    }
}

#[test]
#[ignore = "requires MUTIE_PYTHON; comparison tokens, saved operator identity and stronger reruns"]
fn comparison_boundaries_preserve_native_semantics_and_rerun() {
    let root = tempfile::tempdir().unwrap();
    for name in [
        "pyproject.toml",
        "sample/__init__.py",
        "tests/test_sample.py",
    ] {
        let target = root.path().join(name);
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::copy(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/python-boundaries")
                .join(name),
            target,
        )
        .unwrap();
    }
    let source = fs::read(root.path().join("sample/__init__.py")).unwrap();
    let reports = tempfile::tempdir().unwrap();
    let output = scored_with_timeout(
        root.path(),
        reports.path(),
        &["--operator", "comparison_boundary"],
        "15",
    );
    assert!(output.status.success(), "{output:?}");
    let (original, run, outcomes) = saved(&output);
    assert_eq!(
        run["python_scope"]["test_nodes"].as_array().unwrap().len(),
        10
    );
    assert_eq!(
        outcomes["summary"],
        serde_json::json!({"killed":4,"missed":1,"errors":0,"timeout":0,"build_failed":0})
    );
    let inventory: serde_json::Value =
        serde_json::from_slice(&fs::read(original.join("mutants.json")).unwrap()).unwrap();
    assert!(
        inventory["mutations"]
            .as_array()
            .unwrap()
            .iter()
            .all(|m| m["operator"] == "comparison_boundary")
    );
    let old_outcomes = fs::read(original.join("outcomes.json")).unwrap();
    fs::write(root.path().join("tests/test_stronger.py"), "from sample import within_budget\ndef test_exact_budget(): assert within_budget(2.0, 2.0)\n").unwrap();
    let output = scored_with_timeout(
        root.path(),
        reports.path(),
        &["--rerun", original.to_str().unwrap(), "--id", "5"],
        "15",
    );
    assert!(output.status.success(), "{output:?}");
    let (_, run, outcomes) = saved(&output);
    assert_eq!(
        run["python_scope"]["test_nodes"].as_array().unwrap().len(),
        11
    );
    assert_eq!(outcomes["summary"]["killed"], 1);
    assert_eq!(outcomes["summary"]["errors"], 0);
    assert_eq!(
        fs::read(root.path().join("sample/__init__.py")).unwrap(),
        source
    );
    assert_eq!(
        fs::read(original.join("outcomes.json")).unwrap(),
        old_outcomes
    );
    // An operator/token mismatch in a saved report must be rejected before execution.
    let mut corrupt = inventory;
    corrupt["mutations"][0]["operator"] = serde_json::json!("boolean_flip");
    fs::write(
        original.join("mutants.json"),
        serde_json::to_vec(&corrupt).unwrap(),
    )
    .unwrap();
    let output = scored_with_timeout(
        root.path(),
        reports.path(),
        &["--rerun", original.to_str().unwrap(), "--id", "1"],
        "15",
    );
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("saved Python mutation"),
        "{output:?}"
    );
}

#[test]
#[ignore = "requires MUTIE_PYTHON; changed rich-comparison dispatch must remain a runtime error"]
fn comparison_custom_dispatch_failure_is_not_a_kill() {
    let root = fixture();
    fs::write(root.path().join("sample/__init__.py"),
        "class Value:\n    def __lt__(self, other): return True\n    def __le__(self, other): raise RuntimeError('different native dispatch')\ndef compare(left, right): return left < right\n").unwrap();
    fs::write(root.path().join("tests/test_sample.py"),
        "from sample import Value, compare\ndef test_dispatch(): assert compare(Value(), Value()) is True\n").unwrap();
    let source = fs::read(root.path().join("sample/__init__.py")).unwrap();
    let reports = tempfile::tempdir().unwrap();
    let output = scored_with_timeout(
        root.path(),
        reports.path(),
        &["--operator", "comparison_boundary"],
        "15",
    );
    assert!(!output.status.success(), "{output:?}");
    let (_, run, outcomes) = saved(&output);
    assert_eq!(run["status"], "failed");
    assert_eq!(outcomes["summary"]["errors"], 1);
    assert_eq!(outcomes["summary"]["killed"], 0);
    assert_eq!(
        fs::read(root.path().join("sample/__init__.py")).unwrap(),
        source
    );
}

#[test]
#[ignore = "requires MUTIE_PYTHON; independent random generators retain runtime-error classification"]
fn independent_random_generator_and_invalid_seed_error() {
    let root = fixture();
    fs::write(root.path().join("sample/__init__.py"),
        "import random\ndef draw():\n    seed = 1 if True else object()\n    return random.Random(seed).random()\n").unwrap();
    fs::write(root.path().join("tests/test_sample.py"),
        "from sample import draw\ndef test_draw():\n    assert draw() == draw() == 0.13436424411240122\n").unwrap();
    let source = fs::read(root.path().join("sample/__init__.py")).unwrap();
    let baseline = run("python-check", root.path(), &[]);
    assert!(baseline.status.success(), "{baseline:?}");
    let reports = tempfile::tempdir().unwrap();
    let output = scored_with_timeout(root.path(), reports.path(), &[], "15");
    assert!(!output.status.success(), "{output:?}");
    let (_, run, outcomes) = saved(&output);
    assert_eq!(run["status"], "failed");
    assert_eq!(outcomes["summary"]["errors"], 1);
    assert_eq!(outcomes["summary"]["killed"], 0);
    assert_eq!(
        fs::read(root.path().join("sample/__init__.py")).unwrap(),
        source
    );
}

#[test]
#[ignore = "requires MUTIE_PYTHON; arithmetic tokens, saved operator identity and stronger reruns"]
fn arithmetic_preserves_native_semantics_and_rerun() {
    let root = tempfile::tempdir().unwrap();
    for name in [
        "pyproject.toml",
        "sample/__init__.py",
        "tests/test_sample.py",
    ] {
        let target = root.path().join(name);
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::copy(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/python-arithmetic")
                .join(name),
            target,
        )
        .unwrap();
    }
    let source = fs::read(root.path().join("sample/__init__.py")).unwrap();
    let reports = tempfile::tempdir().unwrap();
    let output = scored_with_timeout(
        root.path(),
        reports.path(),
        &["--operator", "arithmetic_replace"],
        "15",
    );
    assert!(output.status.success(), "{output:?}");
    let (original, run, outcomes) = saved(&output);
    assert_eq!(
        run["python_scope"]["test_nodes"].as_array().unwrap().len(),
        5
    );
    assert_eq!(
        outcomes["summary"],
        serde_json::json!({"killed":4,"missed":1,"errors":0,"timeout":0,"build_failed":0})
    );
    let inventory: serde_json::Value =
        serde_json::from_slice(&fs::read(original.join("mutants.json")).unwrap()).unwrap();
    assert!(
        inventory["mutations"]
            .as_array()
            .unwrap()
            .iter()
            .all(|m| m["operator"] == "arithmetic_replace")
    );
    let old_outcomes = fs::read(original.join("outcomes.json")).unwrap();
    fs::write(
        root.path().join("tests/test_stronger.py"),
        "from sample import net\ndef test_nonzero_fee(): assert net(10, 2) == 8\n",
    )
    .unwrap();
    let output = scored_with_timeout(
        root.path(),
        reports.path(),
        &["--rerun", original.to_str().unwrap(), "--id", "5"],
        "15",
    );
    assert!(output.status.success(), "{output:?}");
    let (_, run, outcomes) = saved(&output);
    assert_eq!(
        run["python_scope"]["test_nodes"].as_array().unwrap().len(),
        6
    );
    assert_eq!(outcomes["summary"]["killed"], 1);
    assert_eq!(outcomes["summary"]["errors"], 0);
    assert_eq!(
        fs::read(root.path().join("sample/__init__.py")).unwrap(),
        source
    );
    assert_eq!(
        fs::read(original.join("outcomes.json")).unwrap(),
        old_outcomes
    );
    // An operator/token mismatch in a saved report must be rejected before execution.
    let mut corrupt = inventory;
    corrupt["mutations"][0]["operator"] = serde_json::json!("boolean_flip");
    fs::write(
        original.join("mutants.json"),
        serde_json::to_vec(&corrupt).unwrap(),
    )
    .unwrap();
    let output = scored_with_timeout(
        root.path(),
        reports.path(),
        &["--rerun", original.to_str().unwrap(), "--id", "1"],
        "15",
    );
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("saved Python mutation"),
        "{output:?}"
    );
}

#[test]
#[ignore = "requires MUTIE_PYTHON; arithmetic native exceptions remain errors"]
fn arithmetic_dispatch_and_zero_division_are_not_kills() {
    for (source, tests) in [
        (
            "class Value:\n def __add__(self, other): return 8\n def __sub__(self, other): raise RuntimeError('native dispatch')\ndef calculate(a,b): return a + b\n",
            "from sample import Value, calculate\ndef test_dispatch(): assert calculate(Value(),Value()) == 8\n",
        ),
        (
            "def calculate(a,b): return a * b\n",
            "from sample import calculate\ndef test_zero(): assert calculate(8,0) == 0\n",
        ),
    ] {
        let root = fixture();
        fs::write(root.path().join("sample/__init__.py"), source).unwrap();
        fs::write(root.path().join("tests/test_sample.py"), tests).unwrap();
        let reports = tempfile::tempdir().unwrap();
        let output = scored_with_timeout(
            root.path(),
            reports.path(),
            &["--operator", "arithmetic_replace"],
            "15",
        );
        assert!(!output.status.success(), "{output:?}");
        let (_, run, outcomes) = saved(&output);
        assert_eq!(run["status"], "failed");
        assert_eq!(outcomes["summary"]["errors"], 1);
        assert_eq!(outcomes["summary"]["killed"], 0);
        assert_eq!(
            fs::read_to_string(root.path().join("sample/__init__.py")).unwrap(),
            source
        );
    }
}

#[test]
#[ignore = "requires MUTIE_PYTHON; bounded continuation and unchanged failure/score policy"]
fn candidate_error_continuation_is_bounded_and_explicit() {
    let root = fixture();
    let reports = tempfile::tempdir().unwrap();
    let source = "def combine(a,b): return a + b\ndef multiply(a,b): return a * b\ndef sum_value(a,b): return a + b\ndef neutral(a,b): return a + b\n";
    let tests = "from sample import combine, multiply, sum_value, neutral\ndef test_string(): assert combine('a','b') == 'ab'\ndef test_number(): assert combine(2,3) == 5\ndef test_zero(): assert multiply(2,0) == 0\ndef test_sum(): assert sum_value(2,3) == 5\ndef test_neutral(): assert neutral(2,0) == 2\n";
    fs::write(root.path().join("sample/__init__.py"), source).unwrap();
    fs::write(root.path().join("tests/test_sample.py"), tests).unwrap();
    let mut complete = None;
    for (limit, completed) in [(None, 1), (Some("1"), 1), (Some("2"), 2), (Some("10"), 4)] {
        let mut args = vec![
            "--operator",
            "arithmetic_replace",
            "--min-score",
            "100",
            "--fail-on-missed",
            "--max-log-bytes",
            "128",
        ];
        if let Some(limit) = limit {
            args.extend(["--continue-on-error", limit]);
        }
        let output = scored_with_timeout(root.path(), reports.path(), &args, "15");
        assert_eq!(output.status.code(), Some(1), "{output:?}");
        let (path, run, outcomes) = saved(&output);
        assert_eq!(run["status"], "failed");
        assert!(run["score"]["percentage"].is_null());
        assert_eq!(
            outcomes["mutations"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|m| m["state"] == "completed")
                .count(),
            completed,
            "{outcomes}"
        );
        assert_eq!(outcomes["summary"]["errors"], completed.min(2));
        if completed == 4 {
            assert_eq!(outcomes["summary"]["killed"], 1);
            assert_eq!(outcomes["summary"]["missed"], 1);
            assert_eq!(run["effective_config"]["continue_on_error"], 10);
            complete = Some(path);
        }
        assert_eq!(
            fs::read_to_string(root.path().join("sample/__init__.py")).unwrap(),
            source
        );
        assert_eq!(
            fs::read_to_string(root.path().join("tests/test_sample.py")).unwrap(),
            tests
        );
    }
    // Reruns must opt in again; policy is not silently inherited from a report.
    let path = complete.unwrap();
    let before = fs::read(path.join("outcomes.json")).unwrap();
    let output = scored_with_timeout(
        root.path(),
        reports.path(),
        &["--rerun", path.to_str().unwrap(), "--id", "1,2"],
        "15",
    );
    let (_, _, outcomes) = saved(&output);
    assert_eq!(outcomes["summary"]["errors"], 1);
    assert_eq!(outcomes["mutations"][1]["state"], "pending");
    assert_eq!(fs::read(path.join("outcomes.json")).unwrap(), before);
    let output = scored_with_timeout(
        root.path(),
        reports.path(),
        &[
            "--rerun",
            path.to_str().unwrap(),
            "--id",
            "3,4",
            "--continue-on-error",
            "10",
        ],
        "15",
    );
    assert!(output.status.success(), "{output:?}");
    let (_, run, outcomes) = saved(&output);
    assert_eq!(run["status"], "completed");
    assert_eq!(outcomes["summary"]["killed"], 1);
    assert_eq!(outcomes["summary"]["missed"], 1);
    assert!(!run["score"]["percentage"].is_null());

    fs::write(
        root.path().join("tests/test_sample.py"),
        format!("{tests}\ndef test_baseline_error(): raise ValueError('pristine')\n"),
    )
    .unwrap();
    let output = scored_with_timeout(
        root.path(),
        reports.path(),
        &["--continue-on-error", "10"],
        "15",
    );
    assert_eq!(output.status.code(), Some(1));
    let (_, run, outcomes) = saved(&output);
    assert_eq!(run["status"], "failed");
    assert_eq!(outcomes["summary"]["errors"], 0);
    assert!(run["score"]["percentage"].is_null());
}

#[test]
#[ignore = "requires MUTIE_PYTHON; lifecycle/resource errors remain fatal under continuation"]
fn candidate_error_continuation_stops_on_unsafe_failures() {
    for (name, body, cleanup) in [
        ("cleanup", "        raise ValueError('body')\n", true),
        (
            "capture",
            "        with capsys.disabled(): print('x' * 65536, flush=True)\n",
            false,
        ),
        (
            "file",
            "        (tmp_path / 'large').write_bytes(b'x' * (5 * 1024 * 1024))\n",
            false,
        ),
    ] {
        let root = fixture();
        let reports = tempfile::tempdir().unwrap();
        let original = fs::read(root.path().join("sample/__init__.py")).unwrap();
        fs::write(root.path().join("tests/test_sample.py"), format!("from sample import enabled\ndef test_body(capsys, tmp_path):\n    if not enabled():\n{body}")).unwrap();
        if cleanup {
            fs::write(root.path().join("tests/conftest.py"), "import pytest\nfrom sample import enabled\n@pytest.fixture(autouse=True)\ndef cleanup():\n yield\n assert enabled()\n").unwrap();
        }
        let output = scored_with_timeout(
            root.path(),
            reports.path(),
            &["--continue-on-error", "10", "--max-capture-bytes", "8192"],
            "15",
        );
        assert_eq!(output.status.code(), Some(1), "{name}: {output:?}");
        let (_, run, outcomes) = saved(&output);
        assert_eq!(run["status"], "failed", "{name}");
        assert_eq!(outcomes["summary"]["errors"], 1, "{name}");
        assert_eq!(
            outcomes["mutations"][1]["state"], "pending",
            "{name}: {outcomes}"
        );
        assert!(run["score"]["percentage"].is_null());
        assert_eq!(
            fs::read(root.path().join("sample/__init__.py")).unwrap(),
            original
        );
    }
}

#[test]
fn candidate_error_continuation_rejects_zero_and_unsupported_runners() {
    let root = tempfile::tempdir().unwrap();
    fs::write(
        root.path().join("go.mod"),
        "module example.com/continuation\n",
    )
    .unwrap();
    for args in [
        vec!["--continue-on-error", "0"],
        vec!["--continue-on-error", "2"],
        vec!["--continue-on-error", "2", "--list"],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_mutie"))
            .arg(root.path())
            .args(args)
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains("continue_on_error"));
        assert!(!root.path().join(".mutie").exists());
    }
}

#[test]
#[ignore = "requires MUTIE_PYTHON; comparison/negation recipes, saved operator identity and stronger reruns"]
fn inversions_preserve_native_semantics_and_rerun() {
    let root = tempfile::tempdir().unwrap();
    for name in [
        "pyproject.toml",
        "sample/__init__.py",
        "tests/test_sample.py",
    ] {
        let target = root.path().join(name);
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::copy(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/python-inversions")
                .join(name),
            target,
        )
        .unwrap();
    }
    let source = fs::read(root.path().join("sample/__init__.py")).unwrap();
    let reports = tempfile::tempdir().unwrap();
    let output = scored_with_timeout(
        root.path(),
        reports.path(),
        &["--operator", "comparison_invert,negation_remove"],
        "15",
    );
    assert!(output.status.success(), "{output:?}");
    let (original, run, outcomes) = saved(&output);
    assert_eq!(
        run["python_scope"]["test_nodes"].as_array().unwrap().len(),
        5
    );
    assert_eq!(
        outcomes["summary"],
        serde_json::json!({"killed":9,"missed":2,"errors":0,"timeout":0,"build_failed":0})
    );
    let inventory: serde_json::Value =
        serde_json::from_slice(&fs::read(original.join("mutants.json")).unwrap()).unwrap();
    assert!(
        inventory["mutations"]
            .as_array()
            .unwrap()
            .iter()
            .all(|m| m["operator"] == "comparison_invert" || m["operator"] == "negation_remove")
    );
    let old_outcomes = fs::read(original.join("outcomes.json")).unwrap();
    fs::write(
        root.path().join("tests/test_stronger.py"),
        "from sample import weak_compare, weak_negate\ndef test_ordered(): assert weak_compare(1, 2) is True\ndef test_value(): assert weak_negate(0) is True\n",
    )
    .unwrap();
    let output = scored_with_timeout(
        root.path(),
        reports.path(),
        &["--rerun-missed", original.to_str().unwrap()],
        "15",
    );
    assert!(output.status.success(), "{output:?}");
    let (_, run, outcomes) = saved(&output);
    assert_eq!(
        run["python_scope"]["test_nodes"].as_array().unwrap().len(),
        7
    );
    assert_eq!(outcomes["summary"]["killed"], 2);
    assert_eq!(outcomes["summary"]["errors"], 0);
    assert_eq!(
        fs::read(root.path().join("sample/__init__.py")).unwrap(),
        source
    );
    assert_eq!(
        fs::read(original.join("outcomes.json")).unwrap(),
        old_outcomes
    );
    // An operator/token mismatch in a saved report must be rejected before execution.
    let mut corrupt = inventory.clone();
    corrupt["mutations"][0]["operator"] = serde_json::json!("boolean_flip");
    fs::write(
        original.join("mutants.json"),
        serde_json::to_vec(&corrupt).unwrap(),
    )
    .unwrap();
    let output = scored_with_timeout(
        root.path(),
        reports.path(),
        &["--rerun", original.to_str().unwrap(), "--id", "1"],
        "15",
    );
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("saved Python mutation"),
        "{output:?}"
    );
    let mut corrupt = inventory.clone();
    let index = corrupt["mutations"]
        .as_array()
        .unwrap()
        .iter()
        .position(|m| m["operator"] == "negation_remove")
        .unwrap();
    corrupt["mutations"][index]["replacement"] = serde_json::json!("(x)");
    fs::write(
        original.join("mutants.json"),
        serde_json::to_vec(&corrupt).unwrap(),
    )
    .unwrap();
    let output = scored_with_timeout(
        root.path(),
        reports.path(),
        &["--rerun", original.to_str().unwrap()],
        "15",
    );
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("saved Python mutation"),
        "{output:?}"
    );
    fs::write(
        original.join("mutants.json"),
        serde_json::to_vec(&inventory).unwrap(),
    )
    .unwrap();
}

#[test]
#[ignore = "requires MUTIE_PYTHON; comparison dispatch and raw negation exceptions stay errors"]
fn inversions_native_exceptions_remain_errors() {
    for (source, tests, operator) in [
        (
            "class Value:\n def __lt__(self, other): return True\n def __ge__(self, other): raise RuntimeError('native comparison')\ndef compare(a,b): return a < b\n",
            "from sample import Value, compare\ndef test_dispatch(): assert compare(Value(), Value()) is True\n",
            "comparison_invert",
        ),
        (
            "def negate(value): return not value\n",
            "from sample import negate\ndef test_raw(): assert negate([]) + 1 == 2\n",
            "negation_remove",
        ),
    ] {
        let root = fixture();
        fs::write(root.path().join("sample/__init__.py"), source).unwrap();
        fs::write(root.path().join("tests/test_sample.py"), tests).unwrap();
        let reports = tempfile::tempdir().unwrap();
        let output =
            scored_with_timeout(root.path(), reports.path(), &["--operator", operator], "15");
        assert!(!output.status.success(), "{output:?}");
        let (_, run, outcomes) = saved(&output);
        assert_eq!(run["status"], "failed");
        assert_eq!(outcomes["summary"]["errors"], 1);
        assert_eq!(outcomes["summary"]["killed"], 0);
        assert_eq!(
            fs::read_to_string(root.path().join("sample/__init__.py")).unwrap(),
            source
        );
    }
}

#[test]
#[ignore = "requires MUTIE_PYTHON; logical swaps, saved operator identity and stronger reruns"]
fn logical_swaps_preserve_native_semantics_and_rerun() {
    let root = tempfile::tempdir().unwrap();
    for name in [
        "pyproject.toml",
        "sample/__init__.py",
        "tests/test_sample.py",
    ] {
        let target = root.path().join(name);
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::copy(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/python-logical")
                .join(name),
            target,
        )
        .unwrap();
    }
    let source = fs::read(root.path().join("sample/__init__.py")).unwrap();
    let reports = tempfile::tempdir().unwrap();
    let output = scored_with_timeout(
        root.path(),
        reports.path(),
        &["--operator", "logical_swap"],
        "15",
    );
    assert!(output.status.success(), "{output:?}");
    let (original, run, outcomes) = saved(&output);
    assert_eq!(
        run["python_scope"]["test_nodes"].as_array().unwrap().len(),
        5
    );
    assert_eq!(
        outcomes["summary"],
        serde_json::json!({"killed":4,"missed":2,"errors":0,"timeout":0,"build_failed":0})
    );
    let inventory: serde_json::Value =
        serde_json::from_slice(&fs::read(original.join("mutants.json")).unwrap()).unwrap();
    assert!(
        inventory["mutations"]
            .as_array()
            .unwrap()
            .iter()
            .all(|m| m["operator"] == "logical_swap")
    );
    let old_outcomes = fs::read(original.join("outcomes.json")).unwrap();
    fs::write(
        root.path().join("tests/test_stronger.py"),
        "from sample import weak_default, weak_both\ndef test_empty_fallback():\n value = object()\n assert weak_default([], value) is value\ndef test_different_values(): assert weak_both(0, 9) == 0\n",
    )
    .unwrap();
    let output = scored_with_timeout(
        root.path(),
        reports.path(),
        &["--rerun-missed", original.to_str().unwrap()],
        "15",
    );
    assert!(output.status.success(), "{output:?}");
    let (_, run, outcomes) = saved(&output);
    assert_eq!(
        run["python_scope"]["test_nodes"].as_array().unwrap().len(),
        7
    );
    assert_eq!(outcomes["summary"]["killed"], 2);
    assert_eq!(outcomes["summary"]["errors"], 0);
    assert_eq!(
        fs::read(root.path().join("sample/__init__.py")).unwrap(),
        source
    );
    assert_eq!(
        fs::read(original.join("outcomes.json")).unwrap(),
        old_outcomes
    );
    // An operator/token mismatch in a saved report must be rejected before execution.
    let mut corrupt = inventory.clone();
    corrupt["mutations"][0]["operator"] = serde_json::json!("boolean_flip");
    fs::write(
        original.join("mutants.json"),
        serde_json::to_vec(&corrupt).unwrap(),
    )
    .unwrap();
    let output = scored_with_timeout(
        root.path(),
        reports.path(),
        &["--rerun", original.to_str().unwrap(), "--id", "1"],
        "15",
    );
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("saved Python mutation"),
        "{output:?}"
    );
    let mut corrupt = inventory.clone();
    corrupt["mutations"][0]["replacement"] = serde_json::json!("||");
    fs::write(
        original.join("mutants.json"),
        serde_json::to_vec(&corrupt).unwrap(),
    )
    .unwrap();
    let output = scored_with_timeout(
        root.path(),
        reports.path(),
        &["--rerun", original.to_str().unwrap()],
        "15",
    );
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("saved Python mutation"),
        "{output:?}"
    );
    fs::write(
        original.join("mutants.json"),
        serde_json::to_vec(&inventory).unwrap(),
    )
    .unwrap();
}

#[test]
#[ignore = "requires MUTIE_PYTHON; newly evaluated logical operand failures remain errors"]
fn logical_swaps_operand_errors_are_not_kills() {
    let root = fixture();
    let source = "def choose(a,b): return a or b()\n";
    fs::write(root.path().join("sample/__init__.py"), source).unwrap();
    fs::write(root.path().join("tests/test_sample.py"), "from sample import choose\ndef fail(): raise ValueError('newly evaluated right operand')\ndef test_short_circuit(): assert choose(7, fail) == 7\n").unwrap();
    let reports = tempfile::tempdir().unwrap();
    let output = scored_with_timeout(
        root.path(),
        reports.path(),
        &["--operator", "logical_swap"],
        "15",
    );
    assert!(!output.status.success(), "{output:?}");
    let (_, run, outcomes) = saved(&output);
    assert_eq!(run["status"], "failed");
    assert_eq!(outcomes["summary"]["errors"], 1);
    assert_eq!(outcomes["summary"]["killed"], 0);
    assert_eq!(
        fs::read_to_string(root.path().join("sample/__init__.py")).unwrap(),
        source
    );
}

#[test]
#[ignore = "requires MUTIE_PYTHON; modulo tokens, saved operator identity and stronger reruns"]
fn modulo_preserves_native_semantics_and_rerun() {
    let root = tempfile::tempdir().unwrap();
    for name in [
        "pyproject.toml",
        "sample/__init__.py",
        "tests/test_sample.py",
    ] {
        let target = root.path().join(name);
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::copy(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/python-modulo")
                .join(name),
            target,
        )
        .unwrap();
    }
    let source = fs::read(root.path().join("sample/__init__.py")).unwrap();
    let reports = tempfile::tempdir().unwrap();
    let output = scored_with_timeout(
        root.path(),
        reports.path(),
        &["--operator", "arithmetic_replace"],
        "15",
    );
    assert!(output.status.success(), "{output:?}");
    let (original, run, outcomes) = saved(&output);
    assert_eq!(
        run["python_scope"]["test_nodes"].as_array().unwrap().len(),
        5
    );
    assert_eq!(
        outcomes["summary"],
        serde_json::json!({"killed":4,"missed":1,"errors":0,"timeout":0,"build_failed":0})
    );
    let inventory: serde_json::Value =
        serde_json::from_slice(&fs::read(original.join("mutants.json")).unwrap()).unwrap();
    assert!(
        inventory["mutations"]
            .as_array()
            .unwrap()
            .iter()
            .all(|m| m["operator"] == "arithmetic_replace")
    );
    let old_outcomes = fs::read(original.join("outcomes.json")).unwrap();
    fs::write(
        root.path().join("tests/test_stronger.py"),
        "from sample import wrap\ndef test_nonzero_wrap(): assert wrap(7, 3) == 1\n",
    )
    .unwrap();
    let output = scored_with_timeout(
        root.path(),
        reports.path(),
        &["--rerun", original.to_str().unwrap(), "--id", "5"],
        "15",
    );
    assert!(output.status.success(), "{output:?}");
    let (_, run, outcomes) = saved(&output);
    assert_eq!(
        run["python_scope"]["test_nodes"].as_array().unwrap().len(),
        6
    );
    assert_eq!(outcomes["summary"]["killed"], 1);
    assert_eq!(outcomes["summary"]["errors"], 0);
    assert_eq!(
        fs::read(root.path().join("sample/__init__.py")).unwrap(),
        source
    );
    assert_eq!(
        fs::read(original.join("outcomes.json")).unwrap(),
        old_outcomes
    );
    // An operator/token mismatch in a saved report must be rejected before execution.
    let mut corrupt = inventory.clone();
    corrupt["mutations"][0]["operator"] = serde_json::json!("boolean_flip");
    fs::write(
        original.join("mutants.json"),
        serde_json::to_vec(&corrupt).unwrap(),
    )
    .unwrap();
    let output = scored_with_timeout(
        root.path(),
        reports.path(),
        &["--rerun", original.to_str().unwrap(), "--id", "1"],
        "15",
    );
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("saved Python mutation"),
        "{output:?}"
    );
    let mut corrupt = inventory.clone();
    corrupt["mutations"][0]["replacement"] = serde_json::json!("/");
    fs::write(
        original.join("mutants.json"),
        serde_json::to_vec(&corrupt).unwrap(),
    )
    .unwrap();
    let output = scored_with_timeout(
        root.path(),
        reports.path(),
        &["--rerun", original.to_str().unwrap(), "--id", "1"],
        "15",
    );
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("saved Python mutation"),
        "{output:?}"
    );
    fs::write(
        original.join("mutants.json"),
        serde_json::to_vec(&inventory).unwrap(),
    )
    .unwrap();
}

#[test]
#[ignore = "requires MUTIE_PYTHON; modulo native exceptions remain errors"]
fn modulo_dispatch_and_formatting_errors_are_not_kills() {
    for (source, tests) in [
        (
            "class Value:\n def __mod__(self, other): return 8\n def __mul__(self, other): raise RuntimeError('native dispatch')\ndef calculate(a,b): return a % b\n",
            "from sample import Value, calculate\ndef test_dispatch(): assert calculate(Value(),Value()) == 8\n",
        ),
        (
            "def calculate(a,b): return a % b\n",
            "from sample import calculate\ndef test_formatting(): assert calculate('%s', 'value') == 'value'\n",
        ),
    ] {
        let root = fixture();
        fs::write(root.path().join("sample/__init__.py"), source).unwrap();
        fs::write(root.path().join("tests/test_sample.py"), tests).unwrap();
        let reports = tempfile::tempdir().unwrap();
        let output = scored_with_timeout(
            root.path(),
            reports.path(),
            &["--operator", "arithmetic_replace"],
            "15",
        );
        assert!(!output.status.success(), "{output:?}");
        let (_, run, outcomes) = saved(&output);
        assert_eq!(run["status"], "failed");
        assert_eq!(outcomes["summary"]["errors"], 1);
        assert_eq!(outcomes["summary"]["killed"], 0);
        assert_eq!(
            fs::read_to_string(root.path().join("sample/__init__.py")).unwrap(),
            source
        );
    }
}

#[test]
#[ignore = "requires MUTIE_PYTHON; branches, saved operator identity and stronger reruns"]
fn branches_preserve_native_semantics_and_rerun() {
    let root = tempfile::tempdir().unwrap();
    for name in [
        "pyproject.toml",
        "sample/__init__.py",
        "tests/test_sample.py",
    ] {
        let target = root.path().join(name);
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::copy(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/python-branches")
                .join(name),
            target,
        )
        .unwrap();
    }
    let source = fs::read(root.path().join("sample/__init__.py")).unwrap();
    let reports = tempfile::tempdir().unwrap();
    let output = scored_with_timeout(
        root.path(),
        reports.path(),
        &["--operator", "branch_force_true,branch_force_false"],
        "15",
    );
    assert!(output.status.success(), "{output:?}");
    let (original, run, outcomes) = saved(&output);
    assert_eq!(
        run["python_scope"]["test_nodes"].as_array().unwrap().len(),
        3
    );
    assert_eq!(
        outcomes["summary"],
        serde_json::json!({"killed":3,"missed":1,"errors":0,"timeout":0,"build_failed":0})
    );
    let inventory: serde_json::Value =
        serde_json::from_slice(&fs::read(original.join("mutants.json")).unwrap()).unwrap();
    assert!(
        inventory["mutations"]
            .as_array()
            .unwrap()
            .iter()
            .all(|m| matches!(
                m["operator"].as_str(),
                Some("branch_force_true" | "branch_force_false")
            ))
    );
    let old_outcomes = fs::read(original.join("outcomes.json")).unwrap();
    fs::write(
        root.path().join("tests/test_stronger.py"),
        "from sample import weak\ndef test_empty(): assert weak([]) == 4\n",
    )
    .unwrap();
    let output = scored_with_timeout(
        root.path(),
        reports.path(),
        &["--rerun", original.to_str().unwrap(), "--id", "3"],
        "15",
    );
    assert!(output.status.success(), "{output:?}");
    let (_, run, outcomes) = saved(&output);
    assert_eq!(
        run["python_scope"]["test_nodes"].as_array().unwrap().len(),
        4
    );
    assert_eq!(outcomes["summary"]["killed"], 1);
    assert_eq!(outcomes["summary"]["errors"], 0);
    assert_eq!(
        fs::read(root.path().join("sample/__init__.py")).unwrap(),
        source
    );
    assert_eq!(
        fs::read(original.join("outcomes.json")).unwrap(),
        old_outcomes
    );
    // An operator/token mismatch in a saved report must be rejected before execution.
    let mut corrupt = inventory.clone();
    corrupt["mutations"][0]["operator"] = serde_json::json!("boolean_flip");
    fs::write(
        original.join("mutants.json"),
        serde_json::to_vec(&corrupt).unwrap(),
    )
    .unwrap();
    let output = scored_with_timeout(
        root.path(),
        reports.path(),
        &["--rerun", original.to_str().unwrap(), "--id", "1"],
        "15",
    );
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("saved Python mutation"),
        "{output:?}"
    );
    let mut corrupt = inventory.clone();
    corrupt["mutations"][0]["replacement"] = serde_json::json!("/");
    fs::write(
        original.join("mutants.json"),
        serde_json::to_vec(&corrupt).unwrap(),
    )
    .unwrap();
    let output = scored_with_timeout(
        root.path(),
        reports.path(),
        &["--rerun", original.to_str().unwrap(), "--id", "1"],
        "15",
    );
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("saved Python mutation"),
        "{output:?}"
    );
    fs::write(
        original.join("mutants.json"),
        serde_json::to_vec(&inventory).unwrap(),
    )
    .unwrap();
}

#[test]
#[ignore = "requires MUTIE_PYTHON; forced-branch runtime failures remain errors"]
fn branch_runtime_errors_are_not_kills() {
    let root = fixture();
    let source = "def choose(value):\n if value: return value.missing\n return 2\n";
    fs::write(root.path().join("sample/__init__.py"), source).unwrap();
    fs::write(
        root.path().join("tests/test_sample.py"),
        "from sample import choose\ndef test_empty(): assert choose(None) == 2\n",
    )
    .unwrap();
    let reports = tempfile::tempdir().unwrap();
    let output = scored_with_timeout(
        root.path(),
        reports.path(),
        &["--operator", "branch_force_true"],
        "15",
    );
    assert!(!output.status.success(), "{output:?}");
    let (_, run, outcomes) = saved(&output);
    assert_eq!(run["status"], "failed");
    assert_eq!(outcomes["summary"]["errors"], 1);
    assert_eq!(outcomes["summary"]["killed"], 0);
    assert_eq!(
        fs::read_to_string(root.path().join("sample/__init__.py")).unwrap(),
        source
    );
}

#[test]
#[ignore = "requires MUTIE_PYTHON; branches, saved operator identity and stronger reruns"]
fn loops_preserve_native_semantics_and_rerun() {
    let root = tempfile::tempdir().unwrap();
    for name in [
        "pyproject.toml",
        "sample/__init__.py",
        "tests/test_sample.py",
    ] {
        let target = root.path().join(name);
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::copy(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/python-loops")
                .join(name),
            target,
        )
        .unwrap();
    }
    let source = fs::read(root.path().join("sample/__init__.py")).unwrap();
    let reports = tempfile::tempdir().unwrap();
    let output = scored_with_timeout(
        root.path(),
        reports.path(),
        &["--operator", "loop_skip"],
        "15",
    );
    assert!(output.status.success(), "{output:?}");
    let (original, run, outcomes) = saved(&output);
    assert_eq!(
        run["python_scope"]["test_nodes"].as_array().unwrap().len(),
        2
    );
    assert_eq!(
        outcomes["summary"],
        serde_json::json!({"killed":1,"missed":1,"errors":0,"timeout":0,"build_failed":0})
    );
    let inventory: serde_json::Value =
        serde_json::from_slice(&fs::read(original.join("mutants.json")).unwrap()).unwrap();
    assert!(
        inventory["mutations"]
            .as_array()
            .unwrap()
            .iter()
            .all(|m| matches!(m["operator"].as_str(), Some("loop_skip")))
    );
    let old_outcomes = fs::read(original.join("outcomes.json")).unwrap();
    fs::write(
        root.path().join("tests/test_stronger.py"),
        "from sample import weak\ndef test_empty(): assert weak(2) == 2\n",
    )
    .unwrap();
    let output = scored_with_timeout(
        root.path(),
        reports.path(),
        &["--rerun", original.to_str().unwrap(), "--id", "2"],
        "15",
    );
    assert!(output.status.success(), "{output:?}");
    let (_, run, outcomes) = saved(&output);
    assert_eq!(
        run["python_scope"]["test_nodes"].as_array().unwrap().len(),
        3
    );
    assert_eq!(outcomes["summary"]["killed"], 1);
    assert_eq!(outcomes["summary"]["errors"], 0);
    assert_eq!(
        fs::read(root.path().join("sample/__init__.py")).unwrap(),
        source
    );
    assert_eq!(
        fs::read(original.join("outcomes.json")).unwrap(),
        old_outcomes
    );
    // An operator/token mismatch in a saved report must be rejected before execution.
    let mut corrupt = inventory.clone();
    corrupt["mutations"][0]["operator"] = serde_json::json!("boolean_flip");
    fs::write(
        original.join("mutants.json"),
        serde_json::to_vec(&corrupt).unwrap(),
    )
    .unwrap();
    let output = scored_with_timeout(
        root.path(),
        reports.path(),
        &["--rerun", original.to_str().unwrap(), "--id", "1"],
        "15",
    );
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("saved Python mutation"),
        "{output:?}"
    );
    let mut corrupt = inventory.clone();
    corrupt["mutations"][0]["replacement"] = serde_json::json!("/");
    fs::write(
        original.join("mutants.json"),
        serde_json::to_vec(&corrupt).unwrap(),
    )
    .unwrap();
    let output = scored_with_timeout(
        root.path(),
        reports.path(),
        &["--rerun", original.to_str().unwrap(), "--id", "1"],
        "15",
    );
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("saved Python mutation"),
        "{output:?}"
    );
    // Same bytes as a false branch, but fresh AST/operator identity must reject it.
    let mut corrupt = inventory.clone();
    corrupt["mutations"][0]["operator"] = serde_json::json!("branch_force_false");
    fs::write(
        original.join("mutants.json"),
        serde_json::to_vec(&corrupt).unwrap(),
    )
    .unwrap();
    let output = scored_with_timeout(
        root.path(),
        reports.path(),
        &["--rerun", original.to_str().unwrap(), "--id", "1"],
        "15",
    );
    assert!(
        !output.status.success(),
        "loop mislabeled as branch must fail: {output:?}"
    );
    fs::write(
        original.join("mutants.json"),
        serde_json::to_vec(&inventory).unwrap(),
    )
    .unwrap();
}

#[test]
#[ignore = "requires MUTIE_PYTHON; skipped-loop runtime failures remain errors"]
fn loop_runtime_errors_are_not_kills() {
    let root = fixture();
    let source = "def choose(value):\n while value:\n  result = 2\n  break\n return result\n";
    fs::write(root.path().join("sample/__init__.py"), source).unwrap();
    fs::write(
        root.path().join("tests/test_sample.py"),
        "from sample import choose\ndef test_empty(): assert choose(1) == 2\n",
    )
    .unwrap();
    let reports = tempfile::tempdir().unwrap();
    let output = scored_with_timeout(
        root.path(),
        reports.path(),
        &["--operator", "loop_skip"],
        "15",
    );
    assert!(!output.status.success(), "{output:?}");
    let (_, run, outcomes) = saved(&output);
    assert_eq!(run["status"], "failed");
    assert_eq!(outcomes["summary"]["errors"], 1);
    assert_eq!(outcomes["summary"]["killed"], 0);
    assert_eq!(
        fs::read_to_string(root.path().join("sample/__init__.py")).unwrap(),
        source
    );
}
