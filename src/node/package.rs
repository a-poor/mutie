//! Owned ESM package graph for the composed lint/test recipe.
use super::{project::Project, recipe};
use crate::cancellation::Cancellation;
use anyhow::{Context, Result, ensure};
use serde_json::Value;
use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

pub fn load(
    root: PathBuf,
    manifest: &Value,
    text: &str,
    cancellation: &Cancellation,
) -> Result<Project> {
    ensure!(
        manifest["type"] == "module",
        "composed Node packages must use ESM"
    );
    ensure!(
        manifest["exports"]["default"] == "./index.js"
            && manifest["exports"]
                .as_object()
                .is_some_and(|exports| exports
                    .keys()
                    .all(|key| matches!(key.as_str(), "default" | "types"))),
        "composed Node scope requires the root index.js package export"
    );
    for key in ["workspaces", "imports", "optionalDependencies"] {
        ensure!(
            manifest
                .get(key)
                .is_none_or(|v| v.as_object().is_some_and(|o| o.is_empty())),
            "unsupported composed Node setting: {key}"
        );
    }
    for parent in root.ancestors().skip(1) {
        ensure!(
            !parent.join("pnpm-workspace.yaml").exists(),
            "enclosing workspaces require separate support"
        );
        if let Ok(bytes) = fs::read(parent.join("package.json")) {
            let value: Value = serde_json::from_slice(&bytes)?;
            ensure!(
                value.get("workspaces").is_none(),
                "enclosing workspaces require separate support"
            );
        }
    }
    let recipe = recipe::Scope::load(&root, manifest, cancellation)?;
    let development_fingerprints =
        crate::dependency_tree::development(&root, manifest, cancellation)?;
    ensure!(
        development_fingerprints.contains_key("node_modules"),
        "composed Node recipe requires owned installed dependencies"
    );
    let mut sources = BTreeSet::from([PathBuf::from("index.js")]);
    collect_source(&root, Path::new("rules"), &mut sources, cancellation)?;
    let mut tests = Vec::new();
    for directory in ["test", "test/unit"] {
        let path = root.join(directory);
        ensure!(
            fs::symlink_metadata(&path)?.is_dir(),
            "Node test directories must be local directories"
        );
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            if entry.path().extension().is_some_and(|e| e == "js") {
                ensure!(
                    entry.file_type()?.is_file(),
                    "selected Node tests must be regular files"
                );
                tests.push(format!(
                    "{directory}/{}",
                    entry
                        .file_name()
                        .to_str()
                        .context("Node test names must be UTF-8")?
                ));
            }
        }
    }
    tests.sort_by(|a, b| a.encode_utf16().cmp(b.encode_utf16()));
    ensure!(
        !tests.is_empty(),
        "composed Node recipe has no selected tests"
    );
    let mut pending = sources.iter().cloned().collect::<Vec<_>>();
    pending.extend(tests.iter().map(PathBuf::from));
    pending.extend(
        [
            "eslint.config.js",
            "eslint.dogfooding.config.js",
            ".eslint-doc-generatorrc.js",
        ]
        .map(PathBuf::from),
    );
    let mut visited = BTreeSet::new();
    while let Some(file) = pending.pop() {
        cancellation.check()?;
        ensure!(visited.len() < 10000, "Node module graph exceeds limit");
        if visited.insert(file.clone()) {
            audit(&root, &file, &sources, manifest, &mut pending, cancellation)?;
        }
    }
    Ok(Project {
        root,
        name: manifest["name"]
            .as_str()
            .filter(|s| !s.is_empty())
            .context("Node package requires a name")?
            .into(),
        manifest: super::fingerprint(text.as_bytes()),
        tests,
        typescript: None,
        default_discovery: false,
        development_fingerprints,
        recipe: Some(recipe),
        sources: sources.into_iter().collect(),
    })
}

fn collect_source(
    root: &Path,
    relative: &Path,
    sources: &mut BTreeSet<PathBuf>,
    cancellation: &Cancellation,
) -> Result<()> {
    cancellation.check()?;
    ensure!(
        relative.components().count() < 100 && sources.len() < 10000,
        "Node source tree exceeds limit"
    );
    let path = root.join(relative);
    let meta = fs::symlink_metadata(&path)?;
    if meta.is_dir() {
        ensure!(
            !path.join("package.json").exists() && !path.join("tsconfig.json").exists(),
            "nested application packages/configuration require separate support"
        );
        for entry in fs::read_dir(path)? {
            collect_source(
                root,
                &relative.join(entry?.file_name()),
                sources,
                cancellation,
            )?;
        }
    } else {
        ensure!(
            meta.is_file(),
            "Node source links/special files are unsupported"
        );
        ensure!(
            path.extension().is_some_and(|e| e == "js"),
            "composed Node application scope requires JavaScript sources"
        );
        sources.insert(relative.into());
    }
    Ok(())
}

fn literal<'a>(node: tree_sitter::Node<'_>, source: &'a str) -> Result<&'a str> {
    ensure!(
        node.kind() == "string",
        "module specifiers must be string literals"
    );
    let spec = source[node.byte_range()].trim_matches(['\'', '"']);
    ensure!(
        !spec.contains('\\'),
        "escaped module specifiers need separate support"
    );
    Ok(spec)
}

