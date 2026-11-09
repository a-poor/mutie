#![cfg(unix)]

use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::{Command, Output},
};

use serde_json::{Value, json};

struct Fixture {
    temp: tempfile::TempDir,
    root: PathBuf,
    invocation: PathBuf,
}

impl Fixture {
    fn new(config: &str) -> Self {
        let temp = tempfile::tempdir_in(std::env::temp_dir().canonicalize().unwrap()).unwrap();
        let root = temp.path().join("project");
        let invocation = temp.path().join("invocation");
        let bin = temp.path().join("bin");
        for path in [&root, &invocation, &bin] {
            fs::create_dir(path).unwrap();
        }
        fs::write(root.join("go.mod"), "module example.com/test\n").unwrap();
        fs::write(root.join("a.go"), "package p\nvar a = true\n").unwrap();
        fs::write(root.join("b.go"), "package p\nvar b = false\n").unwrap();
        fs::write(root.join("mutie.toml"), config).unwrap();
        let go = bin.join("go");
        fs::write(
            &go,
            r#"#!/bin/sh
printf '%s\n' "$1" >> "$CALLS"
if [ -e reports/poison.go ]; then
    printf 'report directory leaked into workspace\n' >&2
    exit 1
fi
if [ "$1" = list ]; then
    printf '{"Dir":"%s","GoFiles":["a.go","b.go"]}\n' "$PWD"
else
    printf '{"Action":"output","Output":"test diagnostic\\n"}\n'
fi
"#,
        )
        .unwrap();
        fs::set_permissions(go, fs::Permissions::from_mode(0o755)).unwrap();
        Self {
            temp,
            root,
            invocation,
        }
    }

    fn command(&self) -> Command {
        let mut paths = vec![self.temp.path().join("bin")];
        paths.extend(std::env::split_paths(
            &std::env::var_os("PATH").unwrap_or_default(),
        ));
        let mut command = Command::new(env!("CARGO_BIN_EXE_mutie"));
        command
            .current_dir(&self.invocation)
            .env("PATH", std::env::join_paths(paths).unwrap())
            .env("CALLS", self.temp.path().join("calls"));
        command
    }

    fn run(&self, args: &[&str]) -> Output {
        self.command()
            .arg("--path")
            .arg(&self.root)
            .arg(args.first().copied().unwrap_or("run"))
            .args(args.iter().skip(1))
            .output()
            .unwrap()
    }
}

