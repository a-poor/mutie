use std::{
    fs,
    process::{Command, Output},
};
fn invoke(root: &std::path::Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_mutie"))
        .arg("init")
        .arg(root)
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
    assert_eq!(parsed["lang"].as_str(), Some("go"));
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
    std::os::unix::fs::symlink(root.path().join("absent"), root.path().join("mutie.toml")).unwrap();
    assert!(!invoke(root.path(), &["--non-interactive"]).status.success());
    assert!(!root.path().join("absent").exists());
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
        assert_eq!(config["lang"].as_str(), Some(adapter));
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
    let output = invoke(
        root.path(),
        &["--python", launcher.to_str().unwrap(), "--non-interactive"],
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let config: toml::Value =
        toml::from_str(&fs::read_to_string(root.path().join("mutie.toml")).unwrap()).unwrap();
    assert_eq!(config["python"].as_str(), launcher.to_str());
}