fn import(
    root: &Path,
    file: &Path,
    spec: &str,
    sources: &BTreeSet<PathBuf>,
    manifest: &Value,
    pending: &mut Vec<PathBuf>,
) -> Result<()> {
    if spec.starts_with("node:") {
        ensure!(
            spec != "node:test" || !sources.contains(file),
            "in-source Node tests are unsupported"
        );
    } else if spec.starts_with("./") || spec.starts_with("../") {
        let path = root.join(file.parent().unwrap()).join(spec);
        ensure!(
            !fs::symlink_metadata(&path)?.file_type().is_symlink(),
            "module links need separate support"
        );
        let path = path.canonicalize()?;
        let relative = path
            .strip_prefix(root)
            .context("Node imports must stay inside the owned project")?;
        if relative == Path::new("package.json") {
            let _: Value = serde_json::from_slice(&fs::read(path)?)?;
            return Ok(());
        }
        ensure!(
            relative.extension().is_some_and(|e| e == "js")
                && (sources.contains(relative)
                    || relative.starts_with("test")
                    || relative.starts_with("scripts")),
            "import outside audited Node executable graph: {spec}"
        );
        ensure!(
            !sources.contains(file) || sources.contains(relative),
            "application imports cannot execute test/tooling helpers: {spec}"
        );
        pending.push(relative.to_owned());
    } else {
        ensure!(
            !spec.starts_with(['/', '#'])
                && !spec.contains(':')
                && spec.split('/').all(|c| !matches!(c, "" | "." | "..")),
            "unsupported Node module specifier: {spec}"
        );
        let package = if spec.starts_with('@') {
            let mut parts = spec.split('/');
            format!(
                "{}/{}",
                parts.next().unwrap(),
                parts.next().context("invalid scoped package")?
            )
        } else {
            spec.split('/').next().unwrap().into()
        };
        ensure!(
            ["dependencies", "devDependencies", "peerDependencies"]
                .iter()
                .any(|key| manifest[*key].get(&package).is_some()),
            "Node import must name a declared dependency: {spec}"
        );
        let installed = root.join("node_modules").join(&package).canonicalize()?;
        ensure!(
            installed.starts_with(root.join("node_modules")),
            "dependency import escapes owned installation"
        );
        let value: Value = serde_json::from_slice(&fs::read(installed.join("package.json"))?)?;
        ensure!(
            value["name"] == package,
            "installed dependency name disagrees with import"
        );
    }
    Ok(())
}

