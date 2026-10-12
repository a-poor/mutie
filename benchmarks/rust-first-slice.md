# First Rust execution slice — 2026-10-11

This records validation of the initial Cargo implementation, following the
[scope audit](rust-initial-scope.md). The selected real project is **Mutie itself**.
The broader Rust milestone remains incomplete; this is evidence for one binary
package, default features and `boolean_flip`, not general Cargo support.

The runs used the working implementation on top of `8d6e515`, Rust/Cargo 1.99.0,
and macOS arm64. The root lockfile fingerprint was
`sha256:96fb4a4b0d628f3281d4ae57e1c5f813c16072dd9c76147cb466a44d1ef4ff1d`.
Registry dependencies were already downloaded. Every baseline and mutation used
fresh owned build/target directories; compiled artifacts were not shared.
Source snapshots and worker resets used the existing copy machinery.

## Fixture and regression checks

The ordinary CLI discovers exactly three mutations in the
[Rust fixture](../tests/fixtures/rust/README.md):

| Function | Change | Outcome |
| --- | --- | --- |
| `unit_flag` | `true` → `false` | Killed by the unit assertion |
| `cli_flag` | `false` → `true` | Killed by the CLI output assertion |
| `untested_flag` | `true` → `false` | Missed; the original test checks only successful exit |

The real-Cargo integration tests repeat this with one and two workers. Adding
`assert_eq!(output.stdout, b"true\n")` to the smoke test in a temporary input copy,
then starting a fresh Mutie run, produces three kills with a passing baseline.
The checked-in fixture keeps its deliberate survivor.

```sh
cargo test --locked --test rust_workflow -- --ignored --test-threads=1
cargo test --locked --test go_workflow go_workflow -- --ignored --exact
cargo fmt --check
cargo clippy --all-targets --locked -- -D warnings
```

All passed. The Cargo tests stay ignored by default to prevent recursive
mutation runs when Mutie tests itself. Their cases cover listing without a
baseline/report, file filters, empty selections, score-policy failure, limited
logs, saved scope/versions, retention previews, failed build/test baselines, a
timed-out mutation, baseline SIGTERM, repeated/parallel runs and stronger tests.
They verify caller target/build directory sentinels and owned temporary-directory
cleanup, including failed and interrupted commands. Default unit tests separately
check compiler/test/tool evidence before log retention and source eligibility.

The full default suite passed inside the real-project baseline: **187 passed,
0 failed, 8 ignored** (123 unit tests and 64 integration tests). The opt-in Go
workflow also passed using the real Go toolchain. Formatting and strict Clippy
passed after the implementation and workflow-test changes.

## Terminal and saved-report checks

A real PTY run of the fixture with `--ui interactive --jobs 2` redrew the live
view, showed building/testing worker phases, finished with two kills and one
survivor, and restored its original termios. Its saved run took 31.57 seconds,
including a 9.91-second baseline, with no execution or workspace errors.

The saved Rust report also opened in a PTY browser. Source rows, diff/log views
and the unsupported-rerun explanation were inspected; quitting restored termios
and left report contents unchanged. The renderer regression checks that Rust
reports cannot export a Go rerun command. Real Rust reports passed the retention
preview's reader and layout validation without being deleted.

Local evidence paths (temporary artifacts, not repository dependencies):

- `/tmp/mutie-rust-live-smoke/run-1791767045-O4QzVS`
- `/tmp/mutie-rust-live-smoke.ansi`
- `/tmp/mutie-rust-browser-smoke.ansi`
- `/tmp/mutie-rust-final-workflow.log`
- `/tmp/mutie-rust-final-clippy.log`

## First real-project attempts

Both attempts selected the two eligible literals in `src/score.rs` and ran the
complete selected Cargo test suite for each mutation. The baseline and workers
used fresh artifacts. Other regression checks overlapped these attempts, so the
timings are observations under load, not a controlled performance comparison.

| Workers | Mutation limit | Baseline | Whole run | Reported outcomes |
| --- | --- | --- | --- | --- |
| 2 | 180 seconds | 134.89 s | 317.57 s | 2 timeouts |
| 2 | 600 seconds | 223.65 s | 449.97 s | 2 killed |

The first attempt reached test execution for both mutations but exhausted the
three-minute overall deadline. In the second attempt each mutation compiled in
about 87 seconds; total mutation commands took 204.34 and 222.82 seconds.
Source copying/setup/reset took only about 0.86 cumulative seconds in that run.

The first reported kill had direct score/report-validation assertion failures.
The second had only an unrelated terminal test failure: it expected CLI exit 3
but received 1. That test does not exercise the mutated score-matching branch.
Therefore the reported 100% score is **not reliable evidence that both changes
are covered**. The serial repeat below confirms that the second kill was noise.

