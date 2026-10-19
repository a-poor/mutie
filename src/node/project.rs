use crate::cancellation::Cancellation;
use anyhow::{Context, Result, ensure};
use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

pub struct Project {
    pub root: PathBuf,
    pub name: String,
    pub manifest: String,
    pub tests: Vec<String>,
}
impl Project {
    pub fn load(root: &Path, cancellation: &Cancellation) -> Result<Self> {
        let root = root.canonicalize()?;
        ensure!(
            fs::symlink_metadata(root.join("package.json"))?
                .file_type()
                .is_file(),
            "Node manifest must be a regular file"
        );
        let text = fs::read_to_string(root.join("package.json"))?;
        let manifest: serde_json::Value = serde_json::from_str(&text)?;
        ensure!(
            manifest["type"] == "module",
            "Node runner requires an ESM package (type: module)"
        );
        let name = manifest["name"]
            .as_str()
            .filter(|s| !s.is_empty())
            .context("Node package requires a name")?
            .to_owned();
        ensure!(
            manifest["scripts"]["test"] == "node --test test/*.test.js",
            "Node scope requires scripts.test: node --test test/*.test.js (no loaders or custom runner flags)"
        );
        for key in [
            "workspaces",
            "imports",
            "dependencies",
            "optionalDependencies",
            "peerDependencies",
        ] {
            ensure!(
                manifest
                    .get(key)
                    .is_none_or(|v| v.as_object().is_some_and(|m| m.is_empty())),
                "unsupported Node package setting: {key}"
            );
        }
        ensure!(
            !root.join("node_modules").exists(),
            "Node initial scope requires no installed dependencies"
        );
        for parent in root.ancestors().skip(1) {
            ensure!(
                !parent.join("pnpm-workspace.yaml").exists(),
                "enclosing workspace is unsupported"
            );
            if let Ok(text) = fs::read_to_string(parent.join("package.json")) {
                let value: serde_json::Value = serde_json::from_str(&text)?;
                ensure!(
                    value.get("workspaces").is_none(),
                    "enclosing workspace is unsupported"
                );
            }
        }
        let mut files = BTreeSet::new();
        for dir in ["src", "test"] {
            collect(&root, Path::new(dir), &mut files, cancellation)?;
        }
        let tests = files
            .iter()
            .filter(|p| {
                p.parent() == Some(Path::new("test")) && p.to_string_lossy().ends_with(".test.js")
            })
            .map(|p| p.to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        ensure!(
            !tests.is_empty(),
            "Node scope requires test/*.test.js files"
        );
        for file in &files {
            validate_imports(&root, file, &files, cancellation)?;
        }
        Ok(Self {
            root,
            name,
            manifest: super::fingerprint(text.as_bytes()),
            tests,
        })
    }
}
fn collect(
    root: &Path,
    relative: &Path,
    files: &mut BTreeSet<PathBuf>,
    cancellation: &Cancellation,
) -> Result<()> {
    cancellation.check()?;
    ensure!(
        relative.components().count() < 100 && files.len() < 10000,
        "Node project exceeds scope limit"
    );
    let path = root.join(relative);
    let meta = fs::symlink_metadata(&path)?;
    ensure!(
        !meta.file_type().is_symlink(),
        "Node source/test symlinks are unsupported: {}",
        relative.display()
    );
    if meta.is_dir() {
        ensure!(
            !path.join("package.json").exists(),
            "nested Node packages are unsupported"
        );
        for entry in fs::read_dir(path)? {
            collect(
                root,
                &relative.join(entry?.file_name()),
                files,
                cancellation,
            )?;
        }
    } else {
        ensure!(meta.is_file(), "unsupported Node project entry");
        let ext = path.extension().and_then(|s| s.to_str());
        ensure!(
            !matches!(
                ext,
                Some("ts" | "tsx" | "jsx" | "mjs" | "cjs" | "mts" | "cts")
            ),
            "Node initial scope supports JavaScript .js only: {}",
            relative.display()
        );
        if ext == Some("js") {
            files.insert(relative.to_owned());
        }
    }
    Ok(())
}
fn validate_imports(
    root: &Path,
    file: &Path,
    files: &BTreeSet<PathBuf>,
    cancellation: &Cancellation,
) -> Result<()> {
    let source = fs::read_to_string(root.join(file))?;
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&tree_sitter_javascript::LANGUAGE.into())?;
    let tree = crate::javascript::parse(&mut parser, &source)
        .with_context(|| format!("cannot parse Node file {}", file.display()))?;
    let mut pending = vec![tree.root_node()];
    while let Some(node) = pending.pop() {
        cancellation.check()?;
        let import = node.child_by_field_name("source");
        if node.kind() == "call_expression"
            && let Some(function) = node.child_by_field_name("function")
        {
            let name = &source[function.byte_range()];
            ensure!(
                name != "require",
                "CommonJS require is unsupported in {}",
                file.display()
            );
            ensure!(
                name != "import",
                "dynamic imports are unsupported: body exceptions cannot distinguish module evaluation failures"
            );
        }
        if let Some(import) = import {
            ensure!(
                import.kind() == "string",
                "Node imports must use literal paths"
            );
            let spec = source[import.byte_range()].trim_matches(['\'', '"']);
            ensure!(!spec.contains('\\'), "escaped Node imports are unsupported");
            if spec.starts_with("node:") {
                ensure!(
                    !file.starts_with("src") || spec != "node:test",
                    "in-source Node tests are unsupported"
                );
            } else {
                ensure!(
                    spec.starts_with("./") || spec.starts_with("../"),
                    "Node initial scope allows only relative or node: imports: {spec}"
                );
                let path = root
                    .join(file.parent().unwrap())
                    .join(spec)
                    .canonicalize()
                    .with_context(|| format!("resolving {spec} from {}", file.display()))?;
                let relative = path
                    .strip_prefix(root)
                    .context("Node imports must stay inside the project")?;
                ensure!(
                    files.contains(relative),
                    "Node imports must resolve to audited src/test .js files: {spec}"
                );
            }
        }
        let mut cursor = node.walk();
        pending.extend(node.named_children(&mut cursor));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> tempfile::TempDir {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir(root.path().join("src")).unwrap();
        fs::create_dir(root.path().join("test")).unwrap();
        fs::write(
            root.path().join("package.json"),
            include_str!("../../tests/fixtures/node/package.json"),
        )
        .unwrap();
        fs::write(
            root.path().join("src/flags.js"),
            "export const enabled = true;",
        )
        .unwrap();
        fs::write(
            root.path().join("test/flags.test.js"),
            "import {test} from 'node:test'; import '../src/flags.js'; test('ok',()=>{});",
        )
        .unwrap();
        root
    }
    #[test]
    fn accepts_only_audited_local_javascript_imports_and_layout() {
        let root = fixture();
        let load = || Project::load(root.path(), &Cancellation::default());
        assert_eq!(load().unwrap().tests, vec!["test/flags.test.js"]);
        for source in [
            "import x from 'dependency';",
            "import('../outside.js');",
            "import(foo);",
            "import('../test/flags.test.js');",
            "import {test} from 'node:test';",
            "require('fs');",
            "const x = ;",
        ] {
            fs::write(root.path().join("src/flags.js"), source).unwrap();
            assert!(load().is_err(), "{source}");
        }
        fs::write(
            root.path().join("src/flags.js"),
            "export const enabled = true;",
        )
        .unwrap();
        fs::write(
            root.path().join("src/unsupported.ts"),
            "export const a=true;",
        )
        .unwrap();
        assert!(load().is_err());
        fs::remove_file(root.path().join("src/unsupported.ts")).unwrap();
        fs::create_dir(root.path().join("node_modules")).unwrap();
        assert!(load().is_err());
    }
    #[test]
    fn rejects_custom_scripts_and_runtime_dependencies() {
        for change in [
            serde_json::json!({"scripts":{"test":"node --test"}}),
            serde_json::json!({"dependencies":{"foo":"1"}}),
            serde_json::json!({"workspaces":["packages/*"]}),
            serde_json::json!({"imports":{"#x":"./src/flags.js"}}),
            serde_json::json!({"type":"commonjs"}),
        ] {
            let root = fixture();
            let mut manifest: serde_json::Value =
                serde_json::from_str(include_str!("../../tests/fixtures/node/package.json"))
                    .unwrap();
            for (k, v) in change.as_object().unwrap() {
                manifest[k] = v.clone();
            }
            fs::write(
                root.path().join("package.json"),
                serde_json::to_vec(&manifest).unwrap(),
            )
            .unwrap();
            assert!(
                Project::load(root.path(), &Cancellation::default()).is_err(),
                "{change}"
            );
        }
    }
}
