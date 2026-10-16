# Pathe survivor review and TypeScript milestone acceptance

The scoped TypeScript/Vitest run → inspect → improve tests → rerun workflow now
has real-project evidence. JavaScript remains Experimental until independently
validated on a real JavaScript project.

## Project and scope

Pathe revision `bc7477a01f0bd60ada017add8142c9f9d69ccdc5`, using the prepared
Node 24.22.0 / pnpm 11.10.0 / Vitest 4.1.9 / Vite 8.1.3 installation. Its original
coverage-only config is retained. One ESM package, TypeScript boolean flips,
serial full-suite runs, fresh isolated command copies, macOS arm64. This does
not validate arbitrary upstream dependency versions or custom configurations.

The [first complete run](vitest-coverage-config.md) selected all 14 mutations:
12 killed, two missed, no execution errors, 85.71% raw score, 94.00 seconds.
Original report:
`/tmp/mutie-pathe-config-reports/run-1792103000-UigGuM`.

## Finding usefulness

**Saved ID 12: `src/_path.ts:106`, `resolvedUNC = false` → `true`.**
The loop overwrites this initializer whenever it sees a nonempty path. With no
arguments (or only empty arguments) and an empty `process.cwd()`, it never does.
The original returns `"."`; the mutant returns `"//"`. Existing Pathe tests already
exercise its empty-cwd fallback, but only with nonempty relative arguments.
The added test covers this missing boundary and restores the cwd spy in `finally`.
This is a distinguishable fallback behavior, not an equivalent mutation or a
claim that ordinary host `cwd()` normally returns an empty string.

**Saved ID 13: `src/utils.ts:42`, normalized-alias marker value `true` → `false`.**
The marker prevents a normalized mapping from being normalized again. For
`{pkg: "pkg/dist", feature: "pkg/feature"}`, one pass produces
`feature: "pkg/dist/feature"`. Without the marker, a second pass expands it to
`"pkg/dist/dist/feature"`. The added tests assert normalization idempotence by
observable mapping contents and exercise `resolveAlias` with the normalized map.
The mutant resolves `feature/file.ts` to `pkg/dist/dist/feature/file.ts` instead
of `pkg/dist/feature/file.ts`. This affects public output, beyond caching speed;
the tests do not inspect the private symbol or require object identity.

The three tests are retained in
[pathe-survivors.spec.ts](fixtures/pathe-survivors.spec.ts). Copy this file into
Pathe's `test/` directory; its imports are relative to that destination.

## Controlled focused reruns

A separate copy at `/tmp/mutie-pathe-survivor-review` retained all original
production source, tests, dependency metadata and config. First, without added
tests, a missed-only rerun of the original report reproduced both survivors.
Then only `test/mutie-survivors.spec.ts` was added and the same saved selection
was rerun with both CI policies enabled.

| Run | Baseline | Selected outcomes | Total elapsed |
| --- | --- | --- | --- |
| Original full discovery | Passed | 12 killed, two missed | 94.00 s |
| Focused control, unchanged tests | Passed, 4.04 s command time | Two missed | 18.18 s |
| Focused stronger tests | Passed, 4.08 s command time | Two killed | 18.79 s |

The stronger rerun maps new IDs 1/2 to original IDs 12/13 and records source
checkpoint 37, status completed and missed-only selection. Both findings and
minimum-score policies pass. Its 100% score applies only to the two selected
mutations; no new full-project 100% score is claimed. All commands still run the
full test suite. These timings are observations with warmed host caches, not a
controlled performance-optimization benchmark.

Control report: `/tmp/mutie-pathe-survivor-reports/run-1792103898-Xov0Jd`.
Stronger report: `/tmp/mutie-pathe-survivor-reports/run-1792103925-9BYTS7`.
Logs: `/tmp/mutie-pathe-survivor-control.log` and
`/tmp/mutie-pathe-survivor-stronger.log`.
Both reports pass reader validation before the expected noninteractive-terminal
error. All 123 hashed files in the original project (excluding installed
dependencies) and original report remain byte-for-byte unchanged. The review
copy's original files also match; the only addition is the stronger test file.

```sh
# Use a separate copy of the prepared pinned Pathe checkout.
mutie /path/to/review-copy --lang vitest --ui plain \
  --rerun-missed /path/to/original-run
cp benchmarks/fixtures/pathe-survivors.spec.ts /path/to/review-copy/test/mutie-survivors.spec.ts
mutie /path/to/review-copy --lang vitest --ui plain \
  --rerun-missed /path/to/original-run --fail-on-missed --min-score 100
```

## Scoped acceptance and remaining limits

The TypeScript row meets the support ledger's **Validated** definition:

- Real run/inspect/improve-tests/rerun, useful findings and runtime: evidence above.
- Saved outcomes, identity and focused provenance: this run plus
  [focused-rerun regressions](vitest-focused-reruns.md).
- Review and raw-score/policy separation: [JS/TS review regressions](vitest-dismissals.md).
- Body/lifecycle failures and scoped teardown: [failure attribution](vitest-lifecycle-attribution.md).
- Timeout, cancellation, raw-output overflow, descendant shutdown and workspace
  cleanup: the real `real_timeout_cancellation_capture_and_isolation` regression
  passed again on the current adapter in 6.86 seconds. It checks that source,
  installed-dependency markers, command temp files and spawned descendants do not
  leak from isolated execution. Log:
  `/tmp/mutie-pathe-milestone-reliability.log`.

Limits remain explicit: one pinned ESM/pnpm package, Node environment, boolean
flips, one mutation worker, full test scope, narrowly supported config and audited
worker bytes. Ambiguous async/assertion-count failures remain errors. Cross-file
fixture reuse, cancellation during scoped teardown, parallel workers, broader
configs, JSX/TSX, CommonJS and other platforms remain outside verified scope.
No Mutie runtime code or package/report/operator versions changed in this review.
The next major evidence gap is an independent real JavaScript validation project.
