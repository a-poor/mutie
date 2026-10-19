//! Conservative evidence reader for the isolated Bun diagnostic.
//!
//! A successful result requires trusted, complete command captures and an independently
//! discovered test-file inventory. This reader does not establish package scope, process
//! cleanup, or observer integrity; callers must establish those before scoring a run.

use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, bail, ensure};
use roxmltree::{Document, Node, ParsingOptions};
use serde::Deserialize;

const MAX_EVIDENCE_BYTES: usize = 8 * 1024 * 1024;
const MAX_EVENTS: usize = 100_000;
const VERSION: &str = "1.3.11";
const REVISION: &str = "af24e281ebacd6ac77c0f14b4206599cf4ae1c9f";

#[derive(Debug, PartialEq, Eq)]
pub enum Completion {
    Passed,
    BodyFailure,
    Timeout,
}

/// Evidence bytes must be complete, never the retained head/tail of a capture.
pub struct Evidence<'a> {
    pub exit_code: Option<i32>,
    pub outer_timeout: bool,
    pub stderr: &'a [u8],
    pub junit: &'a str,
    pub observer: &'a str,
    pub expected_files: &'a [&'a str],
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
enum Phase {
    Body,
    BeforeEach,
    AfterEach,
    BeforeAll,
    AfterAll,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum FailureKind {
    Throw,
    Reject,
}

#[derive(Debug, Deserialize)]
struct Record {
    sequence: usize,
    #[serde(flatten)]
    event: Event,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "event", rename_all = "lowercase")]
