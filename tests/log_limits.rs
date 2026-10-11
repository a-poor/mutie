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
        let temp = tempfile::tempdir_in(std::env::temp_dir().canonicalize().unwrap()).unwrap();
        let root = temp.path().join("module");
        let bin = temp.path().join("bin");
        let captures = temp.path().join("captures");
        for path in [&root, &bin, &captures] {
            fs::create_dir(path).unwrap();
        }
        fs::write(root.join("go.mod"), "module example.com/logs\n").unwrap();
        fs::write(root.join("a.go"), "package p\nvar a = true\n").unwrap();
        let go = bin.join("go");
        fs::write(&go, r#"#!/bin/sh
if [ "$1" = list ]; then
    printf '{"Dir":"%s","GoFiles":["a.go"],"Ignored":"' "$PWD"
    awk 'BEGIN {for(i=0;i<2000;i++) printf "discovery-padding"}'
    printf '"}\n'
    exit 0
fi
noise() {
    awk 'BEGIN {for(i=0;i<1000;i++) print "{\"Action\":\"output\",\"Output\":\"diagnostic padding é界 0123456789\\n\"}"}'
}
printf '{"Action":"output","Output":"HEAD\\n"}\n'
printf 'STDERR-HEAD\n' >&2
noise
awk 'BEGIN {for(i=0;i<1000;i++) print "stderr padding 0123456789"}' >&2
code=0
if grep -q 'a = false' a.go || [ "$MODE" = baseline_error ]; then
    case "$MODE" in
        killed) printf '{"Action":"fail","Package":"p"}\n'; code=1 ;;
        build) printf '{"Action":"build-fail","ImportPath":"p"}\n'; code=1 ;;
        setup) printf '{"Action":"output","Output":"FAIL p [setup failed]\\n"}\n{"Action":"fail","Package":"p"}\n'; code=1 ;;
        tool|baseline_error) printf 'go tool failed in omitted middle\n' >&2; code=1 ;;
        timeout) sleep 30 ;;
        cancel) touch "$READY"; sleep 30 ;;
    esac
fi
noise
printf '{"Action":"output","Output":"TAIL\\n"}\n'
printf 'STDERR-TAIL\n' >&2
exit "$code"
"#).unwrap();
        fs::set_permissions(go, fs::Permissions::from_mode(0o755)).unwrap();
        Self {
            temp,
            root,
            captures,
        }
    }

    fn command(&self, mode: &str, args: &[&str]) -> Command {
        let mut paths = vec![self.temp.path().join("bin")];
        paths.extend(std::env::split_paths(
            &std::env::var_os("PATH").unwrap_or_default(),
        ));
        let mut command = Command::new(env!("CARGO_BIN_EXE_mutie"));
        command
            .arg(&self.root)
            .args(["--ui", "plain", "--timeout", "1"])
            .args(args)
            .env("PATH", std::env::join_paths(paths).unwrap())
            .env("MODE", mode)
            .env("READY", self.temp.path().join("ready"))
            .env("TMPDIR", &self.captures)
            .env("GOTMPDIR", &self.captures);
        command
    }

    fn run(&self, mode: &str, args: &[&str]) -> Output {
        let out = self.command(mode, args).output().unwrap();
        assert_eq!(
            fs::read_dir(&self.captures).unwrap().count(),
            0,
            "temporary captures/workspaces leaked: {out:?}"
        );
        assert_eq!(
            fs::read_to_string(self.root.join("a.go")).unwrap(),
            "package p\nvar a = true\n"
        );
        out
    }

    fn report(&self, output: &Output) -> (PathBuf, Value, Value) {
        let path = PathBuf::from(
            String::from_utf8_lossy(&output.stdout)
                .lines()
                .find_map(|l| l.strip_prefix("Saving report to "))
                .unwrap(),
        );
        let read = |name| serde_json::from_slice(&fs::read(path.join(name)).unwrap()).unwrap();
        let metadata = read("run.json");
        let outcomes = read("outcomes.json");
        (path, metadata, outcomes)
    }
}

#[test]
fn interrupted_noisy_commands_save_limited_logs_and_remove_temporary_captures() {
    use std::{
        process::Stdio,
        time::{Duration, Instant},
    };
    for signal in [libc::SIGINT, libc::SIGTERM] {
        let fixture = Fixture::new();
        let mut command = fixture.command("cancel", &["--max-log-bytes", "128"]);
        let child = command
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let start = Instant::now();
        while !fixture.temp.path().join("ready").exists() {
            assert!(
                start.elapsed() < Duration::from_secs(10),
                "command never became ready"
            );
            std::thread::sleep(Duration::from_millis(5));
        }
        // SAFETY: this is our live child's PID and a standard termination signal.
        assert_eq!(unsafe { libc::kill(child.id() as i32, signal) }, 0);
        let out = child.wait_with_output().unwrap();
        assert_eq!(out.status.code(), Some(1));
        let (path, metadata, outcomes) = fixture.report(&out);
        assert_eq!(metadata["status"], "interrupted");
        assert_eq!(outcomes["mutations"][0]["state"], "interrupted");
        let logs = &outcomes["mutations"][0]["logs"];
        assert!(
            logs["retention"]["stdout"]["omitted_bytes"]
                .as_u64()
                .unwrap()
                > 1000
        );
        let saved = fs::read_to_string(path.join(logs["text"].as_str().unwrap())).unwrap();
        assert!(saved.starts_with("HEAD\n"));
        assert!(saved.contains("Log truncated"));
        assert_eq!(fs::read_dir(&fixture.captures).unwrap().count(), 0);
    }
}

