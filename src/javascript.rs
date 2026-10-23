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

/// Native token replacements: never wrap, reorder or duplicate operands. In
/// particular, ordering inversion is not Boolean negation for NaN operands.
pub fn replacement(operator: Operator, original: &str) -> Option<&'static str> {
    match (operator, original) {
        (Operator::BooleanFlip, "true") => Some("false"),
        (Operator::BooleanFlip, "false") => Some("true"),
        (Operator::ComparisonInvert, "==") => Some("!="),
        (Operator::ComparisonInvert, "!=") => Some("=="),
        (Operator::ComparisonInvert, "===") => Some("!=="),
        (Operator::ComparisonInvert, "!==") => Some("==="),
        (Operator::ComparisonInvert, "<") => Some(">="),
        (Operator::ComparisonInvert, "<=") => Some(">"),
        (Operator::ComparisonInvert, ">") => Some("<="),
        (Operator::ComparisonInvert, ">=") => Some("<"),
        (Operator::ComparisonBoundary, "<") => Some("<="),
        (Operator::ComparisonBoundary, "<=") => Some("<"),
        (Operator::ComparisonBoundary, ">") => Some(">="),
        (Operator::ComparisonBoundary, ">=") => Some(">"),
        _ => None,
    }
}

// Wrap both operands and the result so swapping precedence cannot regroup
// either a child or the enclosing expression. Keep inter-operand trivia intact.
fn expression_replacement(
    operator: Operator,
    node: tree_sitter::Node<'_>,
    source: &str,
) -> Option<String> {
    let token = node.child_by_field_name("operator")?;
    let swapped = match (operator, &source[token.byte_range()]) {
        (Operator::LogicalSwap, "&&") => Some("||"),
        (Operator::LogicalSwap, "||") => Some("&&"),
        (Operator::ArithmeticReplace, "+") => Some("-"),
        (Operator::ArithmeticReplace, "-") => Some("+"),
        (Operator::ArithmeticReplace, "*") => Some("/"),
        (Operator::ArithmeticReplace, "/" | "%") => Some("*"),
        _ => None,
    };
    if node.kind() == "binary_expression"
        && let Some(swapped) = swapped
    {
        let left = node.child_by_field_name("left")?;
        let right = node.child_by_field_name("right")?;
        return Some(format!(
            "(({}){}{}{}({}))",
            &source[left.byte_range()],
            &source[left.end_byte()..token.start_byte()],
            swapped,
            &source[token.end_byte()..right.start_byte()],
            &source[right.byte_range()]
        ));
    }
    if operator == Operator::NegationRemove
        && node.kind() == "unary_expression"
        && &source[token.byte_range()] == "!"
    {
        return Some(format!("({})", &source[token.end_byte()..node.end_byte()]));
    }
    None
}

// The condition includes its required parentheses. Keep its source references,
// but skip evaluation entirely, matching the Go branch-forcing contract.
fn branch_replacement(
    operator: Operator,
    condition: tree_sitter::Node<'_>,
    source: &str,
) -> Option<String> {
    let guard = match operator {
        Operator::BranchForceTrue => "true ||",
        Operator::BranchForceFalse => "false &&",
        _ => return None,
    };
    if condition.kind() != "parenthesized_expression" {
        return None;
    }
    let mut inner = condition;
    while inner.kind() == "parenthesized_expression" {
        let mut cursor = inner.walk();
        let children: Vec<_> = inner
            .named_children(&mut cursor)
            .filter(|n| n.kind() != "comment")
            .collect();
        if children.len() != 1 {
            return None;
        }
        inner = children[0];
    }
    if matches!(inner.kind(), "true" | "false") {
        return None;
    }
    Some(format!("({guard} {})", &source[condition.byte_range()]))
}

