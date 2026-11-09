use super::*;

#[test]
#[ignore = "requires MUTIE_PYTHON; assignment removal, review, identity and stronger-test reruns"]
fn assignments_preserve_scores_identity_and_reruns() {
    let root = fixture();
    for name in [
        "pyproject.toml",
        "sample/__init__.py",
        "tests/test_sample.py",
    ] {
        fs::copy(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/python-assignments")
                .join(name),
            root.path().join(name),
        )
        .unwrap();
    }
    let source_path = root.path().join("sample/__init__.py");
    let source = fs::read(&source_path).unwrap();
    // Listing rechecks the native document against its Rust serialization.
    // Ordinary branches must omit the optional false loop marker in both.
    let listing = run(
        "python-list",
        root.path(),
        &["--operator", "assignment_remove"],
    );
    assert!(listing.status.success(), "{listing:?}");
    let listed: serde_json::Value = serde_json::from_slice(&listing.stdout).unwrap();
    assert_eq!(listed["mutations"].as_array().unwrap().len(), 2);
    let reports = tempfile::tempdir().unwrap();
    let output = scored_with_timeout(
        root.path(),
        reports.path(),
        &["--operator", "assignment_remove", "--fail-on-missed"],
        "15",
    );
    assert_eq!(output.status.code(), Some(3), "{output:?}");
    let (directory, run, outcomes) = saved(&output);
    assert_eq!(run["score"]["percentage"], 50.0);
    assert_eq!(
        outcomes["summary"],
        serde_json::json!({"killed":1,"missed":1,"errors":0,"timeout":0,"build_failed":0})
    );
    let inventory_path = directory.join("mutants.json");
    let original_inventory = fs::read(&inventory_path).unwrap();
    let inventory: serde_json::Value = serde_json::from_slice(&original_inventory).unwrap();
    assert!(
        inventory["mutations"]
            .as_array()
            .unwrap()
            .iter()
            .all(|m| m["operator"] == "assignment_remove" && m["replacement"] == "pass")
    );
    let original_run = fs::read(directory.join("run.json")).unwrap();
    let original_outcomes = fs::read(directory.join("outcomes.json")).unwrap();
    let rerun = |args: &[&str]| scored_with_timeout(root.path(), reports.path(), args, "15");
    for (key, value) in [("operator", "boolean_flip"), ("replacement", "None")] {
        let mut corrupt = inventory.clone();
        corrupt["mutations"][0][key] = value.into();
        fs::write(&inventory_path, serde_json::to_vec(&corrupt).unwrap()).unwrap();
        let output = rerun(&["--rerun", directory.to_str().unwrap(), "--id", "1"]);
        assert!(!output.status.success(), "{output:?}");
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("saved Python mutation"),
            "{output:?}"
        );
    }
    // A real local-binding span with the correct file fingerprint is still not an
    // admitted assignment mutation. The reader recognizes recipes; fresh AST discovery
    // must reject forged syntax before any candidate executes.
    let mut corrupt = inventory.clone();
    let m = &mut corrupt["mutations"][0];
    let text = std::str::from_utf8(&source).unwrap();
    let start = text.find("values = {}").unwrap();
    m["original"] = "values = {}".into();
    m["byte_start"] = start.into();
    m["byte_end"] = (start + "values = {}".len()).into();
    m["line"] = 2.into();
    m["column"] = 5.into();
    m["end_line"] = 2.into();
    m["end_column"] = 16.into();
    fs::write(&inventory_path, serde_json::to_vec(&corrupt).unwrap()).unwrap();
    let output = rerun(&["--rerun", directory.to_str().unwrap(), "--id", "1"]);
    assert!(!output.status.success(), "{output:?}");
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("no longer matches a discovered mutation"),
        "{output:?}"
    );
    fs::write(&inventory_path, &original_inventory).unwrap();
    // A store mislabeled as a call has the same wire recipe but must fail
    // operator matching during fresh AST discovery.
    let mut wrong_family = inventory.clone();
    wrong_family["mutations"][0]["operator"] = "call_remove".into();
    fs::write(&inventory_path, serde_json::to_vec(&wrong_family).unwrap()).unwrap();
    let output = rerun(&["--rerun", directory.to_str().unwrap(), "--id", "1"]);
    assert!(!output.status.success(), "{output:?}");
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("no longer matches a discovered mutation"),
        "{output:?}"
    );
    fs::write(&inventory_path, &original_inventory).unwrap();
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
            "fixture gap",
        ],
        vec!["check"],
    ] {
        let output = dismiss(&args);
        assert!(output.status.success(), "{output:?}");
    }
    let output = rerun(&[
        "--operator",
        "assignment_remove",
        "--fail-on-missed",
        "--min-score",
        "100",
    ]);
    assert_eq!(output.status.code(), Some(3), "{output:?}");
    let (_, reviewed, reviewed_outcomes) = saved(&output);
    assert_eq!(reviewed["policy"]["dismissed_missed"], 1);
    assert_eq!(reviewed["policy"]["status"], "passed");
    assert_eq!(reviewed["score_policy"]["status"], "failed");
    assert_eq!(reviewed_outcomes["summary"], outcomes["summary"]);
    let output = dismiss(&["remove", "1"]);
    assert!(output.status.success(), "{output:?}");
    fs::write(
        root.path().join("tests/test_stronger.py"),
        "from sample import weak\ndef test_included(): assert weak(True) == {'count': 2}\n",
    )
    .unwrap();
    let output = rerun(&[
        "--rerun-missed",
        directory.to_str().unwrap(),
        "--min-score",
        "100",
    ]);
    assert!(output.status.success(), "{output:?}");
    let (_, stronger, results) = saved(&output);
    assert_eq!(
        stronger["python_scope"]["test_nodes"]
            .as_array()
            .unwrap()
            .len(),
        3
    );
    assert_eq!(
        results["summary"],
        serde_json::json!({"killed":1,"missed":0,"errors":0,"timeout":0,"build_failed":0})
    );
    assert_eq!(fs::read(&source_path).unwrap(), source);
    fs::write(&source_path, text.to_owned() + "\n# source drift\n").unwrap();
    let output = rerun(&["--rerun-missed", directory.to_str().unwrap()]);
    assert!(!output.status.success(), "{output:?}");
    fs::write(&source_path, &source).unwrap();
    assert_eq!(fs::read(&inventory_path).unwrap(), original_inventory);
    assert_eq!(fs::read(directory.join("run.json")).unwrap(), original_run);
    assert_eq!(
        fs::read(directory.join("outcomes.json")).unwrap(),
        original_outcomes
    );
}

