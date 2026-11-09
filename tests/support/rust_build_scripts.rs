use super::*;

fn run(fixture: &Fixture, args: &[&str]) -> Output {
    fixture
        .command(args)
        .args(["--operator", "boolean_flip"])
        .output()
        .unwrap()
}

#[test]
#[ignore = "real Cargo build-script cache, identity and focused rerun workflow"]
fn generated_artifacts_rebuild_and_stronger_tests_preserve_build_identity() {
    let fixture = Fixture::from_fixture("rust-build");
    let inputs = files(&fixture.root);
    let mut saved = None;
    for (mode, jobs) in [("fresh", "1"), ("worker", "1"), ("worker", "2")] {
        let output = run(&fixture, &["--rust-build-cache", mode, "--jobs", jobs]);
        assert!(output.status.success(), "{mode}/{jobs}: {output:?}");
        let (directory, record, mutants, outcomes) = report(&output);
        assert_eq!(outcomes["summary"]["killed"], 1);
        assert_eq!(outcomes["summary"]["missed"], 1);
        assert_eq!(mutants["mutations"].as_array().unwrap().len(), 2);
        assert_eq!(
            record["rust_scope"]["build_scripts"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        assert!(
            record["rust_scope"]["build_inputs"]
                .as_str()
                .unwrap()
                .starts_with("sha256:")
        );
        assert_eq!(
            record["rust_scope"]["build_outputs"]["packages"]
                .as_object()
                .unwrap()
                .len(),
            1
        );
        if let Some((_, _, identity)) = &saved {
            assert_eq!(&record["rust_scope"], identity);
        } else {
            saved = Some((
                directory.clone(),
                files(&directory),
                record["rust_scope"].clone(),
            ));
        }
        assert_eq!(files(&fixture.root), inputs);
        fixture.preserved();
    }
    let (source, reports, _) = saved.unwrap();
    let script = fixture.root.join("build.rs");
    let script_bytes = fs::read(&script).unwrap();
    for path in [&script, &fixture.root.join("build-input.txt")] {
        let old = fs::read(path).unwrap();
        let mut changed = old.clone();
        changed.extend_from_slice(b"\n");
        fs::write(path, changed).unwrap();
        let output = run(&fixture, &["--rerun-missed", source.to_str().unwrap()]);
        assert!(!output.status.success());
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("build-script")
                || String::from_utf8_lossy(&output.stderr).contains("build scripts"),
            "{output:?}"
        );
        fs::write(path, old).unwrap();
    }
    let changed_env = fixture
        .command(&["--rerun-missed", source.to_str().unwrap()])
        .args(["--operator", "boolean_flip"])
        .env("MUTIE_BUILD_FIXTURE_VALUE", "changed")
        .output()
        .unwrap();
    assert!(!changed_env.status.success());
    assert!(
        String::from_utf8_lossy(&changed_env.stderr).contains("build-script outputs changed"),
        "{changed_env:?}"
    );
    let test_input = fixture.root.join("tests/build-mode");
    fs::write(&test_input, "changed").unwrap();
    let output = run(&fixture, &["--rerun-missed", source.to_str().unwrap()]);
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("build-script outputs changed"),
        "{output:?}"
    );
    fs::remove_file(test_input).unwrap();
    fs::write(
        fixture.root.join("tests/stronger.rs"),
        "#[test] fn checks_missing_contract() { assert!(build_fixture::unchecked()); }\n",
    )
    .unwrap();
    let stronger = files(&fixture.root);
    let output = run(
        &fixture,
        &[
            "--rust-build-cache",
            "worker",
            "--rerun-missed",
            source.to_str().unwrap(),
        ],
    );
    assert!(output.status.success(), "{output:?}");
    let (_, _, _, outcomes) = report(&output);
    assert_eq!(outcomes["summary"]["killed"], 1);
    assert_eq!(outcomes["summary"]["missed"], 0);
    assert_eq!(files(&fixture.root), stronger);
    assert_eq!(files(&source), reports);
    assert_eq!(fs::read(script).unwrap(), script_bytes);
    fixture.preserved();
}

