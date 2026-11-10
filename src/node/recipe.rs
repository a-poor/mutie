//! A complete, statically recognized npm-run-all lint/test recipe.
//!
//! Commands run directly with the pinned Node executable. Package scripts and
//! shell fragments are never evaluated by Mutie.
use crate::cancellation::Cancellation;
use anyhow::{Context, Result, ensure};
use serde_json::Value;
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

pub const TEST_SCRIPT: &str = "npm-run-all --node-run --continue-on-error lint \"test:*\"";
pub(super) const SCRIPTS: &[(&str, &str)] = &[
    ("test", TEST_SCRIPT),
    ("test:js", "node --test test/*.js test/unit/*.js"),
    ("lint", "run-p --continue-on-error \"lint:*\""),
    (
        "lint:dogfooding",
        "eslint --config=./eslint.dogfooding.config.js",
    ),
    ("lint:eslint-docs", "npm run fix:eslint-docs -- --check"),
    ("lint:js", "eslint"),
    ("lint:markdown", "markdownlint \"**/*.md\""),
    ("lint:package-json", "npmPkgJsonLint ."),
    ("fix:eslint-docs", "eslint-doc-generator"),
];
const CONFIGS: &[&str] = &[
    "eslint.config.js",
    "eslint.dogfooding.config.js",
    ".eslint-doc-generatorrc.js",
    ".markdownlint.json",
    ".markdownlintignore",
    ".npmpackagejsonlintrc.json",
];
pub(super) const TOOLS: &[(&str, &str, &str)] = &[
    ("eslint", "10.14.0", "bin/eslint.js"),
    (
        "eslint-doc-generator",
        "3.7.1",
        "dist/bin/eslint-doc-generator.js",
    ),
    ("markdownlint-cli", "0.49.1", "markdownlint.js"),
    ("npm-package-json-lint", "10.5.1", "dist/cli.js"),
    ("npm-run-all2", "9.0.4", "bin/npm-run-all/index.js"),
];

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Scope {
    pub inputs: BTreeMap<String, String>,
}

pub fn recognizes(manifest: &Value) -> bool {
    manifest["scripts"]["test"] == TEST_SCRIPT
}

pub fn validate_scripts(manifest: &Value) -> Result<()> {
    let scripts = manifest["scripts"]
        .as_object()
        .context("Node scripts must be an object")?;
    for (name, command) in SCRIPTS {
        ensure!(
            scripts.get(*name).and_then(Value::as_str) == Some(command),
            "unsupported composed Node script {name}; expected {command}"
        );
        for prefix in ["pre", "post"] {
            ensure!(
                !scripts.contains_key(&format!("{prefix}{name}")),
                "npm lifecycle scripts need separate support: {prefix}{name}"
            );
        }
    }
    for name in scripts.keys() {
        ensure!(
            !(name.starts_with("test:") || name.starts_with("lint:"))
                || SCRIPTS.iter().any(|(known, _)| name == known),
            "composed Node recipe contains an unaudited task: {name}"
        );
    }
    Ok(())
}

impl Scope {
    pub fn load(root: &Path, manifest: &Value, cancellation: &Cancellation) -> Result<Self> {
        validate_scripts(manifest)?;
        for (name, version, binary) in TOOLS {
            tool(root, name, version, binary)?;
        }
        let mut inputs = BTreeMap::new();
        for name in CONFIGS {
            record(root, Path::new(name), &mut inputs)?;
        }
        // These modules can be imported by tests and ESLint configuration. They
        // are inputs to execution, not mutation targets. Bind all their bytes,
        // including templates and internal lint rules.
        visit_inputs(root, Path::new("scripts"), &mut inputs, cancellation)?;
        // Typed lint cases use real fixture configuration; they are not native
        // TypeScript test modules and must retain their original compiler setup.
        if root.join("test/fixtures/tsconfig.json").exists() {
            record(root, Path::new("test/fixtures/tsconfig.json"), &mut inputs)?;
        }
        let scope = Self { inputs };
        scope.validate_record()?;
        Ok(scope)
    }

