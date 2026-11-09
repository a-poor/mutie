use std::{
    borrow::Cow,
    fs,
    ops::Range,
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::{Context, Result, ensure};
use clap::ValueEnum;
use globset::{GlobBuilder, GlobSet, GlobSetBuilder};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tree_sitter::{Node, Parser};

use crate::cancellation::Cancellation;

pub(crate) mod assignment;

pub const OPERATOR_CATALOG_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize, ValueEnum)]
#[serde(rename_all = "snake_case")]
#[value(rename_all = "snake_case")]
pub enum Operator {
    ArithmeticReplace,
    ComparisonInvert,
    ComparisonBoundary,
    LogicalSwap,
    BooleanFlip,
    NegationRemove,
    CallRemove,
    AssignmentRemove,
    BranchForceTrue,
    BranchForceFalse,
    LoopSkip,
}

impl std::fmt::Display for Operator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.to_possible_value().unwrap().get_name())
    }
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

    pub(crate) fn matches(&self, path: &Path) -> bool {
        (self.include.is_empty() || self.include.is_match(path)) && !self.exclude.is_match(path)
    }
}

#[derive(Debug)]
pub struct SourceFile {
    pub path: PathBuf,
    pub fingerprint: String,
    pub(crate) source: String,
}

#[derive(Clone, Debug)]
pub struct Mutation {
    pub file: Arc<SourceFile>,
    pub operator: Operator,
    pub range: Range<usize>,
    pub replacement: Cow<'static, str>,
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