fn audit(
    root: &Path,
    file: &Path,
    sources: &BTreeSet<PathBuf>,
    manifest: &Value,
    pending: &mut Vec<PathBuf>,
    cancellation: &Cancellation,
) -> Result<()> {
    let path = root.join(file);
    let meta = fs::symlink_metadata(&path)?;
    ensure!(
        meta.is_file() && meta.len() <= 8 * 1024 * 1024,
        "invalid Node module: {}",
        file.display()
    );
    let source = fs::read_to_string(path)?;
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&tree_sitter_javascript::LANGUAGE.into())?;
    let tree = crate::javascript::parse(&mut parser, &source)?;
    let mut allowed = BTreeSet::new();
    let mut loader = None;
    // createRequire is limited to a module-level constant initialized from this
    // module's URL, used only for literal, module-level dependency loads. It may
    // not escape into a test body or load mutated code lazily.
    let mut cursor = tree.root_node().walk();
    for statement in tree.root_node().named_children(&mut cursor) {
        if statement.kind() == "import_statement"
            && statement
                .child_by_field_name("source")
                .is_some_and(|s| literal(s, &source).ok() == Some("node:module"))
        {
            let compact = source[statement.byte_range()]
                .split_whitespace()
                .collect::<String>();
            ensure!(
                matches!(
                    compact.as_str(),
                    "import{createRequire}from'node:module';"
                        | "import{createRequire}from\"node:module\";"
                ),
                "unsupported reflective Node module import"
            );
            let mut nodes = vec![statement];
            while let Some(node) = nodes.pop() {
                if node.kind() == "identifier" && &source[node.byte_range()] == "createRequire" {
                    allowed.insert((node.start_byte(), node.end_byte()));
                }
                let mut c = node.walk();
                nodes.extend(node.named_children(&mut c));
            }
        }
        if statement.kind() != "lexical_declaration" {
            continue;
        }
        let mut c = statement.walk();
        for declaration in statement.named_children(&mut c) {
            let Some(value) = declaration.child_by_field_name("value") else {
                continue;
            };
            let Some(function) = value.child_by_field_name("function") else {
                continue;
            };
            if &source[function.byte_range()] != "createRequire" {
                continue;
            }
            let name = declaration
                .child_by_field_name("name")
                .context("missing loader binding")?;
            ensure!(
                loader.is_none()
                    && name.kind() == "identifier"
                    && source[statement.byte_range()].starts_with("const ")
                    && value
                        .child_by_field_name("arguments")
                        .is_some_and(|a| source[a.byte_range()]
                            .split_whitespace()
                            .collect::<String>()
                            == "(import.meta.url)"),
                "unsupported createRequire binding"
            );
            loader = Some(source[name.byte_range()].to_owned());
            allowed.insert((name.start_byte(), name.end_byte()));
            allowed.insert((function.start_byte(), function.end_byte()));
        }
    }
    let mut nodes = vec![tree.root_node()];
    while let Some(node) = nodes.pop() {
        cancellation.check()?;
        if let Some(spec) = node.child_by_field_name("source") {
            import(
                root,
                file,
                literal(spec, &source)?,
                sources,
                manifest,
                pending,
            )?;
        }
        if node.kind() == "call_expression"
            && let Some(function) = node.child_by_field_name("function")
        {
            let name = &source[function.byte_range()];
            ensure!(
                !matches!(name, "import" | "require"),
                "dynamic Node module loading is unsupported"
            );
            if loader.as_deref() == Some(name) {
                ensure!(
                    node.parent()
                        .is_some_and(|p| p.kind() == "variable_declarator"
                            && p.parent().is_some_and(|p| p.kind() == "lexical_declaration"
                                && p.parent().is_some_and(|p| p.kind() == "program"))),
                    "require bindings may load only at module initialization"
                );
                let args = node
                    .child_by_field_name("arguments")
                    .context("missing loader arguments")?;
                ensure!(
                    args.named_child_count() == 1,
                    "loader requires one literal dependency"
                );
                let spec = literal(args.named_child(0).unwrap(), &source)?;
                ensure!(
                    !spec.starts_with('.'),
                    "require bindings cannot load application modules"
                );
                import(root, file, spec, sources, manifest, pending)?;
                allowed.insert((function.start_byte(), function.end_byte()));
            }
        }
        if matches!(node.kind(), "identifier" | "property_identifier") {
            let name = &source[node.byte_range()];
            ensure!(
                !matches!(name, "eval" | "Function" | "getBuiltinModule")
                    && (!(name == "createRequire" || loader.as_deref() == Some(name))
                        || allowed.contains(&(node.start_byte(), node.end_byte()))),
                "reflective or escaping module loader is unsupported: {name}"
            );
        }
        let mut c = node.walk();
        nodes.extend(node.named_children(&mut c));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dependency_loading_stays_static_and_cannot_escape_into_bodies() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let root = temp.path().canonicalize()?;
        fs::create_dir_all(root.join("scripts"))?;
        fs::create_dir_all(root.join("node_modules/parser"))?;
        fs::write(
            root.join("node_modules/parser/package.json"),
            r#"{"name":"parser","version":"1.0.0"}"#,
        )?;
        let manifest = serde_json::json!({"devDependencies":{"parser":"1.0.0"}});
        let prelude = "import {createRequire} from 'node:module'; const loadModule = createRequire(import.meta.url);";
        for (body, accepted) in [
            ("const parser = loadModule('parser');", true),
            ("const parser = loadModule('undeclared');", false),
            ("const parser = loadModule('./application.js');", false),
            ("const parser = loadModule(name);", false),
            ("const parser = loadModule('parser', {extra: true});", false),
            ("function body() { return loadModule('parser'); }", false),
            ("const escape = loadModule;", false),
            ("const parser = loadModule.call(null, 'parser');", false),
            ("export {loadModule};", false),
            ("const parser = import('parser');", false),
        ] {
            fs::write(root.join("scripts/parser.js"), format!("{prelude} {body}"))?;
            let result = audit(
                &root,
                Path::new("scripts/parser.js"),
                &BTreeSet::new(),
                &manifest,
                &mut Vec::new(),
                &Cancellation::default(),
            );
            assert_eq!(result.is_ok(), accepted, "{body}: {result:?}");
        }
        Ok(())
    }

    #[test]
    fn imports_remain_inside_the_audited_application_and_test_graph() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let root = temp.path().canonicalize()?;
        fs::create_dir_all(root.join("rules"))?;
        fs::create_dir_all(root.join("scripts"))?;
        fs::write(root.join("package.json"), "{}")?;
        fs::write(root.join("rules/value.js"), "export const enabled=true;")?;
        fs::write(root.join("scripts/helper.js"), "export const helper=true;")?;
        fs::write(root.join("outside.js"), "export const hidden=true;")?;
        let sources = BTreeSet::from([PathBuf::from("index.js"), PathBuf::from("rules/value.js")]);
        for (source, accepted) in [
            (
                "import value from './package.json' with {type:'json'};",
                true,
            ),
            ("export * from './rules/value.js';", true),
            ("import './scripts/helper.js';", false),
            ("import './outside.js';", false),
            ("import {test} from 'node:test';", false),
            ("import x from 'missing';", false),
            ("import('./rules/value.js');", false),
        ] {
            fs::write(root.join("index.js"), source)?;
            let result = audit(
                &root,
                Path::new("index.js"),
                &sources,
                &serde_json::json!({}),
                &mut Vec::new(),
                &Cancellation::default(),
            );
            assert_eq!(result.is_ok(), accepted, "{source}: {result:?}");
        }
        Ok(())
    }
}
