# Node body-exception attribution

Follow-up to [the initial Node slice](node-initial-scope.md), recorded 2026-10-18
on macOS with audited Node 24.22.0. JavaScript support remains Experimental;
TypeScript, installed dependencies, loaders and additional runtime versions are
not part of this change. No package/report/catalog version was bumped.

## Problem and attribution boundary

The original `sri-dynamic-loader` campaign encountered four non-assertion failures:
stylesheet timeout propagation (source ID 10), a TypeError (13), registry replacement
rejection (15), and unverified-load rejection (16). The old classifier deliberately
recorded these as execution errors and stopped the campaign. They were not four
missing tests: the existing tests already observed the changed behavior.

Accepting every `testCodeFailure` is insufficient. In the pinned native
`internal/test_runner/test` implementation, that label also covers plan failures
and other harness work outside the body. Diagnostics channels span hooks as well
as the body. We instead observe the test function invocation itself:

1. An arity-preserving Proxy records values escaping synchronous body calls,
   native Promise rejections, and error-first callbacks. It preserves Node's
   choice of callback versus return-value execution.
2. After native execution, the worker requires object/value identity between an
   observed exception and the native `testCodeFailure` cause. Messages, class names
   and error codes are insufficient. Failures produced outside the function have
   no matching body record.
3. Worker evidence exports file-local test IDs. Each reporter body failure must
   consume exactly one matching `(file, test ID)` record. Missing, duplicate,
   wrong-file, wrong-ID and leftover evidence are errors. Structural parent
   `subtestsFailed` events do not count as additional mutant kills.
4. Per-file/global completion, exit/result agreement, successful hook completion,
   absence of process-level errors, and existing timeout/cancellation/capture
   precedence are still required.

The existing exact native-source hash check remains mandatory. Hook `fail()` is
now observed before another invocation can reset shared hook error state. This
prevents concurrent successful cleanup from hiding a failed hook. Worker and
reporter bytes remain part of execution recipe identity.

## Late-failure regression discovered during the audit

A body failure plus a later unhandled exception initially produced a false kill:
Node can represent the extra error only as a top-level diagnostic when the file
is already failing. Failure counts and exit status can therefore agree while
omitting the second error. The same risk applies to errors thrown by exit listeners.

Worker observation now delegates `process.emit` and records
`uncaughtExceptionMonitor`, `unhandledRejection`, and throws from `exit` emission.
It adds no error handler, preserving listener counts and Node's default fatal
handling, return values and thrown values. Evidence is written after exit listeners
and on unhandled events; an earlier completion write cannot hide a later error.
The reporter also retains native diagnostic messages in bounded saved logs, so
the late error can be inspected. These signals prevent both survivor and kill
classification. They do not become
extra mutant outcomes.

This is instrumentation in a trusted test process, not a security sandbox or a
claim that arbitrary monkey-patching of the runner is supported.

## Explicit limits

- Dynamic imports are now rejected during project validation, even when their
  paths are literal and local. Their evaluation exceptions can escape through
  a test's returned Promise and otherwise resemble body exceptions. This is a
  deliberate narrowing of the experimental scope. Static module evaluation and
  parse failures remain errors, including mixtures with a body failure.
- Rejected non-native thenables remain unclassified. Native Promise and callback
  cases have explicit acceptance coverage; no generic asynchronous-error claim is
  inferred from them.
- Plan failures, suite-definition errors, hooks, mixed body/cleanup failures,
  callback-plus-Promise misuse, late activity and incomplete runs remain errors.
- Instrumentation changes execution identity. Old Node recipes are rejected;
  begin a fresh scan and use its saved IDs. Original reports remain unchanged.

## Regression evidence

`tests/node_workflow.rs` contains 37 mutation-triggered attribution cases plus five
cross-file/late mixtures, all with passing baselines. Positive cases include
assertions, TypeError/RangeError, custom numeric error codes, native Promise rejection, callback errors,
primitive throws, nested failures and successful hooks. Negative cases include
all hook phases, async/callback cleanup, distinct concurrent hooks, identical test
names, plans mixed with body failures, suite definition, module evaluation,
callback-plus-Promise misuse, non-native thenables, unhandled rejection, empty and
all-skipped files, early exits, late asynchronous activity and throwing exit listeners.
Errors persist zero kills and withhold scores; attribution does not depend on
retained log length.

Protocol unit regressions require exact file/test matching, reject missing or
duplicate/unused records and process-error flags, and prevent a second failure
with the same message from borrowing the first body's evidence. Project regressions
reject even a resolvable local dynamic import. Existing report/review/score-policy
and stronger-test rerun acceptance is retained, as are real timeout/cancellation/
capture-overflow tests with descendant and temporary-directory cleanup.

## Real-project acceptance

[Machine-readable results](node-body-results.json) record the final recipe,
source-ID outcomes, timings, scores and preservation checks for the original
`sri-dynamic-loader` commit. No production code or original test was changed.

| Final run | Results | Saved elapsed time |
| --- | --- | --- |
| Original 21-candidate full inventory | 12 killed, 8 missed, 1 timeout, 0 errors | 46.85s |
| Saved IDs 10, 13, 15, 16 | 4 killed, 0 errors | 10.24s |
| Eight survivors with the retained stronger tests | 7 killed, 1 missed, 0 errors | 21.03s |

The original 60-test baseline and the review copy's 65-test baseline pass. The
fresh full inventory exactly matches the old source inventory; its only outcome
changes are the four old errors becoming kills. The full raw boolean-mutation
score is 60% (12/20), with one timeout excluded. The focused 100% and stronger-test
87.50% scores apply only to their saved selections. These are not performance
comparisons.

An earlier investigation run overlapped compilation and produced an additional
5-second timeout on source ID 7, formerly killed. Repeating the full inventory
without concurrent builds restored that kill with the same 5-second deadline.
Source ID 8 remains a timeout; the cleanup-guard survivor remains source ID 9.
Those findings were not reclassified to make the attribution change look better.

All 32 original files, plus the expected file inventories, remain unchanged in
both the input and review checkouts. All 271 previously captured historical report
files and 97 files from the fresh full report retain their hashes after the reruns.
An old-recipe rerun is rejected before baseline execution; no identity check was
bypassed to reuse old selections.

Commands used (audited Node on PATH):

```sh
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt --check
cargo test --test node_workflow -- --include-ignored --test-threads=1
cargo test --bin mutie node::tests::timeout_cancellation_capture_and_owned_files -- --ignored
mutie /path/to/sri-dynamic-loader --lang node --ui plain --timeout 5
mutie /path/to/sri-dynamic-loader --lang node --rerun NEW_RUN --id 10,13,15,16
```

Validation: the default suite passed 244 tests (46 ignored). Final Node workflow
acceptance passed all four tests, including 42 mutation-triggered attribution
cases and the saved-log diagnostic check. The four Node project/protocol unit
tests and real timeout/cancellation/capture cleanup probe passed; strict Clippy
and formatting checks passed. Real runner tests remain opt-in to avoid recursive
self-mutation.
