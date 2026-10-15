use super::*;

#[test]
#[ignore = "runs real Cargo library mutations and stronger-test reruns"]
fn library_layouts_run_fresh_and_cached_and_rerun_stronger_tests() {
    for library_only in [false, true] {
        let fixture = Fixture::from_fixture("rust-library");
        if library_only {
            fs::remove_file(fixture.root.join("src/main.rs")).unwrap();
            fs::remove_file(fixture.root.join("tests/cli.rs")).unwrap();
        }
        let original = fs::read(fixture.root.join("src/lib.rs")).unwrap();
        let mut saved_directory = None;
        for cache in ["fresh", "worker"] {
            let output = fixture.run(&[
                "--rust-build-cache",
                cache,
                "--jobs",
                "1",
                "--min-score",
                "100",
            ]);
            assert_eq!(output.status.code(), Some(3), "{output:?}");
            let (directory, run, inventory, outcomes) = report(&output);
            assert_eq!(run["status"], "completed");
            assert_eq!(
                run["rust_scope"]["targets"].as_array().unwrap().len(),
                if library_only { 1 } else { 2 }
            );
            assert_eq!(inventory["mutations"].as_array().unwrap().len(), 2);
            assert_eq!(
                outcomes["summary"],
                serde_json::json!({"killed":1,"missed":1,"timeout":0,"build_failed":0,"errors":0})
            );
            assert_eq!(run["score"]["percentage"], 50.0);
            for mutation in outcomes["mutations"].as_array().unwrap() {
                let stdout = fs::read_to_string(
                    directory.join(mutation["logs"]["stdout"].as_str().unwrap()),
                )
                .unwrap();
                let rebuilt: Vec<Value> = stdout
                    .split("--- Cargo test execution ---")
                    .next()
                    .unwrap()
                    .lines()
                    .filter_map(|line| serde_json::from_str::<Value>(line).ok())
                    .filter(|event| {
                        event["reason"] == "compiler-artifact" && event["fresh"] == false
                    })
                    .collect();
                assert!(
                    rebuilt
                        .iter()
                        .any(|event| event["target"]["kind"] == serde_json::json!(["lib"])),
                    "{stdout}"
                );
                if !library_only {
                    assert!(
                        rebuilt
                            .iter()
                            .any(|event| event["target"]["kind"] == serde_json::json!(["bin"])),
                        "{stdout}"
                    );
                }
            }
            assert_eq!(fs::read(fixture.root.join("src/lib.rs")).unwrap(), original);
            fixture.preserved();
            saved_directory = Some(directory);
        }
        let directory = saved_directory.unwrap();
        let saved = fs::read(directory.join("outcomes.json")).unwrap();
        let tests = fixture.root.join("tests/api.rs");
        let stronger = format!(
            "{}\n#[test]\nfn preview_is_disabled() {{ assert!(!mutie_rust_library_fixture::preview_enabled()); }}\n",
            fs::read_to_string(&tests).unwrap()
        );
        fs::write(&tests, &stronger).unwrap();
        let output = fixture.run(&[
            "--rerun-missed",
            directory.to_str().unwrap(),
            "--rust-build-cache",
            "worker",
            "--min-score",
            "100",
        ]);
        assert!(output.status.success(), "{output:?}");
        assert_eq!(
            report(&output).3["summary"],
            serde_json::json!({"killed":1,"missed":0,"timeout":0,"build_failed":0,"errors":0})
        );
        assert_eq!(fs::read(fixture.root.join("src/lib.rs")).unwrap(), original);
        assert_eq!(fs::read_to_string(&tests).unwrap(), stronger);
        assert_eq!(fs::read(directory.join("outcomes.json")).unwrap(), saved);
        fixture.preserved();
    }
}

