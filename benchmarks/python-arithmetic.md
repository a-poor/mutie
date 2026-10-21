# Scoped Python arithmetic mutations

Python now discovers binary `+ ↔ -` and `* ↔ /` as the existing
`arithmetic_replace` operator. The AST identifies eligible expressions; token
positions select the exact original bytes. Each pair has equal precedence and
associativity, preserving grouping and evaluating each operand once in native
left-to-right order. There is no desugaring, operand duplication, runtime wrapper,
type coercion or exception conversion.

This is syntactic operator replacement, not numeric type inference. Addition
may concatenate strings or use user-defined dispatch; replacement can therefore
produce TypeError or another runtime exception. Such failures remain errors.
True division can change result types, and division by zero remains an error.
Native reflected dispatch, signed zero and NaN behavior are retained.

Only binary addition, subtraction, multiplication and true division are included.
Unary signs, augmented-assignment operators, floor division, modulo, powers,
matrix multiplication, unpacking and annotations are excluded. Eligible binary
subexpressions inside other expressions remain independent candidates. Existing
single-comparison boundaries and boolean flips are unchanged; tests and conftest
files remain outside mutation discovery.

## Integration and semantic controls

CLI/config operator selection, saved reports and focused reruns recognize the
new Python token pairs. Source validation rejects wrong replacement pairs;
saved report loading rejects mismatched operator/token labels. Discovery and
validator bytes already participate in execution identity, so this changes the
recipe and requires fresh acceptance reports. Historical reports are retained,
not rewritten. No package, report-schema or catalog version is bumped.

The 43 static scope tests include UTF-8/CRLF/multiline spans, comments, f-string
expressions, annotations, unsupported syntax exclusions and an AST comparison
showing that only the intended operator changes. Native execution controls check
operand order, early operand exceptions, reflected dispatch, unsupported string
operands, division by zero, true-division types, signed zero and NaN.

The [fixture](../tests/fixtures/python-arithmetic/README.md) contains five sites.
Its five original cases produce four kills and one zero-fee survivor; a sixth
case kills that survivor in a focused rerun. Separate real adapter controls keep
custom arithmetic-dispatch failure and zero division as saved errors with zero
kills. The workflow also rejects a corrupted saved operator label and verifies
original sources and report outcomes remain unchanged.

## Unchanged betting-math-kit validation

The original checkout is still
`0bd3c25e93df8974f432310835768d77a5e45d3e`, using pinned CPython 3.14.7 / pytest
9.0.2. Native and isolated baselines pass all 198 original tests. No application,
test, metadata or dependency files are adapted. The new inventory has 281 sites:
the prior 87 boolean/comparison sites plus 194 arithmetic sites.

The selected original run covers all 30 arithmetic sites in `kelly.py`, always
running the complete original suite. This is file/operator selection, not a
full 194-arithmetic-site or 281-site project score. Exact source identities map
selected IDs through saved reports and stronger reruns. All 30 are accounted for:
**21 kills, eight survivors and one error**, with no timeout or pending candidate,
in 249.31 seconds. Candidate 30 changes the race-scaling reason string's `+` to
`-` and raises TypeError in an original test. The original failed report retains
an unavailable score even though every selected candidate completed.

| Selected ID | Kelly line | Change | Surviving behavior |
| ---: | ---: | --- | --- |
| 9 | 144 | `/` → `*` | Full-Kelly magnitude |
| 13 | 187 | `-` → `+` | Net odds used in expected ROI |
| 18 | 253 | `-` → `+` | No-edge result's edge |
| 19 | 253 | `/` → `*` | No-edge result's implied probability |
| 20 | 260 | `*` → `/` | Fractional sizing multiplier |
| 23 | 282 | `-` → `+` | Below-minimum result's edge |
| 24 | 282 | `/` → `*` | Below-minimum result's implied probability |
| 25 | 292 | `-` → `+` | Ordinary sizing result's edge |

The useful gap is precision: positivity, monotonicity and upper-bound assertions
can pass while the returned fraction or edge has the wrong value. The separate
copy adds [three tests](fixtures/python-arithmetic/test_arithmetic_contracts.py):

- A known full-Kelly fraction is exactly 0.625; multiplying by net odds instead
  of dividing produces the wrong magnitude while existing qualitative checks pass.
- Expected ROI with a specified takeout is exactly 0.25; the test exercises the
  net-odds calculation rather than merely checking the sign.
- Returned edge values are checked in no-edge, below-minimum and ordinary sizing
  paths, together with the corresponding result and sizing conditions.

These test software arithmetic contracts, not financial model quality. All
original tests remain unchanged in the separate copy. No expected-exception
wrappers or classifier changes are needed for these findings.

The complete native **201-test stronger baseline passes** (1.67 seconds), as does
the isolated pristine baseline. The focused rerun kills **all eight survivors**
in 70.61 seconds, with no errors or timeouts. This is a selected rerun score;
the original failed report remains unscored. The other 164 arithmetic sites have
not been executed in this acceptance, and the original string-operand error is
not reclassified or claimed resolved.

## Reproduction and preservation

Use the retained [driver](fixtures/python-arithmetic/run.py) with a clean original
checkout, pinned launcher, final CLI and a fresh output directory:

```sh
python3 -B benchmarks/fixtures/python-arithmetic/run.py \
  --project /tmp/mutie-python-betting-math-original \
  --python /tmp/mutie-pytest-audit-env/bin/python \
  --mutie target/debug/mutie --output /tmp/new-python-arithmetic \
  --prior /tmp/mutie-betting-math-full/results.json \
  --prior /tmp/mutie-betting-math-final/results.json \
  --prior /tmp/mutie-betting-math-acceptance/results.json --stage baseline
# Repeat with the same project/python/mutie/output and --stage score.
# Then --stage stronger --ids <survivor IDs listed in results.json>.
python3 -B benchmarks/fixtures/python-arithmetic/verify.py \
  /tmp/new-python-arithmetic /tmp/mutie-python-betting-math-original
```

The driver rechecks original hashes/Git cleanliness, the final binary, previous
ledgers, previous stronger copies and all finalized report hashes around commands.
The [verifier](fixtures/python-arithmetic/verify.py) independently checks exact
source identities, original and stronger bytes, actual report data, prior mutation
sites, and failed-report score preservation. The new stronger copy adds only one
test file. Native runs disable plugin autoload/cache providers and use separate
bytecode prefixes; the adapter keeps fresh bytecode and full pristine inventories.
Candidate and baseline deadlines remain 30 and 60 seconds for the real project.

Timings and outcomes in the [results ledger](fixtures/python-arithmetic/results.json)
are single observations with concurrent regression checks, not controlled
performance measurements. Prior [87-candidate review](python-betting-math-full.md)
counts and findings remain historical evidence under their original recipe.

## Regression results

- All 43 static scope tests pass, including the new arithmetic semantic controls.
- Both real arithmetic workflow tests pass (113.70 seconds): scored fixture,
  stronger rerun, corrupt-label rejection and independent runtime-error controls.
- Both existing comparison workflows pass (78.91 seconds), preserving their
  fixture outcomes, stronger rerun and custom-dispatch error classification.
- The unfiltered default Cargo suite passes **279 tests**, with **96 opt-in
  ignores** and no failures.
- All-target Clippy with warnings denied and Rust formatting checks pass.

No subprocess readiness bound, assertion, observer or classifier is weakened.
