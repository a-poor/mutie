use std::collections::{BTreeMap, BTreeSet};

use crate::report::Outcome;
use anyhow::{Context, Result, ensure};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Record {
    seq: usize,
    pid: u32,
    nonce: String,
    #[serde(flatten)]
    event: Event,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case", deny_unknown_fields)]
enum Event {
    LifetimeStart {},
    Start {
        python: String,
        pytest: String,
        root: String,
    },
    Collection {
        nodes: Vec<String>,
        unittest: Vec<String>,
    },
    Phase {
        node: String,
        when: String,
        outcome: String,
        #[serde(deserialize_with = "Option::deserialize")]
        exception: Option<String>,
        assertion: bool,
        xfail: bool,
        #[serde(deserialize_with = "Option::deserialize")]
        unittest: Option<UnitEvidence>,
    },
    Finish {
        status: i32,
    },
    Returned {
        status: i32,
        imports: BTreeMap<String, String>,
    },
    Checkpoint {
        stage: String,
        hooks_intact: bool,
        alive_threads: Vec<String>,
    },
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct UnitEvidence {
    steps: Vec<UnitStep>,
    intact: bool,
    body_matches: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct UnitStep {
    stage: String,
    outcome: String,
    assertion: bool,
}

impl UnitEvidence {
    fn validate(&self, body_failure: bool) -> Result<()> {
        ensure!(
            self.intact && self.body_matches,
            "Python unittest body identity or dispatch mismatch"
        );
        ensure!(
            self.steps.len() >= 5,
            "incomplete Python unittest lifecycle"
        );
        let last = self.steps.len() - 1;
        for (index, step) in self.steps.iter().enumerate() {
            let expected = match index {
                0 => "setup",
                1 => "body",
                2 => "teardown",
                3 => "cleanups_begin",
                n if n == last => "cleanups_end",
                _ => "cleanup",
            };
            let failure = index == 1 && body_failure;
            ensure!(
                step.stage == expected
                    && step.outcome == if failure { "failed" } else { "passed" }
                    && step.assertion == failure,
                "Python unittest lifecycle failed or incomplete"
            );
        }
        Ok(())
    }
}

#[cfg(test)]
fn baseline(
    bytes: &[u8],
    pid: u32,
    nonce: &str,
    root: &str,
    collect_only: bool,
) -> Result<Vec<String>> {
    let (nodes, outcome) = classify(bytes, pid, nonce, root, collect_only, 0)?;
    ensure!(outcome == Outcome::Missed, "Python baseline failed");
    Ok(nodes)
}

pub(super) fn classify(
    bytes: &[u8],
    pid: u32,
    nonce: &str,
    root: &str,
    collect_only: bool,
    code: i32,
) -> Result<(Vec<String>, Outcome)> {
    ensure!(
        matches!(code, 0 | 1) && (!collect_only || code == 0),
        "unsupported Python exit status"
    );
    ensure!(
        !bytes.is_empty() && bytes.len() <= 4 * 1024 * 1024 && bytes.ends_with(b"\n"),
        "missing, oversized or incomplete Python evidence"
    );
    let records: Vec<Record> = bytes[..bytes.len() - 1]
        .split(|b| *b == b'\n')
        .map(serde_json::from_slice)
        .collect::<std::result::Result<_, _>>()
        .context("invalid Python evidence")?;
    for (index, record) in records.iter().enumerate() {
        ensure!(
            record.seq == index && record.pid == pid && record.nonce == nonce,
            "Python evidence sequence/process identity mismatch"
        );
    }
    ensure!(records.len() >= 7, "incomplete Python lifecycle");
    ensure!(
        matches!(records[0].event, Event::LifetimeStart {}),
        "missing Python lifetime start"
    );
    ensure!(
        matches!(&records[1].event, Event::Start { python, pytest, root: observed }
        if python.starts_with("3.14.7 ") && pytest == "9.0.2" && observed == root),
        "Python runtime/root evidence mismatch"
    );
    let Event::Collection { nodes, unittest } = &records[2].event else {
        anyhow::bail!("missing Python collection");
    };
    let unique: BTreeSet<_> = nodes.iter().collect();
    let unit_nodes: BTreeSet<_> = unittest.iter().collect();
    ensure!(
        unit_nodes.len() == unittest.len() && unit_nodes.is_subset(&unique),
        "invalid Python unittest inventory"
    );
    ensure!(
        !nodes.is_empty()
            && unique.len() == nodes.len()
            && nodes.iter().all(|n| !n.contains('\0')
                && n.split_once("::")
                    .is_some_and(
                        |(file, test)| super::identity::is_test_file(file) && !test.is_empty()
                    )),
        "invalid Python collected test inventory"
    );
    let tail = records.len() - 4;
    ensure!(
        matches!(records[tail].event, Event::Finish { status } if status == code),
        "Python session exit status mismatch"
    );
    ensure!(
        matches!(&records[tail + 1].event, Event::Returned { status, imports }
        if *status == code && imports.values().all(|p| {
            let path = std::path::Path::new(p);
            path.is_absolute() && path.starts_with(root)
                && !path.components().any(|c| matches!(c, std::path::Component::ParentDir))
        })),
        "Python return status or imports mismatch"
    );
    for (offset, expected) in [(2, "returned"), (3, "atexit")] {
        ensure!(
            matches!(&records[tail + offset].event,
            Event::Checkpoint { stage, hooks_intact: true, alive_threads }
            if stage == expected && alive_threads.is_empty()),
            "Python lifetime checkpoint failed"
        );
    }
    let mut phases = BTreeMap::<&str, usize>::new();
    let mut failed = false;
    for record in &records[3..tail] {
        let Event::Phase {
            node,
            when,
            outcome,
            exception,
            assertion,
            xfail,
            unittest,
        } = &record.event
        else {
            anyhow::bail!("unexpected Python lifecycle evidence");
        };
        let passed = outcome == "passed" && exception.is_none() && !assertion;
        let body_failure = outcome == "failed"
            && when == "call"
            && *assertion
            && exception.as_ref().is_some_and(|kind| !kind.is_empty());
        ensure!(
            !collect_only && unique.contains(node) && !xfail && (passed || body_failure),
            "Python run contains an unsupported phase failure"
        );
        failed |= body_failure;
        if when == "call" && unit_nodes.contains(node) {
            unittest
                .as_ref()
                .context("missing Python unittest body evidence")?
                .validate(body_failure)?;
        } else {
            ensure!(
                unittest.is_none(),
                "unexpected Python unittest body evidence"
            );
        }
        let index = phases.entry(node).or_default();
        ensure!(
            ["setup", "call", "teardown"].get(*index) == Some(&when.as_str()),
            "Python phase ordering mismatch"
        );
        *index += 1;
    }
    ensure!(
        collect_only || (phases.len() == nodes.len() && phases.values().all(|n| *n == 3)),
        "Python run omitted test phases"
    );
    ensure!(
        (code == 1) == failed,
        "Python failure evidence and exit status disagree"
    );
    Ok((
        nodes.clone(),
        if failed {
            Outcome::Killed
        } else {
            Outcome::Missed
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn evidence() -> Vec<u8> {
        [
            json!({"event":"lifetime_start"}),
            json!({"event":"start","python":"3.14.7 test","pytest":"9.0.2","root":"/repo"}),
            json!({"event":"collection","nodes":["tests/test_a.py::test_a"]}),
            json!({"event":"phase","node":"tests/test_a.py::test_a","when":"setup","outcome":"passed","exception":null,"assertion":false,"xfail":false}),
            json!({"event":"phase","node":"tests/test_a.py::test_a","when":"call","outcome":"passed","exception":null,"assertion":false,"xfail":false}),
            json!({"event":"phase","node":"tests/test_a.py::test_a","when":"teardown","outcome":"passed","exception":null,"assertion":false,"xfail":false}),
            json!({"event":"finish","status":0}),
            json!({"event":"returned","status":0,"imports":{}}),
            json!({"event":"checkpoint","stage":"returned","hooks_intact":true,"alive_threads":[]}),
            json!({"event":"checkpoint","stage":"atexit","hooks_intact":true,"alive_threads":[]}),
        ].into_iter().enumerate().flat_map(|(seq, mut value)| {
            if value["event"] == "collection" { value["unittest"] = json!([]); }
            if value["event"] == "phase" { value["unittest"] = json!(null); }
            value["seq"] = json!(seq);
            value["pid"] = json!(17);
            value["nonce"] = json!("token");
            format!("{value}\n").into_bytes()
        }).collect()
    }

    #[test]
    fn unittest_requires_exact_complete_dispatch_evidence() {
        let mut records: Vec<serde_json::Value> = String::from_utf8(evidence())
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        records[2]["unittest"] = records[2]["nodes"].clone();
        records[4]["unittest"] = json!({"intact":true,"body_matches":true,"steps":
            (["setup","body","teardown","cleanups_begin","cleanup","cleanups_end"].map(|stage|
                json!({"stage":stage,"outcome":"passed","assertion":false})))});
        let encode =
            |r: &[serde_json::Value]| r.iter().map(|v| format!("{v}\n")).collect::<String>();
        assert!(baseline(encode(&records).as_bytes(), 17, "token", "/repo", false).is_ok());
        records[4]["outcome"] = json!("failed");
        records[4]["assertion"] = json!(true);
        records[4]["exception"] = json!("AssertionError");
        records[4]["unittest"]["steps"][1]["outcome"] = json!("failed");
        records[4]["unittest"]["steps"][1]["assertion"] = json!(true);
        records[6]["status"] = json!(1);
        records[7]["status"] = json!(1);
        assert_eq!(
            classify(encode(&records).as_bytes(), 17, "token", "/repo", false, 1)
                .unwrap()
                .1,
            Outcome::Killed
        );
        for index in 0..6 {
            let mut changed = records.clone();
            changed[4]["unittest"]["steps"]
                .as_array_mut()
                .unwrap()
                .remove(index);
            // Removing the sole successful cleanup leaves a valid zero-cleanup run.
            if index != 4 {
                assert!(
                    classify(encode(&changed).as_bytes(), 17, "token", "/repo", false, 1).is_err()
                );
            }
            if index != 1 {
                let mut changed = records.clone();
                changed[4]["unittest"]["steps"][index]["outcome"] = json!("failed");
                assert!(
                    classify(encode(&changed).as_bytes(), 17, "token", "/repo", false, 1).is_err()
                );
            }
        }
        for key in ["intact", "body_matches"] {
            let mut changed = records.clone();
            changed[4]["unittest"][key] = json!(false);
            assert!(classify(encode(&changed).as_bytes(), 17, "token", "/repo", false, 1).is_err());
        }
        for index in [2, 4] {
            let mut changed = records.clone();
            changed[index].as_object_mut().unwrap().remove("unittest");
            assert!(classify(encode(&changed).as_bytes(), 17, "token", "/repo", false, 1).is_err());
        }
    }

    #[test]
    fn root_tests_require_normal_paths_and_complete_matching_phases() {
        let original = String::from_utf8(evidence()).unwrap();
        let root = original.replace("tests/test_a.py", "test_a.py");
        assert!(baseline(root.as_bytes(), 17, "token", "/repo", false).is_ok());
        for path in [
            "../test_a.py",
            "/test_a.py",
            "sample/test_a.py",
            "test_a.txt",
            "helper.py",
            "tests/../test_a.py",
        ] {
            let changed = original.replace("tests/test_a.py", path);
            assert!(
                baseline(changed.as_bytes(), 17, "token", "/repo", false).is_err(),
                "{path}"
            );
        }
    }

    #[test]
    fn requires_complete_success_and_strict_records() {
        let bytes = evidence();
        assert_eq!(
            baseline(&bytes, 17, "token", "/repo", false).unwrap().len(),
            1
        );
        for changed in [
            String::from_utf8(bytes.clone())
                .unwrap()
                .replace("\"passed\"", "\"skipped\""),
            String::from_utf8(bytes.clone())
                .unwrap()
                .replace("\"call\"", "\"setup\""),
            String::from_utf8(bytes.clone())
                .unwrap()
                .replace("\"hooks_intact\":true", "\"hooks_intact\":false"),
            String::from_utf8(bytes.clone())
                .unwrap()
                .replace("\"seq\":0", "\"seq\":0,\"seq\":0"),
            String::from_utf8(bytes.clone())
                .unwrap()
                .replace("\"event\":\"start\"", "\"event\":\"start\",\"extra\":true"),
            String::from_utf8(bytes.clone()).unwrap().replace(
                "\"event\":\"lifetime_start\"",
                "\"event\":\"lifetime_start\",\"extra\":true",
            ),
            String::from_utf8(bytes.clone())
                .unwrap()
                .replace("\"exception\":null,", ""),
            format!(
                "{}{{\"event\":\"lifetime_error\"}}\n",
                String::from_utf8(bytes.clone()).unwrap()
            ),
        ] {
            assert!(
                baseline(changed.as_bytes(), 17, "token", "/repo", false).is_err(),
                "{changed}"
            );
        }
        assert!(baseline(&bytes, 18, "token", "/repo", false).is_err());
        assert!(baseline(&bytes, 17, "wrong", "/repo", false).is_err());
        assert!(baseline(&bytes, 17, "token", "/other", false).is_err());
        assert!(baseline(&bytes[..bytes.len() - 1], 17, "token", "/repo", false).is_err());
    }

    #[test]
    fn only_completed_assertion_bodies_can_be_kills() {
        let mut records: Vec<serde_json::Value> = String::from_utf8(evidence())
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        records[4]["outcome"] = json!("failed");
        records[4]["assertion"] = json!(true);
        records[4]["exception"] = json!("AssertionError");
        records[6]["status"] = json!(1);
        records[7]["status"] = json!(1);
        let encode = |records: &[serde_json::Value]| {
            records
                .iter()
                .map(|value| format!("{value}\n"))
                .collect::<String>()
        };
        let killed = encode(&records);
        assert_eq!(
            classify(killed.as_bytes(), 17, "token", "/repo", false, 1)
                .unwrap()
                .1,
            Outcome::Killed
        );
        assert!(classify(killed.as_bytes(), 17, "token", "/repo", false, 0).is_err());
        for (record, key, value) in [
            (4, "assertion", json!(false)),
            (4, "exception", json!(null)),
            (4, "xfail", json!(true)),
            (4, "outcome", json!("skipped")),
            (5, "outcome", json!("failed")),
            (7, "status", json!(0)),
            (7, "imports", json!({"sample":"/repo/../outside/sample.py"})),
            (7, "imports", json!({"sample":"relative/sample.py"})),
            (7, "imports", json!({"sample":"/outside/sample.py"})),
            (9, "hooks_intact", json!(false)),
        ] {
            let mut changed = records.clone();
            changed[record][key] = value;
            assert!(classify(encode(&changed).as_bytes(), 17, "token", "/repo", false, 1).is_err());
        }
        records.remove(5);
        for (seq, record) in records.iter_mut().enumerate() {
            record["seq"] = json!(seq);
        }
        assert!(classify(encode(&records).as_bytes(), 17, "token", "/repo", false, 1).is_err());
    }
}
