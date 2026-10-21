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
        let mut previous: Option<(&str, usize)> = None;
        for site in &self.document.mutations {
            let source = sources
                .get(&site.file)
                .context("Python mutation outside admitted source")?;
            let range = site.start..site.end;
            ensure!(
                source.source.get(range.clone()) == Some(site.original.as_str())
                    && matches!(
                        (site.original.as_str(), site.replacement.as_str()),
                        ("True", "False")
                            | ("False", "True")
                            | ("<", "<=")
                            | ("<=", "<")
                            | (">", ">=")
                            | (">=", ">")
                    ),
                "invalid Python mutation span"
            );
            ensure!(
                previous.is_none_or(|(file, end)| file < site.file.as_str()
                    || (file == site.file && end <= site.start)),
                "unordered/overlapping Python mutations"
            );
            previous = Some((&site.file, site.end));
            let prefix = &source.source.as_bytes()[..site.start];
            let line = prefix.iter().filter(|b| **b == b'\n').count() + 1;
            let column = prefix.len()
                - prefix
                    .iter()
                    .rposition(|b| *b == b'\n')
                    .map_or(0, |p| p + 1)
                + 1;
            ensure!(
                site.line == line
                    && site.end_line == line
                    && site.column == column
                    && site.end_column == column + site.original.len(),
                "Python source coordinates mismatch"
            );
            if scope.matches(&source.path) {
                found.push(Mutation {
                    file: source.clone(),
                    operator: if matches!(site.original.as_str(), "True" | "False") {
                        Operator::BooleanFlip
                    } else {
                        Operator::ComparisonBoundary
                    },
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
        project.document.mutations[0].replacement = ">".into();
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
