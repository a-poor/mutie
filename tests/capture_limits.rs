#![cfg(unix)]

use serde_json::Value;
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::PathBuf,
    process::{Command, Output},
};

struct Fixture {
    temp: tempfile::TempDir,
    root: PathBuf,
    captures: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("module");
        let captures = temp.path().join("captures");
        let bin = temp.path().join("bin");
        for path in [&root, &captures, &bin] {
            fs::create_dir(path).unwrap();
        }
        fs::write(captures.join("keep"), "unrelated").unwrap();
        fs::write(root.join("go.mod"), "module example.com/captures\n").unwrap();
        fs::write(root.join("a.go"), "package p\nvar a = true\n").unwrap();
        let go = bin.join("go");
        fs::write(
            &go,
            r#"#!/bin/sh
set -eu
mkdir "$GOTMPDIR/build-leftover" "$TMPDIR/test-leftover"
printf 'test file' > "$TMPDIR/test-leftover/file"
noise() {
    awk 'BEGIN {for(i=0;i<10000;i++) printf "0123456789abcdef"}'
}
if [ "$1" = list ]; then
    if [ "$MODE" = discovery_noise ]; then noise; fi
    printf '{"Dir":"%s","GoFiles":["a.go"]}\n' "$PWD"
    exit 0
fi
printf '{"Action":"output","Output":"HEAD diagnostic\\n"}\n'
printf 'stderr diagnostic\n' >&2
if grep -q false a.go || [ "$MODE" = baseline_noise ]; then
    # A failure event before overflow must not turn an incomplete run into a kill.
    printf '{"Action":"fail","Package":"p"}\n'
    case "$MODE" in
        stdout|baseline_noise) noise ;;
        stderr) noise >&2 ;;
        mixed) noise & noise >&2; wait ;;
        killed) exit 1 ;;
    esac
fi
exit 0
"#,
        )
        .unwrap();
        fs::set_permissions(go, fs::Permissions::from_mode(0o755)).unwrap();
        Self {
            temp,
            root,
            captures,
        }
    }

    fn run(&self, mode: &str, args: &[&str]) -> Output {
        let mut paths = vec![self.temp.path().join("bin")];
        paths.extend(std::env::split_paths(
            &std::env::var_os("PATH").unwrap_or_default(),
        ));
        let output = Command::new(env!("CARGO_BIN_EXE_mutie"))
            .arg(&self.root)
            .args(["--ui", "plain", "--timeout", "5"])
            .args(args)
            .env("PATH", std::env::join_paths(paths).unwrap())
            .env("MODE", mode)
            .env("TMPDIR", &self.captures)
            .env("GOTMPDIR", &self.captures)
            .output()
            .unwrap();
        assert_eq!(
            fs::read_dir(&self.captures).unwrap().count(),
            1,
            "{output:?}"
        );
        assert_eq!(
            fs::read_to_string(self.captures.join("keep")).unwrap(),
            "unrelated"
        );
        assert_eq!(
            fs::read_to_string(self.root.join("a.go")).unwrap(),
            "package p\nvar a = true\n"
        );
        output
    }

    fn report(&self, output: &Output) -> (PathBuf, Value, Value) {
        let directory = PathBuf::from(
            String::from_utf8_lossy(&output.stdout)
                .lines()
                .find_map(|s| s.strip_prefix("Saving report to "))
                .expect("report path"),
        );
        let read = |name| serde_json::from_slice(&fs::read(directory.join(name)).unwrap()).unwrap();
        let run = read("run.json");
        let outcomes = read("outcomes.json");
        (directory, run, outcomes)
    }
}

