use crate::cancellation::Cancellation;
use anyhow::{Context, Result, ensure};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

#[derive(Debug)]
pub struct Project {
    pub root: PathBuf,
    pub files: BTreeMap<String, String>,
    pub tests: Vec<String>,
}
impl Project {
    pub fn load(root: &Path, cancellation: &Cancellation) -> Result<Self> {
        let root = root.canonicalize()?;
        let mut files = BTreeMap::new();
        let mut code = BTreeSet::new();
        collect(&root, Path::new(""), &mut files, &mut code, cancellation)?;
        let manifest: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(root.join("package.json"))?)?;
        ensure!(
            (manifest.get("type").is_none() || manifest["type"] == "module")
                && manifest["name"].as_str().is_some_and(|n| !n.is_empty()),
            "Bun requires a named package with module or absent type"
        );
        ensure!(
            manifest["scripts"]["test"] == "bun test",
            "Bun requires scripts.test: bun test without custom flags"
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
                "unsupported Bun package setting: {key}"
            );
        }
        files.extend(crate::dependency_tree::development(
            &root,
            &manifest,
            cancellation,
        )?);
        for parent in root.ancestors().skip(1) {
            ensure!(
                !parent.join("pnpm-workspace.yaml").exists(),
                "enclosing workspace unsupported"
            );
            if let Ok(text) = fs::read_to_string(parent.join("package.json")) {
                let value: serde_json::Value = serde_json::from_str(&text)?;
                ensure!(
                    value.get("workspaces").is_none(),
                    "enclosing workspace unsupported"
                );
            }
        }
        super::config::validate(&root)?;
        let tests: Vec<_> = code
            .iter()
            .filter(|p| is_test(p))
            .map(|p| p.to_string_lossy().into_owned())
            .collect();
        ensure!(
            !tests.is_empty(),
            "Bun requires .test.js/.test.ts files under test/ or tests/"
        );
        for file in &code {
            validate_imports(
                &root,
                file,
                &code,
                manifest.get("type").is_none(),
                cancellation,
            )?;
        }
        Ok(Self { root, files, tests })
    }
}
fn is_test(path: &Path) -> bool {
    let text = path.to_string_lossy();
    text.ends_with(".test.js") || text.ends_with(".test.ts")
}
fn collect(
    root: &Path,
    relative: &Path,
    files: &mut BTreeMap<String, String>,
    code: &mut BTreeSet<PathBuf>,
    cancellation: &Cancellation,
) -> Result<()> {
    cancellation.check()?;
    ensure!(
        relative.components().count() < 64 && files.len() < 10000,
        "Bun project exceeds scope limits"
    );
    let path = root.join(relative);
    let meta = fs::symlink_metadata(&path)?;
    ensure!(
        !meta.file_type().is_symlink(),
        "Bun symlinks unsupported: {}",
        relative.display()
    );
    if meta.is_dir() {
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            if entry.file_name() == ".git" {
                continue;
            }
            if relative.as_os_str().is_empty() && entry.file_name() == "node_modules" {
                continue;
            }
            collect(
                root,
                &relative.join(entry.file_name()),
                files,
                code,
                cancellation,
            )?;
        }
    } else {
        ensure!(
            meta.is_file() && meta.len() <= 8 * 1024 * 1024,
            "unsupported or oversized Bun input: {}",
            relative.display()
        );
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .context("Bun paths must be UTF-8")?;
        ensure!(
            name != "bunfig.toml"
                && !name.starts_with(".env")
                && (relative == Path::new("package.json") || name != "package.json")
                && (name != "tsconfig.json" || relative == Path::new("tsconfig.json"))
                && name != "jsconfig.json",
            "unsupported Bun configuration: {}",
            relative.display()
        );
        let ext = path.extension().and_then(|s| s.to_str());
        let executable = relative.starts_with("src")
            || relative.starts_with("test")
            || relative.starts_with("tests");
        let declaration = name.ends_with(".d.ts");
        let ancillary = relative.starts_with("examples")
            || relative.starts_with("scripts")
            || relative.starts_with("dist");
        ensure!(
            !matches!(ext, Some("jsx" | "tsx" | "mts" | "cts")),
            "unsupported Bun source extension"
        );
        if matches!(ext, Some("js" | "ts" | "mjs" | "cjs")) {
            // Bun's native test search must never discover tests in packaging
            // directories. Keep all these bytes fingerprinted but non-importable.
            let test_name = name.contains(".test.")
                || name.contains(".spec.")
                || name.contains("_test.")
                || name.contains("_spec.");
            ensure!(
                !test_name || (executable && !declaration && is_test(relative)),
                "unsupported Bun declaration/test naming"
            );
            if declaration {
                ensure!(
                    !executable
                        && (relative.components().count() == 1 || relative.starts_with("dist")),
                    "Bun declarations are only accepted as root/dist packaging files"
                );
            } else if executable {
                ensure!(
                    matches!(ext, Some("js" | "ts")),
                    "unsupported Bun source extension"
                );
                ensure!(
                    !is_test(relative) || !relative.starts_with("src"),
                    "in-source Bun tests unsupported"
                );
                code.insert(relative.to_owned());
            } else {
                ensure!(
                    ancillary,
                    "Bun source must be audited src/test code or ancillary examples/scripts/dist"
                );
            }
        }
        files.insert(
            relative
                .to_str()
                .context("Bun paths must be UTF-8")?
                .to_owned(),
            super::fingerprint(&fs::read(path)?),
        );
    }
    Ok(())
}

