# Node storage-locking and workspace-profile review

Recorded 2026-10-18 on macOS, using Node 24.22.0 and the unchanged pinned runner
recipe. Project: [openclaw-cc](https://github.com/yukiharukonishi/openclaw-cc) at
`e323faa446f03b5438fa9edc58781b4ef8dd52df`. Source IDs refer to the unchanged
39-candidate run in [second-project acceptance](node-openclaw.md).

## Five useful survivors

The review copy adds [five tests](fixtures/node-openclaw/mutie-storage-profiles.test.js)
and an [independent lock-owner helper](fixtures/node-openclaw/mutie-lock-owner.js).
Every original production file, manifest and test remains unchanged. Earlier
poller boundary tests and the teardown patch are not included in this copy, so
this selection measures only the new tests.

| Source ID | Mutation | Missing observable boundary |
| --- | --- | --- |
| 28 | Storage `while (true)` becomes false | Callback must execute while owning a lock; a contender must wait for another process's fresh lock |
| 34 | Cron `daily: false` becomes true | Cron must exclude daily-memory contents even when the files exist |
| 35 | Cron `bootstrap: false` becomes true | Cron must exclude bootstrap contents and report `bootstrapFound: false` |
| 36 | Group `daily: true` becomes false | Group must load the two daily-memory files |
| 37 | Group `bootstrap: false` becomes true | Group must exclude bootstrap contents and report `bootstrapFound: false` |

These are five missing test boundaries for hypothetical regressions. The pristine
implementation passes; they are not five confirmed upstream production bugs.
None is equivalent within these observable cases.

## Test controls and limits

The first storage test reads the lock record inside a callback, checks the owner
PID, then checks release after success, after a thrown callback error, and on
reacquisition. The second launches a separate Node process that uses native
exclusive creation, without importing mutated production code. Its readiness
handshake establishes ownership before the contender starts. It releases on an
explicit request after 150ms; the callback checks both release ordering and its
own PID in the acquired lock. The ownership check also detects bypass if the
contender is delayed past the release timer. Child exit is awaited and checked;
cleanup kills and awaits a still-running child on assertion failure.

These tests cover one fresh lock, one owner and one contender on this host. They
do not establish stale-lock recovery, crash safety, fairness, multi-host/network
filesystem behavior or long-running critical-section safety. The implementation
has an explicit 30-second stale threshold; no stronger locking claim is inferred.

The profile tests seed every top-level context file with a distinct marker and
check both returned `loadedFiles` and rendered context, plus `bootstrapFound`.
Main is a positive control; cron and group enforce their separate inclusion and
exclusion rules. Daily files cover nearby local dates to tolerate midnight
rollover without mocking Date or test-runner internals. All inputs live in temporary
directories; no real user workspace or service is accessed.

## Acceptance and preservation

- Direct native execution: all five tests pass (about 0.38s).
- Isolated full-suite baseline: **67 passing tests**, **5.34s**.
- Saved IDs 28, 34, 35, 36, 37: **five kills, zero survivors, timeouts or errors**,
  **33.08s** saved elapsed time. Each saved failure log names the expected new test.
- [Machine-readable results](node-storage-profiles-results.json) preserve the
  exact source mapping, recipe, outcomes, times, score and added-file hashes.
- All **63 original files** and inventories are unchanged in original/review
  copies; review adds only the two named files. All **215 earlier Node report
  files** retain their hashes after the rerun. Saved recipe identity matches the
  current observer/reporter bytes; no runner or report version changed.

The 100% score applies only to these five selected survivors. The original
full report stays **22 killed, 15 missed, two timeouts**, with its **59.46%** raw
score. Across the separate follow-ups, eight of those survivors have useful
stronger-test kills and both timeouts complete after the test-only teardown fix.
Those results are not a new full-suite score with all improvements combined.
Seven original survivors remain unreviewed: IDs **10, 12, 13, 15, 16, 18 and 21**
(scheduling, delivery and the unimported entry point). The separate SRI cleanup
guard remains unresolved and its pending-promise mutation remains a timeout.

Reproduce with the audited Node on PATH and a checkout of the pinned commit:

```sh
# Copy both retained files into REVIEW/test/ without editing original files.
node --test REVIEW/test/mutie-storage-profiles.test.js
mutie REVIEW --lang node --ui plain --timeout 15 --baseline-timeout 30 \
  --rerun ORIGINAL_RUN --id 28,34,35,36,37
```

Formatting and diff checks pass. This is test/evidence work; no Rust runtime code
changed, so the default Rust suite was not repeated. The earlier fixture and
failure-classification regression results below apply to the same runner recipe.

## Scoped milestone decision

The Node JavaScript slice now meets **Validated**, as defined in [SUPPORT.md](../SUPPORT.md).
This changes the maturity assessment, not the accepted package/runtime scope.

| Required gate | Evidence |
| --- | --- |
| Real run → inspect → improve tests → rerun | SRI original/stronger runs and two rounds of useful `openclaw-cc` survivor tests, with timings |
| Saved reports, source identity, review and CI policies | `tests/node_workflow.rs` report/rerun/dismissal/policy acceptance; [initial record](node-initial-scope.md) |
| Failure attribution and lifecycle boundaries | Exact body evidence plus 42 mutation-triggered cases, including mixed/late failures; [attribution audit](node-body-attribution.md) |
| Timeout, cancellation, capture and descendant cleanup | Ignored native adapter probe and process regressions recorded in the initial and attribution audits |
| Preserved sources and saved artifacts | Hash/inventory checks in both real projects and the latest 215-file report check |
| Runtime and useful findings | 21-candidate SRI and 39-candidate `openclaw-cc` scans; selected reruns at 18–33 seconds; explicit surviving/timeout limits |

The established gate is not met: two small dependency-free packages on one audited
runtime/platform do not establish broad project compatibility or comparative
performance. Native TypeScript, dependencies, other layouts, dynamic imports,
other Node versions and parallel workers remain separate acceptance work. Open
findings do not invalidate a mutation runner's workflow; they remain visible for
review rather than being dismissed to reach a milestone. Choose the next major
scope explicitly; further operator/UI work is not required by this milestone.
