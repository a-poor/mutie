//! Preview is read-only and does not require a real JavaScript runtime in default tests.
#![cfg(unix)]
use std::{fs, os::unix::fs::PermissionsExt, path::Path, process::Command};

fn write(root: &Path, name: &str, data: &[u8]) {
    let path = root.join(name);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, data).unwrap();
}
fn fixture() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/vitest");
    for name in [
        "package.json",
        "pnpm-lock.yaml",
        "src/flags.js",
        "src/flags.ts",
    ] {
        write(dir.path(), name, &fs::read(source.join(name)).unwrap());
    }
    write(
        dir.path(),
        "node_modules/.pnpm/lock.yaml",
        &fs::read(source.join("pnpm-lock.yaml")).unwrap(),
    );
    write(
        dir.path(),
        "node_modules/.modules.yaml",
        b"packageManager: pnpm@11.10.0\nnodeLinker: isolated\nvirtualStoreDir: .pnpm\n",
    );
    write(
        dir.path(),
        "node_modules/vitest/package.json",
        br#"{"name":"vitest","version":"4.1.9"}"#,
    );
    write(
        dir.path(),
        "node_modules/vitest/vitest.mjs",
        b"throw new Error('preview must not execute Vitest');",
    );
    write(
        dir.path(),
        "node_modules/vite/package.json",
        br#"{"name":"vite","version":"8.3.4"}"#,
    );
    write(dir.path(),"tools/node",b"#!/bin/sh\n[ \"$1\" = --eval ] || exit 99\nprintf '%s\\n' '{\"version\":\"v24.22.0\",\"executable\":\"/fake/node\"}'\n");
    fs::set_permissions(
        dir.path().join("tools/node"),
        fs::Permissions::from_mode(0o755),
    )
    .unwrap();
    dir
}
fn preview(root: &Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_mutie"))
        .arg("vitest-list")
        .arg(root)
        .args(args)
        .env("PATH", root.join("tools"))
        .output()
        .unwrap()
}
#[test]
fn lists_filters_and_preserves_source_without_executing_vitest() {
    let dir = fixture();
    let original = fs::read(dir.path().join("src/flags.ts")).unwrap();
    let result = preview(dir.path(), &[]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let stdout = String::from_utf8(result.stdout).unwrap();
    assert!(stdout.contains("preview only"));
    assert!(stdout.contains("6 mutations"));
    assert_eq!(
        stdout,
        String::from_utf8(preview(dir.path(), &[]).stdout).unwrap()
    );
    let filtered = preview(dir.path(), &["--include", "src/*.ts"]);
    assert!(filtered.status.success());
    assert!(String::from_utf8_lossy(&filtered.stdout).contains("3 mutations"));
    assert_eq!(fs::read(dir.path().join("src/flags.ts")).unwrap(), original);
    assert!(!dir.path().join(".mutie").exists());
}
#[test]
fn rejects_configuration_without_executing_it_and_rejects_runtime_mismatch() {
    let dir = fixture();
    write(
        dir.path(),
        "vite.config.ts",
        b"throw new Error('must not execute');",
    );
    let result = preview(dir.path(), &[]);
    assert!(!result.status.success());
    assert!(
        String::from_utf8_lossy(&result.stderr)
            .contains("unsupported Vitest project configuration")
    );
    assert!(result.stdout.is_empty());
    fs::remove_file(dir.path().join("vite.config.ts")).unwrap();
    write(
        dir.path(),
        "tools/node",
        b"#!/bin/sh\nprintf '%s\\n' '{\"version\":\"v20.5.1\",\"executable\":\"/fake/node\"}'\n",
    );
    let result = preview(dir.path(), &[]);
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("requires Node 24.22.0"));
}
