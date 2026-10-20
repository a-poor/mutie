# Composer remaining Bun findings

Recorded 2026-10-20 using Mutie `dbbcc5a` (development build with optimized
sha2), pinned Bun 1.3.11+af24e281e and unchanged GramIO Composer commit
`3de15b846e3f5ef46cf8aebd5d136908baa43b54`.

## Original control

A fresh full run reproduces the original inventory and every outcome: 12
candidates, six kills, five survivors and one timeout, with zero execution errors.
All 272 baseline tests pass. Total recorded duration is 31.91s. The score remains
6 / 11 = 54.55%; the timeout is excluded.

This fresh report uses the current execution recipe, which changed when test-only
profiling call sites were added. The earlier report is retained unchanged; its
recipe was not edited or bypassed to permit reruns.

## Four observable survivor boundaries

The retained [four tests](fixtures/bun-audit/composer-remaining.test.ts) exercise
behavior rather than merely reading property descriptors:

| Original ID | Mutation | Stronger test |
| --- | --- | --- |
| 4 | Custom method `writable: true` → `false` | Override a method on one instance; another instance still uses the original. |
| 5 | Custom method `configurable: true` → `false` | Replace the generated prototype's method with an accessor and call it. |
| 6 | Custom method `enumerable: false` → `true` | Enumerate an instance with `for...in`; custom methods must not appear. |
| 8 | Macro shorthand comparison `value === true` → `false` | Enabling a function macro with `true` supplies `undefined`, activating its default options and running the handler. |

These are observable differences in the current implementation. The method
descriptor tests record existing extension behavior; they do not establish an
upstream guarantee that every descriptor flag is a documented public contract.
Each test creates its own generated class, so prototype changes cannot leak to
other tests.

## Queue timeout

ID 9 changes the queue's initial active flag to false. `add()` retains the event
but cannot dispatch it; `onIdle()` cannot resolve while that event remains queued.
The unchanged suite's first queue test reaches Bun's 5000ms test timeout, and
Mutie's 8s full-suite deadline expires during the incomplete suite. This is a
semantic blocked wait, not evidence of equivalent behavior or a cleanup failure.
The original timeout remains excluded from the score.

The retained [test-only patch](fixtures/bun-audit/composer-queue-preconditions.patch)
asserts `isActive` before idle waits and before the stop-with-pending-work test.
An inactive queue now fails in a test body before a wait that assumes dispatch.
Existing pending-count and active-state assertions also fail. No test is skipped,
no production queue code changes, and no timeout budget is raised.

## Focused result and preservation

The stronger copy adds four tests and changes only `tests/queue.test.ts` among
the 35 original files. Its full 276-test baseline passes. A saved-ID rerun of
4, 5, 6, 8 and 9 kills all five, with zero errors/timeouts, in 13.03s overall.
The queue candidate finishes in 0.48s with nine ordinary body failures and a
complete 276-test inventory. The selection score is 5 / 5 = 100%.

The prior name-configurability finding (ID 11) already has a separate stronger-test
kill in [shared workflow evidence](bun-shared-runs.md). Together these reviews
cover all five original survivors and the timeout. They are separate selected
runs, not a new 100% full-inventory score or a claim about all possible defects.

All original project hashes match; all source/package/config bytes in the stronger
copy match. Both the historical full report and the fresh source report retain
every file hash. Runtime/config/recipe identity matches between fresh and focused
runs; the test inventory gains exactly the new test file, as allowed for stronger
tests. [Machine-readable evidence](bun-composer-survivors-results.json) retains
input/report hashes, identities, inventories, per-candidate outcomes and timings.
Local report paths identify diagnostics and are not portable dependencies.

## Reproduction

Put the separate pinned Bun executable first on PATH. Use an unchanged checkout
at the commit above, a second copy for stronger tests, and report directories
outside both projects. From the Mutie repository:

```sh
cargo build --offline
target/debug/mutie /path/to/composer --lang bun --timeout 8 \
  --baseline-timeout 15 --ui plain --output-dir /path/to/original-reports
cp benchmarks/fixtures/bun-audit/composer-remaining.test.ts \
  /path/to/stronger/tests/mutie-remaining.test.ts
patch -d /path/to/stronger -p1 \
  < benchmarks/fixtures/bun-audit/composer-queue-preconditions.patch
target/debug/mutie /path/to/stronger --lang bun \
  --rerun /path/to/original-reports/RUN_DIR --id 4,5,6,8,9 \
  --timeout 8 --baseline-timeout 15 --ui plain --output-dir /path/to/reruns
```

Validation is the real full control and full-suite focused rerun plus preservation
checks. This change retains tests/evidence and updates documentation; Mutie's
execution, classification, timeout policy and support scope are unchanged.