fn success(output: &Output) -> String {
    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn read(root: &Path, name: &str) -> Value {
    serde_json::from_slice(&fs::read(root.join(name)).unwrap()).unwrap()
}

fn report(output: &Output) -> (PathBuf, Value, Value) {
    let stdout = success(output);
    let path = PathBuf::from(
        stdout
            .lines()
            .find_map(|line| line.strip_prefix("Saving report to "))
            .unwrap(),
    );
    let metadata = read(&path, "run.json");
    let inventory = read(&path, "mutants.json");
    (path, metadata, inventory)
}

#[test]
fn root_help_and_old_commands_never_launch_tools_or_create_reports() {
    let fixture = Fixture::new("[go]\n");
    let output = Command::new(env!("CARGO_BIN_EXE_mutie"))
        .current_dir(&fixture.root)
        .output()
        .unwrap();
    let text = success(&output);
    assert!(text.contains("Usage: mutie") && text.contains("run"));
    for args in [
        vec![fixture.root.to_str().unwrap()],
        vec!["--list"],
        vec!["bun-list"],
        vec!["python-check"],
        vec!["vitest-list"],
        vec!["run", fixture.root.to_str().unwrap()],
        vec!["list", fixture.root.to_str().unwrap()],
        vec!["check", fixture.root.to_str().unwrap()],
        vec!["init", fixture.root.to_str().unwrap()],
        vec!["rerun", "saved", fixture.root.to_str().unwrap()],
    ] {
        assert!(
            !Command::new(env!("CARGO_BIN_EXE_mutie"))
                .args(args)
                .output()
                .unwrap()
                .status
                .success()
        );
    }
    assert!(!fixture.root.join(".mutie").exists());
    assert!(!fixture.temp.path().join("calls").exists());
}

#[test]
fn current_directory_and_global_path_resolve_the_same_nested_project() {
    let fixture = Fixture::new("output_dir='reports'\n[go]\npath='backend'\ninclude=['a.go']\n");
    let backend = fixture.root.join("backend");
    fs::create_dir(&backend).unwrap();
    for file in ["go.mod", "a.go", "b.go"] {
        fs::rename(fixture.root.join(file), backend.join(file)).unwrap();
    }
    let local = fixture
        .command()
        .current_dir(&fixture.root)
        .args(["list", "--json"])
        .output()
        .unwrap();
    let expected = success(&local);
    for args in [
        vec!["--path", "../project", "list", "--json"],
        vec!["list", "--path", "../project", "--json"],
    ] {
        assert_eq!(
            success(&fixture.command().args(args).output().unwrap()),
            expected
        );
    }
    let checked = fixture
        .command()
        .args(["check", "--path", "../project", "--json"])
        .output()
        .unwrap();
    success(&checked);
    assert_eq!(
        serde_json::from_slice::<Value>(&checked.stdout).unwrap()["passed"],
        true
    );
    let (directory, metadata, inventory) = report(
        &fixture
            .command()
            .current_dir(&fixture.root)
            .arg("run")
            .output()
            .unwrap(),
    );
    assert_eq!(metadata["module_root"], backend.to_str().unwrap());
    assert_eq!(directory.parent().unwrap(), fixture.root.join("reports"));
    assert_eq!(inventory["mutations"][0]["file"], "a.go");
}

#[test]
fn global_path_applies_to_report_retention_and_nested_dismissals() {
    let fixture = Fixture::new("[go]\ninclude=['a.go']\n");
    let (directory, _, _) = report(&fixture.run(&["run"]));
    let relative = directory
        .strip_prefix(&fixture.root)
        .unwrap()
        .to_str()
        .unwrap();
    success(
        &fixture
            .command()
            .args([
                "dismiss",
                "add",
                relative,
                "--id",
                "1",
                "--reason",
                "reviewed",
                "--path",
                "../project",
            ])
            .output()
            .unwrap(),
    );
    let listed = success(
        &fixture
            .command()
            .args(["--path", "../project", "dismiss", "list"])
            .output()
            .unwrap(),
    );
    assert!(listed.contains("reviewed"));
    let runs = success(
        &fixture
            .command()
            .args(["runs", "--path", "../project"])
            .output()
            .unwrap(),
    );
    assert!(runs.contains(directory.file_name().unwrap().to_str().unwrap()));
    success(
        &fixture
            .command()
            .args(["--path", "../project", "prune", "--keep-last", "0"])
            .output()
            .unwrap(),
    );
    assert!(directory.join("run.json").exists());
    assert!(!fixture.invocation.join(".mutie").exists());
}

#[test]
fn invalid_working_directory_fails_before_tools_but_bare_command_still_prints_help() {
    let fixture = Fixture::new("[go]\n");
    let output = fixture
        .command()
        .args(["--path", "missing", "check"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("opening working directory missing"));
    let help = fixture
        .command()
        .args(["--path", "missing"])
        .output()
        .unwrap();
    assert!(success(&help).contains("Usage: mutie"));
    assert!(!fixture.temp.path().join("calls").exists());
    assert!(!fixture.root.join(".mutie").exists());
}
#[test]
fn grouped_settings_drive_execution_and_cli_only_overrides_are_saved() {
    let fixture = Fixture::new(
        "jobs=4\ntimeout=4\noutput_dir='../reports'\n[go]\njobs=2\ninclude=['a.go']\n",
    );
    let before = fs::read(fixture.root.join("a.go")).unwrap();
    let output = fixture.run(&["run", "--verbose", "--plain"]);
    assert!(success(&output).contains("test diagnostic"));
    let (directory, metadata, inventory) = report(&output);
    let config = &metadata["effective_config"];
    assert_eq!(
        directory.parent().unwrap(),
        fixture.temp.path().join("reports")
    );
    assert_eq!(
        config["config_file"],
        fixture.root.join("mutie.toml").to_str().unwrap()
    );
    assert_eq!(config["jobs"], 2);
    assert_eq!(config["timeout"], 4);
    assert_eq!(config["include"], json!(["a.go"]));
    assert_eq!(config["ui"], "plain");
    assert_eq!(config["verbose"], true);
    assert_eq!(inventory["mutations"].as_array().unwrap().len(), 1);
    let (_, metadata, _) = report(&fixture.run(&["run", "--jobs", "1", "--timeout", "5"]));
    assert_eq!(metadata["effective_config"]["jobs"], 1);
    assert_eq!(metadata["effective_config"]["timeout"], 5);
    assert_eq!(metadata["effective_config"]["verbose"], false);
    assert_eq!(fs::read(fixture.root.join("a.go")).unwrap(), before);
}
#[test]
fn list_and_check_share_config_without_reports_or_mutations() {
    let fixture = Fixture::new("output_dir='reports'\n[go]\ninclude=['a.go']\n");
    let original = fs::read(fixture.root.join("a.go")).unwrap();
    let listing = success(&fixture.run(&["list"]));
    assert!(listing.contains("Found 1 mutations"));
    assert!(!listing.contains("Running baseline"));
    assert!(!fixture.root.join("reports").exists());
    fs::create_dir(fixture.root.join("reports")).unwrap();
    fs::write(fixture.root.join("reports/poison.go"), "must be excluded").unwrap();
    let checked = success(&fixture.run(&["check"]));
    assert!(checked.contains("Baseline passed"));
    assert!(!checked.contains("Saving report") && !checked.contains("Found "));
    assert_eq!(
        fs::read_to_string(fixture.temp.path().join("calls")).unwrap(),
        "list\ntest\n"
    );
    let output = fixture.run(&["list", "--json"]);
    success(&output);
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["mutations"].as_array().unwrap().len(), 1);
    let output = fixture.run(&["check", "--json"]);
    success(&output);
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["passed"], true);
    assert_eq!(
        fs::read_dir(fixture.root.join("reports")).unwrap().count(),
        1
    );
    assert_eq!(fs::read(fixture.root.join("a.go")).unwrap(), original);
}
#[test]
fn check_failure_is_an_error_and_never_saves_a_report() {
    let fixture = Fixture::new("[go]\n");
    fs::write(
        fixture.temp.path().join("bin/go"),
        "#!/bin/sh\necho baseline-failure >&2\nexit 1\n",
    )
    .unwrap();
    let output = fixture.run(&["check"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("baseline-failure"));
    assert!(!fixture.root.join(".mutie").exists());
}
#[test]
fn invalid_settings_fail_before_any_execution() {
    let fixture = Fixture::new("");
    for (config, expected) in [
        ("jobs=0", "mutie.toml"),
        ("lang='go'", "unknown field"),
        ("ui='plain'", "unknown field"),
        ("[go]\nverbose=true", "unknown field"),
        ("[go]\ninclude=['[']", "invalid glob"),
        ("output_dir='.'", "module root"),
        ("[rust]\n", "Cargo.toml"),
        ("[go]\noperators=['unknown']", "unknown variant"),
        ("[go]\n[js]\nrunner='vitest'", "multiple languages"),
    ] {
        fs::write(fixture.root.join("mutie.toml"), config).unwrap();
        let output = fixture.run(&["run"]);
        assert!(!output.status.success(), "{config}");
        let error = String::from_utf8_lossy(&output.stderr);
        assert!(error.contains(expected), "{config}: {error}");
        assert!(!fixture.temp.path().join("calls").exists());
        assert!(!fixture.root.join(".mutie").exists());
    }
}
#[test]
fn operator_and_policy_settings_live_in_config() {
    let fixture = Fixture::new(
        "min_score=100\nfail_on_missed=true\n[go]\ninclude=['a.go']\noperators=['boolean_flip']\n",
    );
    let output = fixture.run(&["run"]);
    assert_eq!(output.status.code(), Some(3));
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(text.contains("missed") && text.contains("Saving report to"));
    for args in [
        vec!["run", "--operator", "boolean_flip"],
        vec!["run", "--include", "a.go"],
        vec!["run", "--fail-on-missed"],
        vec!["run", "--clear-timeout"],
        vec!["run", "--rust-package", "core"],
    ] {
        assert!(!fixture.run(&args).status.success());
    }
}
#[test]
fn explicit_rerun_selects_saved_ids_and_preserves_the_source_report() {
    let fixture = Fixture::new("[go]\ninclude=['a.go']\n");
    let (directory, _, _) = report(&fixture.run(&["run"]));
    let original = fs::read(directory.join("outcomes.json")).unwrap();
    // Saved run paths are relative to the selected working directory.
    let mut paths = vec![fixture.temp.path().join("bin")];
    paths.extend(std::env::split_paths(
        &std::env::var_os("PATH").unwrap_or_default(),
    ));
    let invoke = |extra: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_mutie"))
            .arg("rerun")
            .arg(directory.strip_prefix(&fixture.root).unwrap())
            .arg("--path")
            .arg(&fixture.root)
            .args(["--id", "1"])
            .args(extra)
            .env("PATH", std::env::join_paths(&paths).unwrap())
            .env("CALLS", fixture.temp.path().join("calls"))
            .output()
            .unwrap()
    };
    let text = success(&invoke(&["--dry-run"]));
    assert!(text.contains("source [000001]"));
    assert!(!text.contains("Saving report"));
    let (_, metadata, inventory) = report(&invoke(&["--missed"]));
    assert_eq!(metadata["effective_config"]["rerun"]["selection"], "missed");
    assert_eq!(inventory["mutations"].as_array().unwrap().len(), 1);
    assert_eq!(fs::read(directory.join("outcomes.json")).unwrap(), original);
}
#[test]
fn mixed_config_selects_one_project_and_rejects_disabled_languages() {
    let fixture = Fixture::new("[go]\n[rust]\npath='missing'\n");
    assert!(!fixture.run(&["list"]).status.success());
    assert!(!fixture.temp.path().join("calls").exists());
    success(&fixture.run(&["list", "--lang", "go"]));
    fs::write(
        fixture.root.join("mutie.toml"),
        "[go]\n[rust]\nenabled=false\npath='missing'\n",
    )
    .unwrap();
    success(&fixture.run(&["list"]));
    assert!(!fixture.run(&["list", "--lang", "rust"]).status.success());
}
#[test]
fn alternate_config_paths_are_relative_to_that_file() {
    let fixture = Fixture::new("this = 'must not be read'");
    let config = fixture.temp.path().join("ci.toml");
    fs::write(
        &config,
        "output_dir='ci-reports'\n[go]\npath='project'\ninclude=['b.go']\n",
    )
    .unwrap();
    let mut paths = vec![fixture.temp.path().join("bin")];
    paths.extend(std::env::split_paths(
        &std::env::var_os("PATH").unwrap_or_default(),
    ));
    let output = Command::new(env!("CARGO_BIN_EXE_mutie"))
        .current_dir(&fixture.invocation)
        .args(["--path", "../project", "run", "--config", "../ci.toml"])
        .env("PATH", std::env::join_paths(paths).unwrap())
        .env("CALLS", fixture.temp.path().join("calls"))
        .output()
        .unwrap();
    let (directory, metadata, inventory) = report(&output);
    assert_eq!(
        directory.parent().unwrap(),
        fixture.temp.path().join("ci-reports")
    );
    assert_eq!(metadata["module_root"], fixture.root.to_str().unwrap());
    assert_eq!(inventory["mutations"][0]["file"], "b.go");
}
#[test]
fn output_modes_are_invocation_only_and_mutually_exclusive() {
    let fixture = Fixture::new("[go]\n");
    for args in [
        vec!["run", "--ui", "interactive"],
        vec!["run", "--plain", "--json"],
        vec!["rerun", "saved", "--plain", "--json"],
    ] {
        assert_eq!(fixture.run(&args).status.code(), Some(2));
    }
    assert!(!fixture.root.join(".mutie").exists());
    for args in [vec!["run"], vec!["run", "--plain"]] {
        let output = fixture.run(&args);
        success(&output);
        assert!(!output.stdout.contains(&0x1b));
    }
}

