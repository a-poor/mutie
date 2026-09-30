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
        let temp = tempfile::tempdir().unwrap();
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
            .arg(&self.root)
            .args(args)
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
fn merged_settings_drive_execution_and_are_saved_in_reports() {
    let fixture = Fixture::new(
        r#"
lang = "go"
jobs = 2
include = ["*.go"]
exclude = ["b.go"]
timeout = 4
baseline_timeout = 30
output_dir = "../repository-reports"
verbose = true
"#,
    );
    let source = fs::read(fixture.root.join("a.go")).unwrap();
    let output = fixture.run(&[]);
    assert!(success(&output).contains("test diagnostic"));
    let (directory, metadata, inventory) = report(&output);
    assert_eq!(
        directory.parent().unwrap(),
        fixture.temp.path().join("repository-reports")
    );
    assert_eq!(metadata["schema_version"], 1);
    assert_eq!(
        metadata["effective_config"],
        json!({
            "module_root": fixture.root, "config_file": fixture.root.join("mutie.toml"),
            "lang": "go", "jobs": 2, "include": ["*.go"], "exclude": ["b.go"],
            "timeout": 4, "baseline_timeout": 30,
            "output_dir": fixture.temp.path().join("repository-reports"), "verbose": true, "list": false
        })
    );
    assert_eq!(metadata["requested_workers"], 2);
    assert_eq!(metadata["workers"], 1);
    assert_eq!(metadata["mutation_timeout_seconds"], 4.0);
    assert_eq!(inventory["mutations"].as_array().unwrap().len(), 1);
    assert_eq!(inventory["mutations"][0]["file"], "a.go");
    assert_eq!(read(&directory, "outcomes.json")["summary"]["missed"], 1);

    let output = fixture.run(&[
        "--jobs",
        "1",
        "--include",
        "b.go",
        "--exclude",
        "a.go",
        "--timeout",
        "5",
        "--baseline-timeout",
        "300",
        "--no-verbose",
        "--output-dir",
        "cli-reports",
    ]);
    assert!(!success(&output).contains("test diagnostic"));
    let (directory, metadata, inventory) = report(&output);
    assert_eq!(
        directory.parent().unwrap(),
        fixture.invocation.join("cli-reports")
    );
    let config = &metadata["effective_config"];
    assert_eq!(config["jobs"], 1);
    assert_eq!(config["include"], json!(["b.go"]));
    assert_eq!(config["exclude"], json!(["a.go"]));
    assert_eq!(config["timeout"], 5);
    assert_eq!(config["baseline_timeout"], 300);
    assert_eq!(config["verbose"], false);
    assert_eq!(inventory["mutations"].as_array().unwrap().len(), 1);
    assert_eq!(inventory["mutations"][0]["file"], "b.go");
    assert!(
        fs::read_to_string(directory.join("logs/baseline.log"))
            .unwrap()
            .contains("test diagnostic")
    );

    let output = fixture.run(&[
        "--clear-include",
        "--clear-exclude",
        "--clear-timeout",
        "--no-verbose",
    ]);
    let (_, metadata, inventory) = report(&output);
    assert_eq!(metadata["effective_config"]["include"], json!([]));
    assert_eq!(metadata["effective_config"]["exclude"], json!([]));
    assert!(metadata["effective_config"]["timeout"].is_null());
    assert!(metadata["mutation_timeout_seconds"].as_f64().unwrap() >= 10.0);
    assert_eq!(inventory["mutations"].as_array().unwrap().len(), 2);
    assert_eq!(fs::read(fixture.root.join("a.go")).unwrap(), source);
    assert!(!fixture.root.join(".mutie").exists());
}

#[test]
fn listing_uses_config_and_excludes_reports_without_creating_any() {
    let fixture = Fixture::new("include = ['a.go']\noutput_dir = 'reports'");
    let listing = success(&fixture.run(&["--list"]));
    assert!(listing.contains("Found 1 mutations"));
    assert!(!fixture.root.join("reports").exists());
    assert!(!fixture.root.join(".mutie").exists());
    fs::create_dir(fixture.root.join("reports")).unwrap();
    fs::write(fixture.root.join("reports/poison.go"), "not Go").unwrap();
    let listing = success(&fixture.run(&["--list", "--clear-include"]));
    assert!(listing.contains("Found 2 mutations"));
    assert!(!listing.contains("Running baseline"));
    assert_eq!(
        fs::read_to_string(fixture.temp.path().join("calls")).unwrap(),
        "list\nlist\n"
    );
    assert_eq!(
        fs::read_dir(fixture.root.join("reports")).unwrap().count(),
        1
    );
}

#[test]
fn invalid_config_fails_before_running_go_or_creating_reports() {
    let fixture = Fixture::new("");
    for (config, expected) in [
        ("jobs = 0", "mutie.toml"),
        ("typo = true", "unknown field"),
        ("include = ['[']", "invalid glob"),
        ("exclude = ['[']", "invalid glob"),
        ("output_dir = '.'", "module root"),
        ("output_dir = ''", "must not be empty"),
        ("lang = 'rust'", "only Go"),
    ] {
        fs::write(fixture.root.join("mutie.toml"), config).unwrap();
        let output = fixture.run(&[]);
        assert!(!output.status.success(), "{config}");
        let error = String::from_utf8_lossy(&output.stderr);
        assert!(error.contains(expected), "{config}: {error}");
        assert!(!fixture.temp.path().join("calls").exists());
        assert!(!fixture.root.join(".mutie").exists());
    }
    fs::write(fixture.root.join("mutie.toml"), "include = ['[']").unwrap();
    success(&fixture.run(&["--list", "--clear-include"]));
}
