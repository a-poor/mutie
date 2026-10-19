use crate::report::Outcome;
use anyhow::{Context, Result, ensure};
use serde::Deserialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Evidence {
    ended: bool,
    interrupted: bool,
    summaries: Vec<Summary>,
    failures: Vec<Failure>,
}
#[derive(Deserialize)]
struct Summary {
    file: Option<String>,
    success: bool,
    counts: BTreeMap<String, u64>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Failure {
    file: Option<String>,
    name: String,
    #[serde(rename = "type")]
    kind: Option<String>,
    failure: Option<String>,
    code: Option<String>,
    todo: bool,
    message: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Hook {
    file: String,
    started: u64,
    completed: u64,
    failed: u64,
}
fn read<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T> {
    let meta = fs::symlink_metadata(path)
        .with_context(|| format!("missing Node completion evidence: {}", path.display()))?;
    ensure!(
        meta.file_type().is_file() && meta.len() <= 4 * 1024 * 1024,
        "invalid or oversized Node completion evidence"
    );
    serde_json::from_slice(&fs::read(path)?).context("invalid Node completion JSON")
}
pub fn classify(
    path: &Path,
    owned: &Path,
    root: &Path,
    tests: &[String],
    code: Option<i32>,
) -> Result<Outcome> {
    let e: Evidence = read(path)?;
    ensure!(
        e.ended && !e.interrupted,
        "Node test stream did not complete"
    );
    let expected = tests
        .iter()
        .map(|p| root.join(p).to_string_lossy().into_owned())
        .collect::<BTreeSet<_>>();
    let mut seen = BTreeSet::new();
    let mut global = None;
    let mut totals = BTreeMap::<String, u64>::new();
    for summary in &e.summaries {
        let fields = [
            "tests",
            "failed",
            "passed",
            "cancelled",
            "skipped",
            "todo",
            "topLevel",
            "suites",
        ];
        ensure!(
            summary.counts.len() == fields.len()
                && fields.iter().all(|key| summary.counts.contains_key(*key)),
            "invalid Node test counts"
        );
        let c = &summary.counts;
        ensure!(
            c["cancelled"] == 0 && c["todo"] == 0 && c["tests"] > c["skipped"],
            "Node cancelled, todo or empty/all-skipped file is unsupported"
        );
        ensure!(
            summary.success == (c["failed"] == 0),
            "Node summary contains an unclassified failure"
        );
        if let Some(file) = &summary.file {
            ensure!(
                expected.contains(file) && seen.insert(file.clone()),
                "unexpected or duplicate Node file summary"
            );
            for (key, value) in c {
                let total = totals.entry(key.clone()).or_default();
                *total = total
                    .checked_add(*value)
                    .context("Node test count overflow")?;
            }
        } else {
            ensure!(global.is_none(), "duplicate Node global summary");
            global = Some(summary);
        }
    }
    ensure!(
        seen == expected,
        "Node file completion missing (early exit/import/parse failure)"
    );
    let global = global.context("Node global completion missing")?;
    ensure!(
        global.counts == totals,
        "Node global counts disagree with completed files"
    );
    let mut hooks = BTreeSet::new();
    for entry in fs::read_dir(owned)? {
        let entry = entry?;
        if !entry.file_name().to_string_lossy().starts_with("hook-") {
            continue;
        }
        let h: Hook = read(&entry.path())?;
        ensure!(
            expected.contains(&h.file) && hooks.insert(h.file),
            "unexpected or duplicate Node worker evidence"
        );
        ensure!(
            h.started == h.completed && h.failed == 0,
            "Node lifecycle hook failure or incomplete hook"
        );
    }
    ensure!(hooks == expected, "missing Node worker lifecycle evidence");
    let mut assertions = 0;
    for f in &e.failures {
        ensure!(
            !f.todo && f.file.as_ref().is_some_and(|p| expected.contains(p)),
            "unsupported Node failure: {}",
            f.name
        );
        match f.failure.as_deref() {
            Some("testCodeFailure")
                if f.kind.as_deref() == Some("test")
                    && f.code.as_deref() == Some("ERR_ASSERTION") =>
            {
                assertions += 1
            }
            Some("subtestsFailed") => {}
            _ => anyhow::bail!(
                "Node failure is not a validated assertion: {}: {}",
                f.name,
                f.message
            ),
        }
    }
    if global.success {
        ensure!(
            code == Some(0) && e.failures.is_empty(),
            "Node exit/result disagreement"
        );
        Ok(Outcome::Missed)
    } else {
        ensure!(
            code == Some(1) && assertions > 0,
            "Node failed without validated assertion completion"
        );
        Ok(Outcome::Killed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};
    #[test]
    fn rejects_incomplete_conflicting_and_hidden_failure_evidence() {
        let owned = tempfile::tempdir().unwrap();
        let root = owned.path().join("project");
        let file = root.join("test/a.test.js");
        let path = owned.path().join("completion.json");
        let counts = json!({"tests":1,"failed":0,"passed":1,"cancelled":0,"skipped":0,"todo":0,"topLevel":1,"suites":0});
        let good = json!({"ended":true,"interrupted":false,"summaries":[{"file":file,"success":true,"counts":counts},{"success":true,"counts":counts}],"failures":[]});
        let hook = json!({"file":file,"started":1,"completed":1,"failed":0});
        let hpath = owned.path().join("hook-1.json");
        let check = |e: &Value, h: &Value, exit| {
            fs::write(&path, serde_json::to_vec(e).unwrap()).unwrap();
            fs::write(&hpath, serde_json::to_vec(h).unwrap()).unwrap();
            classify(&path, owned.path(), &root, &["test/a.test.js".into()], exit)
        };
        assert_eq!(check(&good, &hook, Some(0)).unwrap(), Outcome::Missed);
        assert!(check(&good, &hook, Some(1)).is_err());
        for pointer in ["/ended", "/interrupted"] {
            let mut bad = good.clone();
            bad[pointer.trim_start_matches('/')] = json!(pointer == "/interrupted");
            assert!(check(&bad, &hook, Some(0)).is_err());
        }
        for field in ["failed", "completed"] {
            let mut bad = hook.clone();
            bad[field] = json!(if field == "failed" { 1 } else { 0 });
            assert!(check(&good, &bad, Some(0)).is_err());
        }
        let mut bad = good.clone();
        bad["summaries"].as_array_mut().unwrap().remove(0);
        assert!(check(&bad, &hook, Some(0)).is_err());
        let mut bad = good.clone();
        bad["summaries"][1]["counts"]["tests"] = json!(2);
        assert!(check(&bad, &hook, Some(0)).is_err());
        fs::remove_file(hpath).unwrap();
        assert!(
            classify(
                &path,
                owned.path(),
                &root,
                &["test/a.test.js".into()],
                Some(0)
            )
            .is_err()
        );
    }
}
