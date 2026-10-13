use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::{Context, Result, ensure};
use sha2::{Digest, Sha256};
use tree_sitter::{Node, Parser};

use crate::{
    cancellation::Cancellation,
    mutation::{Mutation, Operator, Scope, SourceFile},
};

#[derive(Debug)]
pub struct Discovery {
    pub mutations: Vec<Mutation>,
    pub exclusions: Vec<String>,
}

pub fn discover(
    root: &Path,
    entry: &Path,
    scope: &Scope,
    cancellation: &Cancellation,
) -> Result<Discovery> {
    let mut walker = Walker {
        root,
        scope,
        cancellation,
        parser: Parser::new(),
        visited: BTreeSet::new(),
        mutations: Vec::new(),
        exclusions: BTreeSet::new(),
    };
    walker
        .parser
        .set_language(&tree_sitter_rust::LANGUAGE.into())?;
    walker.file(entry, true)?;
    walker
        .mutations
        .sort_by(|a, b| (&a.file.path, a.range.start).cmp(&(&b.file.path, b.range.start)));
    walker
        .mutations
        .dedup_by(|a, b| a.file.path == b.file.path && a.range == b.range);
    Ok(Discovery {
        mutations: walker.mutations,
        exclusions: walker.exclusions.into_iter().collect(),
    })
}

struct Walker<'a> {
    root: &'a Path,
    scope: &'a Scope,
    cancellation: &'a Cancellation,
    parser: Parser,
    visited: BTreeSet<PathBuf>,
    mutations: Vec<Mutation>,
    exclusions: BTreeSet<String>,
}