fn json_events(output: &Output) -> Vec<Value> {
    assert!(output.stdout.ends_with(b"\n"));
    assert!(!output.stdout.contains(&0x1b));
    let events: Vec<Value> = String::from_utf8(output.stdout.clone())
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(events.first().unwrap()["event"], "run_started");
    assert_eq!(events.last().unwrap()["event"], "summary");
    assert!(
        events
            .windows(2)
            .all(|pair| pair[0]["elapsed_seconds"].as_f64().unwrap()
                <= pair[1]["elapsed_seconds"].as_f64().unwrap())
    );
    events
}

#[test]
fn json_runs_and_reruns_preserve_results_policies_and_source_ids() {
    for (policy, exit) in [("", 0), ("fail_on_missed=true\n", 3), ("min_score=90\n", 3)] {
        let fixture = Fixture::new(&format!("{policy}[go]\n"));
        let output = fixture.run(&["run", "--json", "--verbose"]);
        assert_eq!(output.status.code(), Some(exit), "{output:?}");
        let events = json_events(&output);
        let summary = events.last().unwrap();
        assert_eq!(summary["exit_code"], exit);
        assert_eq!(summary["status"], "completed");
        assert_eq!(summary["report_finalized"], true);
        assert_eq!(summary["outcomes"]["missed"], 2);
        assert_eq!(summary["pending"], 0);
        assert!(events.iter().any(|e| {
            e["event"] == "command_finished"
                && e["output"]
                    .as_str()
                    .is_some_and(|s| s.contains("test diagnostic\n"))
        }));
        assert_eq!(
            events
                .iter()
                .filter(|e| e["event"] == "mutation_finished")
                .count(),
            2
        );
        let directory = Path::new(summary["report"].as_str().unwrap());
        let saved = read(directory, "run.json");
        assert_eq!(summary["score"], saved["score"]);
        assert_eq!(summary["policy"], saved["policy"]);
        assert_eq!(summary["score_policy"], saved["score_policy"]);
        assert_eq!(
            summary["outcomes"],
            read(directory, "outcomes.json")["summary"]
        );
        let before = fs::read(directory.join("outcomes.json")).unwrap();
        let rerun = fixture
            .command()
            .current_dir(&fixture.root)
            .args([
                "rerun",
                summary["report"].as_str().unwrap(),
                "--json",
                "--id",
                "2",
            ])
            .output()
            .unwrap();
        assert_eq!(rerun.status.code(), Some(exit));
        let selected = json_events(&rerun);
        let mutation = selected.iter().find(|e| e["event"] == "mutation").unwrap();
        assert_eq!(mutation["id"], 1);
        assert_eq!(mutation["source_id"], 2);
        assert_eq!(selected.last().unwrap()["completed"], 1);
        assert_eq!(fs::read(directory.join("outcomes.json")).unwrap(), before);
    }
}