enum Event {
    Init {
        version: String,
        revision: String,
    },
    Declare {
        #[serde(rename = "testId")]
        test_id: u64,
        name: String,
    },
    Enter {
        id: u64,
        phase: Phase,
        name: String,
        #[serde(rename = "testId")]
        test_id: Option<u64>,
    },
    Caught {
        id: u64,
        phase: Phase,
        name: String,
        kind: FailureKind,
    },
    Return {
        id: u64,
        phase: Phase,
        name: String,
    },
    Throw {
        id: u64,
        phase: Phase,
        name: String,
    },
    Reject {
        id: u64,
        phase: Phase,
        name: String,
    },
    Unsupported {},
    Exit {},
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Terminal {
    Return,
    Failure(FailureKind),
}

struct Invocation {
    phase: Phase,
    name: String,
    test_id: Option<u64>,
    caught: Option<FailureKind>,
    terminal: Option<Terminal>,
}

pub fn classify(evidence: Evidence<'_>) -> Result<Completion> {
    ensure!(
        !evidence.outer_timeout && matches!(evidence.exit_code, Some(0 | 1)),
        "command did not complete normally"
    );
    // Negative evidence only: resource-limit failures are infrastructure errors,
    // even when Bun exposes them as rejected body promises. User text may cause
    // conservative errors, but can never supply positive failure evidence.
    for marker in [
        b"Unhandled error between tests".as_slice(),
        b"EFBIG".as_slice(),
    ] {
        ensure!(
            !evidence.stderr.windows(marker.len()).any(|w| w == marker),
            "native process or file-limit error diagnostic"
        );
    }
    ensure!(
        !evidence.observer.is_empty() && evidence.observer.len() <= MAX_EVIDENCE_BYTES,
        "missing or oversized observer evidence"
    );
    ensure!(
        evidence.observer.ends_with('\n'),
        "truncated observer record"
    );
    let mut declarations = BTreeMap::new();
    let mut invocations = BTreeMap::<u64, Invocation>::new();
    let mut bodies = Vec::new();
    for (index, line) in evidence.observer.lines().enumerate() {
        ensure!(index < MAX_EVENTS, "too many observer events");
        let record: Record = serde_json::from_str(line).context("invalid observer record")?;
        ensure!(record.sequence == index + 1, "observer sequence gap");
        if index == 0 {
            match record.event {
                Event::Init { version, revision } => ensure!(
                    version == VERSION && revision == REVISION,
                    "wrong runtime identity"
                ),
                _ => bail!("missing observer initialization"),
            }
            continue;
        }
        match record.event {
            Event::Init { .. } => bail!("duplicate observer initialization"),
            Event::Unsupported { .. } | Event::Exit { .. } => {
                bail!("unsupported API or explicit exit")
            }
            Event::Declare { test_id, name } => {
                ensure!(
                    test_id > 0 && declarations.insert(test_id, name).is_none(),
                    "invalid declaration ID"
                );
            }
            Event::Enter {
                id,
                phase,
                name,
                test_id,
            } => {
                ensure!(
                    id > 0 && !invocations.contains_key(&id),
                    "invalid invocation ID"
                );
                if phase == Phase::Body {
                    let test_id = test_id.context("missing body test ID")?;
                    ensure!(
                        declarations.get(&test_id) == Some(&name),
                        "body lacks matching prior declaration"
                    );
                    bodies.push(id);
                } else {
                    ensure!(test_id.is_none(), "hook has test ID");
                }
                invocations.insert(
                    id,
                    Invocation {
                        phase,
                        name,
                        test_id,
                        caught: None,
                        terminal: None,
                    },
                );
            }
            Event::Caught {
                id,
                phase,
                name,
                kind,
            } => {
                let begin = invocations
                    .get_mut(&id)
                    .context("provisional failure without invocation")?;
                ensure!(
                    phase == Phase::Body
                        && phase == begin.phase
                        && name == begin.name
                        && begin.caught.is_none()
                        && begin.terminal.is_none(),
                    "invalid provisional failure"
                );
                begin.caught = Some(kind);
            }
            event => {
                let (id, phase, name, terminal) = match event {
                    Event::Return { id, phase, name } => (id, phase, name, Terminal::Return),
                    Event::Throw { id, phase, name } => {
                        (id, phase, name, Terminal::Failure(FailureKind::Throw))
                    }
                    Event::Reject { id, phase, name } => {
                        (id, phase, name, Terminal::Failure(FailureKind::Reject))
                    }
                    _ => unreachable!(),
                };
                let begin = invocations
                    .get_mut(&id)
                    .context("terminal without invocation")?;
                ensure!(
                    phase == begin.phase && name == begin.name && begin.terminal.is_none(),
                    "invalid observer terminal"
                );
                match terminal {
                    Terminal::Failure(kind) => {
                        ensure!(phase == Phase::Body, "hook failure");
                        ensure!(begin.caught == Some(kind), "failure checkpoint mismatch");
                    }
                    Terminal::Return => {
                        ensure!(begin.caught.is_none(), "return after provisional failure")
                    }
                }
                begin.terminal = Some(terminal);
            }
        }
    }

    let cases = parse_junit(evidence.junit, evidence.expected_files)?;
    ensure!(
        declarations.len() == cases.len() && bodies.len() == cases.len(),
        "body/declaration/report count mismatch"
    );
    let mut executed = BTreeSet::new();
    let mut failed = false;
    let mut timed_out = false;
    for (id, case) in bodies.iter().zip(cases) {
        let body = &invocations[id];
        ensure!(executed.insert(body.test_id), "duplicate body execution");
        ensure!(body.name == case.name, "body/report name mismatch");
        match (case.failure.as_deref(), body.terminal) {
            (None, Some(Terminal::Return)) => (),
            (Some("AssertionError"), Some(Terminal::Failure(_))) => failed = true,
            (Some("TimeoutError"), None) if body.caught.is_none() => timed_out = true,
            _ => bail!("failure lacks positive body evidence or result is incomplete"),
        }
    }
    ensure!(
        invocations
            .values()
            .all(|i| i.phase == Phase::Body || i.terminal == Some(Terminal::Return)),
        "incomplete hook"
    );
    ensure!(
        evidence.exit_code == Some(i32::from(failed || timed_out)),
        "exit status mismatch"
    );
    Ok(if timed_out {
        Completion::Timeout
    } else if failed {
        Completion::BodyFailure
    } else {
        Completion::Passed
    })
}

struct Case {
    name: String,
    failure: Option<String>,
}

fn elements<'a, 'input>(node: Node<'a, 'input>) -> impl Iterator<Item = Node<'a, 'input>> {
    node.children().filter(Node::is_element)
}

fn count(node: Node<'_, '_>, key: &str) -> Result<usize> {
    node.attribute(key)
        .context("missing JUnit count")?
        .parse()
        .context("invalid JUnit count")
}

fn parse_junit(xml: &str, expected_files: &[&str]) -> Result<Vec<Case>> {
    ensure!(
        !xml.is_empty() && xml.len() <= MAX_EVIDENCE_BYTES,
        "missing or oversized JUnit report"
    );
    let document = Document::parse_with_options(
        xml,
        ParsingOptions {
            allow_dtd: false,
            nodes_limit: 200_000,
            ..Default::default()
        },
    )
    .context("invalid JUnit report")?;
    ensure!(
        document
            .descendants()
            .all(|node| node.ancestors().take(66).count() <= 65),
        "JUnit nesting limit exceeded"
    );
    let root = document.root_element();
    ensure!(root.tag_name().name() == "testsuites", "invalid JUnit root");
    ensure!(
        root.descendants()
            .filter(Node::is_element)
            .all(|n| n.tag_name().namespace().is_none()),
        "unsupported XML namespace"
    );
    let expected: BTreeSet<_> = expected_files.iter().copied().collect();
    ensure!(
        !expected.is_empty() && expected.len() == expected_files.len() && !expected.contains(""),
        "invalid expected file inventory"
    );
    let mut files = BTreeSet::new();
    for suite in elements(root) {
        ensure!(
            suite.tag_name().name() == "testsuite",
            "invalid JUnit structure"
        );
        let file = suite.attribute("file").context("missing suite file")?;
        ensure!(files.insert(file), "duplicate file suite");
        ensure!(
            suite
                .descendants()
                .filter(Node::is_element)
                .filter(|n| matches!(n.tag_name().name(), "testsuite" | "testcase"))
                .all(|n| n.attribute("file") == Some(file)),
            "inconsistent JUnit file identity"
        );
    }
    ensure!(files == expected, "JUnit file inventory mismatch");
    for suite in root
        .descendants()
        .filter(Node::is_element)
        .filter(|n| matches!(n.tag_name().name(), "testsuites" | "testsuite"))
    {
        ensure!(
            elements(suite)
                .all(|n| matches!(n.tag_name().name(), "testsuite" | "testcase" | "properties")),
            "unsupported suite child"
        );
        // Properties carry metadata only; never allow hidden result nodes there.
        for properties in elements(suite).filter(|n| n.tag_name().name() == "properties") {
            ensure!(
                elements(properties)
                    .all(|n| n.tag_name().name() == "property" && elements(n).next().is_none()),
                "invalid JUnit properties"
            );
        }
        let cases: Vec<_> = suite
            .descendants()
            .filter(|n| n.has_tag_name("testcase"))
            .collect();
        ensure!(
            count(suite, "tests")? == cases.len()
                && count(suite, "failures")?
                    == cases
                        .iter()
                        .filter(|n| elements(**n).next().is_some())
                        .count()
                && count(suite, "skipped")? == 0,
            "JUnit aggregate mismatch"
        );
    }
    let mut cases = Vec::new();
    for node in root.descendants().filter(|n| n.has_tag_name("testcase")) {
        let children: Vec<_> = elements(node).collect();
        ensure!(
            children.is_empty()
                || (children.len() == 1
                    && children[0].has_tag_name("failure")
                    && elements(children[0]).next().is_none()),
            "unsupported test result"
        );
        cases.push(Case {
            name: node
                .attribute("name")
                .context("missing test name")?
                .to_owned(),
            failure: children
                .first()
                .map(|n| {
                    n.attribute("type")
                        .context("missing failure type")
                        .map(str::to_owned)
                })
                .transpose()?,
        });
    }
    ensure!(!cases.is_empty(), "empty JUnit suite");
    Ok(cases)
}
