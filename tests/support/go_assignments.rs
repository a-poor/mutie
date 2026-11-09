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
#[ignore = "requires Go 1.24+; scored stores, build failures, policies, review and rerun identities"]
fn assignments_scored_workflow() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("project");
    fs::create_dir(&root).unwrap();
    fs::write(
        root.join("go.mod"),
        "module example.com/assignments\n\ngo 1.24\n",
    )
    .unwrap();
    let source = "package assignments\ntype State struct { Value int }\nfunc Checked(a *State) { a.Value = 1 }\nfunc Unchecked(a []int) { a[0] = 2 }\nfunc Noisy(a *State) { x := 3; a.Value = x }\nfunc Header(a *State) { if a.Value = 4; a.Value == 4 {} }\n";
    fs::write(root.join("assignments.go"), source).unwrap();
    fs::write(root.join("import.go"), "package assignments\nimport \"strings\"\nfunc ImportOnly(a map[string]string) { a[\"x\"] = strings.ToUpper(\"x\") }\n").unwrap();
    let tests = "package assignments\nimport \"testing\"\nfunc TestChecked(t *testing.T) { a := State{}; Checked(&a); if a.Value != 1 { t.Fatal(a) }; Unchecked([]int{0}); Noisy(&a); ImportOnly(map[string]string{}); Header(&a) }\n";
    fs::write(root.join("assignments_test.go"), tests).unwrap();
    let original_inputs = files(&root);
    let reports = temp.path().join("reports");
    let output = run(
        &root,
        &[
            "--operator",
            "assignment_remove",
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
        assert_eq!(m["operator"], "assignment_remove");
        assert_eq!(m["replacement"], "");
    }
    let original_reports = files(&directory);
    let affected = run(
        &root,
        &[
            "--operator",
            "assignment_remove",
            "--test-scope",
            "affected",
            "--timeout",
            "20",
            "--output-dir",
            reports.to_str().unwrap(),
        ],
    );
    stdout(&affected);
    assert_eq!(saved_report(&affected).3["summary"], outcomes["summary"]);
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
    let stronger = temp.path().join("stronger");
    copy_tree(&root, &stronger);
    fs::write(stronger.join("stronger_test.go"), "package assignments\nimport \"testing\"\nfunc TestStoredItem(t *testing.T) { a := []int{0}; Unchecked(a); if a[0] != 2 { t.Fatal(a) } }\n").unwrap();
    let rerun = run(
        &stronger,
        &[
            "--rerun-missed",
            directory.to_str().unwrap(),
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
    for (key, value) in [("replacement", ";"), ("operator", "call_remove")] {
        let mut corrupt = inventory.clone();
        corrupt["mutations"][0][key] = value.into();
        reject(&corrupt);
    }
    let mut corrupt = inventory.clone();
    let text = "a.Value = 4";
    let start = source.find(text).unwrap();
    let before = &source[..start];
    let line = before.bytes().filter(|b| *b == b'\n').count() + 1;
    let column = before.rsplit('\n').next().unwrap().len() + 1;
    let m = &mut corrupt["mutations"][0];
    m["original"] = text.into();
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
    fs::write(
        stronger.join("assignments.go"),
        format!("{source}// changed\n"),
    )
    .unwrap();
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
fn assignments_keep_timeout_and_capture_errors_out_of_kills() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    fs::write(
        root.join("go.mod"),
        "module example.com/assignments\n\ngo 1.24\n",
    )
    .unwrap();
    let source = "package assignments\ntype State struct { Value int }\nfunc Initialize(a *State) { a.Value = 1 }\n";
    fs::write(root.join("assignments.go"), source).unwrap();
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
        fs::write(root.join("assignments_test.go"), format!("package assignments\nimport (\"testing\"; {imports})\nfunc TestInitialization(t *testing.T) {{ a := State{{}}; Initialize(&a); if a.Value == 0 {{ {effect} }} }}\n")).unwrap();
        let output = run(
            root,
            &[
                "--operator",
                "assignment_remove",
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
        assert_eq!(
            fs::read_to_string(root.join("assignments.go")).unwrap(),
            source
        );
    }
}
