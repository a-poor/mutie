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
        conftests: Vec<String>,
    },
    Phase {
        node: String,
        when: String,
        outcome: String,
        #[serde(deserialize_with = "Option::deserialize")]
        exception: Option<String>,
        assertion: bool,
        candidate_exception: bool,
        xfail: bool,
        #[serde(deserialize_with = "Option::deserialize")]
        unittest: Option<UnitEvidence>,
    },
    Finish {
        status: i32,
    },
    Fixture {
        token: usize,
        stage: String,
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
    let (nodes, outcome) = classify(bytes, pid, nonce, root, collect_only, 0, &[])?;
    ensure!(outcome == Outcome::Missed, "Python baseline failed");
    Ok(nodes)
}

/// A body exception remains an error, but only after the entire ledger is valid.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum CandidateOutcome {
    Scored(Outcome),
    BodyError,
}

#[cfg(test)]
pub(super) fn classify(
    bytes: &[u8],
    pid: u32,
    nonce: &str,
    root: &str,
    collect_only: bool,
    code: i32,
    expected_conftests: &[String],
) -> Result<(Vec<String>, Outcome)> {
    let (nodes, outcome) = inspect(
        bytes,
        pid,
        nonce,
        root,
        collect_only,
        code,
        expected_conftests,
    )?;
    match outcome {
        CandidateOutcome::Scored(outcome) => Ok((nodes, outcome)),
        CandidateOutcome::BodyError => {
            anyhow::bail!("Python run contains an unsupported phase failure")
        }
    }
}

