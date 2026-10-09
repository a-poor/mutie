#![cfg(unix)]

use serde_json::Value;
use std::{
    fs,
    os::unix::fs::{PermissionsExt, symlink},
    path::{Path, PathBuf},
    process::{Command, Output},
};

struct Fixture {
    temp: tempfile::TempDir,
    root: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let temp = tempfile::tempdir_in(std::env::temp_dir().canonicalize().unwrap()).unwrap();
        let root = temp.path().join("project");
        fs::create_dir(&root).unwrap();
        fs::create_dir(temp.path().join("bin")).unwrap();
        fs::write(root.join("go.mod"), "module example.com/test\n").unwrap();
        for name in ["a", "b"] {
            fs::write(
                root.join(format!("{name}.go")),
                format!("package p\nvar {name} = true\n"),
            )
            .unwrap();
        }
        let go = temp.path().join("bin/go");
        fs::write(
            &go,
            r#"#!/bin/sh
printf '%s\n' "$*" >> "$CALLS"
if [ "$1" = list ]; then
    printf '{"Dir":"%s","GoFiles":[' "$PWD"
    separator=''
    for file in a.go b.go; do
        if [ -f "$file" ]; then
            printf '%s"%s"' "$separator" "$file"
            separator=','
        fi
    done
    printf ']}\n'
    # Discovery side effects must not change source identity or the checkout.
    printf 'package p\nvar b = false\n' > b.go
    exit 0
fi
if grep -q 'a = false' a.go; then
    printf '{"Action":"fail","Package":"p","Test":"TestBehavior"}\n'
    exit 1
fi
printf '{"Action":"pass","Package":"p"}\n'
"#,
        )
        .unwrap();
        fs::set_permissions(go, fs::Permissions::from_mode(0o755)).unwrap();
        Self { temp, root }
    }

    fn command(&self, with_go: bool) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_mutie"));
        command
            .current_dir(&self.root)
            .env("CALLS", self.temp.path().join("calls"));
        if with_go {
            let mut paths = vec![self.temp.path().join("bin")];
            paths.extend(std::env::split_paths(
                &std::env::var_os("PATH").unwrap_or_default(),
            ));
            command.env("PATH", std::env::join_paths(paths).unwrap());
        } else {
            command.env("PATH", "");
        }
        command
    }

    fn dismiss(&self, args: &[&str]) -> Output {
        self.command(args == ["check"])
            .arg("dismiss")
            .args(args)
            .output()
            .unwrap()
    }

    fn run(&self, args: &[&str]) -> Output {
        self.command(true)
            .args(["--ui", "plain", "--jobs", "2"])
            .args(args)
            .output()
            .unwrap()
    }

    fn report(&self) -> PathBuf {
        let output = self.run(&[]);
        success(&output);
        report_path(&output)
    }

    fn add(&self, report: &Path, id: &str) -> Output {
        self.dismiss(&[
            "add",
            report.to_str().unwrap(),
            "--id",
            id,
            "--reason",
            "accepted test gap",
            "--note",
            "Reviewed café behavior.\nFollow up later.",
        ])
    }

    fn bytes(&self) -> Vec<u8> {
        fs::read(self.root.join("mutie.dismissals.toml")).unwrap()
    }

    fn store(&self) -> toml::Value {
        toml::from_str(std::str::from_utf8(&self.bytes()).unwrap()).unwrap()
    }

    fn write_store(&self, store: &toml::Value) {
        fs::write(
            self.root.join("mutie.dismissals.toml"),
            toml::to_string_pretty(store).unwrap(),
        )
        .unwrap();
    }

    fn reset_calls(&self) {
        fs::write(self.temp.path().join("calls"), "").unwrap();
    }
    fn calls(&self) -> String {
        fs::read_to_string(self.temp.path().join("calls")).unwrap()
    }
}

fn success(output: &Output) -> String {
    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout.clone()).unwrap()
}

