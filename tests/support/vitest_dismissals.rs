use super::*;

fn dismiss(root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_mutie"))
        .args(["dismiss", "--module"])
        .arg(root)
        .args(["--lang", "vitest"])
        .args(args)
        // CRUD and source rediscovery must not invoke Node, pnpm or Vitest.
        .env("PATH", "")
        .output()
        .unwrap()
}
fn success(output: Output) -> String {
    assert!(output.status.success(), "{output:?}");
    String::from_utf8(output.stdout).unwrap()
}

#[test]
#[ignore = "requires prepared MUTIE_VITEST_FIXTURE; real review and CI policy workflows"]
fn vitest_review_preserves_outcomes_scores_and_error_precedence() {
    let prepared =
        PathBuf::from(std::env::var_os("MUTIE_VITEST_FIXTURE").expect("set MUTIE_VITEST_FIXTURE"));
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("review project");
    assert!(
        Command::new("cp")
            .arg("-R")
            .arg(prepared)
            .arg(&root)
            .status()
            .unwrap()
            .success()
    );
    let reports = temp.path().join("reports");
    let output = run(
        &root,
        &reports,
        &["--include", "src/flags.js", "--fail-on-missed"],
    );
    assert_eq!(output.status.code(), Some(3), "{output:?}");
    let (source, original_run, _, original_outcomes) = report(&output);
    assert_eq!(original_run["status"], "completed");
    assert_eq!(original_run["policy"]["undismissed_missed"], 1);
    let run_bytes = fs::read(source.join("run.json")).unwrap();
    let outcome_bytes = fs::read(source.join("outcomes.json")).unwrap();
    let original_source = fs::read(root.join("src/flags.js")).unwrap();
    let add = [
        "add",
        source.to_str().unwrap(),
        "--id",
        "3",
        "--reason",
        "accepted gap",
        "--note",
        "reviewed JS fallback",
    ];
    assert!(success(dismiss(&root, &add)).contains("Added dismissal 1"));
    let metadata = root.join("mutie.dismissals.toml");
    let before = fs::read(&metadata).unwrap();
    let duplicate = dismiss(&root, &add);
    assert!(!duplicate.status.success());
    assert!(String::from_utf8_lossy(&duplicate.stderr).contains("already has dismissal"));
    assert_eq!(fs::read(&metadata).unwrap(), before);
    success(dismiss(
        &root,
        &["update", "1", "--reason", "reviewed gap", "--clear-note"],
    ));
    let listed = success(dismiss(&root, &["list"]));
    assert!(listed.contains("Language: Vitest") && listed.contains("reviewed gap"));
    assert!(success(dismiss(&root, &["check"])).contains("1 matched; 0 require review"));
    success(dismiss(&root, &["remove", "1"]));
    assert!(success(dismiss(&root, &["list"])).contains("0 dismissal(s)"));
    success(dismiss(&root, &add));
    // Configured policies apply to Vitest; CLI overrides retain shared semantics.
    fs::write(
        root.join("mutie.toml"),
        "lang = 'vitest'\nfail_on_missed = true\nmin_score = 100\n",
    )
    .unwrap();
    let output = run(&root, &reports, &["--include", "src/flags.js"]);
    assert_eq!(output.status.code(), Some(3), "{output:?}");
    let (reviewed_dir, reviewed, inventory, outcomes) = report(&output);
    assert_eq!(outcomes["summary"], original_outcomes["summary"]);
    assert_eq!(
        reviewed["score"]["percentage"],
        original_run["score"]["percentage"]
    );
    assert_eq!(reviewed["policy"]["status"], "passed");
    assert_eq!(reviewed["policy"]["dismissed_missed"], 1);
    assert_eq!(reviewed["score_policy"]["status"], "failed");
    assert_eq!(reviewed["score"]["dismissed_in_score"], 1);
    assert_eq!(
        inventory["dismissals"]["matched"][0]["dismissal"]["language"],
        "vitest"
    );
    let browser = Command::new(env!("CARGO_BIN_EXE_mutie"))
        .arg("report")
        .arg(&reviewed_dir)
        .output()
        .unwrap();
    assert!(
        String::from_utf8_lossy(&browser.stderr).contains("requires interactive terminal"),
        "{browser:?}"
    );
    let output = run(
        &root,
        &reports,
        &[
            "--rerun-missed",
            source.to_str().unwrap(),
            "--clear-min-score",
        ],
    );
    assert!(output.status.success(), "{output:?}");
    let (_, focused, inventory, outcomes) = report(&output);
    assert_eq!(outcomes["summary"]["missed"], 1);
    assert_eq!(focused["score"]["percentage"], 0.0);
    assert_eq!(focused["score_policy"]["status"], "disabled");
    assert_eq!(focused["policy"]["status"], "passed");
    assert_eq!(inventory["dismissals"]["matched"][0]["mutation_id"], 1);
    // Review decisions are source identities, not permanent waivers for a line.
    let mut edited = original_source.clone();
    edited.extend_from_slice(b"\n// source changed\n");
    fs::write(root.join("src/flags.js"), edited).unwrap();
    let stale_check = dismiss(&root, &["check"]);
    assert!(!stale_check.status.success());
    assert!(
        String::from_utf8_lossy(&stale_check.stdout).contains("stale (source fingerprint changed)")
    );
    let output = run(
        &root,
        &reports,
        &["--include", "src/flags.js", "--clear-min-score"],
    );
    assert_eq!(output.status.code(), Some(3), "{output:?}");
    let (_, stale, inventory, outcomes) = report(&output);
    assert_eq!(outcomes["summary"]["missed"], 1);
    assert_eq!(stale["policy"]["undismissed_missed"], 1);
    assert_eq!(
        inventory["dismissals"]["needs_review"][0]["status"],
        "stale"
    );
    fs::write(root.join("src/flags.js"), &original_source).unwrap();
    let test = root.join("test/flags.js.test.js");
    let original_test = fs::read_to_string(&test).unwrap();
    fs::write(
        &test,
        format!("{original_test}\ntest('stronger',()=>expect(fallback(false)).toBe(false));"),
    )
    .unwrap();
    let output = run(
        &root,
        &reports,
        &["--rerun-missed", source.to_str().unwrap()],
    );
    assert!(output.status.success(), "{output:?}");
    let (_, stronger, inventory, outcomes) = report(&output);
    assert_eq!(outcomes["summary"]["killed"], 1);
    assert_eq!(stronger["score"]["percentage"], 100.0);
    assert_eq!(stronger["policy"]["status"], "passed");
    assert_eq!(stronger["score_policy"]["status"], "passed");
    assert_eq!(
        inventory["dismissals"]["matched"].as_array().unwrap().len(),
        1
    );
    assert_eq!(
        fs::read(root.join("src/flags.js")).unwrap(),
        original_source
    );
    assert_eq!(fs::read(source.join("run.json")).unwrap(), run_bytes);
    assert_eq!(
        fs::read(source.join("outcomes.json")).unwrap(),
        outcome_bytes
    );
    // Exercise TypeScript review independently; JS decisions cannot dismiss it.
    let output = run(&root, &reports, &["--include", "src/flags.ts"]);
    assert_eq!(output.status.code(), Some(3), "{output:?}");
    let (ts_source, ts_run, _, _) = report(&output);
    assert_eq!(ts_run["policy"]["undismissed_missed"], 1);
    success(dismiss(
        &root,
        &[
            "add",
            ts_source.to_str().unwrap(),
            "--id",
            "3",
            "--reason",
            "reviewed TS gap",
        ],
    ));
    let output = run(
        &root,
        &reports,
        &[
            "--rerun-missed",
            ts_source.to_str().unwrap(),
            "--clear-min-score",
        ],
    );
    assert!(output.status.success(), "{output:?}");
    let (_, ts_run, inventory, outcomes) = report(&output);
    assert_eq!(outcomes["summary"]["missed"], 1);
    assert_eq!(ts_run["policy"]["dismissed_missed"], 1);
    assert_eq!(ts_run["score"]["percentage"], 0.0);
    assert_eq!(
        inventory["dismissals"]["matched"][0]["dismissal"]["mutation"]["file"],
        "src/flags.ts"
    );
    // Infrastructure errors take precedence over both configured policies.
    fs::write(
        root.join("node_modules/vitest/vitest.mjs"),
        "process.exit(0);",
    )
    .unwrap();
    let output = run(&root, &reports, &[]);
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let (_, failed, _, _) = report(&output);
    assert_eq!(failed["status"], "failed");
    assert_eq!(failed["policy"]["status"], "not_evaluated");
    assert_eq!(failed["score_policy"]["status"], "not_evaluated");
}
