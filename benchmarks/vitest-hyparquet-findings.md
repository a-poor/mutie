# Hyparquet JavaScript mutation findings

Date: 2026-10-15. Project preparation and pinned toolchain are recorded in
[the initial audit](vitest-javascript-hyparquet.md). These runs use the unchanged
prepared Hyparquet source and tests, with its original JavaScript-only NodeNext
config. They are deliberately filtered samples, not a full 133-mutation score.

Subsequent [full-inventory validation](vitest-hyparquet-full.md) attempts all 133
candidates, records callback errors and a timeout separately, and extends the
stronger-test review. The sample results below remain historical evidence.

## Bloom-filter sample

Selected all seven boolean flips in `src/bloom.js` to exercise membership results
and numeric byte order. Every mutation runs the full test suite with one worker,
fresh isolated command copies, and a 120-second timeout.

```sh
mutie /path/to/hyparquet --lang vitest --include src/bloom.js \
  --ui plain --timeout 120 --baseline-timeout 120 \
  --output-dir /path/to/reports
```

Report: `/tmp/mutie-hyparquet-reports/run-1792117902-PYnA2k`.
Result: seven killed, zero missed, zero errors/timeouts/build failures.
End-to-end report duration: 176.08 seconds; baseline command: 12.21 seconds.
The saved score is 100% for this filtered selection only. Existing tests catch
both membership return flips, filter-block little-endian reading, and FLOAT,
DOUBLE, INT32 and INT64 little-endian hash encoding changes. No stronger tests
were needed for these candidates.

The report passes reader validation before the expected noninteractive-terminal
error. Local log: `/tmp/mutie-hyparquet-bloom-run.log`.

## Equality-helper sample and finding review

The second sample selects all 12 boolean flips in `src/utils.js`, with the same
full-suite execution settings. Selection was motivated by inspecting the helper's
explicit-mode tests, not by a random sampling strategy; these results cannot
estimate a whole-project survivor rate.

Two observed survivors have distinct value:

- `equals(a, b, strict = true)` changed to default `false`: existing tests pass
  strictness explicitly, so they miss the omitted-argument contract. The exported
  internal helper then treats `1` and `'1'` as equal by default. Current production
  callers supply the mode explicitly and the package root does not export
  `equals`; this is a lower-priority helper-contract gap, not evidence of a
  user-facing query regression.
- The unequal-byte-array-length guard returns `true` instead of `false`: existing
  tests compare equal-length buffers with equal or unequal contents. A short
  buffer and a longer buffer with the same prefix become equal under the mutant.
  This affects `matchFilter` equality and membership predicates, making it a more
  consequential missing boundary case.

The proposed stronger tests are retained in
[`fixtures/hyparquet-survivors.test.js`](fixtures/hyparquet-survivors.test.js).
They check default strictness, both operand orders for unequal byte lengths, and
binary `$eq`/`$in` filtering alongside an equal-buffer positive control. Tests
belong in `test/mutie-survivors.test.js` in a separate prepared review copy;
production source stays unchanged.

Report: `/tmp/mutie-hyparquet-reports/run-1792118088-p2pcbA`.
Final result: ten killed, two missed (local IDs 1 and 4), no errors, timeouts,
build failures or unfinished candidates. Filtered score: 83.33% (10/12).
Reader validation passes before the expected noninteractive-terminal error.

End-to-end report duration was 978.11 seconds. Baseline command time was 12.01
seconds, and mutation commands summed to 292.63 seconds. Command times varied
from approximately 12 to 38 seconds during the run. Shared workspace resets
accounted for 169.49 cumulative seconds; adapter-internal copy/validation/cleanup
is not included in command timings or those shared workspace metrics. This is
an observed slow run, not a controlled attribution of overhead or a prediction
for all 133 mutations. No optimization or timeout increase was applied to obtain
these outcomes. A feedback-time investigation should separate host variation,
shared resets and adapter preparation before scaling this sample up.

## Rerun environment observation