pub(super) fn inspect(
    bytes: &[u8],
    pid: u32,
    nonce: &str,
    root: &str,
    collect_only: bool,
    code: i32,
    expected_conftests: &[String],
) -> Result<(Vec<String>, CandidateOutcome)> {
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
    let Event::Collection {
        nodes,
        unittest,
        conftests,
    } = &records[2].event
    else {
        anyhow::bail!("missing Python collection");
    };
    ensure!(
        conftests == expected_conftests,
        "Python loaded conftest inventory differs from audited scope"
    );
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
    let mut fixtures = Vec::<usize>::new();
    let mut failed = false;
    let mut body_error = false;
    for record in &records[3..tail] {
        if let Event::Fixture { token, stage } = &record.event {
            ensure!(!collect_only, "Python collection executed a fixture");
            if stage == "started" {
                ensure!(*token == fixtures.len(), "Python fixture identity mismatch");
                fixtures.push(1);
            } else {
                let state = fixtures.get_mut(*token).context("unknown Python fixture")?;
                ensure!(
                    ["started", "ready", "finished"].get(*state) == Some(&stage.as_str()),
                    "Python fixture lifecycle mismatch"
                );
                *state += 1;
            }
            continue;
        }
        let Event::Phase {
            node,
            when,
            outcome,
            exception,
            assertion,
            candidate_exception,
            xfail,
            unittest,
        } = &record.event
        else {
            anyhow::bail!("unexpected Python lifecycle evidence");
        };
        let passed =
            outcome == "passed" && exception.is_none() && !assertion && !candidate_exception;
        let body_failure = outcome == "failed"
            && when == "call"
            && *assertion
            && !candidate_exception
            && exception.as_ref().is_some_and(|kind| !kind.is_empty());
        // Only ordinary pytest body exceptions are eligible. Unittest dispatch,
        // setup/teardown and fixture errors retain their fatal boundary.
        let runtime_failure = outcome == "failed"
            && when == "call"
            && !assertion
            && *candidate_exception
            && exception.as_ref().is_some_and(|kind| !kind.is_empty())
            && !unit_nodes.contains(node);
        ensure!(
            !collect_only
                && unique.contains(node)
                && !xfail
                && (passed || body_failure || runtime_failure),
            "Python run contains an unsupported phase failure"
        );
        failed |= body_failure || runtime_failure;
        body_error |= runtime_failure;
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
        fixtures.iter().all(|state| *state == 3),
        "Python fixture finalization incomplete"
    );
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
        if body_error {
            CandidateOutcome::BodyError
        } else if failed {
            CandidateOutcome::Scored(Outcome::Killed)
        } else {
            CandidateOutcome::Scored(Outcome::Missed)
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
            if value["event"] == "collection" { value["unittest"] = json!([]); value["conftests"] = json!([]); }
            if value["event"] == "phase" { value["unittest"] = json!(null); value["candidate_exception"] = json!(false); }
            value["seq"] = json!(seq);
            value["pid"] = json!(17);
            value["nonce"] = json!("token");
            format!("{value}\n").into_bytes()
        }).collect()
    }

    #[test]
    fn candidate_errors_require_complete_lifetime_and_positive_type_evidence() {
        let mut records: Vec<serde_json::Value> = String::from_utf8(evidence())
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        records[4]["outcome"] = json!("failed");
        records[4]["exception"] = json!("ValueError");
        records[4]["candidate_exception"] = json!(true);
        records[6]["status"] = json!(1);
        records[7]["status"] = json!(1);
        let inspect_records = |r: &[serde_json::Value]| {
            let bytes = r.iter().map(|v| format!("{v}\n")).collect::<String>();
            inspect(bytes.as_bytes(), 17, "token", "/repo", false, 1, &[])
        };
        assert_eq!(
            inspect_records(&records).unwrap().1,
            CandidateOutcome::BodyError
        );
        // Body evidence must not short-circuit any subsequent lifecycle checks.
        for (index, key, value) in [
            (4, "candidate_exception", json!(false)),
            (4, "assertion", json!(true)),
            (4, "exception", json!(null)),
            (4, "xfail", json!(true)),
            (5, "outcome", json!("failed")),
            (5, "when", json!("call")),
            (7, "status", json!(0)),
            (8, "hooks_intact", json!(false)),
            (9, "alive_threads", json!(["late"])),
        ] {
            let mut changed = records.clone();
            changed[index][key] = value;
            assert!(inspect_records(&changed).is_err(), "{index}/{key}");
        }
        for index in 0..records.len() {
            let mut changed = records.clone();
            changed.remove(index);
            assert!(inspect_records(&changed).is_err());
        }
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
            classify(
                encode(&records).as_bytes(),
                17,
                "token",
                "/repo",
                false,
                1,
                &[]
            )
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
                    classify(
                        encode(&changed).as_bytes(),
                        17,
                        "token",
                        "/repo",
                        false,
                        1,
                        &[]
                    )
                    .is_err()
                );
            }
            if index != 1 {
                let mut changed = records.clone();
                changed[4]["unittest"]["steps"][index]["outcome"] = json!("failed");
                assert!(
                    classify(
                        encode(&changed).as_bytes(),
                        17,
                        "token",
                        "/repo",
                        false,
                        1,
                        &[]
                    )
                    .is_err()
                );
            }
        }
        for key in ["intact", "body_matches"] {
            let mut changed = records.clone();
            changed[4]["unittest"][key] = json!(false);
            assert!(
                classify(
                    encode(&changed).as_bytes(),
                    17,
                    "token",
                    "/repo",
                    false,
                    1,
                    &[]
                )
                .is_err()
            );
        }
        for index in [2, 4] {
            let mut changed = records.clone();
            changed[index].as_object_mut().unwrap().remove("unittest");
            assert!(
                classify(
                    encode(&changed).as_bytes(),
                    17,
                    "token",
                    "/repo",
                    false,
                    1,
                    &[]
                )
                .is_err()
            );
        }
    }

    #[test]
    fn fixture_evidence_requires_complete_ordered_lifetimes_and_audited_configs() {
        let mut records: Vec<serde_json::Value> = String::from_utf8(evidence())
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        records[2]["conftests"] = json!(["tests/conftest.py"]);
        records.insert(3, json!({"event":"fixture","token":0,"stage":"started"}));
        records.insert(4, json!({"event":"fixture","token":0,"stage":"ready"}));
        records.insert(7, json!({"event":"fixture","token":0,"stage":"finished"}));
        let encode = |records: &[serde_json::Value]| {
            records
                .iter()
                .enumerate()
                .map(|(seq, v)| {
                    let mut v = v.clone();
                    v["seq"] = json!(seq);
                    v["pid"] = json!(17);
                    v["nonce"] = json!("token");
                    format!("{v}\n")
                })
                .collect::<String>()
        };
        let configs = vec!["tests/conftest.py".into()];
        assert!(
            classify(
                encode(&records).as_bytes(),
                17,
                "token",
                "/repo",
                false,
                0,
                &configs
            )
            .is_ok()
        );
        assert!(
            classify(
                encode(&records).as_bytes(),
                17,
                "token",
                "/repo",
                false,
                0,
                &[]
            )
            .is_err()
        );
        for index in [3, 4, 7] {
            let mut changed = records.clone();
            changed.remove(index);
            assert!(
                classify(
                    encode(&changed).as_bytes(),
                    17,
                    "token",
                    "/repo",
                    false,
                    0,
                    &configs
                )
                .is_err()
            );
            let mut changed = records.clone();
            changed[index]["token"] = json!(1);
            assert!(
                classify(
                    encode(&changed).as_bytes(),
                    17,
                    "token",
                    "/repo",
                    false,
                    0,
                    &configs
                )
                .is_err()
            );
        }
        for stage in ["ready", "failed", "finished"] {
            let mut changed = records.clone();
            changed[3]["stage"] = json!(stage);
            assert!(
                classify(
                    encode(&changed).as_bytes(),
                    17,
                    "token",
                    "/repo",
                    false,
                    0,
                    &configs
                )
                .is_err()
            );
        }
        records[2].as_object_mut().unwrap().remove("conftests");
        assert!(
            classify(
                encode(&records).as_bytes(),
                17,
                "token",
                "/repo",
                false,
                0,
                &configs
            )
            .is_err()
        );
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
            classify(killed.as_bytes(), 17, "token", "/repo", false, 1, &[])
                .unwrap()
                .1,
            Outcome::Killed
        );
        assert!(classify(killed.as_bytes(), 17, "token", "/repo", false, 0, &[]).is_err());
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
            assert!(
                classify(
                    encode(&changed).as_bytes(),
                    17,
                    "token",
                    "/repo",
                    false,
                    1,
                    &[]
                )
                .is_err()
            );
        }
        records.remove(5);
        for (seq, record) in records.iter_mut().enumerate() {
            record["seq"] = json!(seq);
        }
        assert!(
            classify(
                encode(&records).as_bytes(),
                17,
                "token",
                "/repo",
                false,
                1,
                &[]
            )
            .is_err()
        );
    }
}
