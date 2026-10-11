//! Test selection uses Go's actual build graph, including test-only variants.
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, ensure};
use serde::Deserialize;

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct Package {
    pub dir: PathBuf,
    pub go_files: Vec<String>,
    pub cgo_files: Vec<String>,
    import_path: String,
    imports: Vec<String>,
    import_map: BTreeMap<String, String>,
    for_test: String,
    #[serde(rename = "Match")]
    matches: Vec<String>,
    dep_only: bool,
    test_go_files: Vec<String>,
    #[serde(rename = "XTestGoFiles")]
    x_test_go_files: Vec<String>,
    incomplete: bool,
    error: Option<serde_json::Value>,
    deps_errors: Vec<serde_json::Value>,
}

impl Package {
    fn is_target(&self) -> bool {
        !self.dep_only && self.for_test.is_empty() && self.matches.iter().any(|m| m == "./...")
    }

    fn source_import(&self) -> Result<&str> {
        if self.for_test.is_empty() {
            Ok(&self.import_path)
        } else {
            self.import_path
                .strip_suffix(&format!(" [{}.test]", self.for_test))
                .context("unrecognized Go test variant import path")
        }
    }
}

pub struct PackageGraph {
    pub files: Vec<PathBuf>,
    selections: BTreeMap<PathBuf, Vec<String>>,
}

impl PackageGraph {
    pub fn new(root: &Path, packages: &[Package]) -> Result<Self> {
        let mut nodes = BTreeMap::new();
        for package in packages {
            ensure!(
                !package.import_path.is_empty()
                    && !package.incomplete
                    && package.error.is_none()
                    && package.deps_errors.is_empty(),
                "affected test selection requires a complete Go package graph"
            );
            package.source_import()?;
            ensure!(
                nodes
                    .insert(package.import_path.as_str(), package)
                    .is_none(),
                "duplicate package in Go dependency graph: {}",
                package.import_path
            );
        }
        let targets: Vec<_> = packages.iter().filter(|p| p.is_target()).cloned().collect();
        ensure!(
            packages.is_empty() || !targets.is_empty(),
            "Go dependency graph has no packages matched by ./..."
        );
        let files = super::files_in(root, &targets)?;
        let mut reverse: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
        for package in packages {
            for import in &package.imports {
                let dependency = package.import_map.get(import).unwrap_or(import);
                // C is a cgo pseudo-package, not a node in go list output.
                if dependency == "C" {
                    continue;
                }
                ensure!(
                    nodes.contains_key(dependency.as_str()),
                    "Go dependency graph is missing {dependency} imported by {}",
                    package.import_path
                );
                reverse
                    .entry(dependency)
                    .or_default()
                    .push(&package.import_path);
            }
        }
        let mut owners = BTreeMap::new();
        for target in &targets {
            ensure!(
                !target.import_path.starts_with('-')
                    && !target
                        .import_path
                        .chars()
                        .any(|c| c.is_whitespace() || c.is_control())
                    && !target.import_path.contains("..."),
                "invalid Go test package argument: {}",
                target.import_path
            );
            if !target.test_go_files.is_empty() || !target.x_test_go_files.is_empty() {
                ensure!(
                    nodes.contains_key(format!("{}.test", target.import_path).as_str()),
                    "Go dependency graph is missing the test binary for {}",
                    target.import_path
                );
            }
            let relative = target.dir.strip_prefix(root)?;
            ensure!(
                owners
                    .insert(relative.to_owned(), &target.import_path)
                    .is_none(),
                "multiple Go packages own {}",
                relative.display()
            );
        }
        let mut selections = BTreeMap::new();
        for (directory, source) in owners {
            // A source mutation also changes copies rebuilt for other packages'
            // tests. Starting only at the ordinary package would miss them.
            let mut pending: VecDeque<_> = packages
                .iter()
                .filter(|p| p.source_import().is_ok_and(|name| name == source))
                .map(|p| p.import_path.as_str())
                .collect();
            let mut visited = BTreeSet::new();
            while let Some(node) = pending.pop_front() {
                if visited.insert(node)
                    && let Some(importers) = reverse.get(node)
                {
                    pending.extend(importers);
                }
            }
            let selected: BTreeSet<_> = targets
                .iter()
                .filter(|p| {
                    visited.contains(p.import_path.as_str())
                        || visited.contains(format!("{}.test", p.import_path).as_str())
                })
                .map(|p| p.import_path.clone())
                .collect();
            ensure!(
                selected.contains(source),
                "mutated package is missing from test selection"
            );
            selections.insert(directory, selected.into_iter().collect());
        }
        Ok(Self { files, selections })
    }

