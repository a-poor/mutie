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
            manifest["type"] == "module"
                && manifest["name"].as_str().is_some_and(|n| !n.is_empty()),
            "Bun requires a named ESM package"
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
            validate_imports(&root, file, &code, cancellation)?;
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
            ensure!(
                entry.file_name() != "node_modules",
                "installed Bun dependencies unsupported"
            );
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
        ensure!(
            !matches!(ext, Some("jsx" | "tsx" | "mjs" | "cjs" | "mts" | "cts")),
            "unsupported Bun source extension"
        );
        if matches!(ext, Some("js" | "ts")) {
            let executable = relative.starts_with("src")
                || relative.starts_with("test")
                || relative.starts_with("tests");
            ensure!(
                executable || relative.starts_with("examples") || relative.starts_with("scripts"),
                "Bun source must be audited src/test code or non-executable examples/scripts"
            );
            ensure!(
                executable || !is_test(relative),
                "Bun tests outside test/ or tests/ are unsupported"
            );
            ensure!(
                !name.ends_with(".d.ts")
                    && !name.contains(".spec.")
                    && !name.contains("_test.")
                    && !name.contains("_spec."),
                "unsupported Bun declaration/test naming"
            );
            ensure!(
                !is_test(relative) || !relative.starts_with("src"),
                "in-source Bun tests unsupported"
            );
            if executable {
                code.insert(relative.to_owned());
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
