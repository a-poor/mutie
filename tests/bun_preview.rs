use serde_json::Value;
use std::{fs, path::Path, process::Command};

fn fixture(syntax: &str) -> tempfile::TempDir {
    let root = tempfile::tempdir().unwrap();
    fs::create_dir(root.path().join("src")).unwrap();
    fs::create_dir(root.path().join("test")).unwrap();
    fs::write(
        root.path().join("package.json"),
        r#"{"name":"preview","type":"module","scripts":{"test":"bun test"}}"#,
    )
    .unwrap();
    fs::write(root.path().join(format!("src/flags.{syntax}")), if syntax == "ts" { "declare const Brand: unique symbol; declare const TypeOnly: true; type Always = true; export const enabled: boolean = true; export function fallback(){return false;}" } else { "export const enabled = true; export function fallback(){return false;}" }).unwrap();
    fs::write(
        root.path().join(format!("test/flags.test.{syntax}")),
        "import {test} from 'bun:test'; test('body',()=>{});",
    )
    .unwrap();
    root
}
fn list(root: &Path, args: &[&str]) -> std::process::Output {
    // An empty search path proves discovery does not require a Bun installation.
    Command::new(env!("CARGO_BIN_EXE_mutie"))
        .args(["bun-list", "--json"])
        .arg(root)
        .args(args)
        .env("PATH", "")
        .output()
        .unwrap()
}

#[test]
fn previews_js_ts_and_filters_without_a_runtime_or_writes() {
    for syntax in ["js", "ts"] {
        let root = fixture(syntax);
        let source = root.path().join(format!("src/flags.{syntax}"));
        let before = fs::read(&source).unwrap();
        let output = list(root.path(), &[]);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let value: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(value["runtime_checked"], false);
        let mutations = value["mutations"].as_array().unwrap();
        assert_eq!(mutations.len(), 2); // TS type literal is not executable.
        for (i, m) in mutations.iter().enumerate() {
            assert_eq!(m["id"], i + 1);
            let range = m["byte_start"].as_u64().unwrap() as usize
                ..m["byte_end"].as_u64().unwrap() as usize;
            assert_eq!(&before[range], m["original"].as_str().unwrap().as_bytes());
        }
        let filtered = list(root.path(), &["--exclude", "src/**"]);
        assert!(filtered.status.success());
        let filtered: Value = serde_json::from_slice(&filtered.stdout).unwrap();
        assert_eq!(filtered["mutations"], serde_json::json!([]));
        assert_eq!(filtered["input_fingerprint"], value["input_fingerprint"]);
        assert_eq!(fs::read(&source).unwrap(), before);
        assert!(!root.path().join(".mutie").exists());
        // Filtering all production files must not hide an invalid test/import.
        fs::write(
            root.path().join(format!("test/bad.test.{syntax}")),
            "import('missing');",
        )
        .unwrap();
        assert!(!list(root.path(), &["--exclude", "src/**"]).status.success());
    }
}