    pub fn validate_record(&self) -> Result<()> {
        ensure!(
            CONFIGS.iter().all(|name| self.inputs.contains_key(*name)),
            "incomplete composed Node configuration identity"
        );
        for (path, hash) in &self.inputs {
            ensure!(
                super::valid_fingerprint(hash)
                    && Path::new(path)
                        .components()
                        .all(|c| matches!(c, std::path::Component::Normal(_)))
                    && (CONFIGS.contains(&path.as_str())
                        || path.starts_with("scripts/")
                        || path == "test/fixtures/tsconfig.json"),
                "invalid composed Node configuration identity"
            );
        }
        Ok(())
    }
}

fn record(root: &Path, relative: &Path, inputs: &mut BTreeMap<String, String>) -> Result<()> {
    let path = root.join(relative);
    let meta = fs::symlink_metadata(&path)?;
    ensure!(
        meta.is_file() && meta.len() <= 8 * 1024 * 1024,
        "invalid Node recipe input: {}",
        relative.display()
    );
    inputs.insert(
        relative
            .to_str()
            .context("Node input path must be UTF-8")?
            .into(),
        super::fingerprint(&fs::read(path)?),
    );
    Ok(())
}

fn visit_inputs(
    root: &Path,
    relative: &Path,
    inputs: &mut BTreeMap<String, String>,
    cancellation: &Cancellation,
) -> Result<()> {
    cancellation.check()?;
    ensure!(
        relative.components().count() < 100 && inputs.len() < 10000,
        "Node recipe inputs exceed limits"
    );
    let path = root.join(relative);
    let meta = fs::symlink_metadata(&path)?;
    if meta.is_dir() {
        ensure!(
            !path.join("package.json").exists(),
            "nested tooling packages require separate support"
        );
        for entry in fs::read_dir(path)? {
            visit_inputs(
                root,
                &relative.join(entry?.file_name()),
                inputs,
                cancellation,
            )?;
        }
    } else {
        record(root, relative, inputs)?;
    }
    Ok(())
}

fn tool(root: &Path, name: &str, version: &str, binary: &str) -> Result<PathBuf> {
    let modules = root.join("node_modules").canonicalize()?;
    let package = modules
        .join(name)
        .canonicalize()
        .with_context(|| format!("missing Node recipe tool {name}"))?;
    ensure!(
        package.starts_with(&modules),
        "Node recipe tool escapes owned dependencies"
    );
    let manifest: Value = serde_json::from_slice(&fs::read(package.join("package.json"))?)?;
    ensure!(
        manifest["name"] == name && manifest["version"] == version,
        "composed Node recipe requires {name} {version}"
    );
    let executable = package.join(binary).canonicalize()?;
    ensure!(
        executable.starts_with(&modules) && executable.is_file(),
        "Node recipe binary escapes owned dependencies"
    );
    Ok(executable)
}

/// Sequential prerequisites preserve the complete task inventory. Lint failures
/// are execution errors, even if a later runtime test also fails.
pub fn commands(root: &Path) -> Result<Vec<Vec<String>>> {
    let binary = |i: usize| -> Result<String> {
        let (name, version, path) = TOOLS[i];
        Ok(tool(root, name, version, path)?
            .to_string_lossy()
            .into_owned())
    };
    Ok(vec![
        vec![binary(0)?, "--config=./eslint.dogfooding.config.js".into()],
        vec![binary(1)?, "--check".into()],
        vec![binary(0)?],
        vec![binary(2)?, "**/*.md".into()],
        vec![binary(3)?, ".".into()],
    ])
}