/// Validate full-expression edits by reconstructing the same parsed recipe.
/// A different grouping or arbitrary replacement must not enter saved reruns.
pub fn valid_replacement(
    operator: Operator,
    original: &str,
    changed: &str,
    typescript: bool,
) -> bool {
    if let Some(expected) = replacement(operator, original) {
        return expected == changed;
    }
    if matches!(
        operator,
        Operator::BranchForceTrue | Operator::BranchForceFalse
    ) {
        let prefix = "if ";
        let source = format!("{prefix}{original} {{}} ");
        let mut parser = Parser::new();
        let language = if typescript {
            tree_sitter_typescript::LANGUAGE_TYPESCRIPT
        } else {
            tree_sitter_javascript::LANGUAGE
        };
        if parser.set_language(&language.into()).is_err() {
            return false;
        }
        let Ok(tree) = parse(&mut parser, &source) else {
            return false;
        };
        let Some(condition) = tree
            .root_node()
            .named_child(0)
            .filter(|n| n.kind() == "if_statement")
            .and_then(|n| n.child_by_field_name("condition"))
        else {
            return false;
        };
        return condition.byte_range() == (prefix.len()..prefix.len() + original.len())
            && branch_replacement(operator, condition, &source).as_deref() == Some(changed);
    }
    if !matches!(
        operator,
        Operator::LogicalSwap | Operator::NegationRemove | Operator::ArithmeticReplace
    ) {
        return false;
    }
    let prefix = "const value = (\n";
    let source = format!("{prefix}{original}\n);");
    let mut parser = Parser::new();
    let language = if typescript {
        tree_sitter_typescript::LANGUAGE_TYPESCRIPT
    } else {
        tree_sitter_javascript::LANGUAGE
    };
    if parser.set_language(&language.into()).is_err() {
        return false;
    }
    let Ok(tree) = parse(&mut parser, &source) else {
        return false;
    };
    let Some(node) = tree
        .root_node()
        .descendant_for_byte_range(prefix.len(), prefix.len() + original.len())
    else {
        return false;
    };
    node.byte_range() == (prefix.len()..prefix.len() + original.len())
        && expression_replacement(operator, node, &source).as_deref() == Some(changed)
}

/// Tree-sitter treats a literal NUL as end-of-input even inside otherwise valid
/// JavaScript comments and strings. Mask only for parsing, preserve every byte
/// offset, and require each masked byte to be in literal text (never code).
/// Fingerprints, mutation ranges and written source always use original bytes.
pub fn parse(parser: &mut Parser, source: &str) -> Result<tree_sitter::Tree> {
    let bytes = source.as_bytes();
    let masked: Vec<_> = bytes
        .iter()
        .map(|b| if *b == 0 { b' ' } else { *b })
        .collect();
    let tree = parser
        .parse(&masked, None)
        .context("JavaScript/TypeScript parser returned no tree")?;
    ensure!(
        !tree.root_node().has_error(),
        "unsupported JavaScript/TypeScript syntax"
    );
    for (offset, _) in bytes.iter().enumerate().filter(|(_, b)| **b == 0) {
        let mut node = tree
            .root_node()
            .descendant_for_byte_range(offset, offset + 1);
        let mut literal = false;
        while let Some(n) = node {
            if matches!(n.kind(), "comment" | "string" | "string_fragment") {
                literal = true;
                break;
            }
            // A substitution inside a template is executable code, not literal text.
            if n.kind() == "template_substitution" {
                break;
            }
            if n.kind() == "template_string" {
                literal = true;
                break;
            }
            node = n.parent();
        }
        ensure!(
            literal,
            "NUL outside JavaScript literal text is unsupported"
        );
    }
    Ok(tree)
}

pub fn discover(root: &Path, scope: &Scope, cancellation: &Cancellation) -> Result<Discovery> {
    discover_with_ambient(root, scope, cancellation, false)
}

/// Bun's native TS transpiler erases ambient declarations. Keep this acceptance
/// separate from the independently validated Node and Vitest discovery scopes.
pub fn discover_bun(root: &Path, scope: &Scope, cancellation: &Cancellation) -> Result<Discovery> {
    discover_with_ambient(root, scope, cancellation, true)
}

