use super::{Command, fs, saved};
use std::path::Path;

fn run(root: &Path, reports: &Path, extra: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_mutie"))
        .arg(root)
        .args([
            "--lang",
            "python",
            "--python",
            &std::env::var("MUTIE_PYTHON").unwrap(),
            "--ui",
            "plain",
            "--timeout",
            "20",
            "--baseline-timeout",
            "30",
            "--output-dir",
        ])
        .arg(reports)
        .args(extra)
        .env("PYTHONPATH", "/missing-inherited-python-path")
        .env("PYTHONHOME", "/missing-inherited-python-home")
        .output()
        .unwrap()
}

#[test]
#[ignore = "requires MUTIE_PYTHON with audited flit-core 3.12.0"]
fn installed_cli_scores_reruns_and_preserves_selection() {
    let root = super::fixture();
    let reports = tempfile::tempdir().unwrap();
    fs::write(
        root.path().join("pyproject.toml"),
        r#"[project]
name="sample"
version="1.0"
description="Installed fixture"
[build-system]
requires=["flit_core>=3.11,<4"]
build-backend="flit_core.buildapi"
[tool.flit.module]
name="sample"
[tool.pytest.ini_options]
testpaths=["tests"]
markers=["stress: opt-in stress cases"]
addopts="-m 'not stress'"
filterwarnings=["error"]
"#,
    )
    .unwrap();
    let source = fs::read(root.path().join("sample/__init__.py")).unwrap();
    fs::write(root.path().join("tests/test_sample.py"), r#"import importlib.metadata
import subprocess
import sys
import threading
import pytest
from sample import enabled, optional
def test_enabled(): assert enabled()
def test_optional(): assert callable(optional)
def test_installed_metadata():
    assert importlib.metadata.version('sample') == '1.0'
    result = subprocess.run([sys.executable, '-c', 'import os; from sample import enabled; assert enabled(); assert "PYTHONPATH" not in os.environ; assert "PYTHONHOME" not in os.environ'], capture_output=True)
    assert result.returncode == 0, result.stderr
def test_joined_thread(tmp_path, monkeypatch):
    monkeypatch.chdir(tmp_path)
    values=[]
    thread=threading.Thread(target=lambda: values.append(1))
    thread.start()
    thread.join()
    assert values == [1]
@pytest.mark.skipif(sys.platform != 'not-a-platform', reason='platform')
def test_skipped(): assert False
@pytest.mark.xfail
def test_expected(): raise RuntimeError('known failure')
@pytest.mark.stress
def test_stress(): assert False
"#).unwrap();
    let initial = run(root.path(), reports.path(), &[]);
    assert!(initial.status.success(), "{initial:?}");
    let (original, report, outcomes) = saved(&initial);
    assert_eq!(report["score"]["killed"], 1);
    assert_eq!(report["score"]["missed"], 1);
    assert_eq!(
        report["python_scope"]["selection"]["deselected"]["count"],
        1
    );
    assert_eq!(
        report["python_scope"]["selection"]["dispositions"]
            .as_object()
            .unwrap()
            .len(),
        2
    );
    let original_report = fs::read(original.join("outcomes.json")).unwrap();
    fs::write(
        root.path().join("tests/test_stronger.py"),
        "from sample import optional\ndef test_stronger(): assert optional() is False\n",
    )
    .unwrap();
    let rerun = run(
        root.path(),
        reports.path(),
        &["--rerun-missed", original.to_str().unwrap()],
    );
    assert!(rerun.status.success(), "{rerun:?}");
    let (_, stronger, _) = saved(&rerun);
    assert_eq!(stronger["score"]["killed"], 1);
    assert_eq!(stronger["score"]["missed"], 0);
    assert_eq!(
        fs::read(original.join("outcomes.json")).unwrap(),
        original_report
    );
    assert_eq!(
        fs::read(root.path().join("sample/__init__.py")).unwrap(),
        source
    );
    assert_eq!(outcomes["summary"]["killed"], 1);
    // A new mutation-dependent skip must remain an error even when another
    // body supplies a valid assertion failure for the same candidate.
    fs::write(root.path().join("tests/test_stronger.py"), "import pytest\nfrom sample import optional\n@pytest.mark.skipif(optional(), reason='candidate drift')\ndef test_selection(): assert True\ndef test_stronger(): assert optional() is False\n").unwrap();
    let drift = run(
        root.path(),
        reports.path(),
        &["--rerun-missed", original.to_str().unwrap()],
    );
    assert!(!drift.status.success(), "{drift:?}");
    let (drift_directory, _, drift_outcomes) = saved(&drift);
    assert_eq!(drift_outcomes["summary"]["errors"], 1);
    let logs = fs::read_to_string(drift_directory.join("summary.txt")).unwrap();
    assert!(logs.contains("error"), "{logs}");
}
