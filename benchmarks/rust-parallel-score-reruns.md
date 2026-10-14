# Parallel score reruns after capture fixes

Follow-up to `c26fd16` and the [saved survivor isolation](rust-survivor-isolation.md).
The stronger tolerance test now lives in `src/report_reader.rs`, preserving the
historical `src/score.rs` fingerprint for strict saved-ID reruns.

## Setup

Two sequential runs use byte-identical snapshots of all 138 input files, the
same six saved comparison-boundary mutations, and two workers. Fresh artifacts
are the reference; the second run uses opt-in worker-local dependency caches.
Both baselines build fresh. Neither run changes deadlines, skips default tests,
or shares worker artifacts with the baseline or another worker.

The source report is
`/tmp/mutie-reliability-self-_pvnl_6r/reports/run-1791942980-cTkOCu`.
That historical run recorded four timeouts and two kills through unrelated
failures. It remains unchanged. Both new previews select source IDs 1–6 as
new IDs 1–6, with identical inventories. Commands use the current built CLI:

```sh
target/debug/mutie "$PROJECT" --lang rust --rerun "$SOURCE" \
  --include src/score.rs --operator comparison_boundary \
  --rust-build-cache fresh --jobs 2 --baseline-timeout 300 --timeout 600 \
  --min-score 100 --ui plain --output-dir "$FRESH_REPORTS"

# Repeat against an identical isolated copy, with separate caller sentinels.
target/debug/mutie "$PROJECT" --lang rust --rerun "$SOURCE" \
  --include src/score.rs --operator comparison_boundary \
  --rust-build-cache worker --jobs 2 --baseline-timeout 300 --timeout 600 \
  --min-score 100 --ui plain --output-dir "$WORKER_REPORTS"
```

## Outcomes and failure review

Both baselines passed all **199 default tests**. Both runs produced **six killed,
zero missed, zero timeouts, zero build failures and zero errors**. Both 100%
minimum-score policies passed for the rerun selection only. Each mutation has
exactly the same failing-test set in fresh and worker-cache modes.

| Source ID | Boundary | Observed failure meaning |
| --- | --- | --- |
| 1 | Canonical percentage tolerance, `>` → `>=` | Rejects valid zero; parsing, configuration and score-policy assertions fail. |
| 2 | Percentage parser decimal length | Rejects valid two-place decimals such as 50.01; score/report/browser fixtures and CLI policy assertions fail. |
| 3 | Unfinished count, `> 0` → `>= 0` | Completed scores become unavailable; score, report and policy assertions fail. |
| 4 | Error count, `> 0` → `>= 0` | Error-free scores become unavailable; score, report and policy assertions fail. |
| 5 | Saved tolerance, `<=` → `<` | Only the new exact-tolerance regression fails. |
| 6 | Minimum threshold, `>=` → `>` | Equality incorrectly fails the score policy; ratio and CLI exit assertions fail. |

The terminal score-policy test fails under IDs 3 and 4 because the command exits
0 instead of the required policy-failure code 3. Its sessions finish normally;
this is an intended consequence of the mutation, not a readiness/cleanup stall.
Cancellation, capture, retention-lock and unrelated terminal tests do not fail.
For ID 5, every integration test passes and the only failed test is
`report_reader::tests::saved_score_tolerance_includes_endpoint_but_not_next_float`.

The initial external verification script accepted only score/report-reader test
names and rejected the fresh run's legitimate configuration/integration failures
after recording a successful CLI result. Their diagnostics were reviewed as
above; final comparison checks exact per-ID failure sets rather than that narrow
module-name filter. This did not require rerunning or changing any mutation.

## Runtime and artifact evidence

| Measurement | Fresh | Worker cache |
| --- | ---: | ---: |
| Whole run | 686.16 s | 608.64 s |
| Baseline command | 138.16 s | 120.66 s |
| Mutation 1 | 192.17 s | 175.15 s |
| Mutation 2 | 190.35 s | 174.39 s |
| Mutation 3 | 173.30 s | 153.84 s |
| Mutation 4 | 170.56 s | 152.67 s |
| Mutation 5 | 180.95 s | 156.96 s |
| Mutation 6 | 178.10 s | 154.39 s |
| Cumulative mutation compilation | about 345.32 s | about 176.11 s |

Cargo's compilation-phase artifact records show all **14 local targets rebuilt**
for each baseline and mutation. Fresh mode rebuilds all 155 dependency records
for every mutation. Each cached worker's first mutation also starts cold; its
next two reuse all 155 dependency records while rebuilding local targets.
Warm compilation takes 17.12–19.48 seconds versus 51.82–66 seconds for fresh
mutations. Artifact counts refer to the initial compilation command, before the
subsequent test invocation emits its already-built artifact records.

The whole-run difference is 77.53 seconds, including 17.50 seconds of baseline
variation. This is one sequential pair on one macOS arm64 host with Rust/Cargo
1.99.0, not a stable speedup estimate. The full test suite still dominates warm
mutation time. Keep fresh artifacts as the default and worker reuse opt-in.

All input hashes and source-report hashes matched after both runs. Caller
artifact directories retained only their original sentinels, and both owned
temporary directories were empty. Both new reports passed strict retention
preview without deletion.

## Evidence and scope

Temporary artifacts, not repository dependencies:

- Fresh: `/tmp/mutie-score-six-fresh-z6bff8rf`, report
  `reports/run-1792008031-MvNsfq`.
- Worker cache: `/tmp/mutie-score-six-worker-qqblrq7i`, report
  `reports/run-1792008790-I1ZKfL`.
- Each root contains command/preview logs, before-input/source-report hashes,
  result and compilation-metric JSON, and a report-reader preview log.

Together with the isolated reproductions, 100-run capture stress check, six
unmutated full adapter controls and focused stronger-test rerun, this closes the
reproduced nested self-test blocker for this sample. Earlier failures remain
recorded; one passing pair is not a guarantee against every scheduling issue.
The ignored real-Cargo workflow suite was not repeated in this documentation
follow-up. The broader useful Rust operator set remains the next milestone work.