#[test]
fn capped_logs_preserve_outcomes_discovery_reruns_and_cleanup() {
    let fixture = Fixture::new();
    for (mode, outcome, code) in [
        ("missed", "missed", 0),
        ("killed", "killed", 0),
        ("build", "build_failed", 0),
        ("setup", "error", 1),
        ("tool", "error", 1),
        ("timeout", "timeout", 0),
    ] {
        let full = fixture.run(mode, &[]);
        let (full_path, _, full_outcomes) = fixture.report(&full);
        let limited = fixture.run(mode, &["--max-log-bytes", "128", "--verbose"]);
        assert_eq!(limited.status.code(), Some(code), "{mode}: {limited:?}");
        let (path, run, outcomes) = fixture.report(&limited);
        assert_eq!(run["effective_config"]["max_log_bytes"], 128);
        assert_eq!(outcomes["summary"], full_outcomes["summary"], "{mode}");
        assert_eq!(outcomes["mutations"][0]["outcome"], outcome);
        assert!(String::from_utf8_lossy(&limited.stdout).contains("Log truncated"));
        for (logs, original_logs) in [
            (
                &outcomes["baseline"]["logs"],
                &full_outcomes["baseline"]["logs"],
            ),
            (
                &outcomes["discovery"]["logs"],
                &full_outcomes["discovery"]["logs"],
            ),
            (
                &outcomes["mutations"][0]["logs"],
                &full_outcomes["mutations"][0]["logs"],
            ),
        ] {
            for kind in ["text", "stdout", "stderr"] {
                let saved = fs::read(path.join(logs[kind].as_str().unwrap())).unwrap();
                let original =
                    fs::read(full_path.join(original_logs[kind].as_str().unwrap())).unwrap();
                let r = &logs["retention"][kind];
                let total = r["total_bytes"].as_u64().unwrap();
                let head = r["head_bytes"].as_u64().unwrap();
                let tail = r["tail_bytes"].as_u64().unwrap();
                let omitted = r["omitted_bytes"].as_u64().unwrap();
                assert_eq!(total, head + tail + omitted);
                assert!(head + tail <= 128);
                assert!(saved.len() <= 300);
                // Discovery contains a different temporary root on each run.
                if logs != &outcomes["discovery"]["logs"] {
                    assert_eq!(total as usize, original.len());
                    assert!(saved.starts_with(&original[..head as usize]));
                    assert!(saved.ends_with(&original[original.len() - tail as usize..]));
                }
                assert_eq!(
                    String::from_utf8_lossy(&saved).contains("Log truncated"),
                    omitted > 0
                );
            }
        }
        let rerun = fixture.run(
            "missed",
            &[
                "--rerun",
                path.to_str().unwrap(),
                "--max-log-bytes",
                "1",
                "--list",
            ],
        );
        assert!(rerun.status.success(), "{rerun:?}");
        assert!(String::from_utf8_lossy(&rerun.stdout).contains("Selected 1 mutations"));
    }
}

#[test]
fn configured_limits_can_be_overridden_and_baseline_failures_keep_diagnostics() {
    let fixture = Fixture::new();
    fs::write(fixture.root.join("mutie.toml"), "max_log_bytes = 64\n").unwrap();
    let listing = fixture.run("missed", &["--list"]);
    assert!(listing.status.success());
    assert!(!fixture.root.join(".mutie").exists());
    for (args, limit) in [
        (vec![], Some(64)),
        (vec!["--max-log-bytes", "256"], Some(256)),
        (vec!["--clear-max-log-bytes"], None),
    ] {
        let output = fixture.run("baseline_error", &args);
        assert_eq!(output.status.code(), Some(1));
        let (path, run, outcomes) = fixture.report(&output);
        assert_eq!(run["status"], "failed");
        assert_eq!(
            run["effective_config"]["max_log_bytes"],
            serde_json::json!(limit)
        );
        assert_eq!(outcomes["baseline"]["outcome"], "failed");
        assert!(outcomes["mutations"].as_array().unwrap().is_empty());
        let log =
            fs::read_to_string(path.join(outcomes["baseline"]["logs"]["text"].as_str().unwrap()))
                .unwrap();
        assert_eq!(log.contains("Log truncated"), limit.is_some());
        assert!(log.starts_with("HEAD\n"));
        assert!(log.ends_with("STDERR-TAIL\n"));
    }
}