#[test]
#[ignore = "checks real Cargo metadata for library target configurations"]
fn library_metadata_matches_explicit_and_automatic_targets() {
    let fixture = Fixture::from_fixture("rust-library");
    let manifest = fixture.root.join("Cargo.toml");
    let original = fs::read_to_string(&manifest).unwrap();
    // Listing validates metadata without compiling, so custom names need no API edits.
    fs::rename(
        fixture.root.join("src/lib.rs"),
        fixture.root.join("src/api.rs"),
    )
    .unwrap();
    for crate_type in ["lib", "rlib"] {
        fs::write(&manifest, format!("{original}\n[lib]\nname = 'api'\npath = 'src/api.rs'\nedition = '2021'\ncrate-type = ['{crate_type}']\n[[bin]]\nname = 'custom'\npath = './src/main.rs'\n")).unwrap();
        let listing = fixture.run(&["--list"]);
        assert!(listing.status.success(), "{listing:?}");
        assert!(String::from_utf8_lossy(&listing.stdout).contains("Found 2 mutations"));
        fixture.preserved();
    }
    fs::create_dir(fixture.root.join("src/bin")).unwrap();
    fs::write(fixture.root.join("src/bin/extra.rs"), "fn main() {}\n").unwrap();
    let rejected = fixture.run(&["--list"]);
    assert!(!rejected.status.success());
    fixture.preserved();
}

fn acceptance_fixture(library_only: bool) -> Fixture {
    let fixture = Fixture::from_fixture("rust-library");
    if library_only {
        fs::remove_file(fixture.root.join("src/main.rs")).unwrap();
        fs::remove_file(fixture.root.join("tests/cli.rs")).unwrap();
    }
    fixture
}

fn review(fixture: &Fixture, args: &[&str]) -> Output {
    let output = dismissals::command(fixture, args, args.contains(&"check"))
        .output()
        .unwrap();
    fixture.preserved();
    output
}

#[test]
#[ignore = "runs library dismissal, policy, rediscovery and focused rerun acceptance"]
fn library_acceptance_review_preserves_raw_scores_and_saved_reports() {
    for library_only in [true, false] {
        let fixture = acceptance_fixture(library_only);
        if !library_only {
            let main = fixture.root.join("src/main.rs");
            fs::write(
                &main,
                format!(
                    "{}\nfn binary_gap() -> bool {{ false }}\n",
                    fs::read_to_string(&main).unwrap()
                ),
            )
            .unwrap();
        }
        let initial = fixture.run(&["--fail-on-missed", "--rust-build-cache", "worker"]);
        assert_eq!(initial.status.code(), Some(3), "{initial:?}");
        let (directory, _, _, initial_outcomes) = report(&initial);
        let saved = files(&directory);
        for id in if library_only {
            &["2"][..]
        } else {
            &["2", "3"][..]
        } {
            let added = review(
                &fixture,
                &[
                    "add",
                    directory.to_str().unwrap(),
                    "--id",
                    id,
                    "--reason",
                    "reviewed library gap",
                ],
            );
            assert!(added.status.success(), "{added:?}");
        }
        let checked = review(&fixture, &["check"]);
        assert!(checked.status.success(), "{checked:?}");
        assert!(
            String::from_utf8_lossy(&checked.stdout).contains(if library_only {
                "1 matched; 0 require review"
            } else {
                "2 matched; 0 require review"
            })
        );
        let reviewed = fixture.run(&[
            "--rerun-missed",
            directory.to_str().unwrap(),
            "--fail-on-missed",
            "--min-score",
            "100",
            "--rust-build-cache",
            "worker",
        ]);
        assert_eq!(reviewed.status.code(), Some(3), "{reviewed:?}");
        let (reviewed_directory, run, inventory, outcomes) = report(&reviewed);
        let reviewed_saved = files(&reviewed_directory);
        assert_eq!(run["policy"]["status"], "passed");
        assert_eq!(run["score_policy"]["status"], "failed");
        assert_eq!(run["score"]["percentage"], 0.0);
        assert_eq!(
            outcomes["summary"]["missed"],
            initial_outcomes["summary"]["missed"]
        );
        assert_eq!(
            inventory["dismissals"]["matched"].as_array().unwrap().len(),
            if library_only { 1 } else { 2 }
        );
        let tests = fixture.root.join("tests/api.rs");
        fs::write(&tests, format!("{}\n#[test]\nfn preview_disabled() {{ assert!(!mutie_rust_library_fixture::preview_enabled()); }}\n", fs::read_to_string(&tests).unwrap())).unwrap();
        assert!(review(&fixture, &["check"]).status.success());
        let strengthened = files(&fixture.root);
        let rerun = fixture.run(&[
            "--rerun-missed",
            directory.to_str().unwrap(),
            "--id",
            "2",
            "--fail-on-missed",
            "--min-score",
            "100",
            "--rust-build-cache",
            "worker",
        ]);
        assert!(rerun.status.success(), "{rerun:?}");
        let (_, run, _, outcomes) = report(&rerun);
        assert_eq!(outcomes["summary"]["killed"], 1);
        assert_eq!(run["score"]["percentage"], 100.0);
        assert_eq!(run["score"]["dismissed_in_score"], 1);
        assert_eq!(files(&fixture.root), strengthened);
        let library = fixture.root.join("src/lib.rs");
        fs::write(
            &library,
            format!(
                "{}\n// changed production source\n",
                fs::read_to_string(&library).unwrap()
            ),
        )
        .unwrap();
        let checked = review(&fixture, &["check"]);
        assert!(!checked.status.success(), "{checked:?}");
        assert!(String::from_utf8_lossy(&checked.stdout).contains("require review"));
        assert_eq!(files(&directory), saved);
        assert_eq!(files(&reviewed_directory), reviewed_saved);
        fixture.preserved();
    }
}

