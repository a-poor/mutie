# Scoped Python modulo mutations

Python now discovers binary `% → *` under `arithmetic_replace`. This closes the
last empty expression-operator row in the support table within the documented
scopes. It does not remove existing exclusions such as flat Python logical chains.

## Native semantics and scope

Only the parsed `%` operator token changes. Multiplication has the same precedence
and associativity, so existing operand/parent grouping and evaluation order remain
native. Controls compare edited ASTs against a single operator-class change,
including nested modulo, mixed multiplication/powers, unary signs, multiline
comments, UTF-8 and CRLF offsets, and expressions inside f-strings.

This is syntactic support, not numeric type inference. Native integer, negative,
floating-point, signed-zero, NaN/infinity and large-integer multiplication applies.
Zero divisors become multiplication by zero; no division is introduced. Overloaded
`__mul__`/`__rmul__`, NotImplemented fallback, subclass priority and raw return values
are retained. Operand evaluation occurs once, left to right; an early exception
prevents later evaluation. Custom multiplication exceptions retain their native type.

Python's `%` also formats strings/bytes. Such expressions are eligible: replacing
`'%d' % 3` with `'%d' * 3` produces repetition, while a string/tuple/mapping RHS can
raise TypeError. Errors stay errors, not kills. Plain percent text and f-string
format specifiers are not operator tokens. `%=` assignments, annotations, tests,
floor division and powers remain excluded at those nodes; supported nested binary
expressions remain eligible. No observer, classifier or deadline changes are needed.

The shared Python matcher accepts exactly `%` to `*`; saved labels and token pairs
must match. Source fingerprints/spans still bind application and reruns. Changed
validator/discovery bytes join execution identity, requiring fresh original reports.
No schema, package or operator-catalog version is changed.

## Fixture validation

The [five-site fixture](../tests/fixtures/python-modulo/README.md) has five passing
original tests: positive/negative integer, fractional, formatting and zero-input
wrap cases. Four mutants are killed; the zero-input wrap survives. A sixth test
checks nonzero wrapping and kills the survivor in a focused rerun. Original source
bytes and outcome files remain unchanged. The workflow rejects both a corrupted
operator label and a `% → /` replacement in saved reports.

Independent scored controls prove custom multiplication failure and incompatible
string formatting remain failed runs with one error and zero kills. Native scope
controls cover reflected dispatch, subclass priority, raw values and exceptions
without broadening the application's runtime scope.

## Unchanged public-project finding

The clean betting-math-kit checkout is still
`0bd3c25e93df8974f432310835768d77a5e45d3e`, with all 20 tracked input files unchanged.
The pinned CPython 3.14.7 / pytest 9.0.2 environment passes all **198 original tests**.
Discovery finds one modulo site in the project, in `_median` at
`src/betting_math_kit/simulation.py:111`:

```python
if n % 2 == 1:
    return s[mid]
```

The original run selects all 25 arithmetic sites in `simulation.py`, runs the full
suite for each candidate and uses existing bounded continuation for ordinary body
errors. It finishes in **137.77 seconds** with **four kills, 18 survivors and three
errors**, without timeouts or unfinished candidates. Modulo is source-run ID 10
and survives. The original report remains **failed and unscored**; file/operator
selection is not a full-project score. The other arithmetic outcomes are retained,
not reclassified or claimed resolved by this goal.

Replacing `n % 2` with `n * 2` makes the odd-length branch unreachable for positive
integer lengths. Existing original simulation tests predominantly use even trial
counts and do not assert the exact odd-count median. A separate copy adds only
[one public-API test](fixtures/python-modulo/test_odd_trial_median.py): three
one-bet trials with seed 7 produce final bankrolls 1250, 1250 and 750. The median
must be 1250; the mutant averages the lower two values and returns 1000.
This checks a software aggregation contract, not the quality of a financial model.

The **199-test stronger pristine baseline passes**, and a focused rerun of source
ID 10 kills the mutant with no errors or timeouts in **12.79 seconds** (saved run
duration). The focused 100% score does not replace the original failed score.
The stronger report has local ID 1; the ledger verifies the same source hash,
span, operator and replacement across the two reports.

## Reproduction and preservation

```sh
target/debug/mutie /path/to/clean/betting-math-kit \
  --lang python --python /path/to/pinned/venv/bin/python \
  --include src/betting_math_kit/simulation.py --operator arithmetic_replace \
  --continue-on-error 30 --timeout 30 --baseline-timeout 60 --ui plain \
  --output-dir /tmp/python-modulo-original

# Copy every original input into a separate directory and add only the retained
# test_odd_trial_median.py to tests/. Use ID 10 from this exact fresh selection.
target/debug/mutie /path/to/stronger-copy \
  --lang python --python /path/to/pinned/venv/bin/python \
  --rerun /tmp/python-modulo-original/run-... --id 10 \
  --timeout 30 --baseline-timeout 60 --ui plain \
  --output-dir /tmp/python-modulo-stronger
```

The local audit lives at `/tmp/mutie-python-modulo-audit`. Its frozen
`mutie` binary, original/stronger report paths, recipe hash, source identities,
report hashes and timing/outcome data are retained in the
[results ledger](python-modulo-results.json). The preservation manifest was
captured before execution; it records all original inputs and **6,280 historical
report files**, which remain byte-for-byte unchanged. A second manifest freezes
the new original report before the stronger rerun. The original Git checkout is
clean and the stronger copy differs only by the added test file.

The retained [verifier](fixtures/python-modulo/verify.py) checks these manifests,
original/stronger inputs, fresh recipe identity, unchanged original report bytes,
source-identity mapping and actual outcomes before exporting the compact ledger:

```sh
python3 -B benchmarks/fixtures/python-modulo/verify.py \
  /tmp/mutie-python-modulo-audit benchmarks/python-modulo-results.json
```

## Regression results

- All **61 static scope tests** pass, including native modulo controls and existing
  arithmetic/comparison/logical regressions.
- Both opt-in modulo workflows pass: scored fixture/stronger rerun and independent
  custom-dispatch/formatting error controls, plus saved-label/token rejection.
- The default Cargo suite passes **320 tests**, with **138 opt-in ignores** and no
  failures.
- Rust formatting, all-target Clippy with warnings denied and diff checks pass.

The deferred host startup investigation remains open. This work changes no host
security setting, readiness bound, assertion, process recipe or classification rule.
