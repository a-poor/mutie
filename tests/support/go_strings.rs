use super::*;
use serde_json::Value;
use std::collections::BTreeMap;

fn files(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn walk(root: &Path, dir: &Path, result: &mut BTreeMap<PathBuf, Vec<u8>>) {
        for entry in fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                walk(root, &path, result);
            } else {
                result.insert(
                    path.strip_prefix(root).unwrap().to_owned(),
                    fs::read(&path).unwrap(),
                );
            }
        }
    }
    let mut result = BTreeMap::new();
    walk(root, root, &mut result);
    result
}

#[test]
#[ignore = "requires Go 1.24+; scored strings, full/affected selection, build failures, review and reruns"]
fn strings_scored_workflow() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("project");
    copy_tree(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/go-strings"),
        &root,
    );
    let source = fs::read_to_string(root.join("strings.go")).unwrap();
    let listing = run(&root, &["--list", "--operator", "string_replace"]);
    assert!(stdout(&listing).contains("Found 4 mutations"));
    let original_inputs = files(&root);
    let reports = temp.path().join("reports");
    let output = run(
        &root,
        &[
            "--operator",
            "string_replace",
            "--jobs",
            "2",
            "--timeout",
            "20",
            "--min-score",
            "100",
            "--output-dir",
            reports.to_str().unwrap(),
        ],
    );
    assert_eq!(output.status.code(), Some(3), "{output:?}");
    let (directory, saved, inventory, outcomes) = saved_report(&output);
    assert_eq!(saved["status"], "completed");
    assert_eq!(saved["score"]["percentage"], 50.0);
    assert_eq!(
        outcomes["summary"],
        serde_json::json!({"killed":1,"missed":1,"build_failed":2,"errors":0,"timeout":0})
    );
    assert_eq!(inventory["mutations"].as_array().unwrap().len(), 4);
    for m in inventory["mutations"].as_array().unwrap() {
        assert_eq!(m["operator"], "string_replace");
    }
    assert_eq!(inventory["mutations"][0]["original"], "\"ready\"");
    assert_eq!(inventory["mutations"][1]["replacement"], "\"x\"");
    let original_reports = files(&directory);
    let affected = run(
        &root,
        &[
            "--operator",
            "string_replace",
            "--test-scope",
            "affected",
            "--timeout",
            "20",
            "--output-dir",
            reports.to_str().unwrap(),
        ],
    );
    stdout(&affected);
    let (affected_dir, affected_run, affected_inventory, affected_outcomes) =
        saved_report(&affected);
    assert_eq!(affected_outcomes["summary"], outcomes["summary"]);
    assert_eq!(affected_run["score"]["percentage"], 50.0);
    let affected_reports = files(&affected_dir);
    for m in affected_inventory["mutations"].as_array().unwrap() {
        assert_eq!(
            m["test_packages"],
            serde_json::json!([
                "example.com/mutie-go-strings",
                "example.com/mutie-go-strings/consumer"
            ])
        );
    }
    stdout(&dismiss(
        &root,
        &[
            "add",
            directory.to_str().unwrap(),
            "--id",
            "2",
            "--reason",
            "pending stronger test",
        ],
    ));
    let reviewed = run(
        &root,
        &[
            "--operator",
            "string_replace",
            "--fail-on-missed",
            "--timeout",
            "20",
            "--output-dir",
            reports.to_str().unwrap(),
        ],
    );
    stdout(&reviewed);
    let (review_dir, reviewed_run, _, reviewed_results) = saved_report(&reviewed);
    assert_eq!(reviewed_run["score"]["percentage"], 50.0);
    assert_eq!(reviewed_run["score"]["dismissed_in_score"], 1);
    assert_eq!(reviewed_results["summary"], outcomes["summary"]);
    let reviewed_reports = files(&review_dir);
    let stronger = temp.path().join("stronger");
    copy_tree(&root, &stronger);
    fs::write(stronger.join("stronger_test.go"), "package literals\nimport \"testing\"\nfunc TestEmptyDefault(t *testing.T) { if Weak() != \"\" { t.Fatal(Weak()) } }\n").unwrap();
    let stronger_inputs = files(&stronger);
    let rerun = run(
        &stronger,
        &[
            "--rerun-missed",
            directory.to_str().unwrap(),
            "--test-scope",
            "affected",
            "--timeout",
            "20",
            "--output-dir",
            reports.to_str().unwrap(),
        ],
    );
    stdout(&rerun);
    let (_, focused, selected, result) = saved_report(&rerun);
    assert_eq!(focused["rerun"]["mutations"][0]["source_id"], 2);
    assert_eq!(selected["mutations"].as_array().unwrap().len(), 1);
    assert_eq!(selected["dismissals"]["matched"][0]["mutation_id"], 1);
    assert_eq!(result["summary"]["killed"], 1);
    assert_eq!(result["summary"]["missed"], 0);
    assert_eq!(files(&directory), original_reports);
    assert_eq!(files(&affected_dir), affected_reports);
    assert_eq!(files(&review_dir), reviewed_reports);
    assert_eq!(files(&stronger), stronger_inputs);

    // Use disposable report copies to test malformed recipes and exact source
    // matching without editing the original evidence.
    let corrupt_dir = temp.path().join("corrupt");
    copy_tree(&directory, &corrupt_dir);
    let reject = |value: &Value| {
        fs::write(
            corrupt_dir.join("mutants.json"),
            serde_json::to_vec(value).unwrap(),
        )
        .unwrap();
        let out = run(
            &root,
            &[
                "--rerun",
                corrupt_dir.to_str().unwrap(),
                "--id",
                "1",
                "--dry-run",
            ],
        );
        assert!(!out.status.success(), "{out:?}");
        out
    };
    for (key, value) in [
        ("replacement", "``"),
        ("operator", "call_remove"),
        ("file", "strings.rs"),
    ] {
        let mut corrupt = inventory.clone();
        corrupt["mutations"][0][key] = value.into();
        reject(&corrupt);
    }
    let mut corrupt = inventory.clone();
    let text = "\"constant\"";
    let start = source.find(text).unwrap();
    let before = &source[..start];
    let line = before.bytes().filter(|b| *b == b'\n').count() + 1;
    let column = before.rsplit('\n').next().unwrap().len() + 1;
    let m = &mut corrupt["mutations"][0];
    m["original"] = text.into();
    m["replacement"] = "\"\"".into();
    m["byte_start"] = start.into();
    m["byte_end"] = (start + text.len()).into();
    m["line"] = line.into();
    m["end_line"] = line.into();
    m["column"] = column.into();
    m["end_column"] = (column + text.len()).into();
    let rejected = reject(&corrupt);
    assert!(
        String::from_utf8_lossy(&rejected.stderr)
            .contains("no longer matches a discovered mutation"),
        "{rejected:?}"
    );
    fs::write(stronger.join("strings.go"), format!("{source}// changed\n")).unwrap();
    let stale = run(
        &stronger,
        &["--rerun-missed", directory.to_str().unwrap(), "--dry-run"],
    );
    assert!(!stale.status.success());
    assert!(
        String::from_utf8_lossy(&stale.stderr).contains("source fingerprint changed"),
        "{stale:?}"
    );
    for (path, bytes) in original_inputs {
        assert_eq!(fs::read(root.join(path)).unwrap(), bytes);
    }
    assert_eq!(files(&directory), original_reports);
}