#[test]
#[ignore = "runs real Cargo library compile and baseline failure acceptance"]
fn library_acceptance_build_failures_are_not_kills() {
    use std::os::unix::fs::PermissionsExt;
    for library_only in [true, false] {
        let fixture = acceptance_fixture(library_only);
        let library = fixture.root.join("src/lib.rs");
        let original = fs::read_to_string(&library)
            .unwrap()
            .replace("    true\n", "    true // build-guard\n");
        fs::write(&library, &original).unwrap();
        let inputs = files(&fixture.root);
        let bin = fixture.temp.path().join("build-proxy");
        fs::create_dir(&bin).unwrap();
        let proxy = bin.join("cargo");
        // Inject an actual rustc error only into the isolated mutated copy. The
        // ordinary boolean flip is valid Rust; this controls classification.
        fs::write(
            &proxy,
            r#"#!/bin/sh
set -eu
if [ "$1" = test ]; then
  for arg in "$@"; do
    if [ "$arg" = --no-run ] && grep -q 'false // build-guard' src/lib.rs; then
      printf '\ncompile_error!("library mutation build failure");\n' >> src/lib.rs
    fi
  done
fi
exec "$MUTIE_LIBRARY_REAL_CARGO" "$@"
"#,
        )
        .unwrap();
        fs::set_permissions(&proxy, fs::Permissions::from_mode(0o755)).unwrap();
        let paths = std::iter::once(bin)
            .chain(std::env::split_paths(&std::env::var_os("PATH").unwrap()))
            .collect::<Vec<_>>();
        let output = fixture
            .command(&["--jobs", "1", "--rust-build-cache", "worker"])
            .env("PATH", std::env::join_paths(paths).unwrap())
            .env("MUTIE_LIBRARY_REAL_CARGO", env!("CARGO"))
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
        let (directory, run, _, outcomes) = report(&output);
        assert_eq!(run["status"], "completed");
        assert_eq!(
            outcomes["summary"],
            serde_json::json!({"killed":0,"missed":1,"timeout":0,"build_failed":1,"errors":0})
        );
        let log = fs::read_to_string(directory.join("logs/000001.log")).unwrap();
        assert!(log.contains("library mutation build failure"), "{log}");
        assert!(!log.contains("--- Cargo test execution ---"), "{log}");
        assert_eq!(files(&fixture.root), inputs);
        fixture.preserved();
        for (source, diagnostic) in [
            (
                format!("{original}\ncompile_error!(\"library baseline build failure\");\n"),
                "library baseline build failure",
            ),
            (
                original.replace("true // build-guard", "false // build-guard"),
                "test result: FAILED",
            ),
        ] {
            fs::write(&library, source).unwrap();
            let inputs = files(&fixture.root);
            let output = fixture.run(&["--fail-on-missed", "--min-score", "100"]);
            assert_eq!(output.status.code(), Some(1), "{output:?}");
            let (directory, run, inventory, outcomes) = report(&output);
            assert_eq!(run["status"], "failed");
            assert_eq!(outcomes["baseline"]["outcome"], "failed");
            assert_eq!(inventory["complete"], false);
            assert!(outcomes["mutations"].as_array().unwrap().is_empty());
            let log = fs::read_to_string(
                directory.join(outcomes["baseline"]["logs"]["text"].as_str().unwrap()),
            )
            .unwrap();
            assert!(log.contains(diagnostic), "{log}");
            assert_eq!(files(&fixture.root), inputs);
            fixture.preserved();
        }
    }
}

