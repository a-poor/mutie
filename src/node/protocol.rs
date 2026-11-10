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
    #[serde(default)]
    owner: Option<String>,
    id: Option<u64>,
    name: String,
    #[serde(rename = "type")]
    kind: Option<String>,
    failure: Option<String>,
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
    bodies: Vec<u64>,
    process_errors: u64,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Modules {
    file: String,
    started: u64,
    completed: u64,
    errors: u64,
    overflow: bool,
    samples: Vec<String>,
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
    classify_with_helpers(path, owned, root, tests, code, false)
}

pub fn classify_package(
    path: &Path,
    owned: &Path,
    root: &Path,
    tests: &[String],
    code: Option<i32>,
) -> Result<Outcome> {
    classify_with_helpers(path, owned, root, tests, code, true)
}

fn classify_with_helpers(
    path: &Path,
    owned: &Path,
    root: &Path,
    tests: &[String],
    code: Option<i32>,
    helpers: bool,
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
    let mut bodies = BTreeSet::new();
    for entry in fs::read_dir(owned)? {
        let entry = entry?;
        if !entry.file_name().to_string_lossy().starts_with("hook-") {
            continue;
        }
        let h: Hook = read(&entry.path())?;
        ensure!(
            expected.contains(&h.file) && hooks.insert(h.file.clone()),
            "unexpected or duplicate Node worker evidence"
        );
        ensure!(
            h.process_errors == 0,
            "Node unhandled exception, rejection or process exit failure"
        );
        for id in h.bodies {
            ensure!(
                id > 0 && bodies.insert((h.file.clone(), id)),
                "duplicate or invalid Node body evidence"
            );
        }
        ensure!(
            h.started == h.completed && h.failed == 0,
            "Node lifecycle hook failure or incomplete hook"
        );
    }
    ensure!(hooks == expected, "missing Node worker lifecycle evidence");
    if helpers {
        let mut modules = BTreeSet::new();
        for entry in fs::read_dir(owned)? {
            let entry = entry?;
            if !entry.file_name().to_string_lossy().starts_with("modules-") {
                continue;
            }
            let evidence: Modules = read(&entry.path())?;
            ensure!(
                expected.contains(&evidence.file) && modules.insert(evidence.file),
                "unexpected or duplicate Node loader evidence"
            );
            ensure!(
                !evidence.overflow
                    && evidence.started > 0
                    && evidence.started == evidence.completed
                    && evidence.errors == 0
                    && evidence.samples.is_empty(),
                "Node module loading failed or did not complete: {}",
                evidence.samples.join("; ")
            );
        }
        ensure!(
            modules == expected,
            "missing Node worker module-loading evidence"
        );
    }
    let mut body_failures = 0;
    for f in &e.failures {
        let owner = if helpers {
            let owner = f
                .owner
                .as_ref()
                .context("missing Node worker owner for helper failure")?;
            let declaration = f
                .file
                .as_ref()
                .context("missing Node failure declaration location")?;
            let declaration = Path::new(declaration)
                .canonicalize()
                .context("invalid Node failure declaration location")?;
            let relative = declaration
                .strip_prefix(root)
                .context("Node failure declaration escapes owned project")?;
            ensure!(
                (relative.starts_with("test") || relative.starts_with("scripts"))
                    && relative.extension().is_some_and(|e| e == "js"),
                "Node failure declaration is outside audited test helpers"
            );
            owner
        } else {
            ensure!(
                f.owner.is_none(),
                "worker ownership requires the composed Node recipe"
            );
            f.file.as_ref().context("missing Node failure file")?
        };
        ensure!(
            !f.todo && expected.contains(owner),
            "unsupported Node failure: {}",
            f.name
        );
        match f.failure.as_deref() {
            Some("testCodeFailure")
                if f.kind.as_deref() == Some("test")
                    && f.id.is_some_and(|id| bodies.remove(&(owner.clone(), id))) =>
            {
                body_failures += 1
            }
            Some("subtestsFailed") => {}
            _ => anyhow::bail!(
                "Node failure is not a validated body failure: {}: {}",
                f.name,
                f.message
            ),
        }
    }
    ensure!(bodies.is_empty(), "unmatched Node body failure evidence");
    if global.success {
        ensure!(
            code == Some(0) && e.failures.is_empty(),
            "Node exit/result disagreement"
        );
        Ok(Outcome::Missed)
    } else {
        ensure!(
            code == Some(1) && body_failures > 0,
            "Node failed without validated body completion"
        );
        Ok(Outcome::Killed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};
    #[test]
    fn helper_failures_require_the_exact_completed_worker_and_body_id() -> Result<()> {
        let owned = tempfile::tempdir()?;
        let root = owned.path().canonicalize()?.join("project");
        fs::create_dir_all(root.join("test/helpers"))?;
        let helper = root.join("test/helpers/shared.js");
        fs::write(&helper, "")?;
        let first = root.join("test/a.js");
        let second = root.join("test/b.js");
        let counts = |failed| json!({"tests":1,"failed":failed,"passed":1-failed,"cancelled":0,"skipped":0,"todo":0,"topLevel":1,"suites":0});
        let evidence = json!({"ended":true,"interrupted":false,"summaries":[
            {"file":first,"success":false,"counts":counts(1)},
            {"file":second,"success":true,"counts":counts(0)},
            {"success":false,"counts":{"tests":2,"failed":1,"passed":1,"cancelled":0,"skipped":0,"todo":0,"topLevel":2,"suites":0}}
        ],"failures":[{"file":helper,"owner":first,"id":1,"name":"same name","type":"test","failure":"testCodeFailure","todo":false,"message":"assertion"}]});
        for (name, file, bodies) in [("a", &first, vec![1]), ("b", &second, vec![])] {
            fs::write(
                owned.path().join(format!("hook-{name}.json")),
                serde_json::to_vec(
                    &json!({"file":file,"started":0,"completed":0,"failed":0,"bodies":bodies,"process_errors":0}),
                )?,
            )?;
            fs::write(
                owned.path().join(format!("modules-{name}.json")),
                serde_json::to_vec(
                    &json!({"file":file,"started":1,"completed":1,"errors":0,"overflow":false,"samples":[]}),
                )?,
            )?;
        }
        let path = owned.path().join("completion.json");
        let tests = vec!["test/a.js".into(), "test/b.js".into()];
        fs::write(&path, serde_json::to_vec(&evidence)?)?;
        assert_eq!(
            classify_package(&path, owned.path(), &root, &tests, Some(1))?,
            Outcome::Killed
        );
        assert!(classify(&path, owned.path(), &root, &tests, Some(1)).is_err());
        for (field, value) in [
            ("owner", json!(second)),
            ("owner", Value::Null),
            ("id", json!(2)),
            ("file", json!(root.join("node_modules/helper.js"))),
        ] {
            let mut wrong = evidence.clone();
            wrong["failures"][0][field] = value;
            fs::write(&path, serde_json::to_vec(&wrong)?)?;
            assert!(
                classify_package(&path, owned.path(), &root, &tests, Some(1)).is_err(),
                "{field}"
            );
        }
        fs::write(&path, serde_json::to_vec(&evidence)?)?;
        let module_path = owned.path().join("modules-a.json");
        let good_modules: Value = serde_json::from_slice(&fs::read(&module_path)?)?;
        for (field, value) in [
            ("started", json!(0)),
            ("completed", json!(2)),
            ("errors", json!(1)),
            ("overflow", json!(true)),
            ("samples", json!(["caught error"])),
        ] {
            let mut changed = good_modules.clone();
            changed[field] = value;
            fs::write(&module_path, serde_json::to_vec(&changed)?)?;
            assert!(
                classify_package(&path, owned.path(), &root, &tests, Some(1)).is_err(),
                "loader {field}"
            );
        }
        fs::remove_file(&module_path)?;
        assert!(classify_package(&path, owned.path(), &root, &tests, Some(1)).is_err());
        fs::write(&module_path, serde_json::to_vec(&good_modules)?)?;
        fs::write(
            owned.path().join("modules-duplicate.json"),
            serde_json::to_vec(&good_modules)?,
        )?;
        assert!(classify_package(&path, owned.path(), &root, &tests, Some(1)).is_err());
        Ok(())
    }
    #[test]
    fn rejects_incomplete_conflicting_and_hidden_failure_evidence() {
        let owned = tempfile::tempdir().unwrap();
        let root = owned.path().join("project");
        let file = root.join("test/a.test.js");
        let path = owned.path().join("completion.json");
        let counts = json!({"tests":1,"failed":0,"passed":1,"cancelled":0,"skipped":0,"todo":0,"topLevel":1,"suites":0});
        let good = json!({"ended":true,"interrupted":false,"summaries":[{"file":file,"success":true,"counts":counts},{"success":true,"counts":counts}],"failures":[]});
        let hook = json!({"file":file,"started":1,"completed":1,"failed":0,"bodies":[],"process_errors":0});
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

#[cfg(test)]
mod body_tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn requires_exact_file_and_test_body_evidence_for_each_failure() {
        let owned = tempfile::tempdir().unwrap();
        let root = owned.path().join("project");
        let file = root.join("test/a.test.js");
        let path = owned.path().join("completion.json");
        let counts = json!({"tests":1,"failed":1,"passed":0,"cancelled":0,"skipped":0,"todo":0,"topLevel":1,"suites":0});
        let good = json!({"ended":true,"interrupted":false,"summaries":[{"file":file,"success":false,"counts":counts},{"success":false,"counts":counts}],"failures":[{"file":file,"id":1,"name":"body","type":"test","failure":"testCodeFailure","todo":false,"message":"same message"}]});
        let worker = json!({"file":file,"started":1,"completed":1,"failed":0,"bodies":[1],"process_errors":0});
        let check = |e: &serde_json::Value, h: &serde_json::Value| {
            fs::write(&path, serde_json::to_vec(e).unwrap()).unwrap();
            fs::write(
                owned.path().join("hook-1.json"),
                serde_json::to_vec(h).unwrap(),
            )
            .unwrap();
            classify(
                &path,
                owned.path(),
                &root,
                &["test/a.test.js".into()],
                Some(1),
            )
        };
        assert_eq!(check(&good, &worker).unwrap(), Outcome::Killed);
        for ids in [
            json!([]),
            json!([2]),
            json!([1, 1]),
            json!([1, 2]),
            json!([0]),
        ] {
            let mut h = worker.clone();
            h["bodies"] = ids;
            assert!(check(&good, &h).is_err());
        }
        let mut e = good.clone();
        e["failures"][0]["id"] = serde_json::Value::Null;
        assert!(check(&e, &worker).is_err());
        let mut e = good.clone();
        e["failures"][0]["file"] = json!(root.join("test/b.test.js"));
        assert!(check(&e, &worker).is_err());
        let mut e = good.clone();
        e["failures"][0]["failure"] = json!("hookFailed");
        assert!(check(&e, &worker).is_err());
        let mut h = worker.clone();
        h["failed"] = json!(1);
        assert!(check(&good, &h).is_err());
        let mut h = worker.clone();
        h["process_errors"] = json!(1);
        assert!(check(&good, &h).is_err());
        // A second test's matching message/code cannot borrow the first body's evidence.
        let mut e = good.clone();
        let mut other = e["failures"][0].clone();
        other["id"] = json!(2);
        e["failures"].as_array_mut().unwrap().push(other);
        assert!(check(&e, &worker).is_err());
    }
}
