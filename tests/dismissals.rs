#![cfg(unix)]

#[path = "support/invocation.rs"]
mod invocation;
#[allow(unused_imports)]
use invocation::{Child, Command};

use serde_json::Value;
use std::{
    fs,
    os::unix::fs::{PermissionsExt, symlink},
    path::{Path, PathBuf},
    process::Output,
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
    if [ -f mutie.dismissals.toml ]; then
        printf '[broken\n' > mutie.dismissals.toml
    fi
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

fn policy_report(output: &Output, undismissed: usize, dismissed: usize) -> PathBuf {
    let text = String::from_utf8_lossy(&output.stdout);
    assert_eq!(
        output.status.code(),
        Some(if undismissed > 0 { 3 } else { 0 }),
        "{output:?}"
    );
    assert!(output.stderr.is_empty(), "{output:?}");
    let path: PathBuf = text
        .lines()
        .find_map(|line| line.strip_prefix("Saving report to "))
        .unwrap()
        .into();
    let run = json(&path, "run.json");
    let outcomes = json(&path, "outcomes.json");
    let status = if undismissed > 0 { "failed" } else { "passed" };
    assert_eq!(run["status"], "completed");
    assert!(run["error"].is_null());
    assert_eq!(run["effective_config"]["fail_on_missed"], true);
    assert_eq!(run["checkpoint"], outcomes["checkpoint"]);
    assert_eq!(outcomes["status"], "completed");
    assert_eq!(outcomes["summary"]["missed"], undismissed + dismissed);
    assert_eq!(
        run["policy"],
        serde_json::json!({"status":status,"undismissed_missed":undismissed,"dismissed_missed":dismissed})
    );
    let message = format!("Findings policy: {status}");
    assert!(text.contains(&message), "{text}");
    assert!(
        fs::read_to_string(path.join("summary.txt"))
            .unwrap()
            .contains(&message)
    );
    assert!(path.join("missed.txt").is_file());
    path
}

#[test]
fn score_gate_preserves_raw_outcomes_and_labels_filtered_and_rerun_scopes() {
    let fixture = Fixture::new();
    let check = |args: &[&str], code, percentage: Value, scope, policy| {
        let output = fixture.run(args);
        assert_eq!(output.status.code(), Some(code), "{output:?}");
        let text = String::from_utf8_lossy(&output.stdout);
        let path = PathBuf::from(
            text.lines()
                .find_map(|l| l.strip_prefix("Saving report to "))
                .unwrap(),
        );
        let run = json(&path, "run.json");
        assert_eq!(run["status"], "completed");
        assert_eq!(run["score"]["percentage"], percentage);
        assert_eq!(run["score"]["scope"], scope);
        assert_eq!(run["score_policy"]["status"], policy);
        assert_eq!(
            run["checkpoint"],
            json(&path, "outcomes.json")["checkpoint"]
        );
        assert!(text.contains("Mutation score:"));
        assert!(
            fs::read_to_string(path.join("summary.txt"))
                .unwrap()
                .contains("Mutation score:")
        );
        path
    };
    let original = check(
        &[],
        0,
        serde_json::json!(50.0),
        "all_discovered",
        "disabled",
    );
    let original_bytes = fs::read(original.join("run.json")).unwrap();
    check(
        &["--min-score", "50"],
        0,
        serde_json::json!(50.0),
        "all_discovered",
        "passed",
    );
    check(
        &["--min-score", "50.01"],
        3,
        serde_json::json!(50.0),
        "all_discovered",
        "failed",
    );
    // A passing score cannot override the independent missed-findings policy.
    let failed = check(
        &["--min-score", "50", "--fail-on-missed"],
        3,
        serde_json::json!(50.0),
        "all_discovered",
        "passed",
    );
    assert_eq!(json(&failed, "run.json")["policy"]["status"], "failed");
    success(&fixture.add(&original, "2"));
    // Dismissing the survivor passes fail-on-missed but does not raise the raw score.
    let dismissed = check(
        &["--min-score", "51", "--fail-on-missed"],
        3,
        serde_json::json!(50.0),
        "all_discovered",
        "failed",
    );
    assert_eq!(json(&dismissed, "run.json")["policy"]["status"], "passed");
    assert_eq!(
        json(&dismissed, "run.json")["score"]["dismissed_in_score"],
        1
    );
    assert_eq!(
        json(&dismissed, "outcomes.json")["summary"],
        json(&original, "outcomes.json")["summary"]
    );
    check(
        &["--min-score", "100", "--include", "a.go"],
        0,
        serde_json::json!(100.0),
        "filtered",
        "passed",
    );
    check(
        &["--min-score", "100", "--operator", "boolean_flip"],
        3,
        serde_json::json!(50.0),
        "filtered",
        "failed",
    );
    let rerun = check(
        &[
            "--min-score",
            "100",
            "--rerun",
            original.to_str().unwrap(),
            "--id",
            "1",
        ],
        0,
        serde_json::json!(100.0),
        "rerun",
        "passed",
    );
    assert!(
        fs::read_to_string(rerun.join("summary.txt"))
            .unwrap()
            .contains("rerun selection only")
    );
    check(
        &[
            "--min-score",
            "0",
            "--rerun-missed",
            original.to_str().unwrap(),
        ],
        0,
        serde_json::json!(0.0),
        "rerun",
        "passed",
    );
    check(
        &["--min-score", "0", "--include", "absent.go"],
        3,
        Value::Null,
        "filtered",
        "failed",
    );
    check(
        &[
            "--min-score",
            "0",
            "--rerun",
            original.to_str().unwrap(),
            "--operator",
            "negation_remove",
        ],
        3,
        Value::Null,
        "rerun",
        "failed",
    );
    assert_eq!(fs::read(original.join("run.json")).unwrap(), original_bytes);
    assert_eq!(
        fs::read_to_string(fixture.root.join("a.go")).unwrap(),
        "package p\nvar a = true\n"
    );
    assert_eq!(
        fs::read_to_string(fixture.root.join("b.go")).unwrap(),
        "package p\nvar b = true\n"
    );
}

#[test]
fn configured_minimum_score_can_be_overridden_or_cleared_and_listing_does_not_gate() {
    let fixture = Fixture::new();
    fs::write(fixture.root.join("mutie.toml"), "min_score = 80.5").unwrap();
    let listing = success(&fixture.run(&["--list"]));
    assert!(!listing.contains("Mutation score:"));
    assert!(!fixture.root.join(".mutie").exists());
    assert_eq!(fixture.run(&[]).status.code(), Some(3));
    for (args, minimum, policy) in [
        (vec!["--min-score", "50"], serde_json::json!(50.0), "passed"),
        (vec!["--clear-min-score"], Value::Null, "disabled"),
    ] {
        let path = report_path(&fixture.run(&args));
        let run = json(&path, "run.json");
        assert_eq!(run["effective_config"]["min_score"], minimum);
        assert_eq!(run["score_policy"]["status"], policy);
    }
}

#[test]
fn findings_policy_uses_current_dismissals_for_full_and_focused_runs() {
    let fixture = Fixture::new();
    let default = fixture.report();
    assert_eq!(json(&default, "run.json")["policy"]["status"], "disabled");
    let failed = policy_report(&fixture.run(&["--fail-on-missed"]), 1, 0);
    assert_eq!(
        json(&failed, "outcomes.json")["summary"],
        json(&default, "outcomes.json")["summary"]
    );
    let original = fs::read(failed.join("run.json")).unwrap();
    success(&fixture.add(&failed, "2"));
    policy_report(&fixture.run(&["--fail-on-missed"]), 0, 1);
    let focused = policy_report(
        &fixture.run(&[
            "--rerun",
            failed.to_str().unwrap(),
            "--id",
            "2",
            "--fail-on-missed",
        ]),
        0,
        1,
    );
    let inventory = json(&focused, "mutants.json");
    assert_eq!(inventory["dismissals"]["matched"][0]["mutation_id"], 1);
    assert_eq!(inventory["mutations"].as_array().unwrap().len(), 1);
    success(&fixture.dismiss(&["remove", "1"]));
    // The source run's review and policy are historical; the current review governs.
    policy_report(
        &fixture.run(&[
            "--rerun-missed",
            focused.to_str().unwrap(),
            "--fail-on-missed",
        ]),
        1,
        0,
    );
    success(&fixture.add(&failed, "2"));
    fs::write(
        fixture.root.join("b.go"),
        "package p\n// source changed\nvar b = true\n",
    )
    .unwrap();
    let stale = policy_report(&fixture.run(&["--fail-on-missed"]), 1, 0);
    assert_eq!(
        json(&stale, "mutants.json")["dismissals"]["needs_review"][0]["status"],
        "stale"
    );
    assert_eq!(fs::read(failed.join("run.json")).unwrap(), original);
}

#[test]
fn configured_policy_can_be_disabled_and_does_not_gate_listing_or_empty_runs() {
    let fixture = Fixture::new();
    fs::write(fixture.root.join("mutie.toml"), "fail_on_missed = true").unwrap();
    let listing = success(&fixture.run(&["--list", "--fail-on-missed"]));
    assert!(!listing.contains("Findings policy:"));
    assert!(!fixture.root.join(".mutie").exists());
    let failed = policy_report(&fixture.run(&[]), 1, 0);
    let disabled = fixture.run(&["--no-fail-on-missed"]);
    let path = report_path(&disabled);
    assert_eq!(
        json(&path, "run.json")["effective_config"]["fail_on_missed"],
        false
    );
    assert_eq!(json(&path, "run.json")["policy"]["status"], "disabled");
    policy_report(&fixture.run(&["--include", "absent.go"]), 0, 0);
    policy_report(
        &fixture.run(&[
            "--rerun",
            failed.to_str().unwrap(),
            "--operator",
            "negation_remove",
        ]),
        0,
        0,
    );
}

#[test]
fn baseline_failures_and_timeouts_leave_enabled_policy_unevaluated() {
    for timeout in [false, true] {
        let fixture = Fixture::new();
        if timeout {
            fs::write(fixture.temp.path().join("bin/go"), "#!/bin/sh\nsleep 30\n").unwrap();
        } else {
            fs::write(fixture.root.join("a.go"), "package p\nvar a = false\n").unwrap();
        }
        let output = fixture.run(&[
            "--fail-on-missed",
            "--min-score",
            "0",
            "--baseline-timeout",
            "1",
        ]);
        assert_eq!(output.status.code(), Some(1), "{output:?}");
        let text = String::from_utf8_lossy(&output.stdout);
        let path = Path::new(
            text.lines()
                .find_map(|line| line.strip_prefix("Saving report to "))
                .unwrap(),
        );
        let run = json(path, "run.json");
        assert_eq!(run["status"], "failed");
        assert_eq!(run["policy"], serde_json::json!({"status":"not_evaluated"}));
        assert_eq!(run["score_policy"]["status"], "not_evaluated");
        assert!(run["score"]["percentage"].is_null());
        assert!(
            !text.contains("Findings policy: passed") && !text.contains("Findings policy: failed")
        );
    }
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
    // All commands now resolve the same grouped config before selecting a project.
    fs::write(fixture.root.join("mutie.toml"), "[go]\n").unwrap();
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
    let current = report_path(&run);
    let current_inventory = fs::read(current.join("mutants.json")).unwrap();
    let snapshot = json(&current, "mutants.json")["dismissals"].clone();
    assert_eq!(snapshot["matched"][0]["mutation_id"], 2);
    assert_eq!(snapshot["matched"][0]["dismissal"]["id"], 1);
    assert_eq!(
        snapshot["matched"][0]["dismissal"]["reason"],
        "accepted test gap"
    );
    assert!(snapshot["needs_review"].as_array().unwrap().is_empty());
    let summary = fs::read_to_string(current.join("summary.txt")).unwrap();
    assert!(summary.contains("Missed review: 0 undismissed; 1 dismissed"));
    assert!(summary.contains("Reviewed café behavior."));
    assert!(
        fs::read_to_string(current.join("missed.txt"))
            .unwrap()
            .contains("b.go")
    );
    let outcomes = json(&current, "outcomes.json");
    assert_eq!(outcomes["mutations"][0]["outcome"], "killed");
    assert_eq!(outcomes["mutations"][1]["outcome"], "missed");
    let rerun = fixture.run(&["--rerun-missed", report.to_str().unwrap()]);
    assert!(success(&rerun).contains("1 mutations: 0 killed, 1 missed"));
    assert_eq!(
        json(&report_path(&rerun), "run.json")["rerun"]["mutations"][0]["source_id"],
        2
    );
    let rerun_snapshot = json(&report_path(&rerun), "mutants.json")["dismissals"].clone();
    assert_eq!(rerun_snapshot["matched"][0]["mutation_id"], 1);
    assert_eq!(
        rerun_snapshot["matched"][0]["dismissal"],
        snapshot["matched"][0]["dismissal"]
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
    success(&fixture.dismiss(&["remove", "1"]));
    // Reading a report uses its saved review, never today's repository metadata.
    success(&fixture.run(&["--rerun", current.to_str().unwrap(), "--list"]));
    assert_eq!(
        fs::read(current.join("mutants.json")).unwrap(),
        current_inventory
    );
    let fresh = fixture.report();
    assert!(
        json(&fresh, "mutants.json")["dismissals"]["matched"]
            .as_array()
            .unwrap()
            .is_empty()
    );
}

#[test]
fn reports_capture_review_issues_without_applying_them_or_failing_execution() {
    let fixture = Fixture::new();
    let report = fixture.report();
    success(&fixture.add(&report, "1"));
    success(&fixture.add(&report, "2"));
    let original = fixture.store();

    // Valid decisions outside a focused selection are not reported as missing.
    let focused = report_path(&fixture.run(&["--rerun", report.to_str().unwrap(), "--id", "2"]));
    let review = json(&focused, "mutants.json")["dismissals"].clone();
    assert_eq!(review["matched"].as_array().unwrap().len(), 1);
    assert_eq!(review["matched"][0]["mutation_id"], 1);
    assert_eq!(review["matched"][0]["dismissal"]["id"], 2);
    assert!(review["needs_review"].as_array().unwrap().is_empty());
    // The shared reader validates the remapped identity before a further rerun.
    success(&fixture.run(&["--rerun", focused.to_str().unwrap(), "--list"]));

    fs::write(
        fixture.root.join("b.go"),
        "package p\n// changed source\nvar b = true\n",
    )
    .unwrap();
    let stale = fixture.report();
    let review = json(&stale, "mutants.json")["dismissals"].clone();
    assert_eq!(review["matched"].as_array().unwrap().len(), 1);
    assert_eq!(review["needs_review"][0]["status"], "stale");
    assert_eq!(review["needs_review"][0]["dismissal"]["id"], 2);
    let summary = fs::read_to_string(stale.join("summary.txt")).unwrap();
    assert!(summary.contains("Missed review: 1 undismissed; 0 dismissed"));
    assert!(summary.contains("Needs review (stale)"));
    success(&fixture.run(&["--rerun", stale.to_str().unwrap(), "--list"]));

    fs::write(fixture.root.join("b.go"), "package p\nvar b = true\n").unwrap();
    let mut duplicate = original.clone();
    let mut entry = duplicate["dismissals"][1].clone();
    entry["id"] = toml::Value::Integer(3);
    duplicate["dismissals"].as_array_mut().unwrap().push(entry);
    duplicate["next_id"] = toml::Value::Integer(4);
    fixture.write_store(&duplicate);
    let ambiguous = fixture.report();
    let review = json(&ambiguous, "mutants.json")["dismissals"].clone();
    assert_eq!(review["matched"].as_array().unwrap().len(), 1);
    assert_eq!(review["needs_review"].as_array().unwrap().len(), 2);
    assert!(
        review["needs_review"]
            .as_array()
            .unwrap()
            .iter()
            .all(|e| e["status"] == "ambiguous")
    );
    success(&fixture.run(&["--rerun", ambiguous.to_str().unwrap(), "--list"]));

    fixture.write_store(&original);
    fs::write(fixture.root.join("mutie.toml"), "exclude = ['b.go']\n").unwrap();
    let missing = fixture.report();
    assert_eq!(
        json(&missing, "mutants.json")["dismissals"]["needs_review"][0]["status"],
        "missing"
    );
    assert_eq!(json(&missing, "outcomes.json")["summary"]["killed"], 1);
}

#[test]
fn multiline_calls_keep_identity_and_dismissals_across_operator_selection_and_reruns() {
    let fixture = Fixture::new();
    let source = "package p\nvar b = true\nfunc f() {\n\tprintln(\n\t\t\"é\",\n\t)\n}\n";
    fs::write(fixture.root.join("b.go"), source).unwrap();
    let original = fixture.report();
    let original_bytes = fs::read(original.join("mutants.json")).unwrap();
    let inventory = json(&original, "mutants.json");
    assert_eq!(inventory["mutations"].as_array().unwrap().len(), 3);
    let call = &inventory["mutations"][2];
    assert_eq!(call["operator"], "call_remove");
    assert_eq!(call["line"], 4);
    assert_eq!(call["end_line"], 6);
    assert_eq!(call["replacement"], "");
    assert!(
        fs::read_to_string(original.join(call["diff"].as_str().unwrap()))
            .unwrap()
            .contains("-\t\t\"é\",\n")
    );
    success(&fixture.add(&original, "2"));
    success(&fixture.add(&original, "3"));
    let dismissal_bytes = fixture.bytes();
    fs::write(
        fixture.root.join("mutie.toml"),
        "operators = ['call_remove']",
    )
    .unwrap();
    assert!(success(&fixture.dismiss(&["check"])).contains("2 matched; 0 require review"));

    for args in [
        vec![],
        vec!["--rerun-missed", original.to_str().unwrap(), "--id", "2,3"],
    ] {
        let output = fixture.run(&args);
        success(&output);
        let current = report_path(&output);
        let inventory = json(&current, "mutants.json");
        assert_eq!(inventory["mutations"].as_array().unwrap().len(), 1);
        assert_eq!(inventory["mutations"][0]["original"], call["original"]);
        assert_eq!(
            inventory["dismissals"]["matched"].as_array().unwrap().len(),
            1
        );
        assert_eq!(inventory["dismissals"]["matched"][0]["mutation_id"], 1);
        assert_eq!(inventory["dismissals"]["matched"][0]["dismissal"]["id"], 2);
        assert!(
            inventory["dismissals"]["needs_review"]
                .as_array()
                .unwrap()
                .is_empty()
        );
        if !args.is_empty() {
            let metadata = json(&current, "run.json");
            assert_eq!(metadata["rerun"]["mutations"][0]["source_id"], 3);
            assert_eq!(
                metadata["rerun"]["operators"],
                serde_json::json!(["call_remove"])
            );
        }
        // Reading the filtered report validates the multiline review identity.
        success(&fixture.run(&["--rerun", current.to_str().unwrap(), "--id", "1", "--list"]));
    }
    let empty = fixture.run(&["--rerun", original.to_str().unwrap(), "--id", "2", "--list"]);
    assert!(success(&empty).contains("Selected 0 mutations"));
    let cleared = fixture.run(&[
        "--rerun",
        original.to_str().unwrap(),
        "--clear-operators",
        "--list",
    ]);
    assert!(success(&cleared).contains("Selected 3 mutations"));
    assert_eq!(
        fs::read(original.join("mutants.json")).unwrap(),
        original_bytes
    );
    assert_eq!(fixture.bytes(), dismissal_bytes);
    assert_eq!(
        fs::read_to_string(fixture.root.join("b.go")).unwrap(),
        source
    );

    fs::write(fixture.root.join("b.go"), format!("{source}// edit\n")).unwrap();
    let stale = fixture.run(&["--rerun", original.to_str().unwrap(), "--id", "3"]);
    assert!(!stale.status.success());
    assert!(String::from_utf8_lossy(&stale.stderr).contains("stale"));
}

#[test]
fn malformed_dismissal_metadata_fails_runs_before_go_and_never_invents_a_review() {
    let fixture = Fixture::new();
    fs::write(fixture.root.join("mutie.dismissals.toml"), "[broken").unwrap();
    fixture.reset_calls();
    let output = fixture.run(&[]);
    failure(&output, "parsing");
    assert_eq!(fixture.calls(), "");
    let directory: PathBuf = String::from_utf8_lossy(&output.stdout)
        .lines()
        .find_map(|l| l.strip_prefix("Saving report to "))
        .unwrap()
        .into();
    assert_eq!(json(&directory, "run.json")["status"], "failed");
    assert_eq!(json(&directory, "mutants.json")["complete"], false);
    assert!(json(&directory, "mutants.json").get("dismissals").is_none());
    // Listing remains a source-only operation, independent of review metadata.
    success(&fixture.run(&["--list"]));
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
