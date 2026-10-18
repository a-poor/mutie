# Hyparquet full boolean-flip campaign

Date: 2026-10-18. Extends the [19-candidate sample](vitest-hyparquet-findings.md)
after [Node executable reuse](vitest-node-reuse.md).

## Scope and reproduction

Prepared Hyparquet 1.31.4 at commit `49c88a44fa04c10905821e9a6a7b7a08bf091557`,
with unchanged source/tests and the pinned installation described in the
[initial audit](vitest-javascript-hyparquet.md). Mutie commit `e7b97be`, debug build,
macOS arm64. Node 24.22.0's installed binary directory is first on PATH; pnpm
11.10.0, Vitest 4.1.9, Vite 8.3.4. Execution is serial and each candidate runs
the full suite in fresh isolated copies. No include filter or candidate cap.

```sh
mutie /tmp/mutie-hyparquet-scope --lang vitest --ui plain \
  --timeout 120 --baseline-timeout 120 \
  --output-dir /tmp/mutie-hyparquet-full-reports
```

Baseline passed in 6.82 seconds. Discovery found 133 boolean flips across 18
source files; `src/types.d.ts` was excluded. This is the complete discovered
boolean-flip inventory within the supported scope, not coverage of all possible
mutation operators or JavaScript syntax.

Local report: `/tmp/mutie-hyparquet-full-reports/run-1792362624-8Em1Wn`.
Log: `/tmp/mutie-hyparquet-full.log`.
Harness and preservation inventory: `/tmp/mutie-hyparquet-full.py` and
`/tmp/mutie-hyparquet-full-source-before.json`.

## Original-test outcomes and continuations

The first run stopped on an unhandled assertion at original candidate 81. An
explicit saved-ID continuation selected 82–133, then stopped on the same class
of failure at 89. A second continuation selected 90–133 and completed. Each
continuation reran the baseline and validated the original saved source/execution
identity. No candidates were silently dropped or reclassified.

| Report suffix | Original IDs attempted | Killed | Survived | Errors | Timeouts | Wall seconds |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| `run-1792362624-8Em1Wn` | 1–81 | 58 | 22 | 1 | 0 | 830.65 |
| `run-1792363519-GL9n02` | 82–89 | 2 | 5 | 1 | 0 | 72.72 |
| `run-1792363592-1tKD8E` | 90–133 | 29 | 14 | 0 | 1 | 468.75 |
| Unique inventory total | 1–133 | 89 | 41 | 2 | 1 | 1372.12 |

The sum is 22.87 minutes of command wall time across three invocations, including
three baselines, excluding review time between invocations. It is not a
before/after performance comparison. Command times varied materially during the
first run. The original full run has an unavailable score because it failed;
the aggregate above is an inventory accounting, not a newly synthesized saved
full-project score. The final continuation's score covers its selection only.

Candidate 98 changes `while (true)` to `while (false)` in Thrift's unsigned-varint
reader (`src/thrift.js:109`). It timed out under the unchanged 120-second limit;
subsequent candidates completed, providing evidence that worker cleanup and
continuation survived this case. No longer timeout or kill classification was
substituted.

### Unhandled callback assertions

Candidates 81 (`isListLike`'s final return) and 89 (`isMapLike`'s final return)
produce ordinary failed assertions as well as an unhandled assertion from
`test/read.test.js`'s `onChunk` callback. `parquetRead` invokes that callback from
an unawaited `.then` chain; throwing an assertion there does not belong to the
awaited test promise. Mutie preserves unhandled-error precedence and stops rather
than awarding a kill from the simultaneous ordinary failures.

The retained [test-only patch](fixtures/hyparquet-awaited-chunks.patch) collects
chunks in both callbacks and asserts their content after the awaited read.
Existing completion assertions remain. This changes only the review copy's test
structure, not production source or Mutie's error classifier. The original
errors remain errors in their saved reports.

## Survivor review

IDs below always refer to the first report's 133-candidate inventory.

| IDs | Finding | Review priority |
| --- | --- | --- |
| 14, 16, 35 | Default strictness in row matching, row-group pruning and page pruning | Observable coercion boundaries; helper contracts |
| 15 | Unknown filter operators are ignored by the fallback | Outside the declared operator set; intended runtime policy needs a decision |
| 25 | Missing page statistics must not prove a page can be skipped | Defensive pruning boundary |
| 46, 47, 49, 50 | Date / timestamp statistic byte order | Incorrect decoded bounds can affect query pruning |
| 60, 64, 65 | Strict-mode defaults in group planning and index prefetch | Helper defaults; most production paths pass mode explicitly |
| 62 | Direct group planning's default offset-index setting | Helper I/O contract |
| 63 | Unknown page layouts must coalesce disjoint candidate ranges | Conservative physical-read planning |
| 66 | No filter column index should not trigger output-column index reads | Avoid unnecessary I/O |
| 68 | Sorted sparse reads should not include unselected row groups | Avoid fetching projected data from unrelated groups |
| 70 | Public read API defaults to strict filtering | User-visible row selection |
| 72, 73, 74 | Flags assigned to `columnOptions` after planning | Redundant in current call path: downstream decoding does not consult them |
| 75 | Fallback offset-index default in `planCandidateColumn` | Unreachable for ordinary inputs: `parquetScan` already normalizes the option |
| 80, 84–88, 90–92 | List/map repetition and flat-column shape guards | Structural boundaries not distinguished by existing tests |
| 100, 103 | Equality default mode and unequal binary lengths | Previously reviewed helper/filter gaps reproduced |
| 116, 117, 119, 120, 122–126 | Variant int64/float/timestamp/decimal byte order | Nonzero value decoding gaps |

