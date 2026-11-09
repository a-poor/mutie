use super::*;

#[test]
#[ignore = "runs Cargo assignments with ownership/inference failures, cache parity, review and reruns"]
fn assignments_preserve_scores_review_and_source_identity() {
    let fixture = Fixture::from_fixture("rust-assignments");
    fs::write(
        fixture.root.join("mutie.toml"),
        "[rust]\noperators=['assignment_remove']\n",
    )
    .unwrap();
    let original = files(&fixture.root);
    let listing = fixture.run(&["--list"]);
    assert!(listing.status.success(), "{listing:?}");
    assert!(String::from_utf8_lossy(&listing.stdout).contains("Found 4 mutations"));
    let expected =
        serde_json::json!({"killed":1,"missed":1,"timeout":0,"build_failed":2,"errors":0});
    let serial = fixture.run(&["--jobs", "1", "--min-score", "100"]);
    assert_eq!(serial.status.code(), Some(3), "{serial:?}");
    let (directory, run, inventory, outcomes) = report(&serial);
    assert_eq!(run["status"], "completed");
    assert_eq!(run["score"]["percentage"], 50.0);
    assert_eq!(outcomes["summary"], expected);
    assert_eq!(files(&fixture.root), original);
    let saved = files(&directory);
    let added = dismissals::command(
        &fixture,
        &[
            "add",
            directory.to_str().unwrap(),
            "--id",
            "2",
            "--reason",
            "pending stronger test",
        ],
        false,
    )
    .output()
    .unwrap();
    assert!(added.status.success(), "{added:?}");
    let reviewed = files(&fixture.root);
    let cached = fixture.run(&[
        "--jobs",
        "2",
        "--rust-build-cache",
        "worker",
        "--fail-on-missed",
    ]);
    assert!(cached.status.success(), "{cached:?}");
    let (cached_dir, cached_run, cached_inventory, cached_outcomes) = report(&cached);
    let cached_saved = files(&cached_dir);
    assert_eq!(cached_outcomes["summary"], expected);
    assert_eq!(cached_run["score"]["percentage"], 50.0);
    assert_eq!(cached_run["score"]["dismissed_in_score"], 1);
    assert_eq!(
        cached_inventory["dismissals"]["matched"][0]["mutation_id"],
        2
    );
    for (a, b) in outcomes["mutations"]
        .as_array()
        .unwrap()
        .iter()
        .zip(cached_outcomes["mutations"].as_array().unwrap())
    {
        assert_eq!(a["outcome"], b["outcome"]);
    }
    for m in inventory["mutations"].as_array().unwrap() {
        assert_eq!(m["operator"], "assignment_remove");
        assert_eq!(m["replacement"], "{}");
    }
    assert_eq!(files(&fixture.root), reviewed);

    let stronger = Fixture::from_fixture("rust-assignments");
    for file in ["mutie.toml", "mutie.dismissals.toml"] {
        fs::copy(fixture.root.join(file), stronger.root.join(file)).unwrap();
    }
    fs::write(
        stronger.root.join("tests/stronger.rs"),
        "use mutie_rust_assignments::*;\n#[test] fn stored_item() { assert_eq!(weak([0]),[9]); }\n",
    )
    .unwrap();
    let stronger_inputs = files(&stronger.root);
    let rerun = stronger.run(&[
        "--rerun-missed",
        directory.to_str().unwrap(),
        "--rust-build-cache",
        "worker",
        "--min-score",
        "100",
    ]);
    assert!(rerun.status.success(), "{rerun:?}");
    let (_, focused, selected, result) = report(&rerun);
    assert_eq!(
        focused["rerun"]["mutations"],
        serde_json::json!([{"id":1,"source_id":2}])
    );
    assert_eq!(selected["dismissals"]["matched"][0]["mutation_id"], 1);
    assert_eq!(result["summary"]["killed"], 1);
    assert_eq!(result["summary"]["missed"], 0);
    assert_eq!(files(&stronger.root), stronger_inputs);

    let corrupt = fixture.temp.path().join("corrupt");
    for (name, bytes) in &saved {
        let path = corrupt.join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, bytes).unwrap();
    }
    let reject = |value: &Value| {
        fs::write(
            corrupt.join("mutants.json"),
            serde_json::to_vec(value).unwrap(),
        )
        .unwrap();
        let output = fixture.run(&["--rerun", corrupt.to_str().unwrap(), "--id", "1", "--list"]);
        assert!(!output.status.success(), "{output:?}");
        output
    };
    for (key, value) in [
        ("replacement", ";"),
        ("operator", "call_remove"),
        ("file", "src/lib.go"),
    ] {
        let mut bad = inventory.clone();
        bad["mutations"][0][key] = value.into();
        reject(&bad);
    }
    let source = fs::read_to_string(fixture.root.join("src/lib.rs")).unwrap();
    let text = "state.value = 99;";
    let start = source.find(text).unwrap();
    let before = &source[..start];
    let line = before.bytes().filter(|b| *b == b'\n').count() + 1;
    let column = before.rsplit('\n').next().unwrap().len() + 1;
    let mut bad = inventory.clone();
    let m = &mut bad["mutations"][0];
    m["original"] = text.into();
    m["byte_start"] = start.into();
    m["byte_end"] = (start + text.len()).into();
    m["line"] = line.into();
    m["end_line"] = line.into();
    m["column"] = column.into();
    m["end_column"] = (column + text.len()).into();
    let rejected = reject(&bad);
    assert!(
        String::from_utf8_lossy(&rejected.stderr)
            .contains("no longer matches a discovered mutation"),
        "{rejected:?}"
    );
    fs::write(
        stronger.root.join("src/lib.rs"),
        format!("{source}\n// source drift\n"),
    )
    .unwrap();
    let stale = stronger.run(&["--rerun-missed", directory.to_str().unwrap(), "--list"]);
    assert!(!stale.status.success(), "{stale:?}");
    assert!(
        String::from_utf8_lossy(&stale.stderr).contains("source fingerprint changed"),
        "{stale:?}"
    );
    assert_eq!(files(&fixture.root), reviewed);
    assert_eq!(files(&directory), saved);
    assert_eq!(files(&cached_dir), cached_saved);
}

