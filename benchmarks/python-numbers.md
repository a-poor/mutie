# Python integer and float literals

`number_replace` now supports runtime Python integer/float literals. Each eligible
site has one replacement: zero becomes one, nonzero becomes zero. Integer/float
type is preserved: `0 → (1)`, `7.0 → (0.0)`. The existing JS/TS slice remains
unchanged. Go/Rust numeric literals and string literals are separate roadmap work.

## Scope and controls

The pinned CPython 3.14.7 AST determines literal type/value; no project code is
imported during discovery. The [Python numeric grammar](https://docs.python.org/3.14/reference/lexical_analysis.html#numeric-literals)
defines the admitted spellings. Direct unary signs own their literal, suppressing
a duplicate child candidate. Signed zero becomes positive one. A parenthesized
operand retains its outer sign: `-(0.0)` changes the inner `0.0` to `(1.0)`.
Grouping and numeric member syntax remain valid through parenthesized replacements.

Bases, separators, large integers, fractions/exponents and float underflow/overflow
are supported. Integer zero detection does not truncate to a fixed machine width.
Float underflow to zero uses the zero recipe, while overflow to infinity uses the
nonzero recipe. Booleans and imaginary literals are excluded, as are annotations,
type parameters, match patterns and tests. Ordinary mapping keys, indexes, defaults
and f-string expressions remain eligible. A real literal inside a larger
complex-valued expression is still eligible; expression types are not inferred.

The independent Rust wire reader validates exact lexical forms, numeric type and
replacement value. Fresh AST/operator/source matching rejects a forged annotation
span even when its numeric snippet is valid. The new reader joins Python's recipe
fingerprint; old execution identities cannot be silently reused. Versions,
classifiers, process limits and fresh bytecode prefixes are unchanged.

Native controls execute **33 original/mutant pairs under both LF and CRLF**:
66 pairs, checking value, exact type and float sign. They cover signed zero,
bases/separators, integers beyond 2^53, very large integers, exponent rounding,
underflow/overflow, the half-subnormal boundary, comments/line continuations,
member syntax and nested/parenthesized signs. CLI discovery feeds all 33 forms
through the Rust reader as a second check. Static controls cover excluded contexts,
UTF-8 coordinates and ordinary dictionary/f-string expressions.

Three opt-in workflow tests cover native semantics/discovery; a scored integer
kill and float survivor with 50% raw score; minimum-score exit 3; dismissal
preservation; stronger-test reruns; corrupt recipe/type/operator rejection;
forged annotation spans; source drift; runtime/setup errors; and a bounded
three-second timeout. Errors and timeouts never become kills or passing scores.
Linux CI includes these tests in its Python operators job.

## Unchanged-project audit

The [betting-math-kit checkout](https://github.com/bene-art/betting-math-kit)
at `0bd3c25e93df8974f432310835768d77a5e45d3e` retains all **20 tracked inputs**,
including its manifest, layout and complete **198-test suite**. No installation,
source adaptation, suite trimming or configuration change was needed. This revisits
an existing validation project. The contracts below concern software behavior,
not the accuracy of financial models.

All **42 numeric candidates in `src/betting_math_kit/kelly.py`** were attempted,
using the same frozen CLI, pinned interpreter and complete test suite. Candidate
errors stop a run; subsequent runs explicitly select pending original identities.

| Run | Attempted | Killed | Survived | Errors | Seconds | Saved score |
| --- | ---: | ---: | ---: | ---: | ---: | --- |
| Original | 6 | 2 | 3 | 1 | 41.70 | Unavailable; failed with 36 pending |
| Pending selection | 1 | 0 | 0 | 1 | 13.04 | Unavailable; failed with 35 pending |
| Remaining selection | 35 | 18 | 17 | 0 | 180.20 | 51.43%, rerun selection only |

Combined accounting is **20 kills, 20 survivors and two errors**, with no timeouts
or build failures. It is not a full score. Both errors change the probability
validation bounds at line 95 and trigger runtime validation failures; their
failed reports remain unscored. The total saved run duration is **234.94 seconds**,
including repeated baselines. There is one numeric candidate per admitted site;
no additional replacement variants were added to inflate the inventory.

## Stronger tests and preservation

Three [retained tests](fixtures/python-numbers/test_default_contracts.py) in a
separate copy establish observable public default contracts:

| Original ID | Literal | Missing test contract |
| --- | --- | --- |
| 2 | Default takeout `0.16` | An omitted argument agrees with the documented explicit 16% value. |
| 3 | Default impact limit `0.05` | An omitted argument agrees with the explicit 5% limit. |
| 4 | Minimum size `2.0` | A computed one-unit size is rejected by the default minimum. |

The stronger pristine baseline passes **201 tests**. Its focused rerun kills all
three survivors in **28.36 seconds**, without errors or timeouts. The other
**17 survivors remain open**, including alternate boundary checks, zero-valued
result fields, ROI calculations and race-level sizing. They are not declared
equivalent or dismissed. The focused 100% result does not replace any historical
score or establish a stronger full-project score.

The [verifier](fixtures/python-numbers/verify.py) checks all 20 original and
stronger-copy input hashes, the sole added test, clean pinned Git state, all
**287 original report files**, baseline counts and exact source identities across
report-local IDs. It also checks unchanged runtime/recipe identity within the
campaign and matching 42-site discovery from the final CLI. The
[results ledger](python-numbers-results.json) contains relative paths and hashes.
An unsuccessful test-preparation attempt is retained separately with no score.

The campaign uses its frozen CLI throughout. A subsequent equivalent lint
simplification of the Rust digit loop changes the byte-based execution recipe;
the final reader/native/scored controls were rerun, and final discovery matches
the campaign inventory. Replaying these campaign reports requires the retained
CLI; new runs use the final recipe. Neither original reports nor identities were
rewritten to bridge that change.

```sh
python3 benchmarks/fixtures/python-numbers/verify.py \
  /tmp/mutie-python-numbers-audit benchmarks/python-numbers-results.json
```

## Validation

- Default suite: **357 passed, 200 opt-in tests ignored**, with two test threads.
- Numeric Rust recipe unit test, all three real numeric workflows and all
  **81 static Python scope controls** pass on CPython 3.14.7 / pytest 9.0.2.
- Existing Python workflows were checked with two threads. A seed-error control
  now explicitly selects its intended Boolean mutation, since numeric seeds can
  produce valid assertion kills. One conftest rerun rejected a CLI recipe change
  when a concurrent rebuild replaced the shared executable; stable-binary
  rechecks preserve the identity guard. All **43 existing workflows** pass across
  the suite and those targeted rechecks.
- Rustfmt, Clippy with warnings denied, Actionlint and whitespace checks pass.

Artifacts and the campaign CLI are under `/tmp/mutie-python-numbers-audit`;
logs use `/tmp/mutie-python-numbers-*`. The separate macOS startup investigation
remains deferred. No deadline, lifecycle check or classifier was relaxed.

The preceding three commits through `c36e01f` were pushed and passed all nine
[Linux CI jobs](https://github.com/a-poor/mutie/actions/runs/42663671307), including
Go/Rust assignment controls and JS/TS numeric workflows. This Python numeric
slice is locally validated; its remote CI run is still pending a push.
