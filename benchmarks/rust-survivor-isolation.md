# Isolating the saved-percentage survivor

Follow-up to `1c59b5f` and the [unmutated adapter controls](rust-nested-self-tests.md).
The historical six-mutation self-run killed a previous survivor through unrelated
integration failures. This comparison isolates that mutation from the other five
and compares it with pristine source under the complete Cargo adapter.

## Candidate and controls

Only `src/score.rs` differs between the two source copies:

```diff
- (expected - actual).abs() <= 1e-12
+ (expected - actual).abs() < 1e-12
```

The original file SHA-256 is
`ee2d904134ac9918ffae4a3f1c9005321508227712252c5ed6c3d76e920cdda0`, matching the
historical saved mutation. Source-hash maps verify that no other file differs.
Worker 0 runs pristine code; worker 1 runs the candidate. Each owns independent
Cargo artifacts. Both run concurrently in three cases: fresh artifacts, an empty
worker cache, and a second execution using that same worker's cache. Package
cleanup still forces local recompilation before the cached executions.

The ignored `rust::adapter_probe::paired_cargo_adapter` probe shares the existing
adapter matrix implementation but selects two workers only. It accepts isolated
`project-0`/`project-1` directories under `MUTIE_REPRO_ROOT`, with an empty `logs`
directory, and retains each execution's stdout/stderr and result JSON. The setup
and invocation otherwise match the unmutated probe. It does not apply mutations
itself or alter the working checkout. Default tests ignore both probes.

The focused comparison selects only the seven cancellation tests through the
temporary manifests. The full comparison retains the normal manifests and all
198 default tests. Both comparisons use frozen copies from before the new
boundary regression described below. Existing test deadlines are unchanged;
the adapter's outer command bound is 300 seconds. A watcher retains process and
fixture state and attempts a macOS stack sample for test-owned CLIs alive for eight
seconds, before the readiness deadline.

## Focused results

All six focused executions passed all seven cancellation tests, with no timeout,
capture error, interruption or watcher trigger. Source hashes and caller artifact
sentinels were preserved, and owned temporary storage was empty on completion.

| Artifact mode | Pristine | Candidate |
| --- | ---: | ---: |
| Fresh | 54.33 s | 54.81 s |
| Worker cache, initially empty | 53.42 s | 52.99 s |
| Same worker cache, second execution | 17.13 s | 18.31 s |

These totals include compilation and execution. They are one diagnostic sample,
not an estimate of stable speedup.

## Stronger boundary test

The existing saved-score tests exercised JSON round-trips but not the tolerance
endpoint itself. `saved_score_tolerance_includes_endpoint_but_not_next_float`
uses an expected percentage of zero so subtraction introduces no ambiguity. It
checks the representable float just below `1e-12`, the exact tolerance, and the
next representable float above it. The first two must match; the third must not.
This adds coverage for the intended inclusive boundary without changing score
calculation, saved-report validation behavior, or the tolerance value.


The isolated stronger-test check passed on pristine code, failed at exactly
`1e-12` with only the comparison mutation, and passed after restoration (44.89,
13.11 and 12.83 seconds including compilation). Input hashes matched afterward
and owned temporary storage was empty. This establishes an actionable test gap;
it does not validate the earlier unrelated kill as a useful finding.

## Full comparison and remaining capture failure

| Artifact mode | Pristine | Candidate |
| --- | --- | --- |
| Fresh | Passed, 190.32 s | Passed, 190.62 s |
| Worker cache, initially empty | Failed, 183.31 s | Passed, 184.38 s |
| Same worker cache, second execution | Not run after failure | Passed, 98.47 s |

The pristine cold-cache run failed only
`process::tests::concurrent_captures_reach_eof_after_their_own_process_groups_exit`:
command 9 reported capture EOF timeout. All integration suites passed. Thus this
failure does not require the score mutation. The other four executions passed
all 198 tests. All source hashes and caller artifacts were preserved and owned
temporary storage was empty. The watcher observed a CLI during the passing fresh
terminal suite; it retained process/fixture state but no stack sample before the
process exited. That observation does not explain the later unit failure.

Temporary pipe diagnostics reproduced two failures in 30 isolated test runs
(two concurrent test processes, each stopping at its first failure). Both failed
commands' groups had already disappeared. In one occurrence command 55's stderr
reader was fd 251, pipe `0x561631826ed22d3`. The unrelated command 52 shell held
both fd 251 and its writer fd 252 (`0xf262a7d9ec9a9aee`). This directly establishes
cross-command descriptor inheritance, distinct from the surviving-group case
in the earlier investigation.

