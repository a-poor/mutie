# Scoped Python comparison inversion and negation removal

Python now supports `comparison_invert` (`== ↔ !=`, `< ↔ >=`, `<= ↔ >`) and
`negation_remove` (`not operand` to a parenthesized operand). These join Boolean
flips, comparison boundaries and binary arithmetic within the existing pinned,
serial Python scope. No package, report-schema or catalog version is bumped.

## Semantics and integration

Single-comparison inversions replace the native operator token. Operand order,
rich-comparison/reflected dispatch, subclass priority and raw comparison results
remain native. Inversion is not Boolean negation: NaN ordering comparisons can
remain false, and custom methods can return arbitrary values or raise exceptions.
Chained comparisons and `is`/`in` operators are excluded.

Negation removal replaces the complete AST span with `(`, the original bytes
after `not`, and `)`. It preserves operand parentheses/trivia and surrounding
expression grouping, evaluates the operand once, and removes the truthiness
conversion. It can return a list, number, custom object or other raw value.
Nested `not` expressions are independent candidates. Adjacent punctuation,
comments, line continuations, UTF-8 and multiline/CRLF coordinates have controls.
Bitwise `~`, annotations and test/conftest modules are not new mutation targets.

Discovery validates exact source bytes and coordinates. Ordered keys include
file/start/reverse-end/replacement, allowing nested and shared-span candidates
while rejecting duplicates. Both boundary and inversion edits at one comparison
survive discovery. Saved readers require the exact operator/recipe; focused
reruns still rediscover AST-authorized source identities. Validator bytes bind
execution identity, so prior recipes require fresh runs. Native runtime exceptions,
collection failures and lifecycle mixtures retain existing error classification.

## Fixture and semantic evidence

- 56 static scope/semantic tests pass. Controls cover all six inversion pairs,
  custom non-Boolean returns, reflected/subclass dispatch, operand order/failure,
  NaN/signed zero, raw negation values, removed truthiness calls, grouping,
  nesting, annotations and exact native AST transformations.
- Rust unit controls validate exact recipes, shared/nested spans, multiline byte
  coordinates, duplicate rejection and source preservation.
- The [scored fixture](../tests/fixtures/python-inversions/README.md) has five
  original tests and 11 selected candidates: nine kills and two survivors. A
  seven-test stronger baseline kills both survivors in a focused rerun.
- Two native adapter cases preserve custom comparison exceptions and TypeError
  from raw negation results as errors, with no kills. Corrupt saved labels and
  negation replacements are rejected before execution. Both new ignored workflow
  tests pass (93.25 seconds together).

## Unchanged public-project validation

The pinned runtime is CPython 3.14.7 / pytest 9.0.2. No compatibility extension,
source adaptation, test trimming or classifier change was needed.

| Original selection | Baseline tests | Candidates | Killed | Missed | Errors | Saved duration |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Tomli-W, all comparison inversion/negation sites | 271 | 7 | 6 | 0 | 1 | 62.00 s |
| betting-math-kit, Kelly module | 198 | 19 | 14 | 0 | 5 | 110.14 s |
| betting-math-kit, simulation module | 198 | 18 | 5 | 5 | 8 | 99.76 s |

All selections run the complete original test suite. There are no timeouts or
build failures in these completed inventories. Bounded `--continue-on-error`
accounts for candidate errors; every original report remains failed and unscored.
The initial Tomli-W stop-on-error report (one error, six pending) is also preserved.
Do not add that exploratory attempt to the seven-site complete inventory.
These are filtered selections, not full-project mutation scores.

Tomli-W's constructor inversion rejects valid indentation and remains an error.
Its other six mutations are killed, so it supplies execution evidence but no
stronger-test finding. Kelly's 19 inversions likewise have no survivors.

The simulation finding is `simulation.py:111`, selected ID 8: changing
`n % 2 == 1` to `n % 2 != 1` reverses the median's odd/even branches. Existing tests
check reproducibility, positive growth and percentile ordering, allowing the wrong
median through. For `[40, 10, 30, 20]`, the mutation returns 30 instead of 25; for
`[30, 10, 20]`, it averages the wrong pair instead of returning 20.

A separate copy adds only [two median contract tests](fixtures/python-inversions/test_median_contract.py).
They check exact even/odd results and input-order preservation. The full **200-test
stronger baseline passes**, and the saved ID 8 focused rerun is **killed**, with no
error/timeout, in **13.57 seconds**. These are software summary-statistic contracts,
not validation of a betting model. The focused score does not replace the original
failed report's unavailable score.

Four simulation survivors remain open: ID 13 (peak update, line 207), ID 14
(drawdown guard, line 217), and IDs 15/16 (growth guards, line 219). They are not
claimed equivalent, dismissed or covered by this median-only rerun.

## Preservation and reproduction

[Machine-readable results](python-inversions-results.json) retain project commits,
input hashes, inventories/outcomes, selections, original report hashes and the
stronger-test hash. Both originals are Git-clean; all 271 tracked Tomli-W inputs
and 20 betting-math-kit inputs match their pre-run bytes. All 20 betting inputs
also match the stronger copy, and all **260 original report files** are unchanged.

Artifacts live under `/tmp/mutie-python-inversions-audit/`:
`original-reports` (initial Tomli-W stop), `complete-reports` (complete Tomli-W),
`betting-reports` (Kelly), `simulation-reports`, `stronger`, and `stronger-reports`.

```sh
target/debug/mutie /tmp/mutie-python-betting-math-original \
  --lang python --python /tmp/mutie-pytest-audit-env/bin/python \
  --include src/betting_math_kit/simulation.py \
  --operator comparison_invert,negation_remove --continue-on-error 100 \
  --timeout 20 --baseline-timeout 30 --ui plain --output-dir /tmp/new-inversion-reports

target/debug/mutie /tmp/mutie-python-inversions-audit/stronger \
  --lang python --python /tmp/mutie-pytest-audit-env/bin/python \
  --rerun /tmp/mutie-python-inversions-audit/simulation-reports/run-1792714894-W9nkjO \
  --id 8 --timeout 20 --baseline-timeout 30 --ui plain \
  --output-dir /tmp/new-inversion-stronger-reports
```

The first default suite and a targeted report-retention retry hit the unchanged
`fake Go never started` readiness assertion. A separate silent public-run startup
sample showed `_dyld_start` before main; that owned process was stopped and retried.
The host startup issue remains deferred. No security setting, deadline, assertion
or production spawn recipe changed. A redundant full-suite retry was cancelled
when the earlier suite resumed. These attempts remain recorded separately from
completed validation; passing retries do not establish a host-level fix.

Final validation: `cargo test` passed all 319 default tests (134 ignored),
`cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` passed.
The 56 scope/semantic controls and both opt-in inversion workflow tests also pass.
