use super::*;
use std::os::unix::fs::PermissionsExt;

pub(super) fn command(fixture: &Fixture, args: &[&str], tools: bool) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_mutie"));
    command
        .args(["dismiss", "--module"])
        .arg(&fixture.root)
        .args(args)
        .env(
            "CARGO_TARGET_DIR",
            fixture.temp.path().join("caller-target"),
        )
        .env(
            "CARGO_BUILD_BUILD_DIR",
            fixture.temp.path().join("caller-build"),
        )
        .env("TMPDIR", fixture.temp.path().join("owned"));
    if !tools {
        command.env("PATH", "");
    }
    command
}

fn dismiss(fixture: &Fixture, args: &[&str]) -> Output {
    let output = command(fixture, args, args.contains(&"check"))
        .output()
        .unwrap();
    fixture.preserved();
    output
}

fn success(output: Output) -> String {
    assert!(output.status.success(), "{output:?}");
    String::from_utf8(output.stdout).unwrap()
}

fn failure(output: Output, expected: &str) {
    assert!(!output.status.success(), "{output:?}");
    assert!(
        format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
        .contains(expected),
        "{expected}: {output:?}"
    );
}

fn files(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn collect(root: &Path, at: &Path, values: &mut BTreeMap<PathBuf, Vec<u8>>) {
        for entry in fs::read_dir(at).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                collect(root, &path, values);
            } else {
                values.insert(
                    path.strip_prefix(root).unwrap().to_owned(),
                    fs::read(path).unwrap(),
                );
            }
        }
    }
    let mut values = BTreeMap::new();
    collect(root, root, &mut values);
    values
}

fn add(fixture: &Fixture, directory: &Path, id: &str) -> String {
    success(dismiss(
        fixture,
        &[
            "add",
            directory.to_str().unwrap(),
            "--id",
            id,
            "--reason",
            "accepted test gap",
            "--note",
            "Reviewed café behavior.\nFollow up later.",
        ],
    ))
}

