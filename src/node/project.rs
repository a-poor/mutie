use super::typescript::{TestPattern, TypeScriptScope};
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
    pub typescript: Option<TypeScriptScope>,
    pub default_discovery: bool,
    pub development_fingerprints: std::collections::BTreeMap<String, String>,
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
        let test_pattern = match manifest["scripts"]["test"].as_str() {
            Some("node --test") => TestPattern::Default,
            Some("node --test test/*.test.js") => TestPattern::JavaScript,
            Some("node --test test/*.test.ts") => TestPattern::TypeScript,
            _ => anyhow::bail!(
                "Node scope requires scripts.test: node --test, node --test test/*.test.js or node --test test/*.test.ts (no loaders or custom runner flags)"
            ),
        };
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
        let development_fingerprints =
            crate::dependency_tree::development(&root, &manifest, cancellation)?;
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
        let default_discovery = test_pattern == TestPattern::Default;
        let tests = if default_discovery {
            super::discovery::select(&root, &files, cancellation)?
        } else {
            files
                .iter()
                .filter(|p| {
                    p.parent() == Some(Path::new("test"))
                        && p.to_string_lossy().ends_with(test_pattern.suffix())
                })
                .map(|p| p.to_string_lossy().into_owned())
                .collect::<Vec<_>>()
        };
        ensure!(
            !tests.is_empty(),
            "Node scope requires {} files",
            test_pattern.glob()
        );
        let typescript = files
            .iter()
            .any(|p| p.extension().is_some_and(|e| e == "ts"))
            .then(|| TypeScriptScope::load(&root, test_pattern))
            .transpose()?;
        for file in &files {
            validate_imports(&root, file, &files, cancellation)?;
        }
        Ok(Self {
            root,
            name,
            manifest: super::fingerprint(text.as_bytes()),
            tests,
            typescript,
            default_discovery,
            development_fingerprints,
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
            !path.join("package.json").exists() && !path.join("tsconfig.json").exists(),
            "nested Node packages/configurations are unsupported"
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
            !matches!(ext, Some("tsx" | "jsx" | "mjs" | "cjs" | "mts" | "cts")),
            "Node scope supports .js and erasable .ts only: {}",
            relative.display()
        );
        ensure!(
            !relative.to_string_lossy().ends_with(".d.ts"),
            "Node declaration files are outside the initial native TypeScript scope"
        );
        if matches!(ext, Some("js" | "ts")) {
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
    let typescript = file.extension().is_some_and(|e| e == "ts");
    let language = if typescript {
        tree_sitter_typescript::LANGUAGE_TYPESCRIPT
    } else {
        tree_sitter_javascript::LANGUAGE
    };
    parser.set_language(&language.into())?;
    let tree = crate::javascript::parse(&mut parser, &source)
        .with_context(|| format!("cannot parse Node file {}", file.display()))?;
    let installed_development = root.join("node_modules").exists();
    let mut pending = vec![tree.root_node()];
    while let Some(node) = pending.pop() {
        cancellation.check()?;
        if typescript {
            super::typescript::validate_syntax(node, file)?;
        }
        if installed_development && matches!(node.kind(), "identifier" | "property_identifier") {
            ensure!(
                !matches!(
                    &source[node.byte_range()],
                    "createRequire" | "getBuiltinModule" | "eval" | "Function"
                ),
                "reflective module loading is unsupported with development installations"
            );
        }
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
                    "Node imports must resolve to audited src/test .js/.ts files: {spec}"
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
            root.path().join("src/unsupported.tsx"),
            "export const a=true;",
        )
        .unwrap();
        assert!(load().is_err());
        fs::remove_file(root.path().join("src/unsupported.tsx")).unwrap();
        fs::create_dir(root.path().join("node_modules")).unwrap();
        assert!(load().is_err());
    }
    #[test]
    fn installed_development_tools_are_preserved_but_not_importable() -> Result<()> {
        let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/node");
        let work = crate::workspace::Workspace::create(&source, &[], &Cancellation::default())?;
        let root = work.path();
        let mut manifest: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(root.join("package.json"))?)?;
        manifest["devDependencies"] = serde_json::json!({"tool":"1.0.0"});
        fs::write(root.join("package.json"), serde_json::to_vec(&manifest)?)?;
        fs::create_dir_all(root.join("node_modules/tool"))?;
        fs::write(
            root.join("node_modules/tool/index.js"),
            "throw Error('not imported')",
        )?;
        let project = Project::load(&root, &Cancellation::default())?;
        assert!(
            project
                .development_fingerprints
                .contains_key("node_modules")
        );
        for source in [
            "import 'tool'; export const flag=true;",
            "import {createRequire as loader} from 'node:module'; export const flag=true;",
        ] {
            fs::write(root.join("src/flags.js"), source)?;
            assert!(Project::load(&root, &Cancellation::default()).is_err());
        }
        Ok(())
    }

    #[test]
    fn audits_native_typescript_sources_tests_and_mixed_imports() {
        let original = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/node-typescript");
        let project = Project::load(&original, &Cancellation::default()).unwrap();
        assert_eq!(project.tests, vec!["test/flags.test.ts"]);
        assert!(project.typescript.is_some());
        let discovery = crate::javascript::discover(
            &original,
            &crate::mutation::Scope::new(&[], &[]).unwrap(),
            &Cancellation::default(),
        )
        .unwrap();
        assert_eq!(discovery.mutations.len(), 3);
        assert!(
            discovery
                .mutations
                .iter()
                .all(|m| m.file.path == Path::new("src/flags.ts"))
        );
        let root = fixture();
        let load = || Project::load(root.path(), &Cancellation::default());
        // Even helpers outside the mutation selection must pass the syntax audit.
        for source in [
            "enum E { Yes = 1 }",
            "namespace N { export const flag = true }",
            "namespace N { export type Flag = true }",
            "class C { constructor(public flag: boolean) {} }",
            "class C { constructor(readonly flag: boolean) {} }",
            "class C { constructor(private flag?: boolean) {} }",
            "const flag = <boolean>true;",
            "import fs = require('node:fs');",
            "import Alias = N.Value;",
            "declare const flag: boolean;",
            "@decorator class C {}",
            "import('../src/flags.js');",
            "import type {Flag} from 'external';",
            "import {enabled} from '../src/flags';",
        ] {
            fs::write(root.path().join("test/helper.ts"), source).unwrap();
            assert!(load().is_err(), "{source}");
        }
        fs::write(
            root.path().join("test/helper.ts"),
            "import '../src/flags.js'; export type Flag = true | false;",
        )
        .unwrap();
        let mixed = load().unwrap();
        assert_eq!(
            mixed.typescript.unwrap().test_pattern,
            TestPattern::JavaScript
        );
        fs::write(
            root.path().join("src/types.d.ts"),
            "export type Flag = true;",
        )
        .unwrap();
        assert!(load().is_err());
    }
    #[test]
    fn rejects_custom_scripts_and_runtime_dependencies() {
        for change in [
            serde_json::json!({"scripts":{"test":"node --test --watch"}}),
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