#[test]
#[ignore = "runs genuine Cargo timeout and capture-error assignment mutants"]
fn assignments_keep_timeouts_and_capture_errors_separate() {
    for (effect, expected) in [
        (
            "loop { std::thread::sleep(std::time::Duration::from_millis(1)); }",
            "timeout",
        ),
        (
            "use std::io::Write; let mut out=std::io::stdout().lock(); for _ in 0..2048 { out.write_all(&[b'x';2048]).unwrap(); }",
            "errors",
        ),
    ] {
        let fixture = Fixture::from_fixture("rust-assignments");
        fs::write(
            fixture.root.join("src/lib.rs"),
            "pub struct State{pub value:u8}\npub fn initialize(s:&mut State){s.value=1;}\n",
        )
        .unwrap();
        fs::write(fixture.root.join("tests/public.rs"),format!("use mutie_rust_assignments::*;\n#[test] fn initialized(){{let mut s=State{{value:0}};initialize(&mut s);if s.value==0 {{{effect}}}}}\n")).unwrap();
        fs::write(
            fixture.root.join("mutie.toml"),
            "[rust]\noperators=['assignment_remove']\n",
        )
        .unwrap();
        let original = files(&fixture.root);
        let out = fixture.run(&["--timeout", "5", "--min-score", "100"]);
        let (_, saved, _, outcomes) = report(&out);
        assert_eq!(outcomes["summary"][expected], 1, "{out:?}");
        assert_eq!(outcomes["summary"]["killed"], 0);
        if expected == "errors" {
            assert_eq!(out.status.code(), Some(1), "{out:?}");
            assert_eq!(saved["status"], "failed");
            assert!(saved["score"]["percentage"].is_null());
        }
        assert_eq!(files(&fixture.root), original);
    }
}
