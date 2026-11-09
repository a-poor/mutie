use super::*;
use serde_json::{Value, json};

#[test]
#[ignore = "requires MUTIE_PYTHON; native numeric edges and independent AST/Rust recipes"]
fn numbers_native_semantics_and_discovery() {
    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/support/python_numbers.py");
    let native = Command::new(python())
        .args(["-I", "-S", "-B"])
        .arg(&script)
        .output()
        .unwrap();
    assert!(native.status.success(), "{native:?}");
    let source = Command::new(python())
        .args(["-I", "-S", "-B"])
        .arg(script)
        .arg("--source")
        .output()
        .unwrap();
    assert!(source.status.success(), "{source:?}");
    let root = fixture();
    fs::write(root.path().join("sample/__init__.py"), &source.stdout).unwrap();
    fs::write(
        root.path().join("tests/test_sample.py"),
        "def test_value(): assert True\n",
    )
    .unwrap();
    let listing = run(
        "python-list",
        root.path(),
        &["--operator", "number_replace"],
    );
    assert!(listing.status.success(), "{listing:?}");
    let listed: Value = serde_json::from_slice(&listing.stdout).unwrap();
    assert_eq!(listed["mutations"].as_array().unwrap().len(), 33);
}

#[test]
#[ignore = "requires MUTIE_PYTHON; numeric scores, review, identity and stronger-test reruns"]
fn numbers_preserve_scores_identity_and_reruns() {
    let root = fixture();
    let source = "def checked(): return 7\ndef weak(): return 0.0\nvalue: list[99]\n";
    let source_path = root.path().join("sample/__init__.py");
    // Future annotations keep the excluded annotation inert at runtime.
    let source = format!("from __future__ import annotations\n{source}");
    fs::write(&source_path, &source).unwrap();
    fs::write(root.path().join("tests/test_sample.py"), "from sample import checked, weak\ndef test_values():\n assert checked() == 7\n assert type(weak()) is float\n").unwrap();
    let reports = tempfile::tempdir().unwrap();
    let output = scored_with_timeout(
        root.path(),
        reports.path(),
        &["--operator", "number_replace", "--min-score", "100"],
        "15",
    );
    assert_eq!(output.status.code(), Some(3), "{output:?}");
    let (directory, original_run, outcomes) = saved(&output);
    assert_eq!(original_run["score"]["percentage"], 50.0);
    assert_eq!(
        outcomes["summary"],
        json!({"killed":1,"missed":1,"errors":0,"timeout":0,"build_failed":0})
    );
    let bytes: [Vec<u8>; 3] =
        ["run.json", "mutants.json", "outcomes.json"].map(|n| fs::read(directory.join(n)).unwrap());
    let inventory: Value = serde_json::from_slice(&bytes[1]).unwrap();
    assert_eq!(inventory["mutations"].as_array().unwrap().len(), 2);
    assert_eq!(inventory["mutations"][1]["replacement"], "(1.0)");
    let rerun = |args: &[&str]| scored_with_timeout(root.path(), reports.path(), args, "15");
    for (key, value) in [
        ("replacement", "(0.0)"),
        ("replacement", "(1)"),
        ("operator", "boolean_flip"),
    ] {
        let mut corrupt = inventory.clone();
        corrupt["mutations"][1][key] = value.into();
        fs::write(
            directory.join("mutants.json"),
            serde_json::to_vec(&corrupt).unwrap(),
        )
        .unwrap();
        let out = rerun(&["--rerun-missed", directory.to_str().unwrap(), "--dry-run"]);
        assert!(
            String::from_utf8_lossy(&out.stderr).contains("saved Python mutation"),
            "{out:?}"
        );
    }
    let start = source.find("99").unwrap();
    let prefix = &source[..start];
    let line = prefix.bytes().filter(|b| *b == b'\n').count() + 1;
    let column = prefix.rsplit('\n').next().unwrap().len() + 1;
    let mut corrupt = inventory.clone();
    let m = &mut corrupt["mutations"][1];
    m["original"] = "99".into();
    m["replacement"] = "(0)".into();
    m["byte_start"] = start.into();
    m["byte_end"] = (start + 2).into();
    m["line"] = line.into();
    m["end_line"] = line.into();
    m["column"] = column.into();
    m["end_column"] = (column + 2).into();
    fs::write(
        directory.join("mutants.json"),
        serde_json::to_vec(&corrupt).unwrap(),
    )
    .unwrap();
    let out = rerun(&["--rerun-missed", directory.to_str().unwrap(), "--dry-run"]);
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("no longer matches a discovered mutation"),
        "{out:?}"
    );
    fs::write(directory.join("mutants.json"), &bytes[1]).unwrap();
    let dismiss = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_mutie"))
            .args(["dismiss", "--python", &python(), "--module"])
            .arg(root.path())
            .args(["--lang", "python"])
            .args(args)
            .output()
            .unwrap()
    };
    let out = dismiss(&[
        "add",
        directory.to_str().unwrap(),
        "--id",
        "2",
        "--reason",
        "pending value assertion",
    ]);
    assert!(out.status.success(), "{out:?}");
    let out = rerun(&[
        "--operator",
        "number_replace",
        "--fail-on-missed",
        "--min-score",
        "100",
    ]);
    assert_eq!(out.status.code(), Some(3), "{out:?}");
    let (_, reviewed, result) = saved(&out);
    assert_eq!(reviewed["policy"]["status"], "passed");
    assert_eq!(reviewed["score_policy"]["status"], "failed");
    assert_eq!(reviewed["score"]["percentage"], 50.0);
    assert_eq!(result["summary"], outcomes["summary"]);
    fs::write(
        root.path().join("tests/test_stronger.py"),
        "from sample import weak\ndef test_value(): assert weak() == 0.0\n",
    )
    .unwrap();
    let out = rerun(&["--rerun-missed", directory.to_str().unwrap()]);
    assert!(out.status.success(), "{out:?}");
    let (focused, _, result) = saved(&out);
    assert_eq!(result["summary"]["killed"], 1);
    let focused: Value =
        serde_json::from_slice(&fs::read(focused.join("mutants.json")).unwrap()).unwrap();
    assert_eq!(focused["dismissals"]["matched"][0]["mutation_id"], 1);
    fs::write(&source_path, format!("{source}# drift\n")).unwrap();
    let out = rerun(&["--rerun-missed", directory.to_str().unwrap(), "--dry-run"]);
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("source fingerprint changed"),
        "{out:?}"
    );
    fs::write(&source_path, &source).unwrap();
    for (name, bytes) in ["run.json", "mutants.json", "outcomes.json"]
        .iter()
        .zip(bytes)
    {
        assert_eq!(fs::read(directory.join(name)).unwrap(), bytes);
    }
}

