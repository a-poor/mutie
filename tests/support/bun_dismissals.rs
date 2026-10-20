use super::*;

fn dismiss(root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_mutie"))
        .args(["dismiss", "--module"])
        .arg(root)
        .args(["--lang", "bun"])
        .args(args)
        // Review CRUD and rediscovery must work without a Bun executable.
        .env("PATH", "")
        .output()
        .unwrap()
}
fn success(output: Output) -> String {
    assert!(output.status.success(), "{output:?}");
    String::from_utf8(output.stdout).unwrap()
}

#[test]
#[ignore = "requires pinned Bun; independent JS/TS dismissal CRUD, policies and error precedence"]
fn review_preserves_scores_sources_and_saved_history() {
    for syntax in ["js", "ts"] {
        let (temp, root) = fixture(syntax);
        let reports = temp.path().join("reports");
        let source = root.join(format!("src/flags.{syntax}"));
        let original_source = fs::read(&source).unwrap();
        let output = run(&root, &reports, &["--fail-on-missed"]);
        assert_eq!(output.status.code(), Some(3), "{output:?}");
        let (history, original, _, outcomes) = report(&output);
        assert_eq!(outcomes["summary"]["killed"], 1);
        assert_eq!(outcomes["summary"]["missed"], 1);
        let history_bytes: Vec<_> = ["run.json", "mutants.json", "outcomes.json", "summary.txt"]
            .into_iter()
            .map(|name| (name, fs::read(history.join(name)).unwrap()))
            .collect();
        let add = [
            "add",
            history.to_str().unwrap(),
            "--id",
            "2",
            "--reason",
            "accepted fixture gap",
            "--note",
            "reviewed fallback",
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
            &[
                "update",
                "1",
                "--reason",
                "reviewed fixture gap",
                "--clear-note",
            ],
        ));
        let listed = success(dismiss(&root, &["list"]));
        assert!(
            listed.contains("Language: Bun") && listed.contains("reviewed fixture gap"),
            "{listed}"
        );
        assert!(success(dismiss(&root, &["check"])).contains("1 matched; 0 require review"));
        success(dismiss(&root, &["remove", "1"]));
        assert!(success(dismiss(&root, &["list"])).contains("0 dismissal(s)"));
        success(dismiss(&root, &add));
        fs::write(
            root.join("mutie.toml"),
            "lang = 'bun'\nfail_on_missed = true\nmin_score = 100\n",
        )
        .unwrap();
        let output = run(&root, &reports, &[]);
        assert_eq!(output.status.code(), Some(3), "{output:?}");
        let (reviewed_dir, reviewed, inventory, reviewed_outcomes) = report(&output);
        assert_eq!(reviewed_outcomes["summary"], outcomes["summary"]);
        assert_eq!(
            reviewed["score"]["percentage"],
            original["score"]["percentage"]
        );
        assert_eq!(reviewed["policy"]["status"], "passed");
        assert_eq!(reviewed["policy"]["dismissed_missed"], 1);
        assert_eq!(reviewed["score_policy"]["status"], "failed");
        assert_eq!(reviewed["score"]["dismissed_in_score"], 1);
        assert_eq!(
            inventory["dismissals"]["matched"][0]["dismissal"]["language"],
            "bun"
        );
        let browser = Command::new(env!("CARGO_BIN_EXE_mutie"))
            .arg("report")
            .arg(reviewed_dir)
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
                history.to_str().unwrap(),
                "--clear-min-score",
            ],
        );
        assert!(output.status.success(), "{output:?}");
        let (_, focused, inventory, focused_outcomes) = report(&output);
        assert_eq!(focused_outcomes["summary"]["missed"], 1);
        assert_eq!(focused["score"]["percentage"], 0.0);
        assert_eq!(focused["score_policy"]["status"], "disabled");
        assert_eq!(focused["policy"]["status"], "passed");
        assert_eq!(inventory["dismissals"]["matched"][0]["mutation_id"], 1);
        let mut changed = original_source.clone();
        changed.extend_from_slice(b"\n// changed source\n");
        fs::write(&source, changed).unwrap();
        let check = dismiss(&root, &["check"]);
        assert!(!check.status.success());
        assert!(
            String::from_utf8_lossy(&check.stdout).contains("stale (source fingerprint changed)")
        );
        let output = run(&root, &reports, &["--clear-min-score"]);
        assert_eq!(output.status.code(), Some(3), "{output:?}");
        let (_, stale, inventory, _) = report(&output);
        assert_eq!(stale["policy"]["undismissed_missed"], 1);
        assert_eq!(
            inventory["dismissals"]["needs_review"][0]["status"],
            "stale"
        );
        fs::write(&source, &original_source).unwrap();
        let test = root.join(format!("test/flags.test.{syntax}"));
        let original_test = fs::read_to_string(&test).unwrap();
        fs::write(
            &test,
            format!(
                "{original_test}\ntest('empty boundary',()=>expect(fallback(' ')).toBe(false));"
            ),
        )
        .unwrap();
        let output = run(
            &root,
            &reports,
            &["--rerun-missed", history.to_str().unwrap()],
        );
        assert!(output.status.success(), "{output:?}");
        let (_, stronger, inventory, stronger_outcomes) = report(&output);
        assert_eq!(stronger_outcomes["summary"]["killed"], 1);
        assert_eq!(stronger["score"]["percentage"], 100.0);
        assert_eq!(stronger["policy"]["status"], "passed");
        assert_eq!(stronger["score_policy"]["status"], "passed");
        assert_eq!(
            inventory["dismissals"]["matched"].as_array().unwrap().len(),
            1
        );
        // A dismissed mutation causing cleanup failure must still invalidate both policies.
        fs::write(&test, format!("{original_test}\nimport {{afterEach}} from 'bun:test'; afterEach(()=>{{if(fallback(' ')!==false)throw new Error('cleanup')}});")).unwrap();
        let output = run(
            &root,
            &reports,
            &["--rerun-missed", history.to_str().unwrap()],
        );
        assert_eq!(output.status.code(), Some(1), "{output:?}");
        let (_, failed, inventory, failed_outcomes) = report(&output);
        assert_eq!(failed["status"], "failed");
        assert_eq!(failed["policy"]["status"], "not_evaluated");
        assert_eq!(failed["score_policy"]["status"], "not_evaluated");
        assert!(failed["score"]["percentage"].is_null());
        assert_eq!(failed_outcomes["summary"]["errors"], 1);
        assert_eq!(failed_outcomes["summary"]["killed"], 0);
        assert_eq!(
            inventory["dismissals"]["matched"].as_array().unwrap().len(),
            1
        );
        assert_eq!(fs::read(&source).unwrap(), original_source);
        for (name, bytes) in history_bytes {
            assert_eq!(fs::read(history.join(name)).unwrap(), bytes);
        }
    }
}
