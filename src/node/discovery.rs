//! Audit the complete native default selection before executing any project code.
use crate::cancellation::Cancellation;
use anyhow::{Result, ensure};
use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

pub const PATTERN: &str = "**/{test,test/**/*,test-*,*[._-]test}.{js,mjs,cjs,ts,mts,cts}";
pub const HASHES: [&str; 3] = [
    "b1bb09fb385d58ef4bc751153c3e36a8b95165f4768642bd273cc2d466fd0e1f",
    "56bc4b056d4f77ff87a9e691a385e91dff0c15ac47e4444179c5b7fdd0a26013",
    "ecee340045b8d9d2f4ebca8f20ffa69306613c838e28d916d89eecb78ef12a64",
];

pub fn select(
    root: &Path,
    audited: &BTreeSet<PathBuf>,
    cancellation: &Cancellation,
) -> Result<Vec<String>> {
    let mut selected = Vec::new();
    visit(
        root,
        Path::new(""),
        audited,
        cancellation,
        &mut 0,
        &mut selected,
    )?;
    // Native Array.prototype.sort compares UTF-16 code units, not UTF-8 bytes.
    selected.sort_by(|a, b| a.encode_utf16().cmp(b.encode_utf16()));
    Ok(selected)
}
fn visit(
    root: &Path,
    relative: &Path,
    audited: &BTreeSet<PathBuf>,
    cancellation: &Cancellation,
    count: &mut usize,
    selected: &mut Vec<String>,
) -> Result<()> {
    cancellation.check()?;
    *count += 1;
    ensure!(
        *count <= 100_000 && relative.components().count() < 100,
        "Node default discovery exceeds scope limit"
    );
    let path = root.join(relative);
    let meta = fs::symlink_metadata(&path)?;
    ensure!(
        !meta.file_type().is_symlink(),
        "Node default discovery does not support visible symlinks: {}",
        relative.display()
    );
    if meta.is_dir() {
        for entry in fs::read_dir(path)? {
            let name = entry?.file_name();
            let name = name
                .to_str()
                .ok_or_else(|| anyhow::anyhow!("Node default discovery requires UTF-8 paths"))?;
            // Node's glob ignores dot components and excludes node_modules.
            if name.starts_with('.') || name == "node_modules" {
                continue;
            }
            visit(
                root,
                &relative.join(name),
                audited,
                cancellation,
                count,
                selected,
            )?;
        }
    } else {
        ensure!(
            meta.is_file(),
            "unsupported Node discovery entry: {}",
            relative.display()
        );
        if matches_default(relative) {
            ensure!(
                relative.starts_with("test") && audited.contains(relative),
                "native Node test selection is outside audited test/ .js/.ts scope: {}",
                relative.display()
            );
            selected.push(relative.to_string_lossy().into_owned());
        }
    }
    Ok(())
}
fn matches_default(path: &Path) -> bool {
    if !matches!(
        path.extension().and_then(|e| e.to_str()),
        Some("js" | "mjs" | "cjs" | "ts" | "mts" | "cts")
    ) {
        return false;
    }
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
    path.parent()
        .is_some_and(|p| p.components().any(|c| c.as_os_str() == "test"))
        || stem == "test"
        || stem.starts_with("test-")
        || [".test", "_test", "-test"]
            .iter()
            .any(|suffix| stem.ends_with(suffix))
}

pub fn expression() -> String {
    format!(
        "const files=require('node:fs').globSync({},{{cwd:process.cwd(),exclude:name=>name==='node_modules'}}).sort(); const discoveryHashes=['internal/test_runner/runner','internal/test_runner/utils','internal/fs/glob'].map(k=>require('node:crypto').createHash('sha256').update(process.binding('natives')[k]).digest('hex'));",
        serde_json::to_string(PATTERN).unwrap()
    )
}
pub fn validate_native(value: &serde_json::Value, expected: &[String]) -> Result<()> {
    ensure!(
        value["discoveryHashes"] == serde_json::json!(HASHES),
        "Node default discovery requires the audited runner, pattern and glob implementation"
    );
    ensure!(
        value["files"] == serde_json::json!(expected),
        "native Node default test selection disagrees with audited files; no tests were run"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn tree(names: &[&str]) -> tempfile::TempDir {
        let root = tempfile::tempdir().unwrap();
        for name in names {
            let path = root.path().join(name);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, "").unwrap();
        }
        root
    }
    #[test]
    fn audits_full_default_selection_and_never_drops_out_of_scope_tests() {
        let names = [
            "test/one.test.ts",
            "test/helper.js",
            "test/nested/case.ts",
            "src/app.ts",
            "test/data.json",
            ".git/test.js",
            ".hidden/test.js",
            "test/.hidden.ts",
            "extra/node_modules/test.js",
        ];
        let root = tree(&names);
        let audited = names.iter().map(PathBuf::from).collect();
        assert_eq!(
            select(root.path(), &audited, &Cancellation::default()).unwrap(),
            ["test/helper.js", "test/nested/case.ts", "test/one.test.ts"]
        );
        for name in [
            "outside.test.ts",
            "test.js",
            "test-case.js",
            "nested/a_test.ts",
            "nested/a-test.mjs",
            "nested/test/helper.cts",
            "src/test.ts",
            "dist/test.js",
        ] {
            let path = root.path().join(name);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(&path, "").unwrap();
            assert!(
                select(root.path(), &audited, &Cancellation::default()).is_err(),
                "{name}"
            );
            fs::remove_file(path).unwrap();
        }
        assert!(select(root.path(), &BTreeSet::new(), &Cancellation::default()).is_err());
    }
    #[test]
    fn checks_native_inventory_and_implementation_before_execution() {
        let expected = vec!["test/a.ts".to_owned()];
        let mut record = serde_json::json!({"files":expected,"discoveryHashes":HASHES});
        validate_native(&record, &expected).unwrap();
        record["files"] = serde_json::json!(["test/extra.ts"]);
        assert!(validate_native(&record, &expected).is_err());
        record["files"] = serde_json::json!(expected);
        record["discoveryHashes"][0] = "changed".into();
        assert!(validate_native(&record, &expected).is_err());
    }
    #[cfg(unix)]
    #[test]
    fn rejects_visible_symlinks_and_honors_cancellation() {
        let root = tree(&["test/a.ts"]);
        let audited = [PathBuf::from("test/a.ts")].into_iter().collect();
        std::os::unix::fs::symlink(root.path().join("test"), root.path().join("linked")).unwrap();
        assert!(select(root.path(), &audited, &Cancellation::default()).is_err());
        fs::remove_file(root.path().join("linked")).unwrap();
        let cancellation = Cancellation::default();
        cancellation.cancel();
        assert!(select(root.path(), &audited, &cancellation).is_err());
    }
}
