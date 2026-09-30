use std::{
    fs,
    ops::Range,
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::{Context, Result, ensure};
use globset::{GlobBuilder, GlobSet, GlobSetBuilder};
use serde::Serialize;
use sha2::{Digest, Sha256};
use tree_sitter::{Node, Parser};

use crate::cancellation::Cancellation;

pub const OPERATOR_CATALOG_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Operator {
    ArithmeticReplace,
    ComparisonInvert,
    ComparisonBoundary,
    LogicalSwap,
    BooleanFlip,
    NegationRemove,
}

pub struct Scope {
    include: GlobSet,
    exclude: GlobSet,
}

impl Scope {
    pub fn new(include: &[String], exclude: &[String]) -> Result<Self> {
        fn compile(patterns: &[String]) -> Result<GlobSet> {
            let mut builder = GlobSetBuilder::new();
            for pattern in patterns {
                builder.add(
                    GlobBuilder::new(pattern)
                        .literal_separator(true)
                        .build()
                        .with_context(|| format!("invalid glob {pattern:?}"))?,
                );
            }
            Ok(builder.build()?)
        }
        Ok(Self {
            include: compile(include)?,
            exclude: compile(exclude)?,
        })
    }

    fn matches(&self, path: &Path) -> bool {
        (self.include.is_empty() || self.include.is_match(path)) && !self.exclude.is_match(path)
    }
}

#[derive(Debug)]
pub struct SourceFile {
    pub path: PathBuf,
    pub fingerprint: String,
    source: String,
}

#[derive(Clone, Debug)]
pub struct Mutation {
    pub file: Arc<SourceFile>,
    pub operator: Operator,
    pub range: Range<usize>,
    pub replacement: &'static str,
    pub line: usize,
    pub column: usize,
    pub end_line: usize,
    pub end_column: usize,
}

impl Mutation {
    pub fn original(&self) -> &str {
        &self.file.source[self.range.clone()]
    }

    pub fn description(&self) -> String {
        format!(
            "{}:{}:{}: {:?} -> {:?}",
            self.file.path.display(),
            self.line,
            self.column,
            self.original(),
            self.replacement
        )
    }

    /// All current operators replace a token on one line. Include three lines
    /// of context, preserving CRLF and the final-newline marker in unified diffs.
    pub fn diff(&self) -> String {
        let lines: Vec<_> = self.file.source.split_inclusive('\n').collect();
        let index = self.line - 1;
        let offset = self.range.start - lines[..index].iter().map(|line| line.len()).sum::<usize>();
        let mut changed = lines[index].to_owned();
        changed.replace_range(offset..offset + self.original().len(), self.replacement);
        let start = index.saturating_sub(3);
        let end = (index + 4).min(lines.len());
        let path = self
            .file
            .path
            .to_string_lossy()
            .replace(std::path::MAIN_SEPARATOR, "/");
        let mut diff = format!(
            "--- a/{path}\n+++ b/{path}\n@@ -{},{} +{},{} @@\n",
            start + 1,
            end - start,
            start + 1,
            end - start
        );
        fn append(diff: &mut String, prefix: char, line: &str) {
            diff.push(prefix);
            diff.push_str(line);
            if !line.ends_with('\n') {
                diff.push_str("\n\\ No newline at end of file\n");
            }
        }
        for (position, line) in lines.iter().enumerate().take(end).skip(start) {
            if position == index {
                append(&mut diff, '-', line);
                append(&mut diff, '+', &changed);
            } else {
                append(&mut diff, ' ', line);
            }
        }
        diff
    }

    pub fn apply(&self, root: &Path) -> Result<()> {
        let path = root.join(&self.file.path);
        let mut source = fs::read_to_string(&path)?;
        ensure!(
            source == self.file.source,
            "{} changed after discovery",
            path.display()
        );
        source.replace_range(self.range.clone(), self.replacement);
        fs::write(&path, source).with_context(|| format!("mutating {}", path.display()))
    }
}

#[cfg(test)]
pub fn discover(root: &Path, mut files: Vec<PathBuf>, scope: &Scope) -> Result<Vec<Mutation>> {
    discover_cancellable(root, &mut files, scope, &Cancellation::default())
}

pub fn discover_cancellable(
    root: &Path,
    files: &mut Vec<PathBuf>,
    scope: &Scope,
    cancellation: &Cancellation,
) -> Result<Vec<Mutation>> {
    let mut parser = Parser::new();
    parser.set_language(&tree_sitter_go::LANGUAGE.into())?;
    let mut mutations = Vec::new();
    files.sort();
    files.dedup();
    for path in files {
        cancellation.check()?;
        if !scope.matches(path) || path.to_string_lossy().ends_with("_test.go") {
            continue;
        }
        let source = fs::read_to_string(root.join(&path))
            .with_context(|| format!("reading {}", path.display()))?;
        if source
            .lines()
            .any(|line| line.starts_with("// Code generated ") && line.ends_with(" DO NOT EDIT."))
        {
            continue;
        }
        let tree = parser
            .parse(&source, None)
            .context("Go parser did not return a tree")?;
        ensure!(
            !tree.root_node().has_error(),
            "could not parse {} as Go",
            path.display()
        );
        let fingerprint = format!("sha256:{:x}", Sha256::digest(source.as_bytes()));
        let file = Arc::new(SourceFile {
            path: path.clone(),
            source,
            fingerprint,
        });
        visit(tree.root_node(), &file, &mut mutations);
    }
    Ok(mutations)
}

fn visit(node: Node<'_>, file: &Arc<SourceFile>, mutations: &mut Vec<Mutation>) {
    let candidate = match node.kind() {
        "binary_expression" | "unary_expression" => node.child_by_field_name("operator"),
        "true" | "false" => Some(node),
        _ => None,
    };
    if let Some(token) = candidate {
        let original = &file.source[token.byte_range()];
        let replacements: &[&'static str] = match (node.kind(), original) {
            ("binary_expression", "==") => &["!="],
            ("binary_expression", "!=") => &["=="],
            ("binary_expression", "<") => &[">=", "<="],
            ("binary_expression", "<=") => &[">", "<"],
            ("binary_expression", ">") => &["<=", ">="],
            ("binary_expression", ">=") => &["<", ">"],
            ("binary_expression", "+") => &["-"],
            ("binary_expression", "-") => &["+"],
            ("binary_expression", "*") => &["/"],
            ("binary_expression", "/") => &["*"],
            ("binary_expression", "%") => &["*"],
            ("binary_expression", "&&") => &["||"],
            ("binary_expression", "||") => &["&&"],
            ("unary_expression", "!") => &[""],
            ("true", _) => &["false"],
            ("false", _) => &["true"],
            _ => &[],
        };
        let position = token.start_position();
        let end = token.end_position();
        mutations.extend(replacements.iter().map(|replacement| Mutation {
            file: Arc::clone(file),
            operator: match original {
                "true" | "false" => Operator::BooleanFlip,
                "!" => Operator::NegationRemove,
                "&&" | "||" => Operator::LogicalSwap,
                "==" | "!=" => Operator::ComparisonInvert,
                "<" | "<=" | ">" | ">=" => {
                    if original.starts_with('<') == replacement.starts_with('<') {
                        Operator::ComparisonBoundary
                    } else {
                        Operator::ComparisonInvert
                    }
                }
                _ => Operator::ArithmeticReplace,
            },
            range: token.byte_range(),
            replacement,
            line: position.row + 1,
            column: position.column + 1,
            end_line: end.row + 1,
            end_column: end.column + 1,
        }));
    }
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        visit(child, file, mutations);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diff_preserves_unicode_crlf_and_missing_final_newline() -> Result<()> {
        let root = tempfile::tempdir()?;
        let source = "package p\r\n\r\nfunc f() string { return \"é\" + \"x\" }";
        fs::write(root.path().join("code.go"), source)?;
        let mutations = discover(root.path(), vec!["code.go".into()], &Scope::new(&[], &[])?)?;
        assert_eq!(
            mutations[0].diff(),
            concat!(
                "--- a/code.go\n+++ b/code.go\n@@ -1,3 +1,3 @@\n",
                " package p\r\n \r\n",
                "-func f() string { return \"é\" + \"x\" }\n\\ No newline at end of file\n",
                "+func f() string { return \"é\" - \"x\" }\n\\ No newline at end of file\n"
            )
        );
        Ok(())
    }

    #[test]
    fn discovers_syntax_not_comments_or_strings_and_applies_utf8_byte_ranges() -> Result<()> {
        let root = tempfile::tempdir()?;
        let source = "package p\n// true + false\nvar text = \"é true + false\"\nfunc f(a, b int) bool { return a < b && !false }\n";
        fs::write(root.path().join("code.go"), source)?;
        let mutations = discover(root.path(), vec!["code.go".into()], &Scope::new(&[], &[])?)?;
        assert_eq!(mutations.len(), 5);
        assert!(mutations.iter().all(|m| m.line == 4));
        assert_eq!(
            mutations.iter().map(|m| m.operator).collect::<Vec<_>>(),
            vec![
                Operator::LogicalSwap,
                Operator::ComparisonInvert,
                Operator::ComparisonBoundary,
                Operator::NegationRemove,
                Operator::BooleanFlip,
            ]
        );
        for mutation in &mutations {
            assert_eq!(mutation.end_line, mutation.line);
            assert_eq!(
                mutation.end_column - mutation.column,
                mutation.original().len()
            );
        }
        mutations[0].apply(root.path())?;
        assert_eq!(
            fs::read_to_string(root.path().join("code.go"))?,
            source.replace("&&", "||")
        );
        Ok(())
    }

    #[test]
    fn fingerprints_identify_full_source_and_discovery_order_is_deterministic() -> Result<()> {
        let root = tempfile::tempdir()?;
        let source = "package p\nvar a = true\n";
        fs::write(root.path().join("a.go"), source)?;
        fs::write(root.path().join("b.go"), source)?;
        let scope = Scope::new(&[], &[])?;
        let mutations = discover(
            root.path(),
            vec!["b.go".into(), "a.go".into(), "a.go".into()],
            &scope,
        )?;
        assert_eq!(mutations.len(), 2);
        assert_eq!(mutations[0].file.path, Path::new("a.go"));
        assert_eq!(mutations[0].file.fingerprint, mutations[1].file.fingerprint);
        assert!(mutations[0].file.fingerprint.starts_with("sha256:"));
        assert_eq!(mutations[0].file.fingerprint.len(), 71);
        fs::write(
            root.path().join("a.go"),
            format!("{source}// changed elsewhere\n"),
        )?;
        let changed = discover(root.path(), vec!["a.go".into()], &scope)?;
        assert_ne!(mutations[0].file.fingerprint, changed[0].file.fingerprint);
        assert_eq!(mutations[0].range, changed[0].range);
        Ok(())
    }

    #[test]
    fn scope_excludes_tests_generated_files_and_unselected_paths() -> Result<()> {
        let root = tempfile::tempdir()?;
        fs::create_dir(root.path().join("sub"))?;
        for path in ["code.go", "code_test.go", "sub/code.go", "generated.go"] {
            let prefix = if path == "generated.go" {
                "// Code generated by tool. DO NOT EDIT.\n"
            } else {
                ""
            };
            fs::write(
                root.path().join(path),
                format!("{prefix}package p\nvar b = true\n"),
            )?;
        }
        let files = vec![
            "code.go".into(),
            "code_test.go".into(),
            "sub/code.go".into(),
            "generated.go".into(),
        ];
        let scope = Scope::new(&["**/*.go".into()], &["sub/**".into()])?;
        let mutations = discover(root.path(), files, &scope)?;
        assert_eq!(mutations.len(), 1);
        assert_eq!(mutations[0].file.path, Path::new("code.go"));
        Ok(())
    }
}