#[test]
fn json_empty_and_failed_runs_have_final_summaries_without_passing_error_scores() {
    let empty = Fixture::new("[go]\ninclude=['absent.go']\n");
    let output = empty.run(&["run", "--json"]);
    success(&output);
    let events = json_events(&output);
    assert_eq!(events.last().unwrap()["total"], 0);
    assert!(!events.iter().any(|e| e["event"] == "mutation_started"));
    let fixture = Fixture::new("[go]\n");
    fs::write(
        fixture.temp.path().join("bin/go"),
        "#!/bin/sh\necho 'baseline failure' >&2\nexit 1\n",
    )
    .unwrap();
    let output = fixture.run(&["run", "--json"]);
    assert_eq!(output.status.code(), Some(1));
    let events = json_events(&output);
    let summary = events.last().unwrap();
    assert_eq!(summary["status"], "failed");
    assert_eq!(summary["exit_code"], 1);
    assert_eq!(summary["score"]["percentage"], Value::Null);
    assert!(
        summary["error"]
            .as_str()
            .unwrap()
            .contains("baseline failure")
    );
    assert!(!events.iter().any(|e| e["event"] == "mutation_started"));
    let config = Fixture::new("unknown=true\n");
    let output = config.run(&["run", "--json"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert!(!output.stderr.is_empty());
}

struct StreamingRun {
    child: std::process::Child,
    events: std::sync::mpsc::Receiver<Value>,
}
impl StreamingRun {
    fn start(fixture: &Fixture) -> Self {
        use std::{
            io::{BufRead, BufReader},
            process::Stdio,
        };
        // Hold the second mutation so the first result must be observable live.
        fs::write(
            fixture.temp.path().join("bin/go"),
            r#"#!/bin/sh
if [ "$1" = list ]; then
    printf '{"Dir":"%s","GoFiles":["a.go","b.go"]}\n' "$PWD"
else
    if grep -q 'b = true' b.go; then
        touch "$STATE/ready"
        while [ ! -e "$STATE/release" ]; do sleep 0.02; done
    fi
    printf '{"Action":"output","Output":"diagnostic\\n"}\n'
fi
"#,
        )
        .unwrap();
        let mut child = fixture
            .command()
            .current_dir(&fixture.root)
            .args(["run", "--json"])
            .env("STATE", fixture.temp.path())
            .stdout(Stdio::piped())
            .stderr(fs::File::create(fixture.temp.path().join("stderr")).unwrap())
            .spawn()
            .unwrap();
        let stdout = child.stdout.take().unwrap();
        let (sender, events) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let value: Value = serde_json::from_str(&line.unwrap()).unwrap();
                if sender.send(value).is_err() {
                    break;
                }
            }
        });
        Self { child, events }
    }
    fn next(&self) -> Value {
        self.events
            .recv_timeout(std::time::Duration::from_secs(15))
            .expect("JSON stream stopped before completion")
    }
    fn finish(&mut self) -> std::process::ExitStatus {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
        loop {
            if let Some(status) = self.child.try_wait().unwrap() {
                return status;
            }
            assert!(
                std::time::Instant::now() < deadline,
                "JSON command did not exit"
            );
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }
}
impl Drop for StreamingRun {
    fn drop(&mut self) {
        if self.child.try_wait().unwrap().is_none() {
            // Give the CLI its normal opportunity to drain and reap tool processes.
            unsafe {
                libc::kill(self.child.id() as i32, libc::SIGINT);
            }
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
            while self.child.try_wait().unwrap().is_none() && std::time::Instant::now() < deadline {
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

#[test]
fn json_results_stream_before_exit_and_finalize_after_cancellation_or_save_failure() {
    for mode in ["success", "cancel", "save_failure"] {
        let fixture = Fixture::new("[go]\n");
        let mut run = StreamingRun::start(&fixture);
        let started = run.next();
        assert_eq!(started["event"], "run_started");
        let report = PathBuf::from(started["report"].as_str().unwrap());
        loop {
            let event = run.next();
            if event["event"] == "mutation_finished" {
                assert_eq!(event["id"], 1);
                assert_eq!(event["outcome"], "missed");
                break;
            }
            assert_ne!(event["event"], "summary");
        }
        loop {
            let event = run.next();
            if event["event"] == "mutation_started" && event["id"] == 2 {
                break;
            }
        }
        assert!(
            run.child.try_wait().unwrap().is_none(),
            "first result was buffered until exit"
        );
        if mode == "cancel" {
            assert_eq!(
                unsafe { libc::kill(run.child.id() as i32, libc::SIGINT) },
                0
            );
        } else {
            if mode == "save_failure" {
                fs::remove_file(report.join("run.json")).unwrap();
                fs::create_dir(report.join("run.json")).unwrap();
            }
            fs::write(fixture.temp.path().join("release"), "").unwrap();
        }
        let summary = loop {
            let event = run.next();
            if event["event"] == "summary" {
                break event;
            }
        };
        assert_eq!(
            run.finish().code(),
            Some(if mode == "success" { 0 } else { 1 })
        );
        assert_eq!(
            summary["status"],
            match mode {
                "success" => "completed",
                "cancel" => "interrupted",
                _ => "failed",
            }
        );
        if mode == "success" {
            assert_eq!(summary["completed"], 2);
        } else {
            assert!(summary["score"].is_null() || summary["score"]["percentage"].is_null());
        }
        if mode == "save_failure" {
            assert_eq!(summary["report_finalized"], false);
        } else {
            assert_eq!(read(&report, "run.json")["status"], summary["status"]);
        }
    }
}

#[test]
fn closing_the_json_pipe_stops_execution_and_finalizes_a_failed_report() {
    use std::{
        io::{BufRead, BufReader},
        process::Stdio,
        sync::mpsc,
        time::{Duration, Instant},
    };
    let fixture = Fixture::new("[go]\n");
    fs::write(
        fixture.temp.path().join("bin/go"),
        r#"#!/bin/sh
if [ "$1" = list ]; then
    printf '{"Dir":"%s","GoFiles":["a.go","b.go"]}\n' "$PWD"
else
    touch "$STATE/ready"
    while [ ! -e "$STATE/release" ]; do sleep 0.02; done
fi
"#,
    )
    .unwrap();
    let child = fixture
        .command()
        .current_dir(&fixture.root)
        .args(["run", "--json"])
        .env("STATE", fixture.temp.path())
        .stdout(Stdio::piped())
        .stderr(fs::File::create(fixture.temp.path().join("stderr")).unwrap())
        .spawn()
        .unwrap();
    let mut run = StreamingRun {
        child,
        events: mpsc::channel().1,
    };
    let stdout = run.child.stdout.take().unwrap();
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let mut reader = BufReader::new(stdout);
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        sender.send((reader, line)).unwrap();
    });
    let (reader, first) = receiver.recv_timeout(Duration::from_secs(15)).unwrap();
    let first: Value = serde_json::from_str(&first).unwrap();
    assert_eq!(first["event"], "run_started");
    let deadline = Instant::now() + Duration::from_secs(15);
    while !fixture.temp.path().join("ready").exists() {
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(10));
    }
    drop(reader);
    fs::write(fixture.temp.path().join("release"), "").unwrap();
    assert_eq!(run.finish().code(), Some(1));
    let saved = read(Path::new(first["report"].as_str().unwrap()), "run.json");
    assert_eq!(saved["status"], "failed");
    assert_eq!(saved["score"]["percentage"], Value::Null);
    assert!(
        fs::read_to_string(fixture.temp.path().join("stderr"))
            .unwrap()
            .contains("writing JSON event")
    );
}