impl Walker<'_> {
    fn exclude(&mut self, file: &Path, reason: &str) {
        self.exclusions
            .insert(format!("{}: {reason}", file.display()));
    }

    fn file(&mut self, path: &Path, entry: bool) -> Result<()> {
        self.cancellation.check()?;
        let path = super::project::inside(self.root, &self.root.join(path))?;
        ensure!(self.visited.len() < 10_000, "too many Rust module files");
        if !self.visited.insert(path.clone()) {
            return Ok(());
        }
        let mut parent = path.parent();
        while let Some(directory) = parent {
            if !directory.as_os_str().is_empty()
                && self.root.join(directory).join("Cargo.toml").is_file()
            {
                self.exclude(&path, "nested Cargo package");
                return Ok(());
            }
            parent = directory.parent();
        }
        let source = fs::read_to_string(self.root.join(&path))
            .with_context(|| format!("reading Rust source {}", path.display()))?;
        if source
            .lines()
            .take_while(|line| line.trim().is_empty() || line.trim_start().starts_with("//"))
            .any(|line| line.contains("@generated"))
        {
            self.exclude(&path, "generated source");
            return Ok(());
        }
        let tree = self
            .parser
            .parse(&source, None)
            .context("Rust parser did not return a tree")?;
        ensure!(
            !tree.root_node().has_error(),
            "could not parse {} as Rust",
            path.display()
        );
        let file = Arc::new(SourceFile {
            path: path.clone(),
            fingerprint: format!("sha256:{:x}", Sha256::digest(source.as_bytes())),
            source,
        });
        let mut module_dir = path.parent().unwrap_or(Path::new("")).to_owned();
        if !entry && path.file_name().is_some_and(|name| name != "mod.rs") {
            module_dir.push(path.file_stem().context("Rust source has no file stem")?);
        }
        self.visit(tree.root_node(), &file, &module_dir, false)
    }

    fn visit(
        &mut self,
        node: Node<'_>,
        file: &Arc<SourceFile>,
        module_dir: &Path,
        in_body: bool,
    ) -> Result<()> {
        self.cancellation.check()?;
        let mut cursor = node.walk();
        // Inner attributes apply to their containing scope. Some grammar nodes
        // (notably match arms) contain outer attributes instead of sibling ones.
        if node.named_children(&mut cursor).any(|child| {
            matches!(child.kind(), "inner_attribute_item" | "attribute_item")
                && excluded_attribute(child, &file.source)
        }) {
            // Sibling outer attributes in lists apply only to the next node.
            if !matches!(node.kind(), "source_file" | "declaration_list" | "block")
                || node.named_children(&mut node.walk()).any(|c| {
                    c.kind() == "inner_attribute_item" && excluded_attribute(c, &file.source)
                })
            {
                self.exclude(&file.path, "conditional, test-only, or remapped source");
                return Ok(());
            }
        }
        match node.kind() {
            "macro_definition" | "macro_invocation" => {
                self.exclude(&file.path, "macro tokens/expansions (including include!)");
                return Ok(());
            }
            "attribute_item"
            | "inner_attribute_item"
            | "token_tree"
            | "const_item"
            | "static_item"
            | "const_block"
            | "type_arguments"
            | "type_parameters" => return Ok(()),
            kind if kind.ends_with("_type") || kind.ends_with("_pattern") => return Ok(()),
            "function_item" => {
                if let Some(body) = node.child_by_field_name("body") {
                    self.visit(body, file, module_dir, true)?;
                }
                return Ok(());
            }
            "match_arm" => {
                // The grammar nests executable guards inside the pattern field.
                // Visit just the guard; the pattern itself remains excluded below.
                if let Some(condition) = node
                    .child_by_field_name("pattern")
                    .and_then(|pattern| pattern.child_by_field_name("condition"))
                {
                    self.visit(condition, file, module_dir, in_body)?;
                }
            }
            "mod_item" => {
                let name = node
                    .child_by_field_name("name")
                    .context("Rust module has no name")?;
                let name = file.source[name.byte_range()].trim_start_matches("r#");
                let child_dir = module_dir.join(name);
                if let Some(body) = node.child_by_field_name("body") {
                    self.visit(body, file, &child_dir, false)?;
                } else {
                    let flat = module_dir.join(format!("{name}.rs"));
                    let nested = child_dir.join("mod.rs");
                    let flat_exists = self.root.join(&flat).is_file();
                    let nested_exists = self.root.join(&nested).is_file();
                    ensure!(
                        flat_exists != nested_exists,
                        "Rust module {name} in {} has missing or ambiguous source",
                        file.path.display()
                    );
                    self.file(if flat_exists { &flat } else { &nested }, false)?;
                }
                return Ok(());
            }
            "boolean_literal" if in_body && self.scope.matches(&file.path) => {
                let original = &file.source[node.byte_range()];
                self.push(
                    file,
                    node,
                    Operator::BooleanFlip,
                    if original == "true" { "false" } else { "true" },
                );
            }
            "binary_expression" if in_body && self.scope.matches(&file.path) => {
                if let Some(operator) = node.child_by_field_name("operator") {
                    let replacement = match &file.source[operator.byte_range()] {
                        "<" => Some("<="),
                        "<=" => Some("<"),
                        ">" => Some(">="),
                        ">=" => Some(">"),
                        _ => None,
                    };
                    if let Some(replacement) = replacement {
                        // Replace only the parsed operator: preserve operand evaluation,
                        // precedence and borrowing, including overloaded comparisons.
                        self.push(file, operator, Operator::ComparisonBoundary, replacement);
                    }
                }
            }
            _ => {}
        }
        let mut skip_next = false;
        for child in node.named_children(&mut node.walk()) {
            if child.is_extra() {
                continue;
            }
            if child.kind() == "attribute_item" {
                skip_next |= excluded_attribute(child, &file.source);
                continue;
            }
            if skip_next {
                self.exclude(&file.path, "conditional, test-only, or remapped source");
                skip_next = false;
                continue;
            }
            if node
                .child_by_field_name("pattern")
                .is_some_and(|p| p.id() == child.id())
                || node
                    .child_by_field_name("type")
                    .is_some_and(|p| p.id() == child.id())
                // Array repeat lengths are constant expressions, even inside bodies.
                || (node.kind() == "array_expression"
                    && node.child_by_field_name("length").is_some_and(|p| p.id() == child.id()))
            {
                continue;
            }
            self.visit(child, file, module_dir, in_body)?;
        }
        Ok(())
    }

    fn push(
        &mut self,
        file: &Arc<SourceFile>,
        node: Node<'_>,
        operator: Operator,
        replacement: &'static str,
    ) {
        let start = node.start_position();
        let end = node.end_position();
        self.mutations.push(Mutation {
            file: Arc::clone(file),
            operator,
            range: node.byte_range(),
            replacement: replacement.into(),
            line: start.row + 1,
            column: start.column + 1,
            end_line: end.row + 1,
            end_column: end.column + 1,
        });
    }
}