fn validate_imports(
    root: &Path,
    file: &Path,
    files: &BTreeSet<PathBuf>,
    implicit_esm: bool,
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
        .with_context(|| format!("cannot parse Bun file {}", file.display()))?;
    if implicit_esm {
        ensure!(!typescript, "Bun implicit ESM scope is JavaScript-only");
        let mut cursor = tree.root_node().walk();
        ensure!(
            tree.root_node()
                .named_children(&mut cursor)
                .any(|node| matches!(node.kind(), "import_statement" | "export_statement")),
            "Bun implicit ESM requires static import/export syntax in {}",
            file.display()
        );
    }
    let installed_development = root.join("node_modules").exists();
    let mut pending = vec![tree.root_node()];
    while let Some(node) = pending.pop() {
        cancellation.check()?;
        if implicit_esm && node.kind() == "identifier" {
            ensure!(
                !matches!(
                    &source[node.byte_range()],
                    "require" | "module" | "exports" | "__dirname" | "__filename"
                ),
                "CommonJS identifiers are unsupported in Bun implicit ESM: {}",
                file.display()
            );
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
                "Bun imports must use literal paths"
            );
            let spec = source[import.byte_range()].trim_matches(['\'', '"']);
            ensure!(!spec.contains('\\'), "escaped Bun imports are unsupported");
            if spec.starts_with("node:") || spec == "bun:test" {
                ensure!(
                    spec != "node:test" && (!file.starts_with("src") || spec != "bun:test"),
                    "in-source Bun tests are unsupported"
                );
            } else {
                ensure!(
                    spec.starts_with("./") || spec.starts_with("../"),
                    "Bun initial scope allows only relative or node: imports: {spec}"
                );
                let path = root
                    .join(file.parent().unwrap())
                    .join(spec)
                    .canonicalize()
                    .with_context(|| format!("resolving {spec} from {}", file.display()))?;
                let relative = path
                    .strip_prefix(root)
                    .context("Bun imports must stay inside the project")?;
                ensure!(
                    files.contains(relative),
                    "Bun imports must resolve to audited src/test .js/.ts files: {spec}"
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
        for directory in ["src", "test", "dist", "scripts"] {
            fs::create_dir(root.path().join(directory)).unwrap();
        }
        fs::write(
            root.path().join("package.json"),
            r#"{"name":"implicit-esm","scripts":{"test":"bun test"}}"#,
        )
        .unwrap();
        fs::write(
            root.path().join("src/index.js"),
            "export const enabled = true;",
        )
        .unwrap();
        fs::write(root.path().join("test/index.test.js"), "import {test} from 'bun:test'; import {enabled} from '../src/index.js'; test('enabled',()=>{});").unwrap();
        root
    }

    #[test]
    fn implicit_esm_packaging_is_fingerprinted_but_not_executable() -> Result<()> {
        let root = fixture();
        for file in [
            "index.d.ts",
            "dist/index.d.ts",
            "dist/index.js",
            "dist/index.mjs",
            "dist/index.cjs",
            "scripts/build.mjs",
        ] {
            // Deliberately invalid/external content: these are opaque packaging
            // bytes. Importing them would have to fail before execution.
            fs::write(
                root.path().join(file),
                "not executable; require('external');",
            )?;
        }
        let load = || Project::load(root.path(), &Cancellation::default());
        let initial = load()?;
        assert_eq!(initial.tests, ["test/index.test.js"]);
        assert_eq!(initial.files.len(), 9);
        fs::write(root.path().join("dist/index.cjs"), "changed packaging")?;
        assert_ne!(initial.files, load()?.files);
        for target in [
            "../index.d.ts",
            "../dist/index.js",
            "../dist/index.mjs",
            "../dist/index.cjs",
            "../scripts/build.mjs",
        ] {
            fs::write(
                root.path().join("src/index.js"),
                format!("import '{target}'; export const enabled = true;"),
            )?;
            assert!(load().is_err(), "{target}");
        }
        Ok(())
    }

    #[test]
    fn packaging_cannot_hide_tests_or_runtime_extensions() -> Result<()> {
        let root = fixture();
        for file in [
            "dist/hidden.test.js",
            "dist/hidden.test.mjs",
            "dist/hidden.spec.cjs",
            "dist/hidden_test.ts",
            "scripts/build.test.mjs",
            "index.test.d.ts",
            "dist/hidden.test.d.ts",
            "src/index.d.ts",
            "src/extra.mjs",
            "test/extra.cjs",
            "dist/nested/package.json",
        ] {
            let path = root.path().join(file);
            fs::create_dir_all(path.parent().unwrap())?;
            fs::write(&path, "{}")?;
            assert!(
                Project::load(root.path(), &Cancellation::default()).is_err(),
                "{file}"
            );
            fs::remove_file(path)?;
        }
        Ok(())
    }

    #[test]
    fn absent_type_requires_static_javascript_esm() -> Result<()> {
        let root = fixture();
        for source in [
            "const enabled = true;",
            "module.exports = true;",
            "export const enabled = require('fs');",
            "export const enabled = module.exports;",
            "export const enabled = __dirname;",
        ] {
            fs::write(root.path().join("src/index.js"), source)?;
            assert!(
                Project::load(root.path(), &Cancellation::default()).is_err(),
                "{source}"
            );
        }
        fs::write(
            root.path().join("src/index.js"),
            "export const enabled = true;",
        )?;
        for kind in ["commonjs", "unknown", ""] {
            fs::write(
                root.path().join("package.json"),
                format!(r#"{{"name":"test","type":"{kind}","scripts":{{"test":"bun test"}}}}"#),
            )?;
            assert!(Project::load(root.path(), &Cancellation::default()).is_err());
        }
        fs::write(
            root.path().join("package.json"),
            r#"{"name":"test","scripts":{"test":"bun test"}}"#,
        )?;
        fs::write(
            root.path().join("src/extra.ts"),
            "export const enabled: boolean = true;",
        )?;
        assert!(Project::load(root.path(), &Cancellation::default()).is_err());
        Ok(())
    }
}

#[cfg(test)]
mod development_tests {
    use super::*;
    #[test]
    fn development_tree_is_in_identity_and_runtime_imports_stay_rejected() -> Result<()> {
        let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/bun-js");
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
        assert!(project.files.contains_key("node_modules"));
        fs::write(
            root.join("src/dependency.js"),
            "import 'tool'; export const flag=true;",
        )?;
        assert!(Project::load(&root, &Cancellation::default()).is_err());
        Ok(())
    }
}
