//! Replay the native Bun audit without installing Bun or executing project code.
//! The adapter is staged here until package and isolated-execution gates are ready.
#[path = "../src/bun/protocol.rs"]
mod protocol;

use protocol::{Completion, Evidence};
use serde_json::{Value, json};

fn records() -> Value {
    serde_json::from_str(include_str!(
        "../benchmarks/bun-body-attribution-results.json"
    ))
    .unwrap()
}

fn row(syntax: &str, scenario: &str) -> Value {
    records()["results"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["syntax"] == syntax && r["scenario"] == scenario)
        .unwrap()
        .clone()
}

fn observer(row: &Value) -> String {
    row["observer"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| format!("{e}\n"))
        .collect()
}

fn classify(row: &Value, files: &[&str]) -> anyhow::Result<Completion> {
    protocol::classify(Evidence {
        exit_code: row["run"]["exit_code"].as_i64().map(|n| n as i32),
        outer_timeout: row["run"]["outer_timeout"].as_bool().unwrap_or(false),
        stderr: row["run"]["stderr"].as_str().unwrap().as_bytes(),
        junit: row["run"]["junit"].as_str().unwrap_or(""),
        observer: &observer(row),
        expected_files: files,
    })
}

#[test]
fn replays_all_112_native_js_and_ts_cases() {
    let records = records();
    let rows = records["results"].as_array().unwrap();
    assert_eq!(rows.len(), 112);
    let mut counts = [0; 4];
    for row in rows {
        let file = format!("case.test.{}", row["syntax"].as_str().unwrap());
        let result = classify(row, &[&file]);
        let (actual, index) = match result {
            Ok(Completion::Passed) => ("passed", 0),
            Ok(Completion::BodyFailure) => ("body_failure", 1),
            Ok(Completion::Timeout) => ("timeout", 2),
            Err(_) => ("error", 3),
        };
        assert_eq!(
            actual, row["expected"],
            "{} {}",
            row["syntax"], row["scenario"]
        );
        counts[index] += 1;
    }
    assert_eq!(counts, [16, 20, 2, 74]);
}

