# Rust saved-score reruns after derived-report write optimization

End-to-end follow-up to the [derived-report write comparison](derived-report-writes.md).
The optimization is committed as `bb2f190`; its production-code control is
`764c256`. This comparison uses the historical six-mutation score selection on
Mutie itself, rather than inferring mutation-run savings from prebuilt test suites.

## Controlled scope

Prepare two isolated copies from the same tracked `bb2f190` files. In the control
copy, restore only `src/saved_report.rs` from `764c256`. Before execution, verify
the path inventories match and that this is the only file whose hash differs.
This includes the two new regression tests in the candidate: its baseline should
pass 202 tests, versus 200 in the control. All other source/tests, manifests,
lockfiles and configuration are identical. In particular, the selected
`src/score.rs` fingerprint is unchanged.

Use separately built control/candidate CLIs, each with its corresponding source
copy. Run the control and then the candidate sequentially. Both use two workers,
worker-local dependency reuse, a fresh baseline, full default test scope, a
300-second baseline deadline, a 600-second mutation deadline and minimum score
100. Every mutation rebuilds local targets. Neither run filters tests, shares
worker artifacts, or changes a deadline to make the comparison pass.

The saved source report is
`/tmp/mutie-reliability-self-_pvnl_6r/reports/run-1791942980-cTkOCu`.
Its historical timeouts remain part of the record; the
[later verified reruns](rust-parallel-score-reruns.md) define the reviewed expected
failure sets for the same IDs. The new comparison requires those exact per-ID
sets, six kills, no other outcomes, and the expected baseline test count.

For each version, preview the selection, then run:

```sh
"$CLI" "$PROJECT" --lang rust --rerun "$SOURCE" \
  --include src/score.rs --operator comparison_boundary \
  --rust-build-cache worker --jobs 2 --baseline-timeout 300 --timeout 600 \
  --min-score 100 --ui plain --output-dir "$REPORTS"
```

Set `TMPDIR`/`TMP`/`TEMP` to a private owned parent and provide separate caller
`CARGO_TARGET_DIR`/`CARGO_BUILD_BUILD_DIR` sentinel directories. Hash input files
and the historical source report before and after; require owned storage to be
empty and caller sentinels untouched. Retention preview validates each finalized
report without deleting it. Record CLI hashes, command lines, full logs and Cargo
artifact freshness, and compare inventories/diffs independently of timing fields.

This is a small sequential comparison on macOS arm64
and Rust/Cargo 1.99.0. No other benchmark or build is deliberately started during
the pair, but external host load and order effects are uncontrolled. Per-mutation
timings include different failure paths and cold/warm cache states; they are not
interchangeable repetitions of one workload. A measured whole-run saving is a
sample, not a stable speedup guarantee for other projects.

Mutie is a particularly report-heavy test subject: its integration tests launch
many Mutie commands, so this optimization applies both to the outer mutation run
and to report writers inside the tested project. Other projects may benefit only
from the outer writer. Do not present the observed self-run improvement as a
general Cargo compilation or Rust test-runner speedup.

Local evidence root: `/tmp/mutie-report-score-pair-ixs4n8s2`, with separate
`control` and `candidate` projects, reports, logs and preservation snapshots.
The local driver is `/tmp/mutie-report-score-comparison.py`.

## Results and preservation

Both runs completed successfully with six killed, zero missed, zero timeouts,
zero build failures and zero errors. Both 100% minimum-score policies passed.
The control baseline passed all 200 default tests; the candidate passed all 202.
The inventories and all six diff files match exactly. Each mutation's failing-test
set matches both the other version and the reviewed historical reference.
Mutation 5 fails only
`report_reader::tests::saved_score_tolerance_includes_endpoint_but_not_next_float`.
No run needed a retry or a relaxed deadline.

| Measurement | Control | Candidate |
| --- | ---: | ---: |
| Whole run | 563.98 s | 393.12 s |
| Baseline command | 121.45 s | 91.01 s |
| Mutation 1, cold dependencies | 163.47 s | 121.11 s |
| Mutation 2, cold dependencies | 160.94 s | 121.99 s |
| Mutation 3, warm dependencies | 134.05 s | 87.60 s |
| Mutation 4, warm dependencies | 133.49 s | 87.28 s |
| Mutation 5, warm dependencies | 144.70 s | 91.19 s |
| Mutation 6, warm dependencies | 138.62 s | 89.33 s |

The observed whole-run reduction is **170.86 seconds (30.3%)**, from about 9.4
to 6.6 minutes. Worker commands overlap, so their durations must not be added to
calculate whole-run wall time. The separate outer wall clocks were 564.07/393.18
seconds, consistent with the saved run durations.

Every baseline and mutation rebuilt all 14 local targets. Each run's baseline
and first mutation per worker reused zero dependency artifact records; subsequent
mutations reused all 155 dependency records. This is worker-local dependency reuse,
not reuse of local package binaries or test results.

Saved captures contain all 13 harness summaries for every command. Summed baseline
harness duration fell from 81.84 to 56.03 seconds; warm mutation harness durations
fell from 115.19–120.01 to 68.92–73.48 seconds. Compilation also varied: the baseline
compile summary fell from 35.01 to 30.73 seconds, and warm mutation compilation
from 13.62–18.59 to 12.48–13.24 seconds. Do not attribute every millisecond of the
whole-run difference to report writes. The independent serial/parallel prebuilt
suite comparison supports the test-execution improvement without compilation.

All 159 input file hashes matched their own pre-run snapshots afterward. The
historical source report hashes matched, both caller-artifact sentinels were
untouched, and both owned temporary parents were empty. Both finalized reports
passed retention preview without deletion. Exact failure sets, CLI hashes, source
manifest digests, inventories, per-command timings and artifact freshness are
retained in the [comparison data](rust-report-write-reruns.json).

## Acceptance and limits

Accept the first focused-feedback optimization: reduced report writes preserve
the scoped workflow and improve this measured Mutie self-run. Combined evidence
includes the six prebuilt-suite executions, all 17 real-Cargo fixture workflows,
the real-Go workflow, regression tests, Clippy and this end-to-end pair.
No additional production changes were needed during the pair.

This is one sequential pair, not a repeated-run distribution or a universal
performance guarantee. Mutation timings cover different tests/failure paths and
must not be treated as identical replicates. General library/workspace support,
additional real Rust projects and other test runners remain separate milestones;
Rust's support level stays Validated within the existing binary-package scope.
