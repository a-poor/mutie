# Bun body-failure attribution prototype

Recorded 2026-10-18, pinned Bun **1.3.11+af24e281e**, macOS arm64. This follows the
[initial reporting audit](bun-initial-scope.md). The deliverable is an experimental
observer, conservative classifier and reproducible paired probes. **This is not
a production adapter: Bun remains Planned and `--lang bun` is not implemented.**

## Positive evidence and conservative rejection

[`observer.mjs`](fixtures/bun-audit/observer.mjs) preloads a `bun:test` module wrapper.
It copies the original bindings before installing the mock, because Bun's
[module mocks update existing ESM bindings](https://bun.sh/docs/test/mocks).
Native test modifiers require their original receiver; the wrapper preserves
that receiver while marking modifiers unsupported. Native return/exit behavior
is checked against an unobserved control in every retained case.

The observer writes synchronous, sequenced records to a separate file. Each test
registration receives an ID; each invoked body/hook receives another ID. Positive
failure evidence is a caught exception or rejected native Promise from the body
callback, followed by a completed failure checkpoint. Hook failures are separately
recorded and always reject classification. Callback-style tests, non-native object
returns, registration overloads, test modifiers and mocking/configuration APIs are
unsupported in this prototype and retain native execution while disqualifying
classification. It does not inspect arbitrary `then` getters.

The [file classifier](fixtures/bun-audit/classify-file.py) requires complete JUnit
structure/counts, matching observed registrations/invocations/names, completed
bodies/hooks, compatible exit status and positive evidence for every failed body.
It rejects provisional-only failures, missing/truncated records, malformed reports,
explicit process exit, skipped/incomplete results, hook failures and assertion-plan
failures without body evidence. A native TimeoutError paired with an incomplete
body is a timeout, never a body failure.

Full stderr is used only as **negative** evidence: the pinned runner's `Unhandled
error between tests` diagnostic rejects a run. It never supplies a positive kill
or completion signal. A test printing that text can conservatively cause an error.
No log truncation is used in these probes; bounded-capture production integration
remains to be implemented.

## Why the failure checkpoint is necessary

A body that both calls `Promise.reject(...)` and throws synchronously can cause
Bun to exit before reporting the pending rejection. JUnit and the first observer
prototype would see only the caught body exception. Separate-error and shared-Error
controls reproduce this behavior in both JS and TS.

The final observer treats the caught exception as provisional, then waits for a
zero-delay event-loop checkpoint before confirming it. If Bun instead terminates
the test for the pending rejection, the observer never confirms the original body
failure, and the classifier returns an error. Reusing the same Error object cannot
bypass this check. Timer/process-error mixtures and async cleanup also reject.

The checkpoint runs **only after an actual body failure**. An earlier experiment
also delayed successful callbacks and changed timer ordering in hooks. That was
rejected. The retained controls verify ordinary timer/microtask ordering, including
beforeEach scheduling, while successful callbacks retain their native return path.
Failure paths can expose additional asynchronous errors and become conservative
errors; the checkpoint is not a general proof that arbitrary background work has
quiesced. Native Promise observation also remains an instrumentation boundary that
needs regression coverage as scope expands.

## Inspector transport decision

The first approach paired observer records with inspector events. Repeated runs
lost terminal events during Bun shutdown over both WebSocket and the pinned
length-prefixed Unix transport, even after startup waiting and controller-side
drain. `LifecycleReporter.preventExit` did not provide a usable completion barrier
for `bun test`; its pinned command writes JUnit and calls VM global exit. The
in-process `node:inspector` Session supports profiling, not TestReporter.

The [pinned test command](https://github.com/oven-sh/bun/blob/bun-v1.3.11/src/cli/test_command.zig),
[debugger transport](https://github.com/oven-sh/bun/blob/bun-v1.3.11/src/js/internal/debugger.ts)
and [inspector shim](https://github.com/oven-sh/bun/blob/bun-v1.3.11/src/js/node/inspector.ts)
were inspected. The [Unix diagnostic controller](fixtures/bun-audit/inspect-socket.mjs)
is retained for investigation; it is not used by the accepted file-backed probes.
Incomplete inspector captures were rejected, not repaired by guessing outcomes.

## Paired JS/TS verification

[Machine-readable evidence](bun-body-attribution-results.json) retains the source,
control and observed captures, observer events, classifications and recipe-file
hashes for **56 scenarios per syntax**, **112 cases / 224 Bun invocations**:

| Experimental classification | JS | TS | Total |
| --- | ---: | ---: | ---: |
| Positive body failure | 10 | 10 | 20 |
| Passed | 8 | 8 | 16 |
| Native timeout | 1 | 1 | 2 |
| Conservative error / unsupported | 37 | 37 | 74 |

Every native exit status matches its control. Coverage includes synchronous and
async body failures, assertion failures, primitive throws, clean hooks, nested
scopes, aliases, duplicate names, shared Error objects, mixed body/process failures,
all four hook phases, async/nested cleanup, plans, imports, early exit, modifiers,
callback/thenable exclusions and scheduling controls. The replay verifier adds
**30 corrupted/insufficient-evidence checks**, including missing checkpoints,
registration mismatch, truncation, count mismatch, phase mismatch, bad identities,
wrong exits and injected negative diagnostics.

These labels are experimental classifications, not saved Mutie mutation outcomes
or scores. Exact source/file inventory checks, package/config validation and
resource cleanup are still required before production acceptance.

## Unchanged Composer and stronger-test control

[Composer results](bun-observer-composer-results.json) retain source hashes,
control/observed counts, durations, the failure trace and a hash/location for the
full local evidence. The project remains at
`3de15b846e3f5ef46cf8aebd5d136908baa43b54` with all **35 original files preserved**.
Mutations and the added test run only in temporary copies.

| Run | Native / observed tests | Native / observed exit | Classification |
| --- | --- | --- | --- |
| Original baseline | 272 / 272 pass | 0 / 0 | Passed |
| Name-configurability flip | 272 / 272 pass | 0 / 0 | Passed |
| Stronger-test baseline | 273 / 273 pass | 0 / 0 | Passed |
| Same flip with stronger test | 272 pass, 1 fail / same | 1 / 1 | Body failure |

The observed commands took approximately 0.57–0.63 seconds in this batch. These
are acceptance timings, not a controlled performance claim. The stronger test is
unchanged from the initial audit; source, original tests and snapshots are preserved.
No dependencies were installed and no compiler typecheck was run.

## Reproduce and next gate

These probes require no inspector sockets or Node controller:

```sh
python3 benchmarks/fixtures/bun-audit/observe.py \
  --bun /absolute/path/to/bun --out /tmp/bun-observer.json
python3 benchmarks/fixtures/bun-audit/verify-observer.py /tmp/bun-observer.json
python3 benchmarks/fixtures/bun-audit/observe-project.py \
  --bun /absolute/path/to/bun --project /path/to/composer \
  --out /tmp/bun-observed-composer.json
```

The next implementation gate is to port this bounded evidence protocol into the
Rust adapter with strict package/import/config validation, complete source/test-file
accounting, execution-identity hashing, bounded capture and process cleanup. Only
then connect shared reports/reruns/policies and perform scored real-project
acceptance. No existing runner behavior, recipe or project version changes here.

## Rust reader follow-up

The [Rust protocol replay](bun-rust-protocol.md) ports this classifier and adds
strict bounded parsing plus an independent expected-file inventory. All 112
recorded cases and four full Composer records agree. The reader is staged through
an integration test; isolated Bun execution and scored CLI support remain pending.