#[test]
#[ignore = "requires MUTIE_PYTHON; removed initialization and cleanup failures remain errors"]
fn assignments_runtime_and_lifecycle_errors_are_not_kills() {
    for lifecycle in [false, true] {
        let root = fixture();
        let source = if lifecycle {
            "state = {'ready': 0}\ndef prepare():\n state['ready'] = 1\n"
        } else {
            "class Value:\n def __init__(self):\n  self.prepare()\n def prepare(self):\n  self.value = 1\n"
        };
        fs::write(root.path().join("sample/__init__.py"), source).unwrap();
        fs::write(
            root.path().join("tests/test_sample.py"),
            if lifecycle {
                "from sample import state\ndef test_state(): assert state == {'ready': 1}\n"
            } else {
                "from sample import Value\ndef test_value(): assert Value().value == 1\n"
            },
        )
        .unwrap();
        if lifecycle {
            fs::write(root.path().join("conftest.py"), "import pytest\nfrom sample import prepare, state\n@pytest.fixture(autouse=True)\ndef prepared():\n prepare()\n yield\n assert state == {'ready': 1}\n").unwrap();
        }
        let reports = tempfile::tempdir().unwrap();
        let output = scored_with_timeout(
            root.path(),
            reports.path(),
            &["--operator", "assignment_remove"],
            "15",
        );
        assert!(!output.status.success(), "{output:?}");
        let (_, run, outcomes) = saved(&output);
        assert_eq!(run["status"], "failed");
        assert_eq!(outcomes["summary"]["errors"], 1, "{outcomes}");
        assert_eq!(outcomes["summary"]["killed"], 0);
        assert_eq!(
            fs::read_to_string(root.path().join("sample/__init__.py")).unwrap(),
            source
        );
    }
}