#[test]
fn rejects_damaged_and_insufficient_evidence_in_both_syntaxes() {
    for syntax in ["js", "ts"] {
        let body = row(syntax, "body_throw");
        let file = format!("case.test.{syntax}");
        let reject =
            |altered: &Value| assert!(classify(altered, &[&file]).is_err(), "accepted {altered}");
        for observed in [
            json!([]),
            Value::Array(body["observer"].as_array().unwrap()[..4].to_vec()),
            row(syntax, "pass")["observer"].clone(),
        ] {
            let mut altered = body.clone();
            altered["observer"] = observed;
            reject(&altered);
        }
        for (index, key, value) in [
            (4, "sequence", json!(99)),
            (1, "testId", json!(99)),
            (4, "phase", json!("afterEach")),
            (0, "version", json!("other")),
            (0, "revision", json!("other")),
            (2, "id", json!(-1)),
            (2, "testId", json!(true)),
            (4, "event", json!("unknown")),
        ] {
            let mut altered = body.clone();
            altered["observer"][index][key] = value;
            reject(&altered);
        }
        let mut altered = body.clone();
        altered["observer"].as_array_mut().unwrap().remove(3);
        for (i, event) in altered["observer"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .enumerate()
        {
            event["sequence"] = json!(i + 1);
        }
        reject(&altered);
        for (key, value) in [
            ("junit", Value::Null),
            ("junit", json!("<testsuites>")),
            ("exit_code", json!(0)),
            ("exit_code", json!(-9)),
            ("outer_timeout", json!(true)),
        ] {
            let mut altered = body.clone();
            altered["run"][key] = value;
            reject(&altered);
        }
        let mut altered = body.clone();
        altered["run"]["junit"] = json!(body["run"]["junit"].as_str().unwrap().replacen(
            "tests=\"1\"",
            "tests=\"2\"",
            1
        ));
        reject(&altered);
        let mut altered = body.clone();
        altered["run"]["stderr"] = json!("Unhandled error between tests");
        reject(&altered);
        // A provisional failure may never be converted to a successful return.
        let mut altered = body.clone();
        altered["observer"][4]["event"] = json!("return");
        reject(&altered);
        // Even a matching final inventory cannot repair invocation before registration.
        let mut altered = body.clone();
        altered["observer"].as_array_mut().unwrap().swap(1, 2);
        for (i, event) in altered["observer"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .enumerate()
        {
            event["sequence"] = json!(i + 1);
        }
        reject(&altered);
    }
}

#[test]
fn requires_independent_complete_file_inventory() {
    let body = row("js", "body_throw");
    for files in [
        vec![],
        vec![""],
        vec!["other.test.js"],
        vec!["case.test.js", "missing.test.js"],
        vec!["case.test.js", "case.test.js"],
    ] {
        assert!(classify(&body, &files).is_err());
    }
    let mut altered = body.clone();
    let xml = body["run"]["junit"].as_str().unwrap();
    let (prefix, case) = xml.split_once("<testcase").unwrap();
    altered["run"]["junit"] = json!(format!(
        "{prefix}<testcase{}",
        case.replacen("file=\"case.test.js\"", "file=\"other.test.js\"", 1)
    ));
    assert!(classify(&altered, &["case.test.js"]).is_err());
}

#[test]
fn rejects_xml_entities_namespaces_hidden_results_and_oversized_or_truncated_evidence() {
    let body = row("js", "body_throw");
    let original = body["run"]["junit"].as_str().unwrap();
    for xml in [
        original.replacen(
            "<testsuites",
            "<!DOCTYPE testsuites [<!ENTITY x 'body'>]><testsuites",
            1,
        ),
        original.replacen("<testsuites", "<testsuites xmlns=\"urn:foreign\"", 1),
        original.replace(
            "<failure type=\"AssertionError\" />",
            "<properties><testcase name=\"hidden\"/></properties>",
        ),
        " ".repeat(8 * 1024 * 1024 + 1),
    ] {
        let mut altered = body.clone();
        altered["run"]["junit"] = json!(xml);
        assert!(classify(&altered, &["case.test.js"]).is_err());
    }
    let observed = observer(&body);
    for invalid in [
        observed.trim_end().to_owned(),
        format!("{observed}{{"),
        " ".repeat(8 * 1024 * 1024 + 1),
        format!("{observed}\n"),
    ] {
        assert!(
            protocol::classify(Evidence {
                exit_code: Some(1),
                outer_timeout: false,
                stderr: b"",
                junit: original,
                observer: &invalid,
                expected_files: &["case.test.js"]
            })
            .is_err()
        );
    }
}

#[test]
#[ignore = "requires full Composer audit evidence via MUTIE_BUN_COMPOSER_EVIDENCE"]
fn replays_composer_baselines_and_stronger_finding() {
    let path =
        std::env::var("MUTIE_BUN_COMPOSER_EVIDENCE").expect("set MUTIE_BUN_COMPOSER_EVIDENCE");
    let bytes = std::fs::read(path).unwrap();
    let record: Value = serde_json::from_slice(&bytes).unwrap();
    let summary: Value = serde_json::from_str(include_str!(
        "../benchmarks/bun-observer-composer-results.json"
    ))
    .unwrap();
    use sha2::{Digest, Sha256};
    assert_eq!(
        format!("{:x}", Sha256::digest(&bytes)),
        summary["full_local_evidence_sha256"].as_str().unwrap()
    );
    assert_eq!(
        record["original_file_sha256"],
        summary["original_file_sha256"]
    );
    assert_eq!(record["observer_sha256"], summary["observer_sha256"]);
    let files: Vec<_> = record["original_file_sha256"]
        .as_object()
        .unwrap()
        .keys()
        .filter(|p| p.starts_with("tests/") && p.ends_with(".test.ts"))
        .map(String::as_str)
        .collect();
    assert_eq!(files.len(), 11);
    let runs = record["runs"].as_array().unwrap();
    assert_eq!(runs.len(), 4);
    for row in runs {
        let mode = row["mode"].as_str().unwrap();
        let mut expected = files.clone();
        if mode.starts_with("stronger_") {
            expected.push("tests/mutie-stronger.test.ts");
        }
        let outcome = classify(row, &expected).unwrap_or_else(|e| panic!("{mode}: {e:#}"));
        assert_eq!(
            outcome,
            match mode {
                "baseline" | "candidate" | "stronger_baseline" => Completion::Passed,
                "stronger_candidate" => Completion::BodyFailure,
                _ => panic!("unexpected mode {mode}"),
            }
        );
        expected.push("tests/missing.test.ts");
        assert!(classify(row, &expected).is_err());
    }
}