    /// Describe a single edit, including multiline removal, with three context
    /// lines. Preserve CRLF and the final-newline marker in unified diffs.
    pub fn diff(&self) -> String {
        let lines: Vec<_> = self.file.source.split_inclusive('\n').collect();
        let mut changed = self.file.source.clone();
        changed.replace_range(self.range.clone(), &self.replacement);
        let changed: Vec<_> = changed.split_inclusive('\n').collect();
        let prefix = lines
            .iter()
            .zip(&changed)
            .take_while(|(a, b)| a == b)
            .count();
        let suffix = lines[prefix..]
            .iter()
            .rev()
            .zip(changed[prefix..].iter().rev())
            .take_while(|(a, b)| a == b)
            .count();
        let start = prefix.saturating_sub(3);
        let old_end = lines.len() - suffix;
        let new_end = changed.len() - suffix;
        let context = suffix.min(3);
        let old_count = old_end + context - start;
        let new_count = new_end + context - start;
        let path = self
            .file
            .path
            .to_string_lossy()
            .replace(std::path::MAIN_SEPARATOR, "/");
        let mut diff = format!(
            "--- a/{path}\n+++ b/{path}\n@@ -{},{} +{},{} @@\n",
            start + usize::from(old_count > 0),
            old_count,
            start + usize::from(new_count > 0),
            new_count
        );
        fn append(diff: &mut String, prefix: char, line: &str) {
            diff.push(prefix);
            diff.push_str(line);
            if !line.ends_with('\n') {
                diff.push_str("\n\\ No newline at end of file\n");
            }
        }
        for line in &lines[start..prefix] {
            append(&mut diff, ' ', line);
        }
        for line in &lines[prefix..old_end] {
            append(&mut diff, '-', line);
        }
        for line in &changed[prefix..new_end] {
            append(&mut diff, '+', line);
        }
        for line in &lines[old_end..old_end + context] {
            append(&mut diff, ' ', line);
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
        source.replace_range(self.range.clone(), &self.replacement);
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
    if assignment::eligible(node) {
        let start = node.start_position();
        let end = node.end_position();
        mutations.push(Mutation {
            file: Arc::clone(file),
            operator: Operator::AssignmentRemove,
            range: node.byte_range(),
            replacement: "".into(),
            line: start.row + 1,
            column: start.column + 1,
            end_line: end.row + 1,
            end_column: end.column + 1,
        });
    }
    let (condition, guards): (_, &[(Operator, &str)]) = match node.kind() {
        "if_statement" => (
            node.child_by_field_name("condition"),
            &[
                (Operator::BranchForceTrue, "(0 == 0) ||"),
                (Operator::BranchForceFalse, "(0 != 0) &&"),
            ],
        ),
        "for_statement" => (loop_condition(node), &[(Operator::LoopSkip, "(0 != 0) &&")]),
        _ => (None, &[]),
    };
    if let Some(condition) = condition
        // Leave literal conditions (including parentheses/comments) to boolean_flip.
        // This is syntactic eligibility, not constant folding or name resolution.
        && !matches!(unparenthesized(condition).kind(), "true" | "false")
    {
        let original = &file.source[condition.byte_range()];
        let start = condition.start_position();
        let end = condition.end_position();
        for &(operator, guard) in guards {
            // Retain references so imports/locals (including initializers) stay
            // used, but never evaluate the condition. Constant comparisons avoid
            // depending on Go's shadowable true/false identifiers. The initializer
            // and bodies remain untouched; a skipped loop never runs its update.
            mutations.push(Mutation {
                file: Arc::clone(file),
                operator,
                range: condition.byte_range(),
                replacement: format!("{guard} ({original})").into(),
                line: start.row + 1,
                column: start.column + 1,
                end_line: end.row + 1,
                end_column: end.column + 1,
            });
        }
    }
    // Only statements directly in a block/case body. Do not remove initializers,
    // loop updates, labeled statements, go/defer calls, or calls used as values.
    if node.kind() == "expression_statement"
        && node
            .parent()
            .is_some_and(|parent| parent.kind() == "statement_list")
        && let Some(expression) = expression_child(node)
    {
        let expression = unparenthesized(expression);
        if expression.kind() == "call_expression" {
            let start = node.start_position();
            let end = node.end_position();
            mutations.push(Mutation {
                file: Arc::clone(file),
                operator: Operator::CallRemove,
                range: node.byte_range(),
                replacement: "".into(),
                line: start.row + 1,
                column: start.column + 1,
                end_line: end.row + 1,
                end_column: end.column + 1,
            });
        }
    }
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
            replacement: (*replacement).into(),
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

fn loop_condition(node: Node<'_>) -> Option<Node<'_>> {
    // In the pinned Go grammar, a single condition is an unnamed-field expression
    // child. A three-part header instead puts it in for_clause.condition.
    let header = expression_child(node)?;
    match header.kind() {
        "for_clause" => header.child_by_field_name("condition"),
        "range_clause" | "block" => None,
        _ => Some(header),
    }
}

fn expression_child(node: Node<'_>) -> Option<Node<'_>> {
    node.named_children(&mut node.walk())
        .find(|child| !child.is_extra())
}

fn unparenthesized(mut node: Node<'_>) -> Node<'_> {
    while node.kind() == "parenthesized_expression" {
        let Some(inner) = expression_child(node) else {
            break;
        };
        node = inner;
    }
    node
}

/// Stable discovery fields shared by all CLI list commands.
pub fn preview_json(id: usize, mutation: &Mutation) -> serde_json::Value {
    serde_json::json!({"id":id, "operator":mutation.operator, "file":mutation.file.path,
        "source_fingerprint":mutation.file.fingerprint, "line":mutation.line, "column":mutation.column,
        "byte_start":mutation.range.start, "byte_end":mutation.range.end,
        "original":mutation.original(), "replacement":mutation.replacement})
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loop_skip_selects_only_explicit_nonliteral_conditions() -> Result<()> {
        let root = tempfile::tempdir()?;
        let source = r#"package p
func f(b bool, ch chan bool, xs []int) {
    for b { }
    for ( /* before */ b /* after */ ) { }
    for start(); ready(); next() { }
    for ; b; { }
    for i := 0; i < len(xs); i++ { }
    for b && // condition comment
        ready() { }
label: for <-ch { break label }
    for func() bool { for inner { }; return b }() { }
    for { for nested { }; break }
    for i := 0; ; i++ { break }
    for ;; { break }
    for true { break }
    for ( /* literal */ (false)) { }
    for start(); (true); next() { break }
    for ; false; { }
    for range xs { }
    for i := range xs { _ = i }
    for _, x := range xs { _ = x }
    for range 3 { }
    for range func(yield func(int) bool) { yield(1) } { }
    if b { }
    switch { case b: }
}
"#;
        fs::write(root.path().join("code.go"), source)?;
        let scope = Scope::new(&[], &[])?;
        let all = discover(root.path(), vec!["code.go".into()], &scope)?;
        let loops: Vec<_> = all
            .iter()
            .filter(|m| m.operator == Operator::LoopSkip)
            .collect();
        assert_eq!(
            loops.iter().map(|m| m.original()).collect::<Vec<_>>(),
            [
                "b",
                "( /* before */ b /* after */ )",
                "ready()",
                "b",
                "i < len(xs)",
                "b && // condition comment\n        ready()",
                "<-ch",
                "func() bool { for inner { }; return b }()",
                "inner",
                "nested",
            ]
        );
        let again = discover(root.path(), vec!["code.go".into()], &scope)?;
        let mut parser = Parser::new();
        parser.set_language(&tree_sitter_go::LANGUAGE.into())?;
        for m in loops {
            assert_eq!(m.replacement, format!("(0 != 0) && ({})", m.original()));
            let identity = crate::identity::MutationIdentity::from(m);
            identity.validate()?;
            assert_eq!(
                again.iter().filter(|next| identity.matches(next)).count(),
                1
            );
            let mut expected = source.to_owned();
            expected.replace_range(m.range.clone(), &m.replacement);
            fs::write(root.path().join("code.go"), source)?;
            m.apply(root.path())?;
            assert_eq!(fs::read_to_string(root.path().join("code.go"))?, expected);
            assert!(
                !parser
                    .parse(&expected, None)
                    .unwrap()
                    .root_node()
                    .has_error(),
                "{expected}"
            );
            fs::write(root.path().join("code.go"), format!("{source}// changed\n"))?;
            assert!(m.apply(root.path()).is_err());
        }
        // Discovery still visits the original header/body, including excluded loops.
        for (operator, count) in [
            (Operator::BooleanFlip, 4),
            (Operator::LogicalSwap, 1),
            (Operator::ComparisonInvert, 1),
            (Operator::ComparisonBoundary, 1),
            (Operator::BranchForceTrue, 1),
            (Operator::BranchForceFalse, 1),
        ] {
            assert_eq!(all.iter().filter(|m| m.operator == operator).count(), count);
        }
        Ok(())
    }

    #[test]
    fn loop_skip_diff_changes_only_the_multiline_condition() -> Result<()> {
        let root = tempfile::tempdir()?;
        let source = "package p\r\nfunc f(é bool) {\r\n\tfor init(); é && // condition\r\n\t\tready(); next() { // body\r\n\t}\r\n}";
        fs::write(root.path().join("code.go"), source)?;
        let mutations = discover(root.path(), vec!["code.go".into()], &Scope::new(&[], &[])?)?;
        let m = &mutations[0];
        assert_eq!(m.operator, Operator::LoopSkip);
        assert_eq!((m.line, m.column, m.end_line, m.end_column), (3, 14, 4, 10));
        assert_eq!(
            m.diff(),
            concat!(
                "--- a/code.go\n+++ b/code.go\n@@ -1,6 +1,6 @@\n",
                " package p\r\n func f(é bool) {\r\n",
                "-\tfor init(); é && // condition\r\n-\t\tready(); next() { // body\r\n",
                "+\tfor init(); (0 != 0) && (é && // condition\r\n+\t\tready()); next() { // body\r\n",
                " \t}\r\n }\n\\ No newline at end of file\n",
            )
        );
        Ok(())
    }

    #[test]
    fn branches_select_whole_if_conditions_and_keep_nested_operators() -> Result<()> {
        let root = tempfile::tempdir()?;
        let source = r#"package p
func f(b bool, ch chan bool) {
    if b { } else if x := check(); x { }
    if ( /* before */ b /* after */ ) { }
    if b && // continue the condition
        check() { }
    if func() bool { if nested { }; return b }() { }
label: if <-ch { }
    if true { } else if ( /* constant */ (false)) { }
    for b { }
    switch { case b: }
    _ = b
}
"#;
        fs::write(root.path().join("code.go"), source)?;
        let scope = Scope::new(&[], &[])?;
        let all = discover(root.path(), vec!["code.go".into()], &scope)?;
        let branches: Vec<_> = all
            .iter()
            .filter(|m| {
                matches!(
                    m.operator,
                    Operator::BranchForceTrue | Operator::BranchForceFalse
                )
            })
            .collect();
        let originals = [
            "b",
            "x",
            "( /* before */ b /* after */ )",
            "b && // continue the condition\n        check()",
            "func() bool { if nested { }; return b }()",
            "nested",
            "<-ch",
        ];
        assert_eq!(branches.len(), originals.len() * 2);
        let again = discover(root.path(), vec!["code.go".into()], &scope)?;
        let mut parser = Parser::new();
        parser.set_language(&tree_sitter_go::LANGUAGE.into())?;
        for (pair, original) in branches.as_chunks::<2>().0.iter().zip(originals) {
            for (m, operator, guard) in [
                (pair[0], Operator::BranchForceTrue, "(0 == 0) ||"),
                (pair[1], Operator::BranchForceFalse, "(0 != 0) &&"),
            ] {
                assert_eq!(m.original(), original);
                assert_eq!(m.operator, operator);
                assert_eq!(m.replacement, format!("{guard} ({original})"));
                let identity = crate::identity::MutationIdentity::from(m);
                identity.validate()?;
                assert_eq!(
                    again.iter().filter(|next| identity.matches(next)).count(),
                    1
                );
                let mut expected = source.to_owned();
                expected.replace_range(m.range.clone(), &m.replacement);
                fs::write(root.path().join("code.go"), source)?;
                m.apply(root.path())?;
                assert_eq!(fs::read_to_string(root.path().join("code.go"))?, expected);
                assert!(
                    !parser
                        .parse(&expected, None)
                        .unwrap()
                        .root_node()
                        .has_error(),
                    "{expected}"
                );
                fs::write(root.path().join("code.go"), format!("{source}// changed\n"))?;
                assert!(m.apply(root.path()).is_err());
            }
        }
        assert_eq!(
            all.iter()
                .filter(|m| m.operator == Operator::BooleanFlip)
                .count(),
            2
        );
        assert_eq!(
            all.iter()
                .filter(|m| m.operator == Operator::LogicalSwap)
                .count(),
            1
        );
        Ok(())
    }

    #[test]
    fn branch_diff_preserves_multiline_condition_comments_crlf_and_unicode() -> Result<()> {
        let root = tempfile::tempdir()?;
        let source = "package p\r\nfunc f(é bool) {\r\n\tif é && // comment\r\n\t\tcheck() { // body\r\n\t}\r\n}";
        fs::write(root.path().join("code.go"), source)?;
        let mutations = discover(root.path(), vec!["code.go".into()], &Scope::new(&[], &[])?)?;
        let m = &mutations[0];
        assert_eq!(m.operator, Operator::BranchForceTrue);
        assert_eq!((m.line, m.column, m.end_line, m.end_column), (3, 5, 4, 10));
        assert_eq!(
            m.diff(),
            concat!(
                "--- a/code.go\n+++ b/code.go\n@@ -1,6 +1,6 @@\n",
                " package p\r\n func f(é bool) {\r\n",
                "-\tif é && // comment\r\n-\t\tcheck() { // body\r\n",
                "+\tif (0 == 0) || (é && // comment\r\n+\t\tcheck()) { // body\r\n",
                " \t}\r\n }\n\\ No newline at end of file\n",
            )
        );
        Ok(())
    }

    #[test]
    fn call_removal_only_selects_standalone_statements_and_preserves_other_mutations() -> Result<()>
    {
        let root = tempfile::tempdir()?;
        let source = r#"package p
func f(ch chan int, x any) int {
    direct()
    obj.method(nested())
    generic[int]()
    (parenthesized())
    ( /* comment */ commented() )
    println("é") // retain this comment
    multiline(
        nested(), // part of the removed statement
    )
    enabled(true)
    func() { inner() }()
    if initCall(); condition() { ifBody() }
    for start(); condition(); next() { loopBody() }
    switch initSwitch(); value() { case 1: caseBody(); default: defaultBody() }
    switch x.(type) { case int: typeBody() }
    select { case <-ch: selectBody(); default: selectDefault() }
    defer deferred()
    go concurrent()
label: labeled()
    assigned := used()
    assigned = usedAgain()
    _ = assigned
    ch <- sendValue()
    <-ch
    return returned()
}
"#;
        fs::write(root.path().join("code.go"), source)?;
        let mutations = discover(root.path(), vec!["code.go".into()], &Scope::new(&[], &[])?)?;
        let calls: Vec<_> = mutations
            .iter()
            .filter(|m| m.operator == Operator::CallRemove)
            .collect();
        assert_eq!(
            calls.iter().map(|m| m.original()).collect::<Vec<_>>(),
            [
                "direct()",
                "obj.method(nested())",
                "generic[int]()",
                "(parenthesized())",
                "( /* comment */ commented() )",
                "println(\"é\")",
                "multiline(\n        nested(), // part of the removed statement\n    )",
                "enabled(true)",
                "func() { inner() }()",
                "inner()",
                "ifBody()",
                "loopBody()",
                "caseBody()",
                "defaultBody()",
                "typeBody()",
                "selectBody()",
                "selectDefault()",
            ]
        );
        assert_eq!(
            mutations
                .iter()
                .filter(|m| m.operator == Operator::BooleanFlip)
                .count(),
            1
        );
        let mut parser = Parser::new();
        parser.set_language(&tree_sitter_go::LANGUAGE.into())?;
        for mutation in calls {
            crate::identity::MutationIdentity::from(mutation).validate()?;
            assert_eq!(mutation.replacement, "");
            let mut expected = source.to_owned();
            expected.replace_range(mutation.range.clone(), "");
            fs::write(root.path().join("code.go"), source)?;
            mutation.apply(root.path())?;
            assert_eq!(fs::read_to_string(root.path().join("code.go"))?, expected);
            assert!(
                !parser
                    .parse(&expected, None)
                    .unwrap()
                    .root_node()
                    .has_error(),
                "{}",
                mutation.description()
            );
            assert!(expected.contains("// retain this comment"));
        }
        Ok(())
    }

    #[test]
    fn multiline_call_diff_preserves_crlf_unicode_comments_and_final_line() -> Result<()> {
        let root = tempfile::tempdir()?;
        let source = "package p\r\nfunc f() {\r\n\t// before\r\n\tprintln(\r\n\t\t\"é\", // inside\r\n\t) // after\r\n}";
        fs::write(root.path().join("code.go"), source)?;
        let mutations = discover(root.path(), vec!["code.go".into()], &Scope::new(&[], &[])?)?;
        assert_eq!(mutations.len(), 1);
        let m = &mutations[0];
        assert_eq!((m.line, m.column, m.end_line, m.end_column), (4, 2, 6, 3));
        assert_eq!(
            m.diff(),
            concat!(
                "--- a/code.go\n+++ b/code.go\n@@ -1,7 +1,5 @@\n",
                " package p\r\n func f() {\r\n \t// before\r\n",
                "-\tprintln(\r\n-\t\t\"é\", // inside\r\n-\t) // after\r\n",
                "+\t // after\r\n }\n\\ No newline at end of file\n"
            )
        );
        Ok(())
    }

    #[test]
    fn diff_bounds_context_and_handles_whole_line_or_file_removal() {
        for (source, range, expected) in [
            (
                "a\nb\nc\nd\ne\nf\ng\nh\ni\n",
                8..12,
                "@@ -2,8 +2,6 @@\n b\n c\n d\n-e\n-f\n g\n h\n i\n",
            ),
            ("a\nb\n", 0..4, "@@ -1,2 +0,0 @@\n-a\n-b\n"),
            (
                "a\nb",
                0..3,
                "@@ -1,2 +0,0 @@\n-a\n-b\n\\ No newline at end of file\n",
            ),
        ] {
            let m = Mutation {
                file: Arc::new(SourceFile {
                    path: "code.go".into(),
                    fingerprint: String::new(),
                    source: source.into(),
                }),
                operator: Operator::CallRemove,
                range,
                replacement: "".into(),
                line: 1,
                column: 1,
                end_line: 1,
                end_column: 1,
            };
            assert_eq!(
                m.diff(),
                format!("--- a/code.go\n+++ b/code.go\n{expected}")
            );
        }
    }

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