#[test]
#[ignore = "real Cargo build-script failure, timeout, generated-code and output controls"]
fn script_failures_and_limits_never_become_test_kills() {
    let fixture = Fixture::from_fixture("rust-build");
    let script = fixture.root.join("build.rs");
    let original = fs::read_to_string(&script).unwrap();
    for (injected, expected) in [
        ("panic!(\"script body failure\");", "error"),
        (
            "std::thread::sleep(std::time::Duration::from_secs(20));",
            "timeout",
        ),
        (
            "for _ in 0..10000 { println!(\"cargo::warning={}\", \"x\".repeat(1024)); }",
            "error",
        ),
    ] {
        let source = format!(
            "let source = std::fs::read_to_string(\"src/lib.rs\").unwrap(); if source.contains(\"    false\") {{ {injected} }}"
        );
        fs::write(
            &script,
            original.replace("fn main() {", &format!("fn main() {{ {source}")),
        )
        .unwrap();
        let inputs = files(&fixture.root);
        let output = run(
            &fixture,
            &[
                "--timeout",
                if expected == "timeout" { "3" } else { "30" },
                "--max-capture-bytes",
                "32768",
            ],
        );
        let (_, _, _, outcomes) = report(&output);
        let results = outcomes["mutations"].as_array().unwrap();
        assert!(!results.is_empty(), "{output:?}");
        assert!(
            results
                .iter()
                .filter(|r| r["state"] == "completed")
                .all(|r| r["outcome"] == expected),
            "{output:?}\n{outcomes:#}"
        );
        assert_eq!(outcomes["summary"]["killed"], 0);
        assert_eq!(files(&fixture.root), inputs);
        fixture.preserved();
    }
    // Successful script execution with invalid generated Rust is a compiler failure.
    let injection = "if std::fs::read_to_string(\"src/lib.rs\").unwrap().contains(\"    false\") { fs::write(out.join(\"generated.rs\"), \"compile_error!(\\\"bad generated Rust\\\");\").unwrap(); }";
    fs::write(
        &script,
        original.replace(
            "    println!(\"cargo::rerun-if-changed=build-input.txt\");",
            &format!("    {injection}\n    println!(\"cargo::rerun-if-changed=build-input.txt\");"),
        ),
    )
    .unwrap();
    let output = run(&fixture, &[]);
    assert!(output.status.success(), "{output:?}");
    let (_, _, _, outcomes) = report(&output);
    assert_eq!(outcomes["summary"]["build_failed"], 2);
    fixture.preserved();
}

