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
