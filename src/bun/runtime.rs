//! Exact audited runtime pairs; a version string alone is not sufficient.
use anyhow::{Context, Result, bail};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Runtime {
    Bun1311,
    Bun144,
}

impl Runtime {
    pub fn version(self) -> &'static str {
        match self {
            Self::Bun1311 => "1.3.11",
            Self::Bun144 => "1.4.4",
        }
    }

    pub fn revision(self) -> &'static str {
        match self {
            Self::Bun1311 => "af24e281ebacd6ac77c0f14b4206599cf4ae1c9f",
            Self::Bun144 => "38afee6c84fcf48c0edb2466aa4b27d287a59c30",
        }
    }

    pub fn from_parts(version: &str, revision: &str) -> Result<Self> {
        [Self::Bun1311, Self::Bun144]
            .into_iter()
            .find(|runtime| runtime.version() == version && runtime.revision() == revision)
            .map_or_else(
                || bail!("Bun requires audited 1.3.11+af24e281e or 1.4.4+38afee6c8"),
                Ok,
            )
    }

    pub fn from_probe(value: &serde_json::Value) -> Result<Self> {
        Self::from_parts(
            value["version"].as_str().context("missing Bun version")?,
            value["revision"].as_str().context("missing Bun revision")?,
        )
    }

    pub fn body_failure_type(self, name: &str) -> bool {
        // 1.3.11 labels every body exception AssertionError. 1.4.4 preserves
        // arbitrary exception names. Neither label supplies attribution: the
        // reader independently requires completed body/checkpoint evidence.
        match self {
            Self::Bun1311 => name == "AssertionError",
            Self::Bun144 => !name.is_empty(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_and_revisions_must_match_the_exact_audited_pair() {
        for runtime in [Runtime::Bun1311, Runtime::Bun144] {
            assert_eq!(
                Runtime::from_parts(runtime.version(), runtime.revision()).unwrap(),
                runtime
            );
            assert_eq!(
                Runtime::from_probe(
                    &serde_json::json!({"version":runtime.version(), "revision":runtime.revision()})
                )
                .unwrap(),
                runtime
            );
            assert!(Runtime::from_parts(runtime.version(), "unknown").is_err());
            assert!(Runtime::from_parts("unknown", runtime.revision()).is_err());
        }
        assert!(Runtime::from_parts("1.3.11", Runtime::Bun144.revision()).is_err());
        assert!(Runtime::from_parts("1.4.4", Runtime::Bun1311.revision()).is_err());
        assert!(Runtime::from_probe(&serde_json::json!({"version":"1.4.4"})).is_err());
    }
}
