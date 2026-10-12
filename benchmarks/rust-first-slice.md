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

All passed. Both Cargo tests stay ignored by default to prevent recursive
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
tampering cases but no `null` percentage case. A useful next regression is to
reject a saved score with a missing percentage when completed outcomes require
one, plus the reverse presence mismatch. That stronger-test run is still to do;
this slice does not claim to have closed the real-project coverage gap.

## Remaining limits

Fresh dependency compilation plus the full self-test suite makes even this
two-mutation sample expensive. Build reuse must prove correct invalidation,
restoration and parallel isolation before it replaces fresh artifacts. The
observed terminal-test instability under load also needs follow-up; a passing
baseline alone cannot establish that every later failure was caused by a mutation.

Rust focused reruns, dismissals, additional operators, broader Cargo layouts and
conditional-code discovery remain unsupported. Further execution checks include
mutation-stage cancellation with partial results, compilation timeouts, protocol/
tool failures and raw-capture limits. The fixture's stronger-test run does not
complete the real-project run/inspect/improve-tests/focused-rerun milestone.