`process::run_inner` now holds a shared mutex only around `Command::spawn`,
covering pipe creation and close-on-exec setup. Execution, capture draining and
cleanup remain concurrent. The group retry stays in place: serializing spawn
alone did not fix the earlier racing-fork failure. The existing 64-command
regression passed 100/100 runs after combining the fixes, with two test processes
at once. Temporary diagnostics were removed. Deadlines and capture bounds are
unchanged. This is stress evidence, not proof against every interleaving.

Local artifacts (not committed):

- `/tmp/mutie-survivor-focused-hvl8ika4`
- `/tmp/mutie-survivor-full-jlqukwn6`
- `/tmp/mutie-score-boundary-9gy6p5yl`
- `/tmp/mutie-eof-diagnostics`
- `/tmp/mutie-eof-spawn-fixed`

The full candidate comparison above predates the spawn fix and stronger test.
A subsequent unmutated comparison includes both changes in both source copies.
All six complete adapter executions passed all 199 tests:

| Artifact mode | Worker 0 | Worker 1 |
| --- | ---: | ---: |
| Fresh | 192.11 s | 191.64 s |
| Worker cache, initially empty | 188.82 s | 189.29 s |
| Same worker cache, second execution | 147.98 s | 147.24 s |

There were no command timeouts, interruptions or capture errors. Both source
hash maps matched, caller artifact sentinels were preserved, and owned temporary
storage was empty afterward. The watcher triggered for two CLIs during otherwise
passing suites; these observations are not test failures. Artifacts are under
`/tmp/mutie-spawn-fixed-full-ie72p2vx`. This is one successful matrix,
not a guarantee against intermittent failures or a completed mutation self-run.
Next validate the strengthened score test through a saved mutation rerun before
repeating the broader six-mutation comparison.


Default-suite validation after the fix: `cargo test --locked` passed all 199
tests (25 ignored diagnostics/real workflows). `cargo fmt --check`,
`git diff --check`, and `cargo clippy --locked --all-targets -- -D warnings` passed.


## Saved-ID rerun with the stronger test

The follow-up to `c26fd16` moves the exact-tolerance regression to the
`report_reader` test module alongside the saved-percentage presence regressions.
Its assertions are unchanged. Keeping stronger tests outside `src/score.rs`
restores that file's exact historical fingerprint, allowing ordinary strict
rerun matching without weakening source identity checks or editing saved reports.

The available source report is the earlier reliability self-run,
`/tmp/mutie-reliability-self-_pvnl_6r/reports/run-1791942980-cTkOCu`.
It recorded source ID 5 as killed through unrelated failures, so this validation
uses `--rerun SOURCE --id 5`, not missed-only selection. The list preview selects
exactly that tolerance mutation as new ID 1. The older report remains historical
evidence; a useful new kill must name the intended boundary assertion.

The validation uses an isolated copy of current source, one worker, worker-local
Cargo dependency caching, a 300-second baseline limit, a 600-second mutation
limit, and a 100% minimum score for this selection. The baseline still builds
fresh; the single mutation starts with an empty worker cache. Both execute the
complete default suite. Input and source-report hashes, caller target/build
sentinels, and owned temporary-directory cleanup are checked after execution.

```sh
target/debug/mutie "$PROJECT" --lang rust --rerun "$SOURCE" --id 5 \
  --include src/score.rs --operator comparison_boundary --list

target/debug/mutie "$PROJECT" --lang rust --rerun "$SOURCE" --id 5 \
  --include src/score.rs --operator comparison_boundary \
  --rust-build-cache worker --jobs 1 --baseline-timeout 300 --timeout 600 \
  --min-score 100 --ui plain --output-dir "$REPORTS"
```


The saved rerun completed in **256.60 seconds**. Its fresh baseline passed all
199 default tests in 127.63 seconds. The one selected mutation was **killed**,
with zero missed, timeouts, build failures or errors, and the selection's 100%
minimum-score policy passed. The only failing test was
`report_reader::tests::saved_score_tolerance_includes_endpoint_but_not_next_float`;
all integration tests passed. This verifies the intended finding through Mutie's
normal rerun and classification path, rather than the historical unrelated kill.

All copied input files and historical report files retained their hashes. Caller
target/build directories contained only their original sentinels, and owned
temporary storage was empty. The new report passed the strict retention-preview
reader without deletion. The relocated test also passed independently; formatting,
diff checks and Clippy passed. The full unmutated suite was exercised by the
rerun baseline rather than repeated separately.

Local evidence: `/tmp/mutie-score-saved-rerun-fta404yp`, with report
`reports/run-1792007668-nrpQEy`, `preview.log`, `command.json`, `results.json`,
input/source-report hash maps, and `read.log`. The remaining reliability check is
the broader six-mutation comparison under parallel workers; this focused success
does not establish that result.


The subsequent [parallel six-mutation comparison](rust-parallel-score-reruns.md)
passes in both fresh and worker-cache modes, with six intended kills and matching
failure sets. It completes the broader follow-up described above.