#[test]
#[ignore = "real Cargo workspace build-script metadata and full-scope execution"]
fn unselected_workspace_member_build_script_stays_in_execution_identity() {
    let fixture = Fixture::from_fixture("rust-build");
    let manifest = fixture.root.join("Cargo.toml");
    fs::write(
        &manifest,
        fs::read_to_string(&manifest).unwrap().replace(
            "[workspace]",
            "[workspace]\nmembers = ['helper']\n\n[dependencies]\nhelper = { path = 'helper' }\n",
        ),
    )
    .unwrap();
    fs::create_dir_all(fixture.root.join("helper/src")).unwrap();
    fs::write(
        fixture.root.join("helper/Cargo.toml"),
        "[package]\nname='helper'\nversion='0.0.0'\nedition='2024'\nbuild='generate.rs'\n",
    )
    .unwrap();
    fs::write(
        fixture.root.join("helper/generate.rs"),
        "fn main() { println!(\"cargo::rustc-env=HELPER=ready\"); }\n",
    )
    .unwrap();
    fs::write(
        fixture.root.join("helper/src/lib.rs"),
        "#[test] fn build_script_ran() { assert_eq!(env!(\"HELPER\"), \"ready\"); }\n",
    )
    .unwrap();
    let lock = Command::new(env!("CARGO"))
        .current_dir(&fixture.root)
        .env("CARGO_TARGET_DIR", fixture.temp.path().join("lock-target"))
        .args(["generate-lockfile", "--offline"])
        .output()
        .unwrap();
    assert!(lock.status.success(), "{lock:?}");
    let inputs = files(&fixture.root);
    let output = run(
        &fixture,
        &[
            "--rust-package",
            "build-fixture",
            "--rust-build-cache",
            "worker",
        ],
    );
    assert!(output.status.success(), "{output:?}");
    let (_, record, _, outcomes) = report(&output);
    assert_eq!(
        record["rust_scope"]["build_scripts"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        record["rust_scope"]["build_outputs"]["packages"]
            .as_object()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(outcomes["summary"]["killed"], 1);
    assert_eq!(files(&fixture.root), inputs);
    fixture.preserved();
}

#[test]
#[ignore = "real build-script process tree timeout and cancellation"]
fn build_script_descendants_stop_on_timeout_and_interrupt() {
    for interrupt in [false, true] {
        let fixture = Fixture::from_fixture("rust-build");
        fs::write(
            fixture.root.join("build.rs"),
            r##"
use std::{env, fs, path::PathBuf, process::Command};
unsafe extern "C" { fn getpgrp() -> i32; }
fn main() {
    let state = PathBuf::from(env::var_os("MUTIE_RUST_TEST_STATE").unwrap());
    fs::write(state.join("build.pid"), std::process::id().to_string()).unwrap();
    fs::write(state.join("build.group"), unsafe { getpgrp() }.to_string()).unwrap();
    let status = Command::new("/bin/sh").arg("-c").arg(r#"
sleep 120 &
echo $! > "$MUTIE_RUST_TEST_STATE/descendant.pid"
echo $$ > "$MUTIE_RUST_TEST_STATE/shell.pid"
touch "$MUTIE_RUST_TEST_STATE/build.ready"
wait
"#).status().unwrap();
    assert!(status.success());
}

"##,
        )
        .unwrap();
        let inputs = files(&fixture.root);
        let args = [
            "--baseline-timeout",
            if interrupt { "60" } else { "10" },
            "--operator",
            "boolean_flip",
        ];
        let mut child = InterruptibleRun::start(&fixture, &args);
        child.ready(&["build"]);
        let pids: Vec<i32> = ["build.pid", "shell.pid", "descendant.pid"]
            .iter()
            .map(|name| {
                fs::read_to_string(child.state.join(name))
                    .unwrap()
                    .trim()
                    .parse()
                    .unwrap()
            })
            .collect();
        assert!(pids.iter().all(|pid| process_running(*pid)));
        let output = if interrupt {
            child.interrupt(libc::SIGINT)
        } else {
            child.finish(Duration::from_secs(20))
        };
        assert!(!output.status.success(), "{output:?}");
        let (_, record, _, outcomes) = report(&output);
        assert_eq!(
            record["status"],
            if interrupt { "interrupted" } else { "failed" }
        );
        assert!(outcomes["mutations"].as_array().unwrap().is_empty());
        let deadline = Instant::now() + Duration::from_secs(10);
        while pids.iter().any(|pid| process_running(*pid)) {
            assert!(
                Instant::now() < deadline,
                "build-script descendants leaked: {pids:?}"
            );
            thread::sleep(Duration::from_millis(20));
        }
        assert_eq!(files(&fixture.root), inputs);
        fixture.preserved();
    }
}

#[test]
#[ignore = "real Cargo build-script input/output confinement controls"]
fn generated_evidence_rejects_external_inputs_links_and_source_writes() {
    for (injected, diagnostic) in [
        (
            "std::fs::write(\"unexpected-source-write\", b\"changed\").unwrap();",
            "outside OUT_DIR",
        ),
        (
            "println!(\"cargo::rerun-if-changed={}\", std::env::var(\"MUTIE_EXTERNAL_BUILD_INPUT\").unwrap());",
            "must stay inside the snapshot",
        ),
        (
            "std::os::unix::fs::symlink(std::env::current_dir().unwrap().join(\"build-input.txt\"), out.join(\"linked\")).unwrap();",
            "escapes the owned root",
        ),
        (
            "std::fs::File::create(out.join(\"oversized\")).unwrap().set_len(268_435_457).unwrap();",
            "256 MiB / 20000-file bound",
        ),
    ] {
        let fixture = Fixture::from_fixture("rust-build");
        let script = fixture.root.join("build.rs");
        let original = fs::read_to_string(&script).unwrap();
        fs::write(
            &script,
            original.replace(
                "    let input =",
                &format!("    {injected}\n    let input ="),
            ),
        )
        .unwrap();
        let external = fixture.temp.path().join("external");
        fs::write(&external, "unchanged").unwrap();
        let inputs = files(&fixture.root);
        let output = fixture
            .command(&["--operator", "boolean_flip"])
            .env("MUTIE_EXTERNAL_BUILD_INPUT", &external)
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(diagnostic),
            "{output:?}"
        );
        let (_, _, _, outcomes) = report(&output);
        assert!(outcomes["mutations"].as_array().unwrap().is_empty());
        assert_eq!(files(&fixture.root), inputs);
        assert_eq!(fs::read_to_string(external).unwrap(), "unchanged");
        fixture.preserved();
    }
}