/// The native orchestrator continues after ordinary prerequisite failures. Keep
/// that inventory, while sharing one deadline/output budget across every phase.
#[allow(clippy::too_many_arguments)]
pub async fn run_commands(
    program: &str,
    commands: &[Vec<String>],
    root: &Path,
    limit: std::time::Duration,
    capture_limit: Option<std::num::NonZeroUsize>,
    cancellation: &Cancellation,
    environment: &[(&str, &std::ffi::OsStr)],
) -> Result<(crate::process::CommandOutput, Option<String>)> {
    let start = std::time::Instant::now();
    let mut stdout = tempfile::NamedTempFile::new()?;
    let mut stderr = tempfile::NamedTempFile::new()?;
    let mut bytes = 0u64;
    let mut elapsed = std::time::Duration::ZERO;
    let mut prerequisite_error = None;
    for (index, args) in commands.iter().enumerate() {
        let remaining = limit.saturating_sub(start.elapsed());
        let budget = capture_limit.map(|limit| {
            std::num::NonZeroUsize::new(limit.get().saturating_sub(bytes as usize))
                .unwrap_or(std::num::NonZeroUsize::MIN)
        });
        let mut raw = Box::pin(crate::process::run_with_env(
            program,
            &args.iter().map(String::as_str).collect::<Vec<_>>(),
            root,
            remaining,
            budget,
            cancellation,
            environment,
        ))
        .await?;
        elapsed += raw.elapsed;
        bytes += std::io::copy(&mut raw.stdout.reader()?, &mut stdout)?;
        bytes += std::io::copy(&mut raw.stderr.reader()?, &mut stderr)?;
        if capture_limit.is_some_and(|limit| bytes > limit.get() as u64) {
            raw.capture_error = Some("composed Node recipe exceeded its output budget".into());
        }
        let final_command = index + 1 == commands.len();
        let incomplete =
            raw.timed_out || raw.interrupted || raw.capture_error.is_some() || raw.group_remaining;
        if !final_command && (!raw.status.success() || incomplete) {
            prerequisite_error.get_or_insert_with(|| format!("Node lint prerequisite {} failed or did not complete; not counted as a mutation kill", index+1));
        }
        if incomplete || final_command {
            raw.elapsed = elapsed;
            raw.stdout = crate::capture::Capture::new(stdout)?;
            raw.stderr = crate::capture::Capture::new(stderr)?;
            return Ok((raw, prerequisite_error));
        }
    }
    anyhow::bail!("empty composed Node recipe")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn refuses_unaccounted_tasks_and_lifecycle_scripts() {
        let mut scripts = serde_json::Map::new();
        for (key, value) in SCRIPTS {
            scripts.insert((*key).into(), (*value).into());
        }
        let original = serde_json::json!({"scripts": scripts});
        validate_scripts(&original).unwrap();
        for (key, value) in [
            ("test:extra", "node --test extra.js"),
            ("lint:extra", "echo skipped"),
            ("pretest", "echo hidden"),
            ("postlint:js", "echo hidden"),
            ("test:js", "node --test test/*.js"),
        ] {
            let mut altered = original.clone();
            altered["scripts"][key] = value.into();
            assert!(validate_scripts(&altered).is_err(), "{key}");
        }
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn continues_the_complete_task_inventory_but_retains_lint_failure() -> Result<()> {
        let root = tempfile::tempdir()?;
        let commands = [
            "printf lint; exit 1",
            "printf second",
            "printf runtime; exit 1",
        ]
        .map(|script| vec!["-c".into(), script.into()]);
        let (raw, error) = run_commands(
            "/bin/sh",
            &commands,
            root.path(),
            std::time::Duration::from_secs(5),
            None,
            &Cancellation::default(),
            &[],
        )
        .await?;
        assert_eq!(raw.stdout.text()?, "lintsecondruntime");
        assert_eq!(raw.status.code(), Some(1));
        assert!(
            error
                .as_deref()
                .is_some_and(|s| s.contains("prerequisite 1"))
        );
        Ok(())
    }
}
