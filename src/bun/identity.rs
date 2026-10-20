//! Identity for diagnostic evidence; scored reports/reruns are not connected yet.
use super::{EVIDENCE_LIMIT, fingerprint, project::Project};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs,
    num::NonZeroUsize,
    path::{Path, PathBuf},
    time::Duration,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ExecutionIdentity {
    runtime_version: String,
    runtime_revision: String,
    pub(super) executable: PathBuf,
    executable_fingerprint: String,
    manifest_fingerprint: String,
    configuration_fingerprints: BTreeMap<String, String>,
    pub(super) test_files: Vec<String>,
    recipe_fingerprint: String,
    timeout_millis: u128,
    capture_bytes: usize,
    evidence_file_bytes: u64,
}

pub(super) fn recipe_fingerprint() -> String {
    // Include the actual command/environment, classifier, observer, validation,
    // capture/cleanup and identity implementation. Temporary paths are excluded.
    fingerprint(
        &[
            include_bytes!("../bun.rs").as_slice(),
            include_bytes!("identity.rs").as_slice(),
            include_bytes!("observer.mjs").as_slice(),
            include_bytes!("protocol.rs").as_slice(),
            include_bytes!("project.rs").as_slice(),
            include_bytes!("config.rs").as_slice(),
            include_bytes!("../process.rs").as_slice(),
            include_bytes!("../workspace.rs").as_slice(),
            include_bytes!("../javascript.rs").as_slice(),
            include_bytes!("../../Cargo.lock").as_slice(),
            include_bytes!("../capture.rs").as_slice(),
            include_bytes!("../command.rs").as_slice(),
        ]
        .concat(),
    )
}

impl ExecutionIdentity {
    pub(crate) fn validate_record(&self) -> Result<()> {
        anyhow::ensure!(
            self.runtime_version == "1.3.11"
                && self.runtime_revision == "af24e281ebacd6ac77c0f14b4206599cf4ae1c9f"
                && self.executable.is_absolute(),
            "unsupported saved Bun runtime"
        );
        let valid = |s: &str| {
            s.strip_prefix("sha256:")
                .is_some_and(|v| v.len() == 64 && v.bytes().all(|b| b.is_ascii_hexdigit()))
        };
        anyhow::ensure!(
            valid(&self.executable_fingerprint)
                && valid(&self.manifest_fingerprint)
                && valid(&self.recipe_fingerprint)
                && self
                    .configuration_fingerprints
                    .iter()
                    .all(
                        |(k, v)| matches!(k.as_str(), "tsconfig.json" | "bun.lock" | "bun.lockb")
                            && valid(v)
                    ),
            "invalid saved Bun fingerprints"
        );
        let unique: std::collections::BTreeSet<_> = self.test_files.iter().collect();
        anyhow::ensure!(
            !unique.is_empty()
                && unique.len() == self.test_files.len()
                && self.test_files.iter().all(|p| {
                    let path = Path::new(p);
                    (path.starts_with("test") || path.starts_with("tests"))
                        && path
                            .components()
                            .all(|c| matches!(c, std::path::Component::Normal(_)))
                        && (p.ends_with(".test.js") || p.ends_with(".test.ts"))
                }),
            "invalid saved Bun test inventory"
        );
        anyhow::ensure!(
            self.timeout_millis > 0
                && self.capture_bytes > 0
                && self.evidence_file_bytes == EVIDENCE_LIMIT,
            "invalid saved Bun execution limits"
        );
        Ok(())
    }

    pub(crate) fn validate_rerun(&self, current: &Self) -> Result<()> {
        self.validate_record()?;
        current.validate_record()?;
        // Tests may improve or be added and command budgets may change. Source
        // compatibility is checked independently by the shared rerun selector.
        anyhow::ensure!(
            self.runtime_version == current.runtime_version
                && self.runtime_revision == current.runtime_revision
                && self.executable_fingerprint == current.executable_fingerprint
                && self.manifest_fingerprint == current.manifest_fingerprint
                && self.configuration_fingerprints == current.configuration_fingerprints
                && self.recipe_fingerprint == current.recipe_fingerprint,
            "Bun execution identity changed; start a fresh run"
        );
        Ok(())
    }

    pub(super) fn capture(
        project: &Project,
        executable: &Path,
        timeout: Duration,
        capture_limit: NonZeroUsize,
    ) -> Result<Self> {
        Ok(Self {
            runtime_version: "1.3.11".into(),
            runtime_revision: "af24e281ebacd6ac77c0f14b4206599cf4ae1c9f".into(),
            executable: executable.to_owned(),
            executable_fingerprint: fingerprint(&fs::read(executable)?),
            manifest_fingerprint: project
                .files
                .get("package.json")
                .context("missing Bun manifest identity")?
                .clone(),
            configuration_fingerprints: project
                .files
                .iter()
                .filter(|(path, _)| {
                    matches!(path.as_str(), "tsconfig.json" | "bun.lock" | "bun.lockb")
                })
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
            test_files: project.tests.clone(),
            recipe_fingerprint: recipe_fingerprint(),
            timeout_millis: timeout.as_millis(),
            capture_bytes: capture_limit.get(),
            evidence_file_bytes: EVIDENCE_LIMIT,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn identity_tracks_runtime_recipe_config_inventory_and_limits() -> Result<()> {
        let root = tempfile::tempdir()?;
        let executable = root.path().join("bun");
        fs::write(&executable, b"runtime A")?;
        let mut project = Project {
            root: root.path().to_owned(),
            files: BTreeMap::from([("package.json".into(), fingerprint(b"manifest"))]),
            tests: vec!["test/a.test.ts".into()],
        };
        let limit = NonZeroUsize::new(4096).unwrap();
        let capture = |project: &Project| {
            ExecutionIdentity::capture(project, &executable, Duration::from_secs(5), limit)
        };
        let original = capture(&project)?;
        let encoded = serde_json::to_value(&original)?;
        assert_eq!(
            serde_json::from_value::<ExecutionIdentity>(encoded.clone())?,
            original
        );
        let mut extra = encoded;
        extra["unknown"] = true.into();
        assert!(serde_json::from_value::<ExecutionIdentity>(extra).is_err());
        assert_eq!(original.recipe_fingerprint, recipe_fingerprint());
        project
            .files
            .insert("tsconfig.json".into(), fingerprint(b"{}"));
        assert_ne!(capture(&project)?, original);
        project.files.remove("tsconfig.json");
        project.tests.push("test/b.test.ts".into());
        assert_ne!(capture(&project)?, original);
        project.tests.pop();
        assert_ne!(
            ExecutionIdentity::capture(&project, &executable, Duration::from_secs(6), limit)?,
            original
        );
        assert_ne!(
            ExecutionIdentity::capture(
                &project,
                &executable,
                Duration::from_secs(5),
                NonZeroUsize::new(8192).unwrap()
            )?,
            original
        );
        fs::write(&executable, b"runtime B")?;
        assert_ne!(capture(&project)?, original);
        Ok(())
    }
}