fn failure(output: &Output, expected: &str) {
    assert!(
        !output.status.success(),
        "unexpected success: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(text.contains(expected), "expected {expected:?} in {text}");
}

fn report_path(output: &Output) -> PathBuf {
    success(output)
        .lines()
        .find_map(|line| line.strip_prefix("Saving report to "))
        .unwrap()
        .into()
}

fn json(path: &Path, name: &str) -> Value {
    serde_json::from_slice(&fs::read(path.join(name)).unwrap()).unwrap()
}

#[test]
fn crud_persists_reasons_notes_and_independent_ids_without_go() {
    let fixture = Fixture::new();
    assert!(
        success(
            &fixture
                .command(false)
                .args(["dismiss", "check"])
                .output()
                .unwrap()
        )
        .contains("No dismissals to check")
    );
    assert!(!fixture.root.join(".mutie").exists());
    let report = fixture.report();
    fs::write(fixture.root.join("mutie.toml"), "[not valid configuration").unwrap();
    fixture.reset_calls();
    assert!(success(&fixture.dismiss(&["list"])).contains("0 dismissal(s)"));
    assert!(!fixture.root.join("mutie.dismissals.toml").exists());
    let added = success(&fixture.add(&report, "2"));
    assert!(added.contains("Added dismissal 1 from saved mutation 2"));
    let store = fixture.store();
    assert_eq!(store["next_id"].as_integer(), Some(2));
    let entry = &store["dismissals"][0];
    assert_eq!(entry["id"].as_integer(), Some(1));
    assert_eq!(entry["mutation"]["file"].as_str(), Some("b.go"));
    assert_eq!(entry["reason"].as_str(), Some("accepted test gap"));
    assert_eq!(
        entry["note"].as_str(),
        Some("Reviewed café behavior.\nFollow up later.")
    );
    assert!(
        !String::from_utf8(fixture.bytes())
            .unwrap()
            .contains(fixture.root.to_str().unwrap())
    );

    let listed = success(&fixture.dismiss(&["list"]));
    for expected in [
        "current source unchecked",
        "Dismissal 1",
        "b.go",
        "boolean_flip",
        "accepted test gap",
        "café",
        "sha256:",
    ] {
        assert!(listed.contains(expected), "{listed}");
    }
    let before = fixture.bytes();
    failure(&fixture.add(&report, "2"), "already has dismissal 1");
    assert_eq!(fixture.bytes(), before);
    success(&fixture.dismiss(&["update", "1", "--reason", "equivalent behavior"]));
    assert_eq!(fixture.store()["dismissals"][0]["note"], entry["note"]);
    success(&fixture.dismiss(&["update", "1", "--note", "New review"]));
    assert_eq!(
        fixture.store()["dismissals"][0]["reason"].as_str(),
        Some("equivalent behavior")
    );
    success(&fixture.dismiss(&["update", "1", "--clear-note"]));
    assert!(fixture.store()["dismissals"][0].get("note").is_none());
    success(&fixture.dismiss(&["remove", "1"]));
    assert!(fixture.store()["dismissals"].as_array().unwrap().is_empty());
    assert!(
        success(&fixture.add(&report, "1")).contains("Added dismissal 2 from saved mutation 1")
    );
    assert_eq!(fixture.store()["next_id"].as_integer(), Some(3));
    assert_eq!(fixture.calls(), "");
    assert!(!fixture.root.join(".mutie.dismissals.lock").exists());
}

#[test]
fn failed_edits_leave_metadata_untouched_and_release_the_lock() {
    let fixture = Fixture::new();
    let report = fixture.report();
    success(&fixture.add(&report, "2"));
    let before = fixture.bytes();
    for (args, expected) in [
        (vec!["update", "1"], "update requires"),
        (
            vec!["update", "1", "--reason", " \n\t"],
            "reason must not be blank",
        ),
        (vec!["update", "1", "--note", ""], "note must not be blank"),
        (
            vec!["update", "1", "--note", "x", "--clear-note"],
            "cannot be used with",
        ),
        (
            vec!["update", "42", "--reason", "x"],
            "unknown dismissal ID 42",
        ),
        (vec!["remove", "42"], "unknown dismissal ID 42"),
        (vec!["remove", "0"], "invalid value"),
        (
            vec![
                "add",
                report.to_str().unwrap(),
                "--id",
                "3",
                "--reason",
                "x",
            ],
            "unknown source mutation ID 3",
        ),
        (
            vec!["add", report.to_str().unwrap(), "--id", "1", "--reason", ""],
            "reason must not be blank",
        ),
        (
            vec!["add", report.to_str().unwrap(), "--id", "1"],
            "--reason",
        ),
    ] {
        failure(&fixture.dismiss(&args), expected);
        assert_eq!(fixture.bytes(), before);
        assert!(!fixture.root.join(".mutie.dismissals.lock").exists());
    }
    let lock = fixture.root.join(".mutie.dismissals.lock");
    fs::write(&lock, "another writer").unwrap();
    failure(&fixture.dismiss(&["remove", "1"]), "locking dismissals");
    assert_eq!(fs::read_to_string(&lock).unwrap(), "another writer");
    assert_eq!(fixture.bytes(), before);
    // Readers remain available while a writer owns the lock.
    success(&fixture.dismiss(&["list"]));
}

#[test]
fn check_uses_pristine_discovery_and_portable_identity_and_flags_source_changes() {
    let mut fixture = Fixture::new();
    let report = fixture.report();
    success(&fixture.add(&report, "2"));
    let original = fixture.bytes();
    fixture.reset_calls();
    assert!(success(&fixture.dismiss(&["check"])).contains("1 matched; 0 require review"));
    assert_eq!(fixture.calls(), "list -json ./...\n");
    assert_eq!(
        fs::read_to_string(fixture.root.join("b.go")).unwrap(),
        "package p\nvar b = true\n"
    );
    assert_eq!(
        fs::read_dir(fixture.root.join(".mutie")).unwrap().count(),
        1
    );

    let moved = fixture.temp.path().join("moved checkout");
    fs::rename(&fixture.root, &moved).unwrap();
    fixture.root = moved;
    fs::write(
        fixture.root.join("b_test.go"),
        "package p\n// stronger tests\n",
    )
    .unwrap();
    success(&fixture.dismiss(&["check"]));
    fs::write(
        fixture.root.join("b.go"),
        "package p\n// source changed\nvar b = true\n",
    )
    .unwrap();
    failure(
        &fixture.dismiss(&["check"]),
        "stale (source fingerprint changed)",
    );
    fs::write(fixture.root.join("mutie.toml"), "exclude = ['b.go']\n").unwrap();
    failure(
        &fixture.dismiss(&["check"]),
        "missing (file removed, excluded, inactive",
    );
    fs::write(fixture.root.join("mutie.toml"), "").unwrap();
    fs::remove_file(fixture.root.join("b.go")).unwrap();
    failure(
        &fixture.dismiss(&["check"]),
        "missing (file removed, excluded, inactive",
    );
    assert!(!fixture.root.join("b.go").exists());
    assert_eq!(fixture.bytes(), original);
}

#[test]
fn check_requires_exact_coordinates_operators_and_text_and_flags_duplicates() {
    let fixture = Fixture::new();
    let report = fixture.report();
    success(&fixture.add(&report, "2"));
    let original = fixture.store();
    for (field, value) in [
        ("byte_start", toml::Value::Integer(0)),
        ("line", toml::Value::Integer(1)),
        ("column", toml::Value::Integer(1)),
        ("end_line", toml::Value::Integer(3)),
        ("end_column", toml::Value::Integer(99)),
        ("operator", toml::Value::String("logical_swap".into())),
        ("original", toml::Value::String("TRUE".into())),
        ("replacement", toml::Value::String("other".into())),
    ] {
        let mut store = original.clone();
        store["dismissals"][0]["mutation"][field] = value;
        if field == "byte_start" {
            store["dismissals"][0]["mutation"]["byte_end"] = toml::Value::Integer(4);
        }
        fixture.write_store(&store);
        let before = fixture.bytes();
        failure(&fixture.dismiss(&["check"]), "unmatched (source identity");
        assert_eq!(fixture.bytes(), before);
    }
    let mut duplicates = original.clone();
    let mut second = duplicates["dismissals"][0].clone();
    second["id"] = toml::Value::Integer(2);
    duplicates["dismissals"]
        .as_array_mut()
        .unwrap()
        .push(second);
    duplicates["next_id"] = toml::Value::Integer(3);
    fixture.write_store(&duplicates);
    let result = fixture.dismiss(&["check"]);
    failure(&result, "0 matched; 2 require review");
    assert_eq!(
        String::from_utf8(result.stdout)
            .unwrap()
            .matches("ambiguous (")
            .count(),
        2
    );
    success(&fixture.dismiss(&["remove", "2"]));
    success(&fixture.dismiss(&["check"]));
}

#[test]
fn dismissals_preserve_saved_reports_execution_outcomes_and_rerun_selection() {
    let fixture = Fixture::new();
    let report = fixture.report();
    let saved: Vec<_> = ["run.json", "mutants.json", "outcomes.json", "summary.txt"]
        .into_iter()
        .map(|name| (name, fs::read(report.join(name)).unwrap()))
        .collect();
    success(&fixture.add(&report, "2"));
    let metadata = fixture.bytes();
    let run = fixture.run(&[]);
    assert!(success(&run).contains("2 mutations: 1 killed, 1 missed"));
    let outcomes = json(&report_path(&run), "outcomes.json");
    assert_eq!(outcomes["mutations"][0]["outcome"], "killed");
    assert_eq!(outcomes["mutations"][1]["outcome"], "missed");
    let rerun = fixture.run(&["--rerun-missed", report.to_str().unwrap()]);
    assert!(success(&rerun).contains("1 mutations: 0 killed, 1 missed"));
    assert_eq!(
        json(&report_path(&rerun), "run.json")["rerun"]["mutations"][0]["source_id"],
        2
    );
    for (name, bytes) in saved {
        assert_eq!(fs::read(report.join(name)).unwrap(), bytes);
    }
    assert_eq!(fixture.bytes(), metadata);
    for name in ["a", "b"] {
        assert_eq!(
            fs::read_to_string(fixture.root.join(format!("{name}.go"))).unwrap(),
            format!("package p\nvar {name} = true\n")
        );
    }
}

#[test]
fn malformed_or_incompatible_metadata_is_never_overwritten() {
    let fixture = Fixture::new();
    let report = fixture.report();
    success(&fixture.add(&report, "2"));
    let original = fixture.store();
    let mut cases = vec![("[broken".to_owned(), "parsing")];
    for (field, value, expected) in [
        ("format_version", 99, "unsupported dismissal format"),
        ("operator_catalog_version", 99, "incompatible operator"),
        ("next_id", 1, "next_id must exceed"),
        ("unknown_key", 1, "unknown field"),
    ] {
        let mut store = original.clone();
        store
            .as_table_mut()
            .unwrap()
            .insert(field.into(), toml::Value::Integer(value));
        cases.push((toml::to_string(&store).unwrap(), expected));
    }
    let mut duplicate = original.clone();
    let entry = duplicate["dismissals"][0].clone();
    duplicate["dismissals"].as_array_mut().unwrap().push(entry);
    cases.push((
        toml::to_string(&duplicate).unwrap(),
        "duplicate dismissal ID",
    ));
    for (field, value, expected) in [
        ("file", "../b.go", "invalid module-relative source path"),
        (
            "source_fingerprint",
            "sha256:bad",
            "invalid source fingerprint",
        ),
        ("original", "", "invalid source coordinates"),
        ("unknown_key", "x", "unknown field"),
    ] {
        let mut store = original.clone();
        store["dismissals"][0]["mutation"]
            .as_table_mut()
            .unwrap()
            .insert(field.into(), toml::Value::String(value.into()));
        cases.push((toml::to_string(&store).unwrap(), expected));
    }
    fixture.reset_calls();
    for (source, expected) in cases {
        fs::write(fixture.root.join("mutie.dismissals.toml"), &source).unwrap();
        failure(&fixture.dismiss(&["remove", "1"]), expected);
        failure(&fixture.dismiss(&["check"]), expected);
        assert_eq!(fixture.bytes(), source.as_bytes());
        assert!(!fixture.root.join(".mutie.dismissals.lock").exists());
    }
    assert_eq!(fixture.calls(), "");
}

#[test]
fn module_option_selects_destination_and_rejects_symlinked_metadata() {
    let fixture = Fixture::new();
    let report = fixture.report();
    // Both positions of the group-global option are supported.
    for args in [
        vec!["--module", fixture.root.to_str().unwrap(), "list"],
        vec!["list", "--module", fixture.root.to_str().unwrap()],
    ] {
        success(
            &fixture
                .command(false)
                .current_dir(fixture.temp.path())
                .arg("dismiss")
                .args(args)
                .output()
                .unwrap(),
        );
    }
    success(&fixture.add(&report, "2"));
    let path = fixture.root.join("mutie.dismissals.toml");
    let target = fixture.temp.path().join("external.toml");
    let before = fixture.bytes();
    fs::rename(&path, &target).unwrap();
    symlink(&target, &path).unwrap();
    failure(&fixture.dismiss(&["remove", "1"]), "must be a regular file");
    assert_eq!(fs::read(target).unwrap(), before);
    assert!(fs::symlink_metadata(path).unwrap().is_symlink());
}