#[test]
#[ignore = "interrupts real library baseline and mutation tests and verifies descendant cleanup"]
fn library_acceptance_cancellation_preserves_partial_reports_and_inputs() {
    for library_only in [true, false] {
        for baseline in [true, false] {
            let fixture = acceptance_fixture(library_only);
            let tests = fixture.root.join("tests/api.rs");
            fs::write(
                &tests,
                format!(
                    "{}\n{}",
                    fs::read_to_string(&tests).unwrap(),
                    r##"
unsafe extern "C" { fn getpgrp() -> i32; }
#[test]
fn cancellation_gate() {
    if std::env::var_os("MUTIE_LIBRARY_BLOCK_BASELINE").is_none()
        && mutie_rust_library_fixture::enabled() { return; }
    let state = std::path::PathBuf::from(std::env::var_os("MUTIE_RUST_TEST_STATE").unwrap());
    std::fs::write(state.join("library.test.pid"), std::process::id().to_string()).unwrap();
    std::fs::write(state.join("library.group"), unsafe { getpgrp() }.to_string()).unwrap();
    let status = std::process::Command::new("sh").args(["-c", r#"
set -eu
state="$MUTIE_RUST_TEST_STATE"
printf '%s\n' "$$" > "$state/library.shell.pid"
sleep 300 &
printf '%s\n' "$!" > "$state/library.descendant.pid"
printf 'library cancellation diagnostic\n' >&2
touch "$state/library.ready"
wait
"#]).status().unwrap();
    panic!("blocking helper unexpectedly exited: {status}");
}
"##
                ),
            )
            .unwrap();
            let inputs = files(&fixture.root);
            let mut command = fixture.command(&[
                "--jobs",
                "1",
                "--timeout",
                "120",
                "--baseline-timeout",
                "120",
                "--rust-build-cache",
                "worker",
                "--fail-on-missed",
                "--min-score",
                "100",
            ]);
            if baseline {
                command.env("MUTIE_LIBRARY_BLOCK_BASELINE", "1");
            }
            let mut child = InterruptibleRun::start_command(&fixture, command);
            child.ready(&["library"]);
            let pids: Vec<i32> = fs::read_dir(&child.state)
                .unwrap()
                .map(|entry| entry.unwrap().path())
                .filter(|path| {
                    path.extension()
                        .is_some_and(|ext| ext == "pid" || ext == "group")
                })
                .map(|path| fs::read_to_string(path).unwrap().trim().parse().unwrap())
                .collect();
            assert_eq!(pids.len(), 4);
            assert!(pids.iter().all(|pid| process_running(*pid)));
            let output = child.interrupt(if baseline {
                libc::SIGTERM
            } else {
                libc::SIGINT
            });
            assert_eq!(output.status.code(), Some(1), "{output:?}");
            let (directory, run, inventory, outcomes) = report(&output);
            assert_eq!(run["status"], "interrupted");
            assert_eq!(run["checkpoint"], outcomes["checkpoint"]);
            assert_eq!(run["policy"]["status"], "not_evaluated");
            assert_eq!(run["score_policy"]["status"], "not_evaluated");
            assert!(run["score"]["percentage"].is_null());
            let logs = if baseline {
                assert_eq!(inventory["complete"], false);
                assert_eq!(outcomes["baseline"]["outcome"], "interrupted");
                assert!(outcomes["mutations"].as_array().unwrap().is_empty());
                &outcomes["baseline"]["logs"]
            } else {
                assert_eq!(inventory["complete"], true);
                assert_eq!(outcomes["baseline"]["outcome"], "passed");
                assert_eq!(outcomes["mutations"][0]["state"], "interrupted");
                assert!(outcomes["mutations"][0]["outcome"].is_null());
                assert_eq!(outcomes["mutations"][1]["state"], "pending");
                &outcomes["mutations"][0]["logs"]
            };
            let diagnostic =
                fs::read_to_string(directory.join(logs["stderr"].as_str().unwrap())).unwrap();
            assert!(
                diagnostic.contains("library cancellation diagnostic"),
                "{diagnostic}"
            );
            let deadline = Instant::now() + Duration::from_secs(10);
            while pids.iter().any(|pid| process_running(*pid)) {
                assert!(
                    Instant::now() < deadline,
                    "library test descendants leaked: {pids:?}"
                );
                thread::sleep(Duration::from_millis(20));
            }
            assert_eq!(files(&fixture.root), inputs);
            fixture.preserved();
        }
    }
}
