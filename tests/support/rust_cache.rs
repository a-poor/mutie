use super::*;
use std::os::unix::fs::PermissionsExt;

fn proxy(fixture: &Fixture) -> PathBuf {
    let bin = fixture.temp.path().join("cache-bin");
    fs::create_dir(&bin).unwrap();
    let path = bin.join("cargo");
    // The optional cleanup bypass is a negative control: setting it makes the
    // backdated mutant run stale binaries and this regression must fail.
    fs::write(
        &path,
        r#"#!/bin/sh
if [ "$1" = clean ] && [ "$MUTIE_SKIP_PACKAGE_CLEAN" = yes ]; then exit 0; fi
if [ "$1" = test ]; then
  for arg in "$@"; do
    if [ "$arg" = --no-run ]; then
      if [ "$MUTIE_CACHE_BUILD_FAIL" = yes ] && sed -n '2p' src/flags.rs | grep -q false; then
        printf '\ncompile_error!("injected local build failure");\n' >> src/flags.rs
      fi
      touch -t 200001010000 Cargo.toml Cargo.lock src/*.rs tests/*.rs
    fi
  done
fi
exec "$REAL_CARGO" "$@"
"#,
    )
    .unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    bin
}

fn cached_command(fixture: &Fixture, bin: &Path, args: &[&str]) -> Command {
    let mut paths = vec![bin.to_owned()];
    paths.extend(std::env::split_paths(&std::env::var_os("PATH").unwrap()));
    let mut command = fixture.command(args);
    command
        .env("PATH", std::env::join_paths(paths).unwrap())
        .env("REAL_CARGO", env!("CARGO"));
    command
}

fn artifacts(directory: &Path, logs: &Value) -> Vec<Value> {
    let text = fs::read_to_string(directory.join(logs["stdout"].as_str().unwrap())).unwrap();
    text.split("--- Cargo test execution ---")
        .next()
        .unwrap()
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter(|v| v["reason"] == "compiler-artifact")
        .collect()
}

#[test]
#[ignore = "runs real Cargo cache/restoration/parallel/rerun checks; opt-in to avoid recursive self-testing"]
fn worker_cache_rebuilds_backdated_sources_and_tests_with_isolated_dependencies() {
    let fixture = Fixture::new();
    let manifest = fixture.root.join("Cargo.toml");
    fs::write(
        &manifest,
        format!(
            "{}\n[dependencies]\nitoa = '=1.0.18'\n",
            fs::read_to_string(&manifest).unwrap()
        ),
    )
    .unwrap();
    let lock = Command::new(env!("CARGO"))
        .current_dir(&fixture.root)
        .env("CARGO_TARGET_DIR", fixture.temp.path().join("lock-target"))
        .env(
            "CARGO_BUILD_BUILD_DIR",
            fixture.temp.path().join("lock-build"),
        )
        .args(["generate-lockfile", "--offline"])
        .output()
        .unwrap();
    assert!(lock.status.success(), "{lock:?}");
    let flags = fixture.root.join("src/flags.rs");
    let original = fs::read_to_string(&flags).unwrap();
    fs::write(&flags, format!("{original}\n// Dependency is used by the binary, outside the test-only module.\npub fn dependency_value() -> usize {{ itoa::Buffer::new().format(42).len() }}\n")).unwrap();
    let main = fixture.root.join("src/main.rs");
    fs::write(
        &main,
        fs::read_to_string(&main).unwrap().replace(
            "fn main() {",
            "fn main() {\n    let _ = flags::dependency_value();",
        ),
    )
    .unwrap();
    let bin = proxy(&fixture);
    let inputs = files(&fixture.root);
    let mut saved = None;
    let mut prior_targets = std::collections::BTreeSet::new();
    for (mode, jobs) in [("fresh", "1"), ("worker", "1"), ("worker", "2")] {
        let output = cached_command(
            &fixture,
            &bin,
            &["--rust-build-cache", mode, "--jobs", jobs],
        )
        .output()
        .unwrap();
        assert!(output.status.success(), "{mode}/{jobs}: {output:?}");
        let (directory, run, _, outcomes) = report(&output);
        assert_eq!(run["effective_config"]["rust_build_cache"], mode);
        let baseline = artifacts(&directory, &outcomes["baseline"]["logs"]);
        assert!(baseline.iter().all(|record| record["fresh"] == false));
        let baseline_target = baseline
            .iter()
            .find(|record| record["target"]["name"] == "itoa")
            .unwrap()["filenames"][0]
            .as_str()
            .unwrap();
        assert_eq!(
            outcomes["summary"],
            serde_json::json!({"killed":2,"missed":1,"timeout":0,"build_failed":0,"errors":0})
        );
        let mut by_worker = BTreeMap::<u64, String>::new();
        let mut warm = 0;
        for execution in outcomes["mutations"].as_array().unwrap() {
            let records = artifacts(&directory, &execution["logs"]);
            let dependency = records
                .iter()
                .find(|v| v["target"]["name"] == "itoa")
                .unwrap();
            warm += usize::from(dependency["fresh"] == true);
            let target = dependency["filenames"][0].as_str().unwrap().to_owned();
            assert_ne!(target, baseline_target, "worker reused baseline artifacts");
            assert!(!prior_targets.contains(&target), "cache leaked across runs");
            if mode == "worker" {
                let worker = execution["worker"].as_u64().unwrap();
                if let Some(previous) = by_worker.insert(worker, target.clone()) {
                    assert_eq!(previous, target, "worker lost its dependency cache");
                }
            }
            for artifact in records.iter().filter(|v| v["target"]["name"] != "itoa") {
                assert_eq!(
                    artifact["fresh"], false,
                    "local target was not rebuilt: {artifact}"
                );
            }
        }
        if mode == "worker" {
            assert_eq!(by_worker.len(), jobs.parse::<usize>().unwrap());
            assert_eq!(
                by_worker
                    .values()
                    .collect::<std::collections::BTreeSet<_>>()
                    .len(),
                by_worker.len()
            );
            assert_eq!(warm, 3 - by_worker.len());
            prior_targets.extend(by_worker.into_values());
        } else {
            assert_eq!(warm, 0);
        }
        assert_eq!(files(&fixture.root), inputs);
        fixture.preserved();
        if saved.is_none() {
            saved = Some((directory.clone(), files(&directory)));
        }
    }
    let (source, before) = saved.unwrap();
    let failed_build = cached_command(
        &fixture,
        &bin,
        &["--rust-build-cache", "worker", "--jobs", "1"],
    )
    .env("MUTIE_CACHE_BUILD_FAIL", "yes")
    .output()
    .unwrap();
    assert!(failed_build.status.success(), "{failed_build:?}");
    let (_, _, _, outcomes) = report(&failed_build);
    assert_eq!(outcomes["mutations"][0]["outcome"], "build_failed");
    assert_eq!(outcomes["mutations"][1]["outcome"], "killed");
    assert_eq!(outcomes["mutations"][2]["outcome"], "missed");
    assert_eq!(files(&fixture.root), inputs);
    fixture.preserved();
    let cli = fixture.root.join("tests/cli.rs");
    fs::write(
        &cli,
        fs::read_to_string(&cli).unwrap().replace(
            "    let output = run(\"unchecked\");",
            "    let output = run(\"unchecked\");\n    assert_eq!(output.stdout, b\"true\\n\");",
        ),
    )
    .unwrap();
    let strengthened = files(&fixture.root);
    let output = cached_command(
        &fixture,
        &bin,
        &[
            "--rust-build-cache",
            "worker",
            "--rerun-missed",
            source.to_str().unwrap(),
            "--min-score",
            "100",
        ],
    )
    .output()
    .unwrap();
    assert!(output.status.success(), "{output:?}");
    let (_, run, _, outcomes) = report(&output);
    assert_eq!(outcomes["summary"]["killed"], 1);
    assert_eq!(
        run["rerun"]["mutations"],
        serde_json::json!([{"id":1,"source_id":3}])
    );
    assert_eq!(files(&fixture.root), strengthened);
    assert_eq!(files(&source), before);
    fixture.preserved();
}
