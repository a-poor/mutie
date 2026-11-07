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

    fn run(&self, args: &[&str]) -> Output {
        let mut paths = vec![self.temp.path().join("bin")];
        paths.extend(std::env::split_paths(
            &std::env::var_os("PATH").unwrap_or_default(),
        ));
        Command::new(env!("CARGO_BIN_EXE_mutie"))
            .current_dir(&self.invocation)
            .arg(args.first().copied().unwrap_or("run"))
            .arg(&self.root)
            .args(args.iter().skip(1))
            .env("PATH", std::env::join_paths(paths).unwrap())
            .env("CALLS", self.temp.path().join("calls"))
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
fn grouped_settings_drive_execution_and_cli_only_overrides_are_saved() {
    let fixture = Fixture::new(
        "jobs=4\ntimeout=4\noutput_dir='../reports'\n[go]\njobs=2\ninclude=['a.go']\n",
    );
    let before = fs::read(fixture.root.join("a.go")).unwrap();
    let output = fixture.run(&["run", "--verbose", "--ui", "plain"]);
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
    // `rerun` takes its saved run before the optional project directory.
    let mut paths = vec![fixture.temp.path().join("bin")];
    paths.extend(std::env::split_paths(
        &std::env::var_os("PATH").unwrap_or_default(),
    ));
    let invoke = |extra: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_mutie"))
            .arg("rerun")
            .arg(&directory)
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
        .args(["run", "--config"])
        .arg(config)
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
fn ui_options_are_invocation_only() {
    let fixture = Fixture::new("[go]\n");
    let output = fixture.run(&["run", "--ui", "interactive"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("requires terminal"));
    assert!(!fixture.root.join(".mutie").exists());
    for mode in ["auto", "plain"] {
        let output = fixture.run(&["run", "--ui", mode]);
        success(&output);
        assert!(!output.stdout.contains(&0x1b));
    }
}
