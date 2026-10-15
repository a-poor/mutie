//! Completion evidence is independent of user logs and of process exit status.
use crate::report::Outcome;
use anyhow::{Context, Result, ensure};
use serde::Deserialize;
use std::{
    collections::BTreeSet,
    fs,
    io::Read,
    path::{Component, Path},
};

pub const EVIDENCE_LIMIT: usize = 1024 * 1024;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct Evidence {
    completed: bool,
    scheduled: Vec<String>,
    reason: String,
    hooks_seen: bool,
    active_hooks: i64,
    unhandled_errors: Vec<Error>,
    modules: Vec<Module>,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Module {
    path: String,
    state: String,
    errors: Vec<Error>,
    suite_errors: Vec<Error>,
    tests: Vec<Test>,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Test {
    id: String,
    unsupported_options: bool,
    name: String,
    state: String,
    errors: Vec<Error>,
}
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Error {
    name: String,
    code: String,
    plugin: String,
    transform_file: String,
    message: String,
}

pub fn read(path: &Path) -> Result<Evidence> {
    let metadata = fs::symlink_metadata(path).context("missing Vitest completion evidence")?;
    ensure!(
        metadata.file_type().is_file() && metadata.len() <= EVIDENCE_LIMIT as u64,
        "invalid or oversized Vitest completion evidence"
    );
    let mut bytes = Vec::new();
    fs::File::open(path)?
        .take(EVIDENCE_LIMIT as u64 + 1)
        .read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() <= EVIDENCE_LIMIT,
        "Vitest completion evidence grew beyond its byte limit"
    );
    serde_json::from_slice(&bytes).context("malformed Vitest completion evidence")
}

impl Error {
    fn transform(&self) -> bool {
        // Only the pinned Vite transformer is recognized. Import/runtime and
        // arbitrary plugin errors must not be promoted to compiler failures.
        let path = Path::new(&self.transform_file);
        !self.transform_file.is_empty()
            && path
                .components()
                .all(|c| matches!(c, Component::Normal(s) if s != "node_modules"))
            && matches!(path.extension().and_then(|s| s.to_str()), Some("js" | "ts"))
            && self.plugin == "vite:oxc"
            && self.name == "Error"
            && self.code.is_empty()
            && !self.message.is_empty()
    }
}

impl Evidence {
    pub fn counts(&self) -> (usize, usize, usize) {
        let mut counts = (0, 0, 0);
        for test in self.modules.iter().flat_map(|m| &m.tests) {
            match test.state.as_str() {
                "passed" => counts.0 += 1,
                "failed" => counts.1 += 1,
                "skipped" => counts.2 += 1,
                _ => {}
            }
        }
        counts
    }
    pub fn classify(&self, exit: Option<i32>) -> Result<Outcome> {
        ensure!(
            self.completed && matches!(self.reason.as_str(), "passed" | "failed"),
            "Vitest did not complete normally"
        );
        ensure!(!self.scheduled.is_empty(), "Vitest found no test modules");
        let scheduled: BTreeSet<_> = self.scheduled.iter().collect();
        let actual: BTreeSet<_> = self.modules.iter().map(|m| &m.path).collect();
        ensure!(
            scheduled.len() == self.scheduled.len()
                && actual.len() == self.modules.len()
                && scheduled == actual,
            "incomplete or duplicate Vitest module evidence"
        );
        ensure!(
            self.unhandled_errors.is_empty(),
            "Vitest reported unhandled errors: {:?}",
            self.unhandled_errors
        );
        ensure!(
            self.active_hooks == 0,
            "Vitest reported an unfinished or failed hook"
        );
        let mut ids = BTreeSet::new();
        let mut module_errors = Vec::new();
        for module in &self.modules {
            let path = Path::new(&module.path);
            ensure!(
                path.components().all(|c| matches!(c, Component::Normal(_)))
                    && matches!(path.extension().and_then(|s| s.to_str()), Some("js" | "ts")),
                "unsupported Vitest test module path: {}",
                module.path
            );
            ensure!(
                matches!(module.state.as_str(), "passed" | "failed" | "skipped"),
                "unfinished Vitest module: {}",
                module.path
            );
            ensure!(
                module.suite_errors.is_empty(),
                "Vitest suite/hook errors: {:?}",
                module.suite_errors
            );
            ensure!(
                module.errors.is_empty() || module.state == "failed",
                "inconsistent Vitest module errors"
            );
            ensure!(
                !module.tests.is_empty() || !module.errors.is_empty(),
                "Vitest module has no test or failure evidence"
            );
            ensure!(
                module.state != "skipped" || module.tests.iter().all(|t| t.state == "skipped"),
                "inconsistent skipped Vitest module"
            );
            module_errors.extend(&module.errors);
            for test in &module.tests {
                ensure!(
                    !test.unsupported_options,
                    "Vitest per-test retries/repeats/timeouts are unsupported: {}",
                    test.name
                );
                ensure!(
                    !test.id.is_empty() && !test.name.is_empty() && ids.insert(&test.id),
                    "invalid or duplicate Vitest test identity"
                );
                ensure!(
                    matches!(test.state.as_str(), "passed" | "failed" | "skipped"),
                    "unfinished Vitest test: {}",
                    test.name
                );
                ensure!(
                    (test.state == "failed") != test.errors.is_empty(),
                    "inconsistent Vitest test errors: {}",
                    test.name
                );
                ensure!(
                    test.state != "failed" || module.state == "failed",
                    "failed Vitest test in successful module"
                );
            }
            ensure!(
                module.state != "failed"
                    || !module.errors.is_empty()
                    || module.tests.iter().any(|t| t.state == "failed"),
                "Vitest module failed without attributed evidence"
            );
        }
        let (passed, failed, _) = self.counts();
        if !module_errors.is_empty() {
            ensure!(
                failed == 0 && !self.hooks_seen && module_errors.iter().all(|e| e.transform()),
                "Vitest collection/import errors or mixed failures: {:?}",
                module_errors
            );
            ensure!(
                exit == Some(1) && self.reason == "failed",
                "inconsistent Vitest transform failure status"
            );
            return Ok(Outcome::BuildFailed);
        }
        ensure!(
            passed + failed > 0,
            "Vitest executed no tests (empty or all skipped)"
        );
        if failed > 0 {
            // The public reporter API does not reliably attribute returned hook
            // cleanup/fixture errors. Fail conservatively until that scope is proven.
            ensure!(
                !self.hooks_seen,
                "Vitest failures with lifecycle hooks require review; not counted as kills"
            );
            ensure!(
                exit == Some(1) && self.reason == "failed",
                "inconsistent Vitest test failure status"
            );
            Ok(Outcome::Killed)
        } else {
            ensure!(
                exit == Some(0) && self.reason == "passed",
                "Vitest exited unsuccessfully despite passing tests"
            );
            Ok(Outcome::Missed)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};
    fn passed() -> Value {
        json!({"completed":true,"scheduled":["test/a.test.ts"],"reason":"passed","hooksSeen":false,"activeHooks":0,"unhandledErrors":[],"modules":[{"path":"test/a.test.ts","state":"passed","errors":[],"suiteErrors":[],"tests":[{"id":"one","unsupportedOptions":false,"name":"passes","state":"passed","errors":[]}]}]})
    }
    fn error(plugin: &str) -> Value {
        json!({"name":"Error","code":"","plugin":plugin,"transformFile":"src/a.ts","message":"probe"})
    }
    fn classify(value: Value, exit: Option<i32>) -> Result<Outcome> {
        serde_json::from_value::<Evidence>(value)?.classify(exit)
    }
    #[test]
    fn only_complete_consistent_nonempty_evidence_can_pass() {
        assert_eq!(classify(passed(), Some(0)).unwrap(), Outcome::Missed);
        for exit in [Some(1), Some(2), None] {
            assert!(classify(passed(), exit).is_err());
        }
        for (key, value) in [
            ("completed", json!(false)),
            ("scheduled", json!([])),
            ("reason", json!("interrupted")),
            ("unhandledErrors", json!([error("")])),
            ("activeHooks", json!(1)),
        ] {
            let mut evidence = passed();
            evidence[key] = value;
            assert!(classify(evidence, Some(0)).is_err(), "{key}");
        }
        for (key, value) in [
            ("state", json!("pending")),
            ("path", json!("../a.test.ts")),
            ("tests", json!([])),
        ] {
            let mut evidence = passed();
            evidence["modules"][0][key] = value;
            assert!(classify(evidence, Some(0)).is_err());
        }
        let mut evidence = passed();
        evidence["modules"][0]["tests"][0]["state"] = json!("skipped");
        assert!(classify(evidence, Some(0)).is_err());
        let mut duplicate_modules = passed();
        let duplicate = duplicate_modules["modules"][0].clone();
        duplicate_modules["modules"]
            .as_array_mut()
            .unwrap()
            .push(duplicate);
        assert!(classify(duplicate_modules, Some(0)).is_err());
        let mut evidence = passed();
        let duplicate = evidence["modules"][0]["tests"][0].clone();
        evidence["modules"][0]["tests"]
            .as_array_mut()
            .unwrap()
            .push(duplicate);
        assert!(classify(evidence, Some(0)).is_err());
    }
    #[test]
    fn test_transform_hook_and_mixed_failures_have_distinct_policies() {
        let mut failed = passed();
        failed["reason"] = json!("failed");
        failed["modules"][0]["state"] = json!("failed");
        failed["modules"][0]["tests"][0]["state"] = json!("failed");
        failed["modules"][0]["tests"][0]["errors"] = json!([error("")]);
        assert_eq!(classify(failed.clone(), Some(1)).unwrap(), Outcome::Killed);
        assert!(classify(failed.clone(), Some(0)).is_err());
        let mut hooked = failed.clone();
        hooked["hooksSeen"] = json!(true);
        assert!(classify(hooked, Some(1)).is_err());
        let mut mixed = failed.clone();
        mixed["modules"][0]["errors"] = json!([error("vite:oxc")]);
        assert!(classify(mixed, Some(1)).is_err());
        failed["modules"][0]["tests"] = json!([]);
        failed["modules"][0]["errors"] = json!([error("vite:oxc")]);
        assert_eq!(
            classify(failed.clone(), Some(1)).unwrap(),
            Outcome::BuildFailed
        );
        failed["modules"][0]["errors"][0]["plugin"] = json!("");
        assert!(classify(failed, Some(1)).is_err());
    }
    #[test]
    fn reader_rejects_missing_corrupt_oversized_and_ambiguous_evidence() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("evidence");
        assert!(read(&path).is_err());
        for text in ["{", "{}", "{\"completed\":true,\"completed\":false}"] {
            fs::write(&path, text).unwrap();
            assert!(read(&path).is_err());
        }
        fs::write(&path, vec![b' '; EVIDENCE_LIMIT + 1]).unwrap();
        assert!(read(&path).is_err());
        fs::write(&path, serde_json::to_vec(&passed()).unwrap()).unwrap();
        assert!(read(&path).is_ok());
        #[cfg(unix)]
        {
            let link = dir.path().join("link");
            std::os::unix::fs::symlink(&path, &link).unwrap();
            assert!(read(&link).is_err());
        }
    }
}