#[test]
#[ignore = "requires MUTIE_PYTHON; numeric runtime/setup errors and bounded timeout"]
fn numbers_runtime_lifecycle_errors_and_timeouts_are_not_kills() {
    for mode in ["runtime", "setup", "timeout"] {
        let root = fixture();
        let source = "def chosen(): return 1\n";
        fs::write(root.path().join("sample/__init__.py"), source).unwrap();
        let tests = match mode {
            "runtime" => "from sample import chosen\ndef test_value(): assert 2 / chosen() == 2\n",
            "setup" => {
                "import pytest\nfrom sample import chosen\n@pytest.fixture(autouse=True)\ndef setup(): assert chosen() == 1\ndef test_value(): assert chosen() == 1\n"
            }
            _ => "from sample import chosen\ndef test_value():\n while chosen() == 0: pass\n",
        };
        fs::write(root.path().join("tests/test_sample.py"), tests).unwrap();
        let reports = tempfile::tempdir().unwrap();
        let out = scored_with_timeout(
            root.path(),
            reports.path(),
            &["--operator", "number_replace", "--min-score", "100"],
            "3",
        );
        let (_, run, result) = saved(&out);
        assert_eq!(result["summary"]["killed"], 0, "{out:?}");
        assert!(run["score"]["percentage"].is_null(), "{out:?}");
        if mode == "timeout" {
            assert_eq!(result["summary"]["timeout"], 1, "{out:?}");
            assert_eq!(run["score"]["unavailable_reason"], "no_scored_mutations");
        } else {
            assert_eq!(out.status.code(), Some(1), "{out:?}");
            assert_eq!(result["summary"]["errors"], 1, "{out:?}");
        }
        assert_eq!(
            fs::read_to_string(root.path().join("sample/__init__.py")).unwrap(),
            source
        );
    }
}
