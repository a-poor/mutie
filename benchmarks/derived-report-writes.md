# Avoiding unchanged derived report writes

Comparison on 2026-10-14 against `764c256`. The implementation changes only
publication of derived `summary.txt` and outcome lists. It compares the generated
bytes with the current regular file, skips identical contents, and otherwise uses
the existing temporary-file/write/sync/atomic-replacement path. Missing or edited
files are repaired; symlinks are replaced rather than treated as published files.
Metadata/read failures are surfaced. There is no in-memory publication cache.

Authoritative `run.json` and `outcomes.json` still publish on every checkpoint,
with matching checkpoint numbers. Inventory, diffs, logs, report format, mutation
selection, Cargo rebuild rules and process cleanup are unchanged. No versions or
timeouts were changed.

## Controlled test execution

The control is an isolated source copy of `764c256`; the candidate is that revision
plus the report change and two regression tests. Both were built before measurement
with `cargo test --locked --tests --no-run --message-format=json`. The executable
artifacts identify all 13 test harnesses, including the ignored-only Rust workflow
harness. Execute every harness directly, in the same sorted target order, without
test filters or thread overrides. Each suite gets its own temporary parent.

Run one complete control suite, then one candidate suite. Next run two candidate
suites concurrently, followed by two control suites concurrently. Default libtest
concurrency is retained within each harness; no other benchmark/build is deliberately
started during measurement. This is one small matrix on macOS arm64 with Rust/Cargo 1.99.0, not a randomized performance distribution.
External host load is uncontrolled. New code adds two unit tests to the candidate.

The measurements exclude Cargo startup, compilation, workspace copying and
mutation execution. They directly measure the full default test workload that
dominated earlier warm Cargo commands. They do not establish the same percentage
improvement for complete mutation runs or unrelated target projects.

Local evidence root: `/tmp/mutie-derived-report-osc7ae9u`. Build artifact
records are `control-build.jsonl` and `candidate-build.jsonl`; each labeled suite
directory contains per-target logs and `result.json`. The measurement driver is
`/tmp/mutie-derived-report-compare.py`.

All six suites passed. Controls passed 200 tests each; candidates passed 202 each,
including the two new regressions. Each suite ignored the same 27 opt-in tests.
Every original passing test name matched, and all six owned temporary parents
were empty afterward. [Per-target data](derived-report-writes.json) preserves
all timings and counts.

| Execution load | Control suite time | Candidate suite time | Observed reduction |
| --- | --- | --- | --- |
| One suite | 85.38 s | 55.06 s | 35.5% |
| Two concurrent suites, individual times | 121.08, 122.07 s | 81.74, 82.07 s | 32.6% by mean |

The dismissal harness fell from 19.35 to 11.38 seconds in the serial comparison,
and from 30.51–31.76 to 20.29–21.13 seconds in paired suites. Terminal tests fell
from 18.89 to 14.26 seconds serially; log-limit tests fell from 11.67 to 7.95 seconds.
These are measured samples, not guaranteed runtime improvements. The candidate
unit harness includes extra tests, and the serial/pair ordering differs as above.

## Actual report-write reduction

The [workflow probe](rust-workflow-costs.md) previously counted 4,347 report writes
and syncs in each full dismissal harness, with 524 flushes, 182 commands and 131
CLI invocations. Applying the same diagnostic to an isolated candidate copy gives
**2,024 writes/syncs**, retaining all 524 flushes, 182 commands and 131 invocations.
All 15 dismissal tests pass. This removes 2,323 writes, about 53.4%, without omitting
checkpoints or test work. Counts include initial/inventory writes as well as
checkpoints; summary content can vary with elapsed-time formatting.

Evidence: `/tmp/mutie-workflow-profile-wnr6acxr/counts.json` and
`logs/counts/`. This diagnostic ran alongside real-Cargo validation, so its timings
are not used for performance claims; only event counts are compared. The runtime
table above uses uninstrumented executables with no competing benchmark builds.

## Regression coverage

The new saved-report tests keep open file handles to distinguish unchanged files
from atomic replacements without relying on timestamps. They check that unchanged
lists retain their file, authoritative files are replaced with advancing matching
checkpoints, and removed lists are recreated. They also exercise changed content,
same-length external edits, symlinks, failed publication to a directory, and a
successful retry after the obstruction is removed.

Existing report tests check inventory publication, completed/interrupted records,
text output and reader parity. The complete default suite also covers report
browser/retention behavior, dismissal policies, focused reruns, cancellation,
captures, terminal cleanup and source preservation.

Additional checks passed:

- `cargo test --locked --bin mutie saved_report::tests`: all five report tests.
- `cargo test --locked --test rust_workflow -- --ignored --test-threads=1`:
  all 17 real-Cargo workflows, 288.54 seconds. Covers fresh/cache rebuilding,
  stronger-test reruns, dismissals/policies, failure classification, cancellation,
  partial reports and source/artifact preservation.
- `cargo test --locked --test go_workflow go_workflow -- --ignored --exact`:
  real-Go fixture workflow passed, 25.99 seconds.
- `cargo clippy --locked --all-targets -- -D warnings`, formatting and diff checks.

Cargo/Go validation logs are retained in the comparison evidence root. These
checks overlapped diagnostic compilation/counting and, briefly, each other; their
durations are validation records, not performance comparison samples. No test
failed or needed a retry.

## End-to-end follow-up

The [saved six-score-mutation comparison](rust-report-write-reruns.md) now completes
the end-to-end acceptance check on Mutie. It retains exact failure sets, package
rebuild evidence, source/report hashes and caller-artifact preservation. Its
observed improvement is recorded separately from these prebuilt-suite samples.
