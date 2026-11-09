use std::{
    fs,
    process::{Command, Output},
};
fn invoke(root: &std::path::Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_mutie"))
        .arg("--path")
        .arg(root)
        .arg("init")
        .args(args)
        .output()
        .unwrap()
}
#[test]
fn config_round_trip_and_noninteractive_ambiguity() {
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join("go.mod"), "module example.test\n").unwrap();
    fs::write(
        root.path().join("Cargo.toml"),
        "[package]\nname='example'\nversion='0.1.0'\n",
    )
    .unwrap();
    let output = invoke(root.path(), &["--non-interactive"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("ambiguous"));
    assert!(!root.path().join("mutie.toml").exists());
    let output = invoke(root.path(), &["--lang", "go", "--non-interactive"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let config = fs::read_to_string(root.path().join("mutie.toml")).unwrap();
    let parsed: toml::Value = toml::from_str(&config).unwrap();
    assert!(parsed["go"].is_table());
    assert!(
        !invoke(root.path(), &["--lang", "go", "--non-interactive"])
            .status
            .success()
    );
    assert_eq!(
        fs::read_to_string(root.path().join("mutie.toml")).unwrap(),
        config
    );
}

#[test]
fn force_creates_or_replaces_config_and_dry_run_never_writes() {
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join("go.mod"), "module example.test\n").unwrap();
    let destination = root.path().join("mutie.toml");
    let preview = invoke(root.path(), &["--force", "--dry-run", "--non-interactive"]);
    assert!(preview.status.success());
    assert!(!destination.exists());
    let created = invoke(root.path(), &["--force", "--non-interactive"]);
    assert!(
        created.status.success(),
        "{}",
        String::from_utf8_lossy(&created.stderr)
    );
    assert_eq!(fs::read(&destination).unwrap(), preview.stdout);

    // Regenerate even malformed old settings, without loading or merging them.
    let original = b"old settings = [invalid\n";
    fs::write(&destination, original).unwrap();
    let refused = invoke(root.path(), &["--non-interactive"]);
    assert!(!refused.status.success());
    assert!(String::from_utf8_lossy(&refused.stderr).contains("--force"));
    assert_eq!(fs::read(&destination).unwrap(), original);
    let preview = invoke(root.path(), &["--force", "--dry-run", "--non-interactive"]);
    assert!(preview.status.success());
    assert_eq!(fs::read(&destination).unwrap(), original);
    let replaced = invoke(root.path(), &["--force", "--non-interactive"]);
    assert!(
        replaced.status.success(),
        "{}",
        String::from_utf8_lossy(&replaced.stderr)
    );
    assert_eq!(fs::read(&destination).unwrap(), preview.stdout);
    let config: toml::Value =
        toml::from_str(std::str::from_utf8(&preview.stdout).unwrap()).unwrap();
    assert!(config["go"].is_table());
}

#[test]
fn failed_force_preserves_the_existing_config() {
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join("go.mod"), "module example.test\n").unwrap();
    fs::write(
        root.path().join("Cargo.toml"),
        "[package]\nname='example'\nversion='0.1.0'\n",
    )
    .unwrap();
    let destination = root.path().join("mutie.toml");
    let original = b"[go]\njobs=3\n";
    fs::write(&destination, original).unwrap();
    let output = invoke(root.path(), &["--force", "--non-interactive"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("ambiguous"));
    assert_eq!(fs::read(destination).unwrap(), original);
}

#[test]
fn init_defaults_to_current_directory_and_does_not_search_parents() {
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join("mutie.toml"), "invalid = true\n").unwrap();
    let project = root.path().join("project");
    fs::create_dir(&project).unwrap();
    fs::write(project.join("go.mod"), "module example.test\n").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_mutie"))
        .current_dir(&project)
        .args(["init", "--non-interactive"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let config: toml::Value =
        toml::from_str(&fs::read_to_string(project.join("mutie.toml")).unwrap()).unwrap();
    assert!(config["go"].is_table());
    assert_eq!(
        fs::read_to_string(root.path().join("mutie.toml")).unwrap(),
        "invalid = true\n"
    );
}
#[test]
fn unsupported_manager_is_actionable_and_never_writes() {
    let root = tempfile::tempdir().unwrap();
    fs::write(
        root.path().join("package.json"),
        r#"{"scripts":{"test":"vitest run"},"packageManager":"npm@11.0.0"}"#,
    )
    .unwrap();
    let output = invoke(root.path(), &["--non-interactive"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("not implemented"));
    assert!(!root.path().join("mutie.toml").exists());
}
#[cfg(unix)]
#[test]
fn existing_symlink_is_never_followed_for_writing() {
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join("go.mod"), "module example.test\n").unwrap();
    let target = root.path().join("target");
    let destination = root.path().join("mutie.toml");
    std::os::unix::fs::symlink(&target, &destination).unwrap();
    assert!(!invoke(root.path(), &["--non-interactive"]).status.success());
    assert!(
        !invoke(root.path(), &["--force", "--non-interactive"])
            .status
            .success()
    );
    assert!(!target.exists());
    fs::write(&target, "preserve me\n").unwrap();
    assert!(
        !invoke(root.path(), &["--force", "--non-interactive"])
            .status
            .success()
    );
    assert_eq!(fs::read_to_string(&target).unwrap(), "preserve me\n");
    assert_eq!(fs::read_link(destination).unwrap(), target);
}

#[test]
fn fixture_manifests_generate_the_expected_adapters() {
    for (fixture, adapter) in [
        ("go", "go"),
        ("rust", "rust"),
        ("node", "node"),
        ("node-typescript", "node"),
        ("bun-js", "bun"),
        ("bun-typescript", "bun"),
        ("vitest", "vitest"),
    ] {
        let root = tempfile::tempdir().unwrap();
        for manifest in ["go.mod", "Cargo.toml", "package.json"] {
            let source = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures")
                .join(fixture)
                .join(manifest);
            if source.is_file() {
                fs::copy(source, root.path().join(manifest)).unwrap();
            }
        }
        let output = invoke(root.path(), &["--non-interactive"]);
        assert!(
            output.status.success(),
            "{fixture}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let config: toml::Value =
            toml::from_str(&fs::read_to_string(root.path().join("mutie.toml")).unwrap()).unwrap();
        if ["node", "bun", "vitest"].contains(&adapter) {
            assert_eq!(config["js"]["runner"].as_str(), Some(adapter));
        } else {
            assert!(config[adapter].is_table());
        }
    }
}

#[cfg(unix)]
#[test]
fn python_launcher_symlink_is_preserved_and_not_executed() {
    let root = tempfile::tempdir().unwrap();
    fs::write(
        root.path().join("pyproject.toml"),
        "[project]\nname='fixture'\n",
    )
    .unwrap();
    let launcher = root.path().join("python");
    fs::write(root.path().join("target"), "must not execute").unwrap();
    std::os::unix::fs::symlink(root.path().join("target"), &launcher).unwrap();
    let output = invoke(root.path(), &["--python", "python", "--non-interactive"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let config: toml::Value =
        toml::from_str(&fs::read_to_string(root.path().join("mutie.toml")).unwrap()).unwrap();
    assert_eq!(config["python"]["executable"].as_str(), Some("python"));
}

#[test]
fn package_managers_do_not_change_runner_selection_and_vite_plus_is_explicit() {
    for manager in ["npm@11.19.1", "pnpm@11.10.0", "yarn@4.18.1", "bun@1.3.11"] {
        let root = tempfile::tempdir().unwrap();
        let manifest = serde_json::json!({"name":"fixture","type":"module","packageManager":manager,"scripts":{"test":"vitest run"}});
        fs::write(
            root.path().join("package.json"),
            serde_json::to_vec(&manifest).unwrap(),
        )
        .unwrap();
        let output = invoke(root.path(), &["--non-interactive"]);
        assert!(
            output.status.success(),
            "{manager}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let config: toml::Value =
            toml::from_str(&fs::read_to_string(root.path().join("mutie.toml")).unwrap()).unwrap();
        assert_eq!(config["js"]["runner"].as_str(), Some("vitest"));
    }
    let root = tempfile::tempdir().unwrap();
    fs::write(
        root.path().join("package.json"),
        r#"{"scripts":{"test":"vp test"},"packageManager":"pnpm@11.10.0"}"#,
    )
    .unwrap();
    let output = invoke(root.path(), &["--non-interactive"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("separate attribution audit"));
    assert!(!root.path().join("mutie.toml").exists());
}