#[test]
fn overflow_is_an_error_with_bounded_logs_and_unevaluated_findings_policies() {
    let fixture = Fixture::new();
    for mode in ["stdout", "stderr", "mixed"] {
        let output = fixture.run(
            mode,
            &[
                "--max-capture-bytes",
                "4096",
                "--max-log-bytes",
                "64",
                "--fail-on-missed",
                "--min-score",
                "100",
            ],
        );
        assert_eq!(output.status.code(), Some(1), "{output:?}");
        assert!(String::from_utf8_lossy(&output.stderr).contains("--max-capture-bytes"));
        let (path, run, outcomes) = fixture.report(&output);
        assert_eq!(run["status"], "failed");
        assert_eq!(run["effective_config"]["max_capture_bytes"], 4096);
        assert_eq!(run["policy"]["status"], "not_evaluated");
        assert_eq!(run["score_policy"]["status"], "not_evaluated");
        assert!(run["score"]["percentage"].is_null());
        assert_eq!(
            outcomes["summary"],
            serde_json::json!({"killed":0,"missed":0,"timeout":0,"build_failed":0,"errors":1})
        );
        let mutation = &outcomes["mutations"][0];
        assert!(
            mutation["error"]
                .as_str()
                .unwrap()
                .contains("captured output is incomplete")
        );
        let logs = &mutation["logs"];
        assert_eq!(
            logs["retention"]["stdout"]["total_bytes"].as_u64().unwrap()
                + logs["retention"]["stderr"]["total_bytes"].as_u64().unwrap(),
            4096
        );
        for kind in ["text", "stdout", "stderr"] {
            let r = &logs["retention"][kind];
            assert!(r["head_bytes"].as_u64().unwrap() + r["tail_bytes"].as_u64().unwrap() <= 64);
            assert!(path.join(logs[kind].as_str().unwrap()).is_file());
        }
        let saved = fs::read(path.join("run.json")).unwrap();
        let rerun = fixture.run(
            "killed",
            &[
                "--rerun",
                path.to_str().unwrap(),
                "--id",
                "1",
                "--max-capture-bytes",
                "4096",
            ],
        );
        assert!(rerun.status.success(), "{rerun:?}");
        let (_, _, rerun_outcomes) = fixture.report(&rerun);
        assert_eq!(rerun_outcomes["summary"]["killed"], 1);
        assert_eq!(fs::read(path.join("run.json")).unwrap(), saved);
    }
}

#[test]
fn baseline_discovery_and_listing_fail_explicitly_without_using_partial_output() {
    let fixture = Fixture::new();
    for (mode, phase) in [
        ("baseline_noise", "baseline"),
        ("discovery_noise", "discovery"),
    ] {
        let output = fixture.run(mode, &["--max-capture-bytes", "4096"]);
        assert_eq!(output.status.code(), Some(1), "{output:?}");
        let (path, run, outcomes) = fixture.report(&output);
        assert_eq!(run["status"], "failed");
        assert_eq!(outcomes[phase]["outcome"], "failed");
        assert!(
            outcomes[phase]["capture_error"]
                .as_str()
                .unwrap()
                .contains("--max-capture-bytes")
        );
        assert!(outcomes["mutations"].as_array().unwrap().is_empty());
        let text = fs::read_to_string(path.join(outcomes[phase]["logs"]["text"].as_str().unwrap()))
            .unwrap();
        assert!(text.contains("Output capture error"));
    }
    let fresh = Fixture::new();
    let output = fresh.run(
        "discovery_noise",
        &["--list", "--max-capture-bytes", "4096"],
    );
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(String::from_utf8_lossy(&output.stderr).contains("--max-capture-bytes"));
    assert!(!fresh.root.join(".mutie").exists());
}

#[test]
fn configured_capture_limits_can_be_overridden_or_cleared_independently_of_log_limits() {
    let fixture = Fixture::new();
    fs::write(
        fixture.root.join("mutie.toml"),
        "max_capture_bytes = 4096\nmax_log_bytes = 64\n",
    )
    .unwrap();
    for (args, code, cap) in [
        (vec![], 1, Some(4096)),
        (vec!["--max-capture-bytes", "500000"], 0, Some(500000)),
        (vec!["--clear-max-capture-bytes"], 0, None),
    ] {
        let output = fixture.run("stdout", &args);
        assert_eq!(output.status.code(), Some(code), "{output:?}");
        let (_, run, _) = fixture.report(&output);
        assert_eq!(
            run["effective_config"]["max_capture_bytes"],
            serde_json::json!(cap)
        );
        assert_eq!(run["effective_config"]["max_log_bytes"], 64);
    }
    for args in [
        vec!["--max-capture-bytes", "0"],
        vec!["--max-capture-bytes", "4096", "--clear-max-capture-bytes"],
    ] {
        assert_eq!(fixture.run("stdout", &args).status.code(), Some(2));
    }
}