The score source fingerprint still matched the original saved inventory after
execution. Source/artifact preservation is also covered by the fixture tests;
these self-runs do not claim that the developer's build directory was unchanged
while separate root Cargo checks were running.

Local saved runs:

- `/tmp/mutie-rust-self-first/run-1791766200-hZNHpi`
- `/tmp/mutie-rust-self-validated/run-1791766754-wP4Kmn`

## Serial repeat and first finding

Repeated through the ordinary CLI with one worker and no competing build/test
checks:

```sh
target/debug/mutie . --include src/score.rs --operator boolean_flip \
  --jobs 1 --timeout 600 --baseline-timeout 300 --ui plain \
  --output-dir /tmp/mutie-rust-self-serial
```

The baseline passed, followed by **one killed mutation and one survivor**, with
no timeouts, build failures or execution errors. This sample's score is 50%; it
is a filtered two-mutation result, not an overall score for Mutie's test suite.

| Phase | Fresh compilation | Whole command |
| --- | --- | --- |
| Baseline | 46.66 s | 135.73 s |
| `score.rs:183`, `true` → `false` | 53.88 s | 146.80 s — killed |
| `score.rs:187`, `false` → `true` | 50.11 s | 143.99 s — missed |

The complete run took **429.59 seconds** (about 7 minutes 10 seconds).
Workspace snapshot/setup/reset cost 0.409 cumulative seconds, all five copy
operations succeeded, and 1,500 files were reflinked. The original score source
and lockfile still matched the report's saved fingerprints after execution.
The saved report also passed the retention preview without deletion:
`/tmp/mutie-rust-self-serial/run-1791767255-Yqd2C2`.

The survivor changes `MutationScore::matches_saved` so a mismatch between
`Some(percentage)` and `None` can pass its initial comparison. All 187 default
tests still pass under that mutation, including the previously failing terminal
test. Inspection of the existing saved-score validation tests shows numeric
tampering cases but no `null` percentage case. This identified an actionable
coverage gap: reject a saved score with a missing percentage when completed
outcomes require one, plus the reverse presence mismatch. The follow-up below
closes that gap through a fresh mutation run.

## Stronger-test follow-up — 2026-10-11

Added two report-reader regressions on top of `b72a5d5` (`Implements first rust
slice`). They load saved reports and change only the percentage's presence:

- `saved_score_requires_percentage_for_scored_outcomes` checks both explicit
  `null` and an omitted field when completed outcomes require a score.
- `saved_score_rejects_percentage_for_an_interrupted_run` checks fabricated
  percentages of 0 and 100 while preserving the counts and unavailable reason.

Both first verify a valid report and then require the corrupt report to fail
score validation. The production score logic already implements this behavior;
the change adds coverage for it.

The targeted tests, `cargo fmt --check` and strict Clippy passed. Then repeated
the same two-mutation sample through the ordinary CLI, with one worker and no
competing builds/tests:

```sh
cargo run --locked -- . --include src/score.rs --operator boolean_flip \
  --jobs 1 --timeout 600 --baseline-timeout 300 --ui plain \
  --output-dir /tmp/mutie-rust-score-regression
```

The fresh baseline passed **189 tests**, with 8 opt-in tests ignored. Both
mutations were killed, with no missed mutations, timeouts, build failures or
execution errors. The former survivor failed **exactly the two new tests**;
all of its integration tests passed, including the terminal tests. This verifies
that the new assertions detected the change without the unrelated failure from
the earlier parallel attempt.

| Phase | Fresh compilation | Whole command |
| --- | --- | --- |
| Baseline | 21.29 s | 113.08 s |
| `score.rs:183`, `true` → `false` | 21.76 s | 111.49 s — killed |
| `score.rs:187`, `false` → `true` | 22.32 s | 119.37 s — killed |

The complete run took **346.28 seconds** (about 5 minutes 46 seconds), with
0.347 cumulative seconds spent copying/resetting workspaces. The inventory was
identical to the earlier serial run, and all 81 source, test and Cargo input
hashes matched their pre-run values. Saved-report listing accepted the completed
report. The build strategy still uses fresh artifacts; these timings are another
observation, not an optimization result.

The filtered sample improved from **one killed / one missed to two killed /
zero missed**. This establishes the real-project run/inspect/improve-tests/fresh-run
workflow. Focused Rust reruns remain unimplemented.

Local evidence:

- `/tmp/mutie-rust-score-regression/run-1791770817-gCQbph`
- `/tmp/mutie-rust-score-regression.log`
- `/tmp/mutie-rust-score-regression-inputs.json`

## Mutation-test cancellation follow-up

