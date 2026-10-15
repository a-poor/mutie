use crate::{
    cancellation::Cancellation,
    mutation::{Mutation, Operator, Scope, SourceFile},
};
use anyhow::{Context, Result, ensure};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};
use tree_sitter::Parser;

pub struct Discovery {
    pub mutations: Vec<Mutation>,
    pub exclusions: Vec<String>,
}

pub fn discover(root: &Path, scope: &Scope, cancellation: &Cancellation) -> Result<Discovery> {
    let mut files = Vec::new();
    let mut exclusions = BTreeSet::new();
    collect(
        root,
        Path::new("src"),
        scope,
        cancellation,
        &mut files,
        &mut exclusions,
    )?;
    files.sort();
    let mut mutations = Vec::new();
    for path in files {
        cancellation.check()?;
        let source = fs::read_to_string(root.join(&path))
            .with_context(|| format!("reading {}", path.display()))?;
        if source
            .lines()
            .take_while(|l| l.trim().is_empty() || l.trim_start().starts_with("//"))
            .any(|l| l.contains("@generated"))
        {
            exclusions.insert(format!("{}: generated source", path.display()));
            continue;
        }
        let mut parser = Parser::new();
        let language = if path.extension().is_some_and(|e| e == "ts") {
            tree_sitter_typescript::LANGUAGE_TYPESCRIPT
        } else {
            tree_sitter_javascript::LANGUAGE
        };
        parser.set_language(&language.into())?;
        let tree = parser
            .parse(&source, None)
            .context("JavaScript/TypeScript parser returned no tree")?;
        ensure!(
            !tree.root_node().has_error(),
            "cannot parse {} in the supported JS/TS scope",
            path.display()
        );
        let file = Arc::new(SourceFile {
            path,
            fingerprint: format!("sha256:{:x}", Sha256::digest(source.as_bytes())),
            source,
        });
        let mut pending = vec![tree.root_node()];
        while let Some(node) = pending.pop() {
            cancellation.check()?;
            let kind = node.kind();
            ensure!(
                !matches!(
                    kind,
                    "decorator"
                        | "jsx_element"
                        | "jsx_self_closing_element"
                        | "import_require_clause"
                        | "export_assignment"
                        | "internal_module"
                        | "ambient_declaration"
                        | "enum_declaration"
                ),
                "unsupported {kind} in {}",
                file.path.display()
            );
            if matches!(kind, "import_statement" | "export_statement")
                && let Some(source) = node.child_by_field_name("source")
            {
                let name = file.source[source.byte_range()].trim_matches(['\'', '"']);
                ensure!(
                    name != "vitest" && !name.starts_with("vitest/"),
                    "in-source Vitest tests are unsupported: {}",
                    file.path.display()
                );
                ensure!(
                    !name.starts_with('/') && !name.starts_with("file:"),
                    "absolute imports are unsupported: {}",
                    file.path.display()
                );
            }
            if kind == "call_expression"
                && let Some(function) = node.child_by_field_name("function")
            {
                ensure!(
                    &file.source[function.byte_range()] != "require",
                    "CommonJS require is unsupported: {}",
                    file.path.display()
                );
            }
            if matches!(kind, "member_expression" | "subscript_expression") {
                let object = node.child_by_field_name("object");
                let property = node
                    .child_by_field_name("property")
                    .or_else(|| node.child_by_field_name("index"));
                let object_text = object.map(|n| &file.source[n.byte_range()]);
                let property_text =
                    property.map(|n| file.source[n.byte_range()].trim_matches(['\'', '"']));
                ensure!(
                    object_text != Some("exports")
                        && !(object_text == Some("module") && property_text == Some("exports"))
                        && !(object.is_some_and(|n| n.kind() == "meta_property")
                            && property_text == Some("vitest")),
                    "unsupported module/test expression in {}",
                    file.path.display()
                );
            }
            if type_only(kind) {
                continue;
            }
            if matches!(kind, "true" | "false") {
                let start = node.start_position();
                let end = node.end_position();
                mutations.push(Mutation {
                    file: file.clone(),
                    operator: Operator::BooleanFlip,
                    range: node.byte_range(),
                    replacement: if kind == "true" {
                        "false".into()
                    } else {
                        "true".into()
                    },
                    line: start.row + 1,
                    column: start.column + 1,
                    end_line: end.row + 1,
                    end_column: end.column + 1,
                });
            }
            let mut cursor = node.walk();
            pending.extend(node.named_children(&mut cursor));
        }
    }
    mutations.sort_by(|a, b| (&a.file.path, a.range.start).cmp(&(&b.file.path, b.range.start)));
    Ok(Discovery {
        mutations,
        exclusions: exclusions.into_iter().collect(),
    })
}