    pub fn packages_for(&self, file: &Path) -> Result<Vec<String>> {
        self.selections
            .get(file.parent().context("mutation file has no parent")?)
            .cloned()
            .with_context(|| format!("no Go package owns mutation file {}", file.display()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn package(name: &str, imports: &[&str], target: bool) -> Package {
        serde_json::from_value(json!({
            "ImportPath": name, "Dir": format!("/module/{name}"),
            "Imports": imports, "GoFiles": ["code.go"],
            "Match": if target { vec!["./..."] } else { vec![] }, "DepOnly": !target
        }))
        .unwrap()
    }

    #[test]
    fn selects_transitive_dependents_and_internal_external_test_imports() -> Result<()> {
        let mut packages = vec![
            package("leaf", &["C"], true),
            package("mid", &["leaf"], true),
            package("top", &["mid"], true),
            package("internal", &[], true),
            package("external", &[], true),
            package("unrelated", &[], true),
            package("internal [internal.test]", &["mid"], false),
            package("internal.test", &["internal [internal.test]"], false),
            package("external_test [external.test]", &["bridge"], false),
            package(
                "external.test",
                &["external", "external_test [external.test]"],
                false,
            ),
            // A dependency rebuilt for this test imports a rebuilt leaf, not
            // the ordinary leaf node. The graph must start at both copies.
            package("bridge [external.test]", &["leaf [external.test]"], false),
            package("leaf [external.test]", &[], false),
        ];
        packages[3].test_go_files = vec!["inside_test.go".into()];
        packages[4].x_test_go_files = vec!["outside_test.go".into()];
        packages[6].for_test = "internal".into();
        for index in [8, 10, 11] {
            packages[index].for_test = "external".into();
        }
        packages[8]
            .import_map
            .insert("bridge".into(), "bridge [external.test]".into());
        packages[10].dir = "/outside/bridge".into();
        packages[7].go_files = vec!["/cache/generated-test-main".into()];
        let graph = PackageGraph::new(Path::new("/module"), &packages)?;
        assert_eq!(graph.files.len(), 6);
        assert!(graph.files.iter().all(|p| p.ends_with("code.go")));
        assert_eq!(
            graph.packages_for(Path::new("leaf/code.go"))?,
            ["external", "internal", "leaf", "mid", "top"]
        );
        assert_eq!(
            graph.packages_for(Path::new("mid/code.go"))?,
            ["internal", "mid", "top"]
        );
        assert_eq!(
            graph.packages_for(Path::new("unrelated/code.go"))?,
            ["unrelated"]
        );
        assert!(graph.packages_for(Path::new("unknown/code.go")).is_err());
        Ok(())
    }

    #[test]
    fn rejects_incomplete_or_ambiguous_graphs_before_selecting_tests() {
        let root = Path::new("/module");
        let valid = package("p", &[], true);
        for invalid in [
            Package {
                imports: vec!["missing".into()],
                ..valid.clone()
            },
            Package {
                incomplete: true,
                ..valid.clone()
            },
            Package {
                error: Some(json!({"Err":"broken"})),
                ..valid.clone()
            },
            Package {
                deps_errors: vec![json!({"Err":"broken"})],
                ..valid.clone()
            },
            Package {
                test_go_files: vec!["code_test.go".into()],
                ..valid.clone()
            },
            Package {
                x_test_go_files: vec!["code_test.go".into()],
                ..valid.clone()
            },
            Package {
                dir: "/outside/p".into(),
                ..valid.clone()
            },
            Package {
                matches: vec![],
                ..valid.clone()
            },
            Package {
                import_path: "".into(),
                ..valid.clone()
            },
            Package {
                import_path: "-invalid".into(),
                ..valid.clone()
            },
        ] {
            assert!(PackageGraph::new(root, &[invalid]).is_err());
        }
        assert!(PackageGraph::new(root, &[valid.clone(), valid.clone()]).is_err());
        let same_directory = Package {
            import_path: "other".into(),
            ..valid.clone()
        };
        assert!(PackageGraph::new(root, &[valid, same_directory]).is_err());
        assert!(PackageGraph::new(root, &[]).unwrap().files.is_empty());
    }
}