The opt-in `rust_mutation_cancellation_preserves_partial_results_and_cleans_processes`
regression uses real Cargo and the fixture in temporary copies. It adds a fourth
boolean flip and makes tests for the second/third mutations wait in instrumented
helpers. The baseline passes and the first mutation is killed. Readiness markers
and the saved checkpoint establish active test execution before the signal;
there is no timed guess about when compilation or testing should have finished.

| Workers | Signal | Capture | Completed | Interrupted | Pending |
| --- | --- | --- | --- | --- | --- |
| 1 | SIGTERM | Direct files, no raw limit | 1 killed | 1 | 2 |
| 2 | SIGINT | Drained pipes, 1 MiB raw limit | 1 killed | 2 | 1 |

Both cases exit 1 with an interrupted run and matching report checkpoints.
The regression verifies unchanged completed results/logs and inventory, merged
compilation/test diagnostics for interrupted mutations, no outcome or logs for
pending work, unevaluated finding/score policies, and no partial score percentage.
The text summary reports the same counts. A retention preview accepts each run
through the browser's shared strict reader and log/diff layout checks, leaving
the report unchanged.

Recorded Cargo, test-runner, shell-helper and descendant PIDs all stop. Their
workspace, target and test-temporary directories disappear; all input files and
caller target/build sentinels stay unchanged. Checks cover both capture paths,
but do not exercise raw-capture overflow. The checked-in fixture retains its
original three mutations. No production change was needed for these cases.

Validation: all three opt-in Rust workflow tests passed (56.75 seconds), along
with all seven existing cancellation tests. The new regression also passed on
its own inside the sandbox after replacing a blocked `ps` helper with a direct
`getpgrp` call. The test harness includes saved command-log tails on readiness
failure and bounded shutdown waits. Formatting, strict all-target Clippy and
`git diff --check` passed. The new test remains ignored by default to avoid
recursive mutation runs during self-testing.

## Compilation interruption and deadline follow-up

Three more opt-in tests in `tests/support/rust_compilation.rs` exercise real
Cargo and rustc through test-only PATH proxies. Normal commands delegate to the
installed tools. For stopped builds, a real rustc reads source from an open stdin
pipe while its proxy records readiness, PIDs, diagnostics and owned directories.
That supplies a live compiler checkpoint without depending on compilation speed.
The proxies and their state stay outside the input project; user compiler
overrides and project build scripts remain unsupported.

| Case | Expected saved result |
| --- | --- |
| SIGTERM during baseline compilation | Interrupted baseline/run; incomplete inventory |
| SIGINT during parallel mutation compilation | 1 killed, 2 interrupted, 1 pending |
| 5-second baseline compilation timeout | Failed run; timed-out baseline; no mutations |
| 10-second parallel mutation compilation timeouts | Completed run; 1 killed, 2 timeouts |
| 20-second shared compilation/test deadline | Completed run; 1 killed, 1 timeout, 1 missed |

Stopped builds retain their diagnostics and never enter the test phase. Partial
results, completed logs, pending work, policy precedence and report checkpoints
are checked. Retention previews accept the reports through the shared reader
without changing them. Recorded Cargo, compiler proxy, real rustc and helper
processes stop; owned workspace/build/test files are removed, and all source and
caller artifact files stay unchanged. Build cancellation uses bounded captures;
mutation build timeouts also cover direct file captures.

The deadline case completes a real build, holds its completion for eight more
seconds, then starts a real test that blocks. The mutation must time out within
20–23 seconds overall, rather than receive another 20 seconds for testing. A
deliberate reset of the test-phase budget made this regression fail at **28.52
seconds**. Restoring the existing shared deadline makes it pass. These checks
needed no production changes.

Validation: the full opt-in Rust workflow suite passed all **6 tests** in
**106.37 seconds** after restoring the shared deadline. All **7 existing
cancellation tests** passed, as did formatting (including the standalone proxy),
strict all-target Clippy and `git diff --check`. All three new tests remain
ignored by default to avoid recursive self-testing.

## Remaining limits

Fresh dependency compilation plus the full self-test suite makes even this
two-mutation sample expensive. Build reuse must prove correct invalidation,
restoration and parallel isolation before it replaces fresh artifacts. The
terminal follow-up in [`terminal-reliability.md`](terminal-reliability.md) fixes
reproduced descriptor-inheritance and test-watchdog defects and records parallel
stress checks. The original exit-only failure cannot be assigned a specific
cause; a passing baseline alone cannot establish that every later failure was
caused by a mutation.

Rust focused reruns, dismissals, additional operators, broader Cargo layouts and
conditional-code discovery remain unsupported. Further execution checks include
protocol/tool failures and raw-capture limits. Stronger tests have been verified
through fresh runs on both the fixture and Mutie; the full workflow including
focused reruns remains incomplete.
