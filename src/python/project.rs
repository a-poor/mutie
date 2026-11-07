use std::{
    borrow::Cow,
    collections::BTreeMap,
    fs,
    path::{Component, Path, PathBuf},
    sync::Arc,
};

use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    cancellation::Cancellation,
    mutation::{Mutation, Operator, Scope, SourceFile},
};

/// Exact source recipes; AST admission and fresh discovery establish syntax/context.
pub(crate) fn mutation_operator(original: &str, replacement: &str) -> Option<Operator> {
    if !original.trim().is_empty() && !matches!(original, "True" | "False") {
        for (operator, guard) in [
            (Operator::BranchForceTrue, "True or"),
            (Operator::BranchForceFalse, "False and"),
        ] {
            if replacement == format!("({guard} ({original}))") {
                return Some(operator);
            }
        }
    }
    Some(match (original, replacement) {
        ("and", "or") | ("or", "and") => Operator::LogicalSwap,
        ("True", "False") | ("False", "True") => Operator::BooleanFlip,
        ("<", "<=") | ("<=", "<") | (">", ">=") | (">=", ">") => Operator::ComparisonBoundary,
        ("==", "!=") | ("!=", "==") | ("<", ">=") | ("<=", ">") | (">", "<=") | (">=", "<") => {
            Operator::ComparisonInvert
        }
        ("+", "-") | ("-", "+") | ("*", "/") | ("/", "*") | ("%", "*") => {
            Operator::ArithmeticReplace
        }
        _ => {
            let operand = original.strip_prefix("not")?;
            if !operand.starts_with([
                ' ', '\t', '\r', '\n', '\x0c', '\\', '(', '[', '{', '"', '\'', '~', '+', '-', '.',
                '#',
            ]) || operand.trim().is_empty()
                || replacement != format!("({operand})")
            {
                return None;
            }
            Operator::NegationRemove
        }
    })
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Site {
    file: String,
    start: usize,
    end: usize,
    line: usize,
    column: usize,
    end_line: usize,
    end_column: usize,
    original: String,
    replacement: String,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    loop_skip: bool,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Document {
    package: String,
    #[serde(default)]
    source_root: String,
    files: BTreeMap<String, String>,
    tests: Vec<String>,
    conftests: Vec<String>,
    sources: Vec<String>,
    mutations: Vec<Site>,
    #[serde(default)]
    expected_failure_calls: bool,
    #[serde(default)]
    data_files: Vec<String>,
    #[serde(default)]
    directories: Vec<String>,
    #[serde(default)]
    application_dependencies: std::collections::BTreeMap<String, String>,
}

pub(crate) struct Project {
    root: PathBuf,
    document: Document,
}

impl Project {
    pub(crate) fn validate_snapshot(&self, copied: &Self) -> Result<()> {
        ensure!(
            self.document.package == copied.document.package
                && self.document.source_root == copied.document.source_root
                && self.document.sources == copied.document.sources
                && self.document.tests == copied.document.tests
                && self.document.conftests == copied.document.conftests
                && self.document.data_files == copied.document.data_files
                && self.document.directories == copied.document.directories
                && self.document.application_dependencies
                    == copied.document.application_dependencies
                && self
                    .document
                    .data_files
                    .iter()
                    .all(|path| self.document.files.get(path) == copied.document.files.get(path))
                && ["pytest.ini", "tests/__init__.py"].iter().all(|path| self
                    .document
                    .files
                    .get(*path)
                    == copied.document.files.get(*path))
                && self
                    .document
                    .sources
                    .iter()
                    .chain(self.document.tests.iter())
                    .chain(self.document.conftests.iter())
                    .map(String::as_str)
                    .chain(["pyproject.toml"])
                    .all(|path| self.document.files.get(path) == copied.document.files.get(path)),
            "Python executable inputs changed during snapshotting; report exclusions cannot hide tests or source"
        );
        Ok(())
    }
    pub(crate) async fn load(
        root: &Path,
        executable: &Path,
        cancellation: &Cancellation,
    ) -> Result<Self> {
        let root = root.canonicalize()?;
        let raw = super::helper(
            executable,
            super::SCOPE,
            &[root.to_str().context("Python root is not UTF-8")?],
            cancellation,
        )
        .await?;
        Ok(Self {
            root,
            document: serde_json::from_value(raw).context("invalid Python scope evidence")?,
        })
    }

    pub(crate) fn value(&self) -> Result<serde_json::Value> {
        Ok(serde_json::to_value(&self.document)?)
    }

    pub(crate) fn discover(&self, scope: &Scope) -> Result<Vec<Mutation>> {
        let mut sources = BTreeMap::<String, Arc<SourceFile>>::new();
        for name in &self.document.sources {
            let path = Path::new(name);
            ensure!(
                matches!(self.document.source_root.as_str(), "" | "src")
                    && path.starts_with(
                        Path::new(&self.document.source_root).join(&self.document.package)
                    )
                    && path.components().all(|c| matches!(c, Component::Normal(_)))
                    && path.extension().is_some_and(|extension| extension == "py"),
                "invalid Python source path"
            );
            let source = fs::read_to_string(self.root.join(path))?;
            let fingerprint = format!("sha256:{:x}", Sha256::digest(source.as_bytes()));
            ensure!(
                self.document.files.get(name) == Some(&fingerprint),
                "Python source changed during discovery: {name}"
            );
            ensure!(
                sources
                    .insert(
                        name.clone(),
                        Arc::new(SourceFile {
                            path: path.to_owned(),
                            fingerprint,
                            source,
                        })
                    )
                    .is_none(),
                "duplicate Python source path"
            );
        }
        let mut found = Vec::new();
        let mut previous: Option<(&str, usize, std::cmp::Reverse<usize>, &str)> = None;
        for site in &self.document.mutations {
            let source = sources
                .get(&site.file)
                .context("Python mutation outside admitted source")?;
            let range = site.start..site.end;
            let mut operator = mutation_operator(&site.original, &site.replacement)
                .context("invalid Python mutation recipe")?;
            if site.loop_skip {
                ensure!(
                    operator == Operator::BranchForceFalse,
                    "invalid Python loop recipe"
                );
                operator = Operator::LoopSkip;
            }
            ensure!(
                source.source.get(range.clone()) == Some(site.original.as_str()),
                "invalid Python mutation span"
            );
            let key = (
                site.file.as_str(),
                site.start,
                std::cmp::Reverse(site.end),
                site.replacement.as_str(),
            );
            ensure!(
                previous.is_none_or(|prior| prior < key),
                "unordered/duplicate Python mutations"
            );
            previous = Some(key);
            let prefix = &source.source.as_bytes()[..site.start];
            let line = prefix.iter().filter(|b| **b == b'\n').count() + 1;
            let column = prefix.len()
                - prefix
                    .iter()
                    .rposition(|b| *b == b'\n')
                    .map_or(0, |p| p + 1)
                + 1;
            let end_prefix = &source.source.as_bytes()[..site.end];
            let end_line = end_prefix.iter().filter(|b| **b == b'\n').count() + 1;
            let end_column = end_prefix.len()
                - end_prefix
                    .iter()
                    .rposition(|b| *b == b'\n')
                    .map_or(0, |p| p + 1)
                + 1;
            ensure!(
                site.line == line
                    && site.end_line == end_line
                    && site.column == column
                    && site.end_column == end_column,
                "Python source coordinates mismatch"
            );
            if scope.matches(&source.path) {
                found.push(Mutation {
                    file: source.clone(),
                    operator,
                    range,
                    replacement: Cow::Owned(site.replacement.clone()),
                    line,
                    column,
                    end_line: site.end_line,
                    end_column: site.end_column,
                });
            }
        }
        Ok(found)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn branch_recipes_are_exact() {
        for original in ["value", "a or b", "a # comment\n or b", "π()"] {
            assert_eq!(
                mutation_operator(original, &format!("(True or ({original}))")),
                Some(Operator::BranchForceTrue)
            );
            assert_eq!(
                mutation_operator(original, &format!("(False and ({original}))")),
                Some(Operator::BranchForceFalse)
            );
            assert_eq!(
                mutation_operator(original, &format!("(True and ({original}))")),
                None
            );
            assert_eq!(mutation_operator(original, "True"), None);
        }
        for original in ["", " ", "True", "False"] {
            assert_eq!(
                mutation_operator(original, &format!("(True or ({original}))")),
                None
            );
        }
    }

    #[test]
    fn optional_loop_flag_preserves_discovery_json() -> Result<()> {
        let mut value = json!({
            "file":"sample/__init__.py", "start":6, "end":10,
            "line":1, "column":7, "end_line":1, "end_column":11,
            "original":"True", "replacement":"False"
        });
        let site: Site = serde_json::from_value(value.clone())?;
        assert_eq!(serde_json::to_value(site)?, value);
        value["loop_skip"] = true.into();
        let site: Site = serde_json::from_value(value.clone())?;
        assert_eq!(serde_json::to_value(site)?, value);
        Ok(())
    }

    #[test]
    fn loop_sites_require_false_guard_and_preserve_source() -> Result<()> {
        let root = tempfile::tempdir()?;
        fs::create_dir(root.path().join("sample"))?;
        let source = "while value: break\n";
        fs::write(root.path().join("sample/__init__.py"), source)?;
        let document = serde_json::from_value(json!({
            "package":"sample", "files":{"sample/__init__.py":format!("sha256:{:x}",Sha256::digest(source.as_bytes()))},
            "sources":["sample/__init__.py"], "tests":["tests/test_sample.py"], "conftests":[],
            "mutations":[{"file":"sample/__init__.py","start":6,"end":11,
                "line":1,"column":7,"end_line":1,"end_column":12,
                "original":"value","replacement":"(False and (value))", "loop_skip":true}]
        }))?;
        let mut project = Project {
            root: root.path().to_owned(),
            document,
        };
        let scope = Scope::new(&[], &[])?;
        let mutations = project.discover(&scope)?;
        assert_eq!(mutations[0].operator, Operator::LoopSkip);
        for invalid in ["(True or (value))", "False", "(False and value)"] {
            project.document.mutations[0].replacement = invalid.into();
            assert!(project.discover(&scope).is_err());
        }
        mutations[0].apply(root.path())?;
        assert_eq!(
            fs::read_to_string(root.path().join("sample/__init__.py"))?,
            "while (False and (value)): break\n"
        );
        assert!(mutations[0].apply(root.path()).is_err());
        Ok(())
    }

    #[test]
    fn logical_spans_require_exact_pairs_and_source_identity() -> Result<()> {
        for (original, replacement) in [("and", "or"), ("or", "and")] {
            let root = tempfile::tempdir()?;
            fs::create_dir(root.path().join("sample"))?;
            let source = format!("label = 'π'; result = (a){original}(b)\n");
            fs::write(root.path().join("sample/__init__.py"), &source)?;
            let start = source.find(original).unwrap();
            let document = serde_json::from_value(json!({
                "package":"sample", "files":{"sample/__init__.py":format!("sha256:{:x}",Sha256::digest(source.as_bytes()))},
                "sources":["sample/__init__.py"], "tests":["tests/test_sample.py"], "conftests":[],
                "mutations":[{"file":"sample/__init__.py","start":start,"end":start+original.len(),
                    "line":1,"column":start+1,"end_line":1,"end_column":start+original.len()+1,
                    "original":original,"replacement":replacement}],
            }))?;
            let mut project = Project {
                root: root.path().to_owned(),
                document,
            };
            let mutations = project.discover(&Scope::new(&[], &[])?)?;
            assert_eq!(mutations[0].operator, Operator::LogicalSwap);
            assert_eq!(mutation_operator(original, original), None);
            project.document.mutations[0].replacement = "||".into();
            assert!(project.discover(&Scope::new(&[], &[])?).is_err());
            mutations[0].apply(root.path())?;
            assert_eq!(
                fs::read_to_string(root.path().join("sample/__init__.py"))?,
                format!("label = 'π'; result = (a){replacement}(b)\n")
            );
            assert!(mutations[0].apply(root.path()).is_err());
        }
        Ok(())
    }

    #[test]
    fn inversion_and_negation_recipes_are_exact() {
        for (original, replacement) in [
            ("==", "!="),
            ("!=", "=="),
            ("<", ">="),
            ("<=", ">"),
            (">", "<="),
            (">=", "<"),
        ] {
            assert_eq!(
                mutation_operator(original, replacement),
                Some(Operator::ComparisonInvert)
            );
        }
        for original in [
            "not x",
            "not(x)",
            "not not x",
            "not (x # comment\n)",
            "not\x0c x",
            "not[1]",
            "not\"text\"",
            "not~1",
            "not# comment\n x",
        ] {
            assert_eq!(
                mutation_operator(original, &format!("({})", &original[3..])),
                Some(Operator::NegationRemove)
            );
            assert_eq!(mutation_operator(original, "x"), None);
        }
        for (original, replacement) in [
            ("notable", "(able)"),
            ("not", "()"),
            ("not ", "( )"),
            ("<", "=="),
            ("is", "is not"),
            ("~x", "x"),
        ] {
            assert_eq!(mutation_operator(original, replacement), None);
        }
    }

    #[test]
    fn nested_and_shared_spans_preserve_coordinates_and_reject_duplicates() -> Result<()> {
        let root = tempfile::tempdir()?;
        fs::create_dir(root.path().join("sample"))?;
        let source = "not (\n x < y)";
        fs::write(root.path().join("sample/__init__.py"), source)?;
        let start = source.find('<').unwrap();
        let document = serde_json::from_value(json!({
            "package":"sample", "files":{"sample/__init__.py":format!("sha256:{:x}",Sha256::digest(source))},
            "sources":["sample/__init__.py"], "tests":["tests/test_sample.py"], "conftests":[],
            "mutations":[
                {"file":"sample/__init__.py","start":0,"end":source.len(), "line":1,"column":1,"end_line":2,"end_column":8, "original":source,"replacement":"( (\n x < y))"},
                {"file":"sample/__init__.py","start":start,"end":start+1, "line":2,"column":4,"end_line":2,"end_column":5, "original":"<","replacement":"<="},
                {"file":"sample/__init__.py","start":start,"end":start+1, "line":2,"column":4,"end_line":2,"end_column":5, "original":"<","replacement":">="}
            ],
        }))?;
        let mut project = Project {
            root: root.path().to_owned(),
            document,
        };
        let scope = Scope::new(&[], &[])?;
        assert_eq!(project.discover(&scope)?.len(), 3);
        project.document.mutations[2].replacement = "<=".into();
        assert!(project.discover(&scope).is_err());
        project.document.mutations[2].replacement = ">=".into();
        project.document.mutations[0].end_column += 1;
        assert!(project.discover(&scope).is_err());
        Ok(())
    }

    #[test]
    fn arithmetic_spans_require_exact_pairs_and_operator_identity() -> Result<()> {
        for (original, replacement) in [("+", "-"), ("-", "+"), ("*", "/"), ("/", "*"), ("%", "*")]
        {
            let root = tempfile::tempdir()?;
            fs::create_dir(root.path().join("sample"))?;
            let source = format!("label = 'π'; result = a {original} b\n");
            fs::write(root.path().join("sample/__init__.py"), &source)?;
            let start = source.find(original).unwrap();
            let document = serde_json::from_value(json!({
                "package":"sample", "files":{"sample/__init__.py":format!("sha256:{:x}",Sha256::digest(source.as_bytes()))},
                "sources":["sample/__init__.py"], "tests":["tests/test_sample.py"], "conftests":[],
                "mutations":[{"file":"sample/__init__.py","start":start,"end":start+1,
                    "line":1,"column":start+1,"end_line":1,"end_column":start+2,
                    "original":original,"replacement":replacement}],
            }))?;
            let mut project = Project {
                root: root.path().to_owned(),
                document,
            };
            let mutations = project.discover(&Scope::new(&[], &[])?)?;
            assert_eq!(mutations[0].operator, Operator::ArithmeticReplace);
            project.document.mutations[0].replacement = "%".into();
            assert!(project.discover(&Scope::new(&[], &[])?).is_err());
            mutations[0].apply(root.path())?;
            assert_eq!(
                fs::read_to_string(root.path().join("sample/__init__.py"))?,
                format!("label = 'π'; result = a {replacement} b\n")
            );
        }
        Ok(())
    }

    #[test]
    fn comparison_spans_and_replacements_are_validated() -> Result<()> {
        let root = tempfile::tempdir()?;
        fs::create_dir(root.path().join("sample"))?;
        let source = "label = 'π'; value = left <= right\n";
        fs::write(root.path().join("sample/__init__.py"), source)?;
        let start = source.find("<=").unwrap();
        let document = serde_json::from_value(json!({
            "package":"sample", "files":{"sample/__init__.py":format!("sha256:{:x}",Sha256::digest(source))},
            "sources":["sample/__init__.py"], "tests":["tests/test_sample.py"], "conftests":[],
            "mutations":[{"file":"sample/__init__.py","start":start,"end":start+2,
                "line":1,"column":start+1,"end_line":1,"end_column":start+3,
                "original":"<=","replacement":"<"}],
        }))?;
        let mut project = Project {
            root: root.path().to_owned(),
            document,
        };
        let scope = Scope::new(&[], &[])?;
        let mutations = project.discover(&scope)?;
        assert_eq!(mutations[0].operator, Operator::ComparisonBoundary);
        assert_eq!(mutations[0].replacement, "<");
        project.document.mutations[0].replacement = "==".into();
        assert!(project.discover(&scope).is_err());
        project.document.mutations[0].replacement = "<".into();
        project.document.mutations[0].column += 1;
        assert!(project.discover(&scope).is_err());
        mutations[0].apply(root.path())?;
        assert_eq!(
            fs::read_to_string(root.path().join("sample/__init__.py"))?,
            "label = 'π'; value = left < right\n"
        );
        Ok(())
    }

    #[test]
    fn shared_mutations_preserve_unicode_spans_and_reject_stale_inputs() -> Result<()> {
        let root = tempfile::tempdir()?;
        fs::create_dir(root.path().join("sample"))?;
        let source = "label = 'π'; value = True\n";
        fs::write(root.path().join("sample/__init__.py"), source)?;
        let start = source.find("True").unwrap();
        let document = serde_json::from_value(json!({
            "package":"sample", "files":{"sample/__init__.py":format!("sha256:{:x}",Sha256::digest(source))},
            "sources":["sample/__init__.py"], "tests":["tests/test_sample.py"], "conftests":[],
            "mutations":[{"file":"sample/__init__.py","start":start,"end":start+4,
                "line":1,"column":start+1,"end_line":1,"end_column":start+5,
                "original":"True","replacement":"False"}],
        }))?;
        let project = Project {
            root: root.path().to_owned(),
            document,
        };
        let mut copied = Project {
            root: root.path().to_owned(),
            document: serde_json::from_value(project.value()?)?,
        };
        project.validate_snapshot(&copied)?;
        copied
            .document
            .data_files
            .push("tests/data/input.toml".into());
        assert!(project.validate_snapshot(&copied).is_err());
        copied.document.data_files.clear();
        copied.document.directories.push("tests/data/empty".into());
        assert!(project.validate_snapshot(&copied).is_err());
        copied.document.directories.clear();
        copied.document.source_root = "src".into();
        assert!(project.validate_snapshot(&copied).is_err());
        copied.document.source_root.clear();
        copied.document.tests.push("tests/test_hidden.py".into());
        assert!(project.validate_snapshot(&copied).is_err());
        copied.document.tests = project.document.tests.clone();
        copied.document.conftests.push("tests/conftest.py".into());
        assert!(project.validate_snapshot(&copied).is_err());
        let mut fixture_project = Project {
            root: root.path().to_owned(),
            document: serde_json::from_value(copied.value()?)?,
        };
        fixture_project
            .document
            .files
            .insert("tests/conftest.py".into(), "original".into());
        copied
            .document
            .files
            .insert("tests/conftest.py".into(), "changed".into());
        assert!(fixture_project.validate_snapshot(&copied).is_err());
        copied
            .document
            .files
            .insert("tests/conftest.py".into(), "original".into());
        fixture_project.validate_snapshot(&copied)?;
        copied.document.conftests.clear();
        assert!(fixture_project.validate_snapshot(&copied).is_err());
        copied.document = serde_json::from_value(project.value()?)?;
        for path in ["pytest.ini", "tests/__init__.py"] {
            copied.document.files.insert(path.into(), "support".into());
            assert!(project.validate_snapshot(&copied).is_err());
            assert!(copied.validate_snapshot(&project).is_err());
            copied.document.files.remove(path);
        }
        let all = Scope::new(&[], &[])?;
        let found = project.discover(&all)?;
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].original(), "True");
        assert!(
            project
                .discover(&Scope::new(&[], &["sample/**".into()])?)?
                .is_empty()
        );
        found[0].apply(root.path())?;
        assert_eq!(
            fs::read_to_string(root.path().join("sample/__init__.py"))?,
            "label = 'π'; value = False\n"
        );
        assert!(project.discover(&all).is_err());
        assert!(
            project
                .discover(&Scope::new(&[], &["sample/**".into()])?)
                .is_err()
        );
        Ok(())
    }
}