#[test]
#[ignore = "runs real Cargo dismissal/review/policy workflows; opt-in to avoid recursive self-testing"]
fn rust_dismissals_preserve_outcomes_scores_reports_and_focused_reruns() {
    let mut fixture = Fixture::new();
    assert!(success(dismiss(&fixture, &["list"])).contains("0 dismissal(s)"));
    assert!(
        success(command(&fixture, &["check"], false).output().unwrap()).contains("No dismissals")
    );
    let initial = fixture.run(&["--jobs", "2", "--fail-on-missed"]);
    assert_eq!(initial.status.code(), Some(3), "{initial:?}");
    let (source, original_run, _, original_outcomes) = report(&initial);
    let original_report = files(&source);
    let flags = fs::read(fixture.root.join("src/flags.rs")).unwrap();
    let config = fixture.root.join("mutie.toml");
    let original_config = fs::read(&config).unwrap();
    let metadata = fixture.root.join("mutie.dismissals.toml");
    fs::write(&config, "[broken").unwrap();
    // Grouped config resolves project paths even for metadata-only commands.
    failure(dismiss(&fixture, &["list"]), "parsing configuration");
    failure(dismiss(&fixture, &["check"]), "parsing configuration");
    fs::write(&config, &original_config).unwrap();
    assert!(add(&fixture, &source, "3").contains("Added dismissal 1"));
    assert!(add(&fixture, &source, "1").contains("Added dismissal 2"));
    let before = fs::read(&metadata).unwrap();
    failure(
        dismiss(
            &fixture,
            &[
                "add",
                source.to_str().unwrap(),
                "--id",
                "3",
                "--reason",
                "duplicate",
            ],
        ),
        "already has dismissal 1",
    );
    assert_eq!(fs::read(&metadata).unwrap(), before);
    success(dismiss(
        &fixture,
        &["update", "1", "--reason", "reviewed gap", "--clear-note"],
    ));
    let listed = success(dismiss(&fixture, &["list"]));
    assert!(listed.contains("reviewed gap") && listed.contains("Language: Rust"));
    success(dismiss(&fixture, &["check"]));
    assert!(!fixture.root.join(".mutie.dismissals.lock").exists());

    fs::write(fixture.root.join("go.mod"), "module example.com/mixed\n").unwrap();
    // The explicit Rust group resolves a mixed repository without guessing.
    success(dismiss(&fixture, &["list"]));
    success(dismiss(&fixture, &["--lang", "rust", "list"]));
    let before = fs::read(&metadata).unwrap();
    fs::write(&config, [original_config.as_slice(), b"\n[go]\n"].concat()).unwrap();
    failure(
        std::process::Command::new(env!("CARGO_BIN_EXE_mutie"))
            .arg("--path")
            .arg(&fixture.root)
            .args([
                "dismiss",
                "--lang",
                "go",
                "add",
                source.to_str().unwrap(),
                "--id",
                "3",
                "--reason",
                "wrong language",
            ])
            .env("PATH", "")
            .output()
            .unwrap(),
        "does not match project language",
    );
    assert_eq!(fs::read(&metadata).unwrap(), before);
    fs::write(&config, &original_config).unwrap();
    fs::remove_file(fixture.root.join("go.mod")).unwrap();
    fs::write(
        &config,
        "lang = 'rust'\noperators = ['boolean_flip']\nmax_capture_bytes = 1048576\nmax_log_bytes = 1\nbaseline_timeout = 60\n",
    )
    .unwrap();
    assert!(success(dismiss(&fixture, &["check"])).contains("2 matched; 0 require review"));
    // Metadata-only checking succeeds even when a new test target cannot compile.
    let broken_test = fixture.root.join("tests/broken.rs");
    fs::write(
        &broken_test,
        "compile_error!(\"checking must not compile tests\");",
    )
    .unwrap();
    success(dismiss(&fixture, &["check"]));
    fs::remove_file(broken_test).unwrap();

    let dismissed = fixture.run(&["--jobs", "2", "--fail-on-missed", "--min-score", "100"]);
    assert_eq!(dismissed.status.code(), Some(3), "{dismissed:?}");
    let (reviewed_dir, reviewed, inventory, outcomes) = report(&dismissed);
    assert_eq!(outcomes["summary"], original_outcomes["summary"]);
    assert_eq!(
        reviewed["score"]["percentage"],
        original_run["score"]["percentage"]
    );
    assert_eq!(reviewed["score"]["dismissed_in_score"], 2);
    assert_eq!(reviewed["score_policy"]["status"], "failed");
    assert_eq!(
        reviewed["policy"],
        serde_json::json!({"status":"passed","undismissed_missed":0,"dismissed_missed":1})
    );
    assert_eq!(
        inventory["dismissals"]["matched"].as_array().unwrap().len(),
        2
    );
    assert_eq!(
        inventory["dismissals"]["matched"][1]["dismissal"]["language"],
        "rust"
    );
    let reviewed_files = files(&reviewed_dir);
    let focused = fixture.run(&[
        "--rerun-missed",
        source.to_str().unwrap(),
        "--fail-on-missed",
        "--min-score",
        "0",
    ]);
    assert!(focused.status.success(), "{focused:?}");
    let (focused_dir, focused_run, focused_inventory, focused_outcomes) = report(&focused);
    assert_eq!(focused_outcomes["summary"]["missed"], 1);
    assert_eq!(focused_run["score"]["percentage"], 0.0);
    assert_eq!(focused_run["score"]["scope"], "rerun");
    assert_eq!(
        focused_run["rerun"]["mutations"],
        serde_json::json!([{"id":1,"source_id":3}])
    );
    assert_eq!(
        focused_inventory["dismissals"]["matched"][0]["mutation_id"],
        1
    );
    assert_eq!(
        focused_inventory["dismissals"]["matched"][0]["dismissal"]["id"],
        1
    );
    assert_eq!(
        focused_inventory["dismissals"]["matched"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert!(
        focused_inventory["dismissals"]["needs_review"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    let focused_files = files(&focused_dir);

    let stored = fs::read_to_string(&metadata).unwrap();
    fs::write(
        &metadata,
        stored.replace("language = \"rust\"", "language = \"go\""),
    )
    .unwrap();
    failure(dismiss(&fixture, &["check"]), "language mismatch");
    let mismatched = fixture.run(&[
        "--rerun-missed",
        source.to_str().unwrap(),
        "--fail-on-missed",
    ]);
    assert_eq!(mismatched.status.code(), Some(3), "{mismatched:?}");
    let (_, mismatch_run, mismatch_inventory, _) = report(&mismatched);
    assert_eq!(mismatch_run["policy"]["undismissed_missed"], 1);
    assert!(
        mismatch_inventory["dismissals"]["matched"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        mismatch_inventory["dismissals"]["needs_review"][0]["status"],
        "language_mismatch"
    );
    fs::write(&metadata, &stored).unwrap();
    success(dismiss(&fixture, &["remove", "1"]));
    assert!(add(&fixture, &source, "3").contains("Added dismissal 3"));

    // Test-only changes keep the dismissal, and the dismissed survivor still executes.
    let tests = fixture.root.join("tests/cli.rs");
    let contents = fs::read_to_string(&tests).unwrap();
    fs::write(
        &tests,
        contents.replace(
            "    let output = run(\"unchecked\");",
            "    let output = run(\"unchecked\");\n    assert_eq!(output.stdout, b\"true\\n\");",
        ),
    )
    .unwrap();
    let moved = fixture.temp.path().join("moved checkout");
    fs::rename(&fixture.root, &moved).unwrap();
    fixture.root = moved;
    success(dismiss(&fixture, &["check"]));
    let stronger = fixture.run(&[
        "--rerun-missed",
        source.to_str().unwrap(),
        "--fail-on-missed",
        "--min-score",
        "100",
    ]);
    assert!(stronger.status.success(), "{stronger:?}");
    let (_, stronger_run, stronger_inventory, stronger_outcomes) = report(&stronger);
    assert_eq!(stronger_outcomes["summary"]["killed"], 1);
    assert_eq!(stronger_run["score"]["percentage"], 100.0);
    assert_eq!(stronger_run["score"]["dismissed_in_score"], 1);
    assert_eq!(
        stronger_inventory["dismissals"]["matched"][0]["dismissal"]["id"],
        3
    );

    let metadata = fixture.root.join("mutie.dismissals.toml");
    let stored = fs::read(&metadata).unwrap();
    fs::write(
        fixture.root.join("src/flags.rs"),
        [flags.as_slice(), b"\n// edited\n"].concat(),
    )
    .unwrap();
    failure(
        dismiss(&fixture, &["check"]),
        "stale (source fingerprint changed)",
    );
    fs::write(fixture.root.join("src/flags.rs"), &flags).unwrap();
    fs::write(
        fixture.root.join("mutie.toml"),
        "exclude = ['src/flags.rs']",
    )
    .unwrap();
    failure(
        dismiss(&fixture, &["check"]),
        "missing (file removed, excluded, inactive",
    );
    assert_eq!(fs::read(&metadata).unwrap(), stored);
    assert_eq!(files(&source), original_report);
    assert_eq!(files(&reviewed_dir), reviewed_files);
    assert_eq!(files(&focused_dir), focused_files);
    assert_eq!(fs::read(fixture.root.join("src/flags.rs")).unwrap(), flags);
    let validation = Command::new(env!("CARGO_BIN_EXE_mutie"))
        .arg("prune")
        .arg(fixture.temp.path().join("reports"))
        .args(["--keep-last", "0"])
        .output()
        .unwrap();
    assert!(success(validation).contains("Selected runs: 5"));
}

#[test]
#[ignore = "runs Cargo metadata and cancellation/capture checks; opt-in with Rust workflows"]
fn rust_dismissal_checks_isolate_metadata_side_effects_and_clean_up_failures() {
    let fixture = Fixture::new();
    let initial = fixture.run(&[]);
    assert!(initial.status.success(), "{initial:?}");
    let (source, _, _, _) = report(&initial);
    add(&fixture, &source, "3");
    let state = fixture.temp.path().join("check-state");
    let bin = fixture.temp.path().join("bin");
    fs::create_dir(&state).unwrap();
    fs::create_dir(&bin).unwrap();
    let proxy = bin.join("cargo");
    fs::write(&proxy, r#"#!/bin/sh
printf '%s\n' "$*" >> "$CHECK_STATE/calls"
printf '%s' "$PWD" > "$CHECK_STATE/workspace"
printf '%s' "$CARGO_TARGET_DIR" > "$CHECK_STATE/target"
printf '%s' "$TMPDIR" > "$CHECK_STATE/tmpdir"
case "$1" in metadata) ;; *) exit 99 ;; esac
case "$CHECK_MODE" in
  bad) printf 'not json\n'; exit 0 ;;
  overflow) while :; do printf '012345678901234567890123456789012345678901234567890123456789\n'; done ;;
  hang) sleep 1000 & child=$!; printf '%s' "$child" > "$CHECK_STATE/pid"; wait "$child"; exit $? ;;
esac
"$REAL_CARGO" "$@" || exit $?
printf 'pub fn replaced() -> bool { false }\n' > src/flags.rs
printf '[broken\n' > mutie.dismissals.toml
"#).unwrap();
    fs::set_permissions(&proxy, fs::Permissions::from_mode(0o755)).unwrap();
    let mut paths = vec![bin];
    paths.extend(std::env::split_paths(&std::env::var_os("PATH").unwrap()));
    let path = std::env::join_paths(paths).unwrap();
    let make_command = |mode: &str| {
        let mut command = command(&fixture, &["check"], true);
        command
            .env("PATH", &path)
            .env("CHECK_STATE", &state)
            .env("CHECK_MODE", mode)
            .env("REAL_CARGO", env!("CARGO"));
        command
    };
    let config = fixture.root.join("mutie.toml");
    fs::write(
        &config,
        "max_log_bytes = 1\nmax_capture_bytes = 65536\nbaseline_timeout = 30",
    )
    .unwrap();
    let original = files(&fixture.root);
    assert!(success(make_command("ok").output().unwrap()).contains("1 matched; 0 require review"));
    let calls = fs::read_to_string(state.join("calls")).unwrap();
    assert_eq!(calls.lines().count(), 1);
    assert!(
        calls.starts_with(
            "metadata --format-version 1 --no-deps --locked --config build.target-dir="
        )
    );
    assert!(calls.contains("--config build.build-dir="));
    failure(
        make_command("bad").output().unwrap(),
        "decoding Cargo metadata",
    );
    failure(
        make_command("overflow").output().unwrap(),
        "--max-capture-bytes (65536 bytes",
    );
    assert_eq!(files(&fixture.root), original);
    fixture.preserved();

    fs::write(&config, "baseline_timeout = 1").unwrap();
    failure(
        make_command("hang").output().unwrap(),
        "Cargo metadata failed",
    );
    let pid: i32 = fs::read_to_string(state.join("pid"))
        .unwrap()
        .parse()
        .unwrap();
    assert!(!process_running(pid));
    fs::remove_file(state.join("pid")).unwrap();
    fs::write(&config, "baseline_timeout = 30").unwrap();
    let before = files(&fixture.root);
    let mut child = make_command("hang")
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while !state.join("pid").exists() {
        assert!(Instant::now() < deadline, "metadata did not start");
        assert!(
            child.try_wait().unwrap().is_none(),
            "check exited before metadata"
        );
        thread::sleep(Duration::from_millis(20));
    }
    assert_eq!(unsafe { libc::kill(child.id() as i32, libc::SIGTERM) }, 0);
    let deadline = Instant::now() + Duration::from_secs(10);
    while child.try_wait().unwrap().is_none() {
        if Instant::now() >= deadline {
            child.kill().unwrap();
            panic!("check did not stop");
        }
        thread::sleep(Duration::from_millis(20));
    }
    failure(child.wait_with_output().unwrap(), "interrupted");
    let pid: i32 = fs::read_to_string(state.join("pid"))
        .unwrap()
        .parse()
        .unwrap();
    assert!(!process_running(pid));
    for name in ["workspace", "target", "tmpdir"] {
        let path = PathBuf::from(fs::read_to_string(state.join(name)).unwrap());
        assert!(
            path.starts_with(fixture.temp.path().join("owned")),
            "{path:?}"
        );
        assert!(!path.exists(), "{path:?} leaked");
    }
    assert_eq!(files(&fixture.root), before);
    fixture.preserved();
    assert_eq!(
        fs::read_dir(fixture.temp.path().join("reports"))
            .unwrap()
            .count(),
        1
    );
}