fn type_only(kind: &str) -> bool {
    matches!(
        kind,
        "type_annotation"
            | "type_alias_declaration"
            | "interface_declaration"
            | "type_arguments"
            | "type_parameters"
            | "enum_declaration"
            | "type_predicate_annotation"
    ) || kind.ends_with("_type")
}

fn collect(
    root: &Path,
    relative: &Path,
    scope: &Scope,
    cancellation: &Cancellation,
    files: &mut Vec<PathBuf>,
    exclusions: &mut BTreeSet<String>,
) -> Result<()> {
    cancellation.check()?;
    let path = root.join(relative);
    let meta =
        fs::symlink_metadata(&path).with_context(|| format!("reading {}", relative.display()))?;
    ensure!(
        !meta.file_type().is_symlink(),
        "source symlinks are unsupported: {}",
        relative.display()
    );
    ensure!(
        relative.components().count() < 100 && files.len() < 10_000,
        "source tree exceeds preview discovery limit"
    );
    let name = relative
        .file_name()
        .context("source path has no name")?
        .to_string_lossy();
    if meta.is_dir() {
        if matches!(
            name.as_ref(),
            "node_modules" | "dist" | "test" | "tests" | "__tests__" | ".git"
        ) {
            exclusions.insert(format!(
                "{}: test, dependency or generated-output directory",
                relative.display()
            ));
            return Ok(());
        }
        ensure!(
            !path.join("package.json").exists() && !path.join("tsconfig.json").exists(),
            "nested package/tsconfig is unsupported: {}",
            relative.display()
        );
        let mut entries = fs::read_dir(&path)?
            .map(|e| e.map(|e| e.file_name()))
            .collect::<std::io::Result<Vec<_>>>()?;
        entries.sort();
        for entry in entries {
            collect(
                root,
                &relative.join(entry),
                scope,
                cancellation,
                files,
                exclusions,
            )?;
        }
    } else {
        ensure!(
            meta.is_file(),
            "unsupported source file: {}",
            relative.display()
        );
        if !scope.matches(relative) {
            exclusions.insert(format!("{}: path filter", relative.display()));
            return Ok(());
        }
        if name.ends_with(".d.ts") || name.contains(".test.") || name.contains(".spec.") {
            exclusions.insert(format!("{}: declaration or test file", relative.display()));
            return Ok(());
        }
        let extension = relative.extension().and_then(|s| s.to_str());
        ensure!(
            !matches!(
                extension,
                Some("jsx" | "tsx" | "mts" | "cts" | "cjs" | "mjs")
            ),
            "unsupported production source extension: {}",
            relative.display()
        );
        if matches!(extension, Some("js" | "ts")) {
            files.push(relative.to_owned());
        } else {
            exclusions.insert(format!("{}: not JS/TS source", relative.display()));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture(files: &[(&str, &str)]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        for (path, source) in files {
            let path = dir.path().join(path);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, source).unwrap();
        }
        dir
    }
    fn find(root: &Path) -> Result<Discovery> {
        discover(
            root,
            &Scope::new(&[], &[]).unwrap(),
            &Cancellation::default(),
        )
    }
    #[test]
    fn separate_grammars_exclude_types_and_text_but_keep_runtime_values() {
        let dir = fixture(&[
            (
                "src/b.ts",
                "type T = true | false; interface I { x: true } function f(x: false = false): true { return true; } const a = true as true; const b = <true>true; const c = false satisfies boolean;",
            ),
            (
                "src/a.js",
                "// true\nconst s = 'false'; const r = /true/; const t = `false ${true}`; const obj = { true: false }; function f(){return true;}",
            ),
        ]);
        let found = find(dir.path()).unwrap();
        let edits: Vec<_> = found
            .mutations
            .iter()
            .map(|m| (m.file.path.to_str().unwrap(), m.original()))
            .collect();
        assert_eq!(
            edits,
            vec![
                ("src/a.js", "true"),
                ("src/a.js", "false"),
                ("src/a.js", "true"),
                ("src/b.ts", "false"),
                ("src/b.ts", "true"),
                ("src/b.ts", "true"),
                ("src/b.ts", "true"),
                ("src/b.ts", "false")
            ]
        );
        for m in &found.mutations {
            assert_eq!(m.operator, Operator::BooleanFlip);
            assert!(m.file.fingerprint.starts_with("sha256:"));
        }
    }
    #[test]
    fn fixture_has_six_stable_edits_and_exact_byte_ranges() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/vitest");
        let found = find(&root).unwrap();
        assert_eq!(found.mutations.len(), 6);
        assert_eq!(
            found
                .mutations
                .iter()
                .map(Mutation::description)
                .collect::<Vec<_>>(),
            find(&root)
                .unwrap()
                .mutations
                .iter()
                .map(Mutation::description)
                .collect::<Vec<_>>()
        );
        let dir = fixture(&[(
            "src/a.ts",
            "// 🦀\r\nexport const flag: boolean = true;\r\n",
        )]);
        let m = find(dir.path()).unwrap().mutations.remove(0);
        m.apply(dir.path()).unwrap();
        assert_eq!(
            fs::read_to_string(dir.path().join("src/a.ts")).unwrap(),
            "// 🦀\r\nexport const flag: boolean = false;\r\n"
        );
        assert!(m.apply(dir.path()).is_err());
    }
    #[test]
    fn excludes_tests_generated_types_and_filtered_paths() {
        let dir = fixture(&[
            ("src/a.js", "export const a=true;"),
            ("src/b.ts", "export const b=false;"),
            ("src/x.test.ts", "bad syntax"),
            ("src/x.d.ts", "bad syntax"),
            ("src/generated.js", "// @generated\nconst a=true;"),
            ("src/tests/a.js", "bad syntax"),
        ]);
        let found = discover(
            dir.path(),
            &Scope::new(&["src/*.js".into()], &[]).unwrap(),
            &Cancellation::default(),
        )
        .unwrap();
        assert_eq!(found.mutations.len(), 1);
        assert_eq!(found.exclusions.len(), 5);
    }
    #[test]
    fn rejects_unsupported_syntax_layouts_and_parse_errors() {
        for (path, source) in [
            ("src/a.tsx", "const a=true"),
            ("src/a.js", "const = true"),
            ("src/a.ts", "import {test} from 'vitest'; const a=true"),
            ("src/a.js", "const a=require('a')"),
            ("src/a.ts", "@foo class C {}"),
            ("src/nested/package.json", "{}"),
            ("src/a.ts", "namespace N { export const a=true; }"),
        ] {
            let dir = fixture(&[(path, source)]);
            assert!(find(dir.path()).is_err(), "{path}: {source}");
        }
    }
    #[test]
    fn rejects_spaced_and_computed_module_expressions_and_cancellation() {
        for source in [
            "module . exports = true",
            "module['exports'] = true",
            "exports['flag'] = true",
            "if (import.meta['vitest']) { const flag=true; }",
            "enum Flags { Value = true as any }",
        ] {
            let dir = fixture(&[("src/a.ts", source)]);
            assert!(find(dir.path()).is_err(), "{source}");
        }
        let dir = fixture(&[("src/a.js", "const flag=true;")]);
        let cancellation = Cancellation::default();
        cancellation.cancel();
        assert!(discover(dir.path(), &Scope::new(&[], &[]).unwrap(), &cancellation).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn rejects_source_links() {
        let dir = fixture(&[("src/a.js", "const a=true;")]);
        std::os::unix::fs::symlink("a.js", dir.path().join("src/b.js")).unwrap();
        assert!(find(dir.path()).is_err());
    }
}