The first unchanged-test control failed before baseline execution with
`could not identify Node for Vitest preview` (report
`/tmp/mutie-hyparquet-reruns/run-1792119089-SWCffk`, 39.80 seconds).
PATH selected `/path/to/home/.vite-plus/bin/node`; a separate identity probe
through that launcher also took more than ten seconds, then resolved to the same
Node 24.22.0 binary used by the original runs. Mutie's identity probe has a fixed
ten-second limit. The report does not expose which individual probe rejection
condition fired, so this observation alone does not prove the failure's cause.

Subsequent reruns put the existing pinned binary directory first on PATH:
`/path/to/home/.vite-plus/js_runtime/node/24.22.0/bin`.
This retains the same resolved executable/version in execution identity and
avoids the launcher. No adapter code or saved identity was relaxed. The failed
attempt remains a failed attempt, not mutation evidence.

## Controlled stronger-test reruns

The unchanged review copy reproduced both survivors in 103.59 seconds, with no
errors/timeouts. Report:
`/tmp/mutie-hyparquet-reruns/run-1792119191-M1u9Hk`.

Adding only the three retained tests produced a passing baseline and two kills
in 55.60 seconds. Report:
`/tmp/mutie-hyparquet-reruns/run-1792119320-KBNtNx`.
Both `--fail-on-missed` and `--min-score 100` passed. The 100% score is for these
two saved candidates only, not the original utils sample or the entire project.
The default-mode test kills original ID 1; the byte-length and filter tests both
fail for original ID 4 (renumbered to ID 2 in the rerun). Saved logs identify these
specific new assertions as the failures.

```sh
# Start with a separate copy of the prepared pinned project.
mutie /path/to/review-copy --lang vitest --ui plain \
  --rerun-missed /path/to/utils-run --timeout 120 --baseline-timeout 120
cp benchmarks/fixtures/hyparquet-survivors.test.js /path/to/review-copy/test/mutie-survivors.test.js
mutie /path/to/review-copy --lang vitest --ui plain \
  --rerun-missed /path/to/utils-run --timeout 120 --baseline-timeout 120 \
  --fail-on-missed --min-score 100
```

All four completed reports pass reader validation before the expected
noninteractive-terminal error. SHA-256 checks confirm all 305 original project
files and all 102 files in the two original reports remain unchanged. The review
copy matches all 305 original files; its only additional project file is the
stronger test, byte-identical to the retained artifact. Dependency caches are
excluded from these project hashes. No production source or adapter code changed
in this findings review.

## Scoped JavaScript acceptance

JavaScript now meets the support ledger's **Validated** definition within the
pinned serial ESM/pnpm boolean-flip scope:

- Independent real-project findings and stronger-test saved-selection reruns:
  evidence above, with usefulness differentiated and runtime limits explicit.
- Saved reports, identity, provenance and preservation: these four completed
  reports plus [focused-rerun regressions](vitest-focused-reruns.md).
- Review and policy separation: [JS/TS dismissal regressions](vitest-dismissals.md)
  plus both CI policies on the stronger real-project rerun.
- Body/lifecycle failure classification: [lifecycle evidence](vitest-lifecycle-attribution.md).
- Timeout, cancellation, raw capture limits and isolation: the real
  `real_timeout_cancellation_capture_and_isolation` regression passed again in
  8.07 seconds using the pinned fixture and direct Node PATH. Local log:
  `/tmp/mutie-hyparquet-reliability.log`.

This is independent JavaScript evidence, not an extrapolation from Pathe's
TypeScript acceptance. It covers 19 selected mutations, not a whole-project run.
The remaining 114 discovered candidates were not executed. Full-suite execution
is not full mutation coverage. JSX/TSX, CommonJS, arbitrary configs, parallel
workers, cross-file fixture reuse and other platforms remain outside this scope.

Next investigate focused feedback time with repeated unchanged selections and
phase measurements before expanding to the complete inventory. The slow sample
and launcher failure warrant investigation, not an unsupported performance claim.
