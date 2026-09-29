use std::{
    fs,
    path::Path,
    process::{Command, Output},
};

fn copy_tree(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let destination = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&entry.path(), &destination);
        } else {
            fs::copy(entry.path(), destination).unwrap();
        }
    }
}

fn run(root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_mutie"))
        .arg(root)
        .args(args)
        .output()
        .expect("could not start mutie")
}

fn stdout(output: &Output) -> String {
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).into_owned()
}

#[test]
#[ignore = "requires Go 1.24+ on PATH; runs real builds and a deliberate timeout"]
fn go_workflow() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("module");
    copy_tree(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/go"),
        &root,
    );

    let listing = stdout(&run(&root, &["--list"]));
    assert!(listing.contains("Found 4 mutations"), "{listing}");
    assert!(!listing.contains("Running baseline"));
    assert!(!listing.contains("disabled.go:"));
    assert!(!listing.contains("generated.go:"));
    assert!(!listing.contains("nested.go:"));
    assert!(!listing.contains("fixture_test.go:"));

    let results = stdout(&run(&root, &["--jobs", "2", "--timeout", "5"]));
    assert!(
        results.contains("4 mutations: 1 killed, 1 missed, 1 timed out, 1 failed to compile"),
        "{results}"
    );

    // Multiple mutations on one worker also prove that test side effects reset.
    let scoped = stdout(&run(
        &root,
        &[
            "--include",
            "*.go",
            "--exclude",
            "ready.go",
            "--timeout",
            "10",
        ],
    ));
    assert!(
        scoped.contains("3 mutations: 1 killed, 1 missed, 0 timed out, 1 failed to compile"),
        "{scoped}"
    );
    assert!(!root.join("test-artifact").exists());
    assert_eq!(
        fs::read_to_string(root.join("testdata/state.txt")).unwrap(),
        "pristine\n"
    );
    assert_eq!(
        fs::read(root.join("add.go")).unwrap(),
        fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/go/add.go")).unwrap()
    );

    fs::write(root.join("fail_test.go"), "package fixture\nimport \"testing\"\nfunc TestFail(t *testing.T) { t.Fatal(\"baseline fails\") }\n").unwrap();
    let failed = run(&root, &[]);
    assert!(!failed.status.success());
    assert!(
        String::from_utf8_lossy(&failed.stderr)
            .contains("baseline tests failed; no mutations were run")
    );
    assert!(!String::from_utf8_lossy(&failed.stdout).contains("Running 1 worker"));
    assert!(!root.join("test-artifact").exists());

    fs::write(root.join("fail_test.go"), "package fixture\nimport (\"testing\"; \"time\")\nfunc TestHang(t *testing.T) { time.Sleep(time.Hour) }\n").unwrap();
    let timed_out = run(&root, &["--baseline-timeout", "1"]);
    assert!(!timed_out.status.success());
    assert!(String::from_utf8_lossy(&timed_out.stderr).contains("baseline timed out"));
}