The four scan mutations are assessed as behaviorally redundant within the current
ordinary call path, based on tracing `parquetScan → readColumnRange →
planCandidateColumn/readParquetPlan → readRowGroup/readColumn`. This is a scoped
code review, not a general equivalence proof. No artificial assertions or
production changes were added to kill them. No dismissals were created.

The new [boundary tests](fixtures/hyparquet-full-survivors.test.js) cover the
remaining concrete findings, with explicit loose-mode/valid-shape controls where
applicable. The page-index default test uses a backing-store error to distinguish
an attempted read from an incorrectly pruned request; it is a mocked I/O contract
check, not validation of a complete synthetic Parquet file. Existing
[equality tests](fixtures/hyparquet-survivors.test.js) cover IDs 100 and 103.

## Stronger-test rerun and preservation

A fresh `/tmp/mutie-hyparquet-full-review` copy differs from the prepared
source only in two added test files and the callback assertion patch. It adds
30 new boundary tests plus the three retained equality tests. The targeted
new/equality/read test files passed 69 tests before the mutation rerun. The
full-suite isolated baseline then passed in 5.64 seconds.

Selected all 41 survivors and both error candidates by original saved IDs,
including IDs that were pending in the first report but completed in later
continuations. The original inventory remains the source of IDs and fingerprints.

```sh
mutie /tmp/mutie-hyparquet-full-review --lang vitest --ui plain \
  --rerun /tmp/mutie-hyparquet-full-reports/run-1792362624-8Em1Wn \
  --id 14,15,16,25,35,46,47,49,50,60,62,63,64,65,66,68,70,72,73,74,75,80,81,84,85,86,87,88,89,90,91,92,100,103,116,117,119,120,122,123,124,125,126 \
  --timeout 120 --baseline-timeout 120 \
  --output-dir /tmp/mutie-hyparquet-full-stronger-reports
```

Report: `run-1792364118-ufG8Xs` under that output directory.
Result: **38 killed, five survivors, zero errors/timeouts/build failures**.
Report duration 423.05 seconds; external wall 423.09 seconds. The saved 88.37%
score applies only to these 43 selected candidates. It is not a rescore of the
original inventory or evidence that the timeout disappeared.

All 36 newly killed original survivors name assertions in the retained stronger
test files in their saved logs. Original IDs 81 and 89 now produce ordinary
failed-test kills with the callback patch; their original error records are
untouched. Remaining IDs are exactly 15, 72, 73, 74, 75, matching the policy and
redundant-setting review above. No minimum-score target or dismissal was used to
hide them.

Final checks:

- All 305 original project files retain their SHA-256 hashes (excluding installed
  dependencies and caches, as in the earlier audit).
- All 667 artifacts in the three original reports retain their SHA-256 hashes.
- All four reports pass the shared reader before its expected noninteractive
  terminal error; execution scope matches across all reports.
- Review production files match the original source exactly. Added tests match
  retained artifacts; the callback patch reproduces the modified test file.
- After cosmetic indentation cleanup of the callback patch, direct Vitest from
  the review directory passes **757 tests in 33 files**. A preceding direct
  invocation from Mutie's directory failed to locate relative `test/files`
  fixtures despite `--root`; it is excluded from acceptance evidence. The scored
  rerun used the correct isolated working directory throughout.
- `git diff --check` passes. No Mutie production code changed, so the Rust suite
  was not rerun for this evidence/test-artifact-only change.

Local verification script: `/tmp/mutie-hyparquet-final-verify.py`.
Combined original outcome accounting: `/tmp/mutie-hyparquet-original-results.json`.
Continuation timing/provenance: `/tmp/mutie-hyparquet-remainder-result.json`.
Stronger-run log/timing: `/tmp/mutie-hyparquet-full-stronger.log` and
`/tmp/mutie-hyparquet-full-stronger-result.json`.
Final direct baseline: `/tmp/mutie-hyparquet-final-baseline-correct-cwd.log`.

## Outcome and limits

The broader run found useful decoding, filtering, schema and I/O test gaps beyond
the initial equality sample. It also exercised fail-closed handling of escaped
assertions, explicit continuation after failed runs, and recovery after a timeout.
The complete discovered inventory has been attempted, but there is no single
successful full-inventory report: two originals failed and the final stronger
run was selected. Four reviewed redundant mutations, one policy question and the
original timeout remain explicit. This does not expand supported runners, configs,
parallelism or platforms, or promote JavaScript beyond the existing scoped
Validated level. Reassess the next major scope objective rather than treating
more boolean operators or a cosmetic score increase as the automatic next step.