fn excluded_attribute(node: Node<'_>, source: &str) -> bool {
    let Some(attribute) = node.named_child(0) else {
        return false;
    };
    let Some(name) = attribute.named_child(0) else {
        return false;
    };
    let name = &source[name.byte_range()];
    matches!(name, "cfg" | "cfg_attr" | "test" | "path") || name.ends_with("::test")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scan(root: &Path, include: &[String]) -> Result<Discovery> {
        discover(
            root,
            Path::new("src/main.rs"),
            &Scope::new(include, &[])?,
            &Cancellation::default(),
        )
    }

    #[test]
    fn fixture_discovers_only_the_three_production_literals() -> Result<()> {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/rust")
            .canonicalize()?;
        let found = scan(&root, &[])?;
        assert_eq!(found.mutations.len(), 3, "{:?}", found.mutations);
        assert!(found.mutations.iter().all(
            |m| m.file.path == Path::new("src/flags.rs") && m.operator == Operator::BooleanFlip
        ));
        assert_eq!(
            found
                .mutations
                .iter()
                .map(Mutation::original)
                .collect::<Vec<_>>(),
            ["true", "false", "true"]
        );
        assert!(
            found
                .exclusions
                .iter()
                .any(|reason| reason.contains("generated source"))
        );
        assert!(
            found
                .exclusions
                .iter()
                .any(|reason| reason.contains("macro tokens"))
        );
        assert_eq!(scan(&root, &["src/main.rs".into()])?.mutations.len(), 0);
        assert_eq!(scan(&root, &["src/flags.rs".into()])?.mutations.len(), 3);
        Ok(())
    }

    #[test]
    fn respects_attributes_patterns_constants_and_nested_module_resolution() -> Result<()> {
        let temp = tempfile::tempdir_in(std::env::temp_dir().canonicalize()?)?;
        let root = temp.path();
        fs::create_dir_all(root.join("src/nested"))?;
        fs::write(
            root.join("src/main.rs"),
            r#"
#[cfg(any())]
// Comments must not consume the preceding attribute.
mod missing;
#[path = "elsewhere.rs"] mod remapped;
#[cfg_attr(unix, allow(dead_code))] fn conditional() -> bool { true }
#[test] fn test_without_cfg() { let expected = true; }
#[tokio::test] async fn async_test() { let expected = true; }
const FLAG: bool = true;
static OTHER: bool = false;
mod nested { mod leaf; }
fn main() {
    #[cfg(unix)] let skipped = true;
    let included = false;
    const LOCAL: bool = true;
    assert_eq!(included, true);
    match included {
        #[cfg(unix)] true => false,
        false => { let answer = true; },
        _ => {},
    }
}
"#,
        )?;
        fs::write(
            root.join("src/nested/leaf.rs"),
            "// π\npub fn flag() -> bool { false }\n",
        )?;
        fs::write(root.join("src/orphan.rs"), "this is not valid rust")?;
        let found = scan(root, &[])?;
        assert_eq!(found.mutations.len(), 3, "{:?}", found.mutations);
        assert_eq!(
            found
                .mutations
                .iter()
                .map(Mutation::original)
                .collect::<Vec<_>>(),
            ["false", "true", "false"]
        );
        assert_eq!(
            found.mutations[2].file.path,
            Path::new("src/nested/leaf.rs")
        );
        let original = fs::read_to_string(root.join("src/nested/leaf.rs"))?;
        assert!(
            found.mutations[2]
                .diff()
                .contains("+pub fn flag() -> bool { true }")
        );
        found.mutations[2].apply(root)?;
        assert!(fs::read_to_string(root.join("src/nested/leaf.rs"))?.contains("{ true }"));
        assert!(found.mutations[2].apply(root).is_err());
        fs::write(root.join("src/nested/leaf.rs"), original)?;
        Ok(())
    }

    #[test]
    fn rejects_ambiguous_modules_and_skips_file_level_conditions() -> Result<()> {
        let temp = tempfile::tempdir_in(std::env::temp_dir().canonicalize()?)?;
        let root = temp.path();
        fs::create_dir_all(root.join("src/child"))?;
        fs::write(root.join("src/main.rs"), "mod child; fn main() {}")?;
        fs::write(
            root.join("src/child.rs"),
            "#![cfg(unix)]\nfn flag() -> bool { true }",
        )?;
        assert!(scan(root, &[])?.mutations.is_empty());
        fs::write(root.join("src/child/mod.rs"), "fn flag() -> bool { false }")?;
        assert!(
            scan(root, &[])
                .unwrap_err()
                .to_string()
                .contains("ambiguous")
        );
        Ok(())
    }

    #[test]
    fn comparison_boundaries_edit_only_ordering_tokens_in_function_bodies() -> Result<()> {
        let temp = tempfile::tempdir_in(std::env::temp_dir().canonicalize()?)?;
        let root = temp.path();
        fs::create_dir(root.join("src"))?;
        let source = "fn compare<T: PartialOrd>(a: T, b: T) -> bool {\r\n\
            let π = || a /* < */ < b;\r\n\
            let pairs = (a<=b, a > b, a>=b);\r\n\
            let nested = (a < b) <= (a > b);\r\n\
            let equal = a == b || a != b;\r\n\
            let shifts = (1 << 2, 8 >> 1);\r\n\
            let generic = Vec::<Vec<u8>>::new();\r\n\
            let text = \"a <= b\";\r\n\
            let ranges = (0..=3, 0..3);\r\n\
            a >= b\r\n\
        }";
        fs::write(root.join("src/main.rs"), source)?;
        let found = scan(root, &[])?;
        let edits = found
            .mutations
            .iter()
            .map(|m| (m.original(), m.replacement.as_ref()))
            .collect::<Vec<_>>();
        assert_eq!(
            edits,
            [
                ("<", "<="),
                ("<=", "<"),
                (">", ">="),
                (">=", ">"),
                ("<", "<="),
                ("<=", "<"),
                (">", ">="),
                (">=", ">")
            ]
        );
        for mutation in &found.mutations {
            assert_eq!(mutation.operator, Operator::ComparisonBoundary);
            let prefix = &source[..mutation.range.start];
            assert_eq!(
                mutation.line,
                prefix.bytes().filter(|b| *b == b'\n').count() + 1
            );
            assert_eq!(
                mutation.column,
                prefix.rsplit('\n').next().unwrap().len() + 1
            );
            assert_eq!(mutation.end_line, mutation.line);
            assert_eq!(
                mutation.end_column,
                mutation.column + mutation.original().len()
            );
            mutation.apply(root)?;
            let changed = fs::read_to_string(root.join("src/main.rs"))?;
            assert_eq!(&changed[..mutation.range.start], prefix);
            assert_eq!(
                &changed[mutation.range.start + mutation.replacement.len()..],
                &source[mutation.range.end..]
            );
            fs::write(root.join("src/main.rs"), source)?;
        }
        assert!(
            found
                .mutations
                .last()
                .unwrap()
                .diff()
                .contains("\\ No newline at end of file")
        );
        assert!(scan(root, &["src/other.rs".into()])?.mutations.is_empty());
        Ok(())
    }

    #[test]
    fn comparison_boundaries_respect_excluded_source_and_const_contexts() -> Result<()> {
        let temp = tempfile::tempdir_in(std::env::temp_dir().canonicalize()?)?;
        let root = temp.path();
        fs::create_dir(root.join("src"))?;
        fs::write(
            root.join("src/main.rs"),
            r#"
#[cfg(test)] mod tests { fn compare() { let x = 1 < 2; } }
#[cfg(any())] mod missing;
#[path = "missing.rs"] mod remapped;
#[test] fn test() { let x = 1 <= 2; }
#[tokio::test] async fn async_test() { let x = 1 > 2; }
#[cfg_attr(unix, allow(dead_code))] fn conditional() { let x = 1 >= 2; }
macro_rules! compare { () => { 1 < 2 }; }
const FLAG: bool = 1 < 2;
static OTHER: bool = 1 > 2;
mod generated;
mod nested { pub fn compare(a: i32, b: i32) -> bool { a <= b } }
fn main() {
    #[cfg(unix)] let skipped = 1 < 2;
    let constant = const { 1 < 2 };
    let typed: [u8; (1 < 2) as usize] = [0];
    let repeated = [1 < 2; (3 < 4) as usize];
    let generic = f::<{ 1 > 2 }>();
    const LOCAL: bool = 1 >= 2;
    assert!(1 < 2);
    match 1 {
        #[cfg(unix)] x if x <= 3 => (),
        x if x > 4 => (),
        _ => (),
    }
}
"#,
        )?;
        fs::write(
            root.join("src/generated.rs"),
            "// @generated\nfn f() { let x = 1 < 2; }",
        )?;
        fs::write(root.join("src/orphan.rs"), "fn f() { let x = 1 < 2; }")?;
        let found = scan(root, &[])?;
        assert_eq!(
            found
                .mutations
                .iter()
                .map(Mutation::original)
                .collect::<Vec<_>>(),
            ["<=", "<", ">"]
        );
        let excluded = discover(
            root,
            Path::new("src/main.rs"),
            &Scope::new(&[], &["src/main.rs".into()])?,
            &Cancellation::default(),
        )?;
        assert!(excluded.mutations.is_empty());
        Ok(())
    }
}