fn discover_with_ambient(
    root: &Path,
    scope: &Scope,
    cancellation: &Cancellation,
    allow_ambient: bool,
) -> Result<Discovery> {
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
        let tree = parse(&mut parser, &source).with_context(|| {
            format!(
                "cannot parse {} in the supported JS/TS scope",
                path.display()
            )
        })?;
        let file = Arc::new(SourceFile {
            path,
            fingerprint: format!("sha256:{:x}", Sha256::digest(source.as_bytes())),
            source,
        });
        let mut pending = vec![tree.root_node()];
        while let Some(node) = pending.pop() {
            cancellation.check()?;
            let kind = node.kind();
            if allow_ambient && kind == "ambient_declaration" {
                continue;
            }
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
            if kind == "binary_expression"
                && let Some(token) = node.child_by_field_name("operator")
            {
                let original = &file.source[token.byte_range()];
                for operator in [Operator::ComparisonInvert, Operator::ComparisonBoundary] {
                    if let Some(replacement) = replacement(operator, original) {
                        mutations.push(Mutation {
                            file: file.clone(),
                            operator,
                            range: token.byte_range(),
                            replacement: replacement.into(),
                            line: token.start_position().row + 1,
                            column: token.start_position().column + 1,
                            end_line: token.end_position().row + 1,
                            end_column: token.end_position().column + 1,
                        });
                    }
                }
            }
            if kind == "if_statement"
                && let Some(condition) = node.child_by_field_name("condition")
            {
                for operator in [Operator::BranchForceTrue, Operator::BranchForceFalse] {
                    if let Some(replacement) = branch_replacement(operator, condition, &file.source)
                    {
                        mutations.push(Mutation {
                            file: file.clone(),
                            operator,
                            range: condition.byte_range(),
                            replacement: replacement.into(),
                            line: condition.start_position().row + 1,
                            column: condition.start_position().column + 1,
                            end_line: condition.end_position().row + 1,
                            end_column: condition.end_position().column + 1,
                        });
                    }
                }
            }
            for operator in [
                Operator::LogicalSwap,
                Operator::NegationRemove,
                Operator::ArithmeticReplace,
            ] {
                if let Some(replacement) = expression_replacement(operator, node, &file.source) {
                    mutations.push(Mutation {
                        file: file.clone(),
                        operator,
                        range: node.byte_range(),
                        replacement: replacement.into(),
                        line: node.start_position().row + 1,
                        column: node.start_position().column + 1,
                        end_line: node.end_position().row + 1,
                        end_column: node.end_position().column + 1,
                    });
                }
            }
            let mut cursor = node.walk();
            pending.extend(node.named_children(&mut cursor));
        }
    }
    mutations.sort_by(|a, b| {
        (&a.file.path, a.range.start, a.operator, a.range.end).cmp(&(
            &b.file.path,
            b.range.start,
            b.operator,
            b.range.end,
        ))
    });
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
    fn comparisons_use_exact_tokens_and_stable_operator_order_in_both_grammars() -> Result<()> {
        for syntax in ["js", "ts"] {
            let source = "// 🦀 < >=\r\nexport const f = (a,b) => [a == b, a != b, a === b, a !== b, a < b, a <= b, a > b, a >= b];";
            let path = format!("src/comparisons.{syntax}");
            let dir = fixture(&[(&path, source)]);
            let found = find(dir.path())?;
            let pairs: Vec<_> = found
                .mutations
                .iter()
                .map(|m| (m.original(), m.replacement.as_ref()))
                .collect();
            assert_eq!(
                pairs,
                [
                    ("==", "!="),
                    ("!=", "=="),
                    ("===", "!=="),
                    ("!==", "==="),
                    ("<", ">="),
                    ("<", "<="),
                    ("<=", ">"),
                    ("<=", "<"),
                    (">", "<="),
                    (">", ">="),
                    (">=", "<"),
                    (">=", ">")
                ]
            );
            for m in found.mutations {
                assert_eq!(m.line, 2);
                assert_eq!(&source[m.range.clone()], m.original());
                m.apply(dir.path())?;
                let edited = fs::read_to_string(dir.path().join(&path))?;
                assert_eq!(
                    edited,
                    format!(
                        "{}{}{}",
                        &source[..m.range.start],
                        m.replacement,
                        &source[m.range.end..]
                    )
                );
                fs::write(dir.path().join(&path), source)?;
            }
        }
        Ok(())
    }
    #[test]
    fn comparisons_exclude_type_syntax_text_and_noncomparison_operators() -> Result<()> {
        let source = "type F<T> = T extends string ? true : false; interface X<T> { f: F<T> } const text = '< ==='; const re = /<=/; function f<T>(a: T, b: T) { return (a as T) < b; } const value = f<number>(1, 2); const x = 1 << 2; const y = 2 >> 1; const obj = { '<': 1 };";
        let dir = fixture(&[("src/code.ts", source)]);
        let found = find(dir.path())?;
        assert_eq!(found.mutations.len(), 2);
        assert!(found.mutations.iter().all(|m| m.original() == "<"));
        // Nested comparisons remain separate native binary expressions; each
        // mutation preserves JavaScript's existing left-associative grouping.
        fs::write(dir.path().join("src/code.ts"), "const x = 1 < 2 < 3;")?;
        assert_eq!(find(dir.path())?.mutations.len(), 4);
        for (op, original, changed) in [
            (Operator::ComparisonInvert, "===", "!="),
            (Operator::ComparisonBoundary, "==", "!="),
            (Operator::ComparisonBoundary, "<", ">="),
        ] {
            assert_ne!(replacement(op, original), Some(changed));
        }
        Ok(())
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
    fn fixture_has_ten_stable_edits_and_exact_byte_ranges() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/vitest");
        let found = find(&root).unwrap();
        assert_eq!(found.mutations.len(), 10);
        assert_eq!(
            found
                .mutations
                .iter()
                .filter(|m| m.operator == Operator::BooleanFlip)
                .count(),
            6
        );
        assert_eq!(
            found
                .mutations
                .iter()
                .filter(|m| matches!(
                    m.operator,
                    Operator::BranchForceTrue | Operator::BranchForceFalse
                ))
                .count(),
            4
        );
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

#[cfg(test)]
mod nul_tests {
    use super::*;
    #[test]
    fn literal_nuls_preserve_boolean_offsets_and_do_not_hide_code() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir(root.path().join("src")).unwrap();
        let source = "// before\0comment\nconst a=`x\0${true}`; const b='x\0'; const c=false;";
        fs::write(root.path().join("src/flags.js"), source).unwrap();
        let found = discover(
            root.path(),
            &Scope::new(&[], &[]).unwrap(),
            &Cancellation::default(),
        )
        .unwrap();
        assert_eq!(found.mutations.len(), 2);
        for mutation in found.mutations {
            assert_eq!(&source[mutation.range.clone()], mutation.original());
        }
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_javascript::LANGUAGE.into())
            .unwrap();
        for source in ["const x=\0true;", "const x=`${\0true}`;", "const x=tr\0ue;"] {
            assert!(parse(&mut parser, source).is_err(), "{source:?}");
        }
    }
}