#[test]
#[ignore = "requires Go 1.24+; bounded genuine timeout and capture-error controls"]
fn strings_keep_timeout_and_capture_errors_out_of_kills() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    fs::write(
        root.join("go.mod"),
        "module example.com/strings\n\ngo 1.24\n",
    )
    .unwrap();
    let source = "package literals\nfunc Chosen() string { return \"ready\" }\n";
    fs::write(root.join("strings.go"), source).unwrap();
    for (imports, effect, expected) in [
        (
            "\"time\"",
            "for { time.Sleep(time.Millisecond) }",
            "timeout",
        ),
        (
            "\"fmt\"; \"strings\"",
            "fmt.Print(strings.Repeat(\"x\", 2<<20))",
            "errors",
        ),
    ] {
        fs::write(root.join("strings_test.go"), format!("package literals\nimport (\"testing\"; {imports})\nfunc TestInitialization(t *testing.T) {{ if Chosen() == \"\" {{ {effect} }} }}\n")).unwrap();
        let output = run(
            root,
            &[
                "--operator",
                "string_replace",
                "--timeout",
                "4",
                "--baseline-timeout",
                "30",
                "--min-score",
                "100",
            ],
        );
        let (_, metadata, _, outcomes) = saved_report(&output);
        assert_eq!(outcomes["summary"][expected], 1, "{output:?}");
        assert_eq!(outcomes["summary"]["killed"], 0);
        if expected == "errors" {
            assert_eq!(output.status.code(), Some(1), "{output:?}");
            assert_eq!(metadata["status"], "failed");
            assert!(metadata["score"]["percentage"].is_null());
        }
        assert_eq!(fs::read_to_string(root.join("strings.go")).unwrap(), source);
    }
}