#[cfg(test)]
mod bun_scope_tests {
    use super::*;
    #[test]
    fn ambient_declarations_are_erased_only_in_bun_discovery() -> Result<()> {
        let root = tempfile::tempdir()?;
        fs::create_dir(root.path().join("src"))?;
        fs::write(
            root.path().join("src/flags.ts"),
            "declare const Brand: unique symbol; declare const Flag: true; export const enabled = false;",
        )?;
        let scope = Scope::new(&[], &[])?;
        let cancellation = Cancellation::default();
        assert!(discover(root.path(), &scope, &cancellation).is_err());
        let found = discover_bun(root.path(), &scope, &cancellation)?;
        assert_eq!(found.mutations.len(), 1);
        assert_eq!(found.mutations[0].original(), "false");
        fs::write(
            root.path().join("src/flags.ts"),
            "enum State { Yes = 1 } const flag=true;",
        )?;
        assert!(discover_bun(root.path(), &scope, &cancellation).is_err());
        Ok(())
    }
}

#[cfg(test)]
mod comparison_semantics {
    use super::*;
    #[tokio::test]
    #[ignore = "requires pinned Node and Bun; executes discovered JS/TS comparison replacements"]
    async fn native_comparison_evaluation_coercion_and_exceptions() -> Result<()> {
        for runtime in ["node", "bun"] {
            for syntax in ["js", "ts"] {
                let root = tempfile::tempdir()?;
                fs::create_dir(root.path().join("src"))?;
                fs::write(root.path().join("package.json"), r#"{"type":"module"}"#)?;
                let path = root.path().join(format!("src/control.{syntax}"));
                let params = if syntax == "ts" {
                    "a: any, b: any"
                } else {
                    "a,b"
                };
                for original in ["==", "!=", "===", "!==", "<", "<=", ">", ">="] {
                    let source =
                        format!("export function compare({params}) {{ return a {original} b; }}\n");
                    fs::write(&path, &source)?;
                    let discovered = discover(
                        root.path(),
                        &Scope::new(&[], &[])?,
                        &Cancellation::default(),
                    )?;
                    for mutation in discovered.mutations {
                        mutation.apply(root.path())?;
                        let relational =
                            matches!(mutation.replacement.as_ref(), "<" | "<=" | ">" | ">=");
                        let expected = match mutation.replacement.as_ref() {
                            "==" => "[true,false,true,false,true,true]",
                            "!=" => "[false,true,false,true,false,false]",
                            "===" => "[true,false,false,false,true,false]",
                            "!==" => "[false,true,true,true,false,true]",
                            "<" => "[false,false,false,false,false,false]",
                            "<=" => "[true,false,true,false,true,false]",
                            ">" => "[false,false,false,true,false,false]",
                            ">=" => "[true,false,true,true,true,false]",
                            _ => unreachable!(),
                        };
                        let trace = if relational {
                            "['eval:left','eval:right','coerce:left:number','coerce:right:number']"
                        } else {
                            "['eval:left','eval:right']"
                        };
                        let throws = if matches!(mutation.replacement.as_ref(), "===" | "!==") {
                            "compare(broken, right()); assert.deepEqual(trace, ['right']);"
                        } else {
                            "assert.throws(() => compare(broken, right()), e => e === sentinel); assert.deepEqual(trace, ['right','throw']);"
                        };
                        let script = format!(
                            r#"
import assert from 'node:assert/strict';
import {{compare}} from './src/control.{syntax}';
const pairs = [[1,1], [NaN,0], [1n,'1'], ['2','10'], [-0,0], [null,undefined]];
assert.deepEqual(pairs.map(([a,b]) => compare(a,b)), {expected});
const trace = [];
function operand(name) {{ trace.push('eval:'+name); return {{[Symbol.toPrimitive](hint) {{ trace.push('coerce:'+name+':'+hint); return 1; }} }}; }}
compare(operand('left'), operand('right'));
assert.deepEqual(trace, {trace});
trace.length = 0;
const sentinel = new Error('coercion');
const broken = {{[Symbol.toPrimitive]() {{ trace.push('throw'); throw sentinel; }} }};
function right() {{ trace.push('right'); return 1; }}
{throws}
let calls = 0; const value = false && compare(++calls, 0); assert.equal(calls, 0); assert.equal(value, false);
"#
                        );
                        let harness = root.path().join(format!("control.{syntax}"));
                        fs::write(&harness, script)?;
                        let output = crate::process::run(
                            runtime,
                            &[harness.to_str().unwrap()],
                            root.path(),
                            std::time::Duration::from_secs(10),
                            std::num::NonZeroUsize::new(64 * 1024),
                            &Cancellation::default(),
                        )
                        .await?;
                        ensure!(
                            output.status.success()
                                && !output.timed_out
                                && output.capture_error.is_none(),
                            "{runtime}/{syntax}/{original} -> {}: {}",
                            mutation.replacement,
                            output.stderr.text()?
                        );
                        fs::write(&path, &source)?;
                    }
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "javascript/logical_tests.rs"]
mod logical_tests;

#[cfg(test)]
#[path = "javascript/arithmetic_tests.rs"]
mod arithmetic_tests;

#[cfg(test)]
#[path = "javascript/branch_tests.rs"]
mod branch_tests;
