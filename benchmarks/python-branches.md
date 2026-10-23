# Scoped Python branch forcing

Python `branch_force_true` and `branch_force_false` now support ordinary `if`
and `elif` conditions. This closes the README's branch-forcing row within each
runner's documented scope; it does not add loop or call removal.

## Semantics and admission

The exact recipes are `(True or (original))` and `(False and (original))`.
They preserve source bytes inside the wrapper and deliberately skip the entire
condition, including calls, attribute access, comparisons and `__bool__`/`__len__`.
Bodies and surrounding control flow remain unchanged. Parentheses prevent a
condition's internal `and`/`or` grouping from escaping the guard. Native Python
compilation still sees retained syntax (including generator classification).

Literal Boolean conditions, including parenthesized literals, stay with
`boolean_flip`. Every condition subtree containing `ast.NamedExpr` (`:=`) is
excluded, even inside a nested lambda or comprehension. This conservative audit
avoids claiming flow-sensitive binding safety. Other operators inside an excluded
condition keep their own eligibility. Skipped calls can still establish state
needed by a body: resulting runtime exceptions remain errors, not kills.
Ternaries, loops, comprehension guards, annotations and test code gain no branch
sites. Existing synchronous Python admission and lifecycle rules are unchanged.

Saved edits require the exact recipe/operator pairing; fresh discovery and source
identity establish AST context. The changed Python recipe participates in execution
identity, so reports made with older recipes cannot silently rerun under this one.
No schema, catalog or package version was bumped.

## Controls

`tests/support/python_scope.py` passes 65 tests, including branch-specific checks:

- UTF-8/CRLF/multiline comment spans and AST comparison against explicitly wrapped
  conditions; ordinary `if` and `elif` grouping.
- Literal and assignment-expression exclusions, including nested lambda and
  comprehension bindings; test and non-`if` context exclusions.
- Skipped effects and failing truthiness through direct values, calls, Boolean
  combinations, missing attributes, division and comparisons. Body return values
  retain identity and earlier `if` evaluation remains observable before `elif`.
- Retained generator kind, skipped yield and unchanged body exception behavior.

`tests/fixtures/python-branches` has four candidates: three kills and one survivor.
An added empty-input assertion kills the survivor on focused rerun, with fresh
pristine collection. The ignored workflow also rejects corrupt operator labels
and recipes and checks source/report preservation. A separate scored control
forces an `AttributeError` and verifies one error, zero kills and a failed run.

Validation:

```sh
/tmp/mutie-pytest-audit-env/bin/python -I -S -B tests/support/python_scope.py
MUTIE_PYTHON=/tmp/mutie-pytest-audit-env/bin/python \
  cargo test --test python_workflow branch -- --ignored --test-threads=1
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt --check
git diff --check
```

Both scored workflows pass. The default suite passes 324 tests with 149 opt-in
tests ignored. Clippy, formatting and whitespace checks pass. These passing runs
do not resolve the separately deferred macOS CLI startup issue.

## Unchanged-project acceptance

The existing clean [betting-math-kit checkout](https://github.com/bene-art/betting-math-kit)
at `0bd3c25e93df8974f432310835768d77a5e45d3e` retains all 20 tracked inputs.
CPython 3.14.7 / pytest 9.0.2 runs its unchanged 198-test suite. This revisits an
existing validation project; it is not a new independent project acceptance.

Using the release CLI, the complete branch inventory for
`src/betting_math_kit/simulation.py` contains 38 candidates: **12 killed,
15 survivors and 11 errors**, with no timeouts or build failures. Total elapsed
run time is 163.35 seconds. This is a file/operator-filtered inventory; the full
upstream test suite runs for every candidate. The report stays failed and unscored.
`--continue-on-error 40` records all candidate errors without changing classification.
Errors account for 11/38 candidates (28.9%); forced validation failures and skipped
preconditions are deliberately not treated as assertion kills.

A useful survivor forces every simulated bet to win (`rng.random() < win_prob`
forced true, source ID 22), skipping random draws entirely. Original assertions
also miss both forced median branches at line 111 (source IDs 15 and 16).
Two retained tests in a separate copy use the public `simulate_bankroll` API:

- Seed 1, four one-bet trials: win/loss/loss/win. Mean and median must both be
  1000, distinguishing all-wins behavior and an even median's upper-middle value.
- Seed 7, three one-bet trials: win/win/loss. Median must be 1250, distinguishing
  the odd median from averaging the two lower values.

The stronger pristine baseline passes 200 tests. A focused rerun kills all three
selected survivors with no errors in 19.08 seconds. This is a focused result, not a full stronger
score. Twelve other survivors and all eleven original errors remain open; no
survivor is declared equivalent or dismissed.

Inputs and old reports are verified by
[`fixtures/python-branches/verify.py`](fixtures/python-branches/verify.py).
The original Git checkout remains clean, the stronger copy differs only by
[`test_trial_branches.py`](fixtures/python-branches/test_trial_branches.py), and
all 9,394 historical report files plus the new original report remain unchanged.
The verifier also checks current execution recipe and immutable source/edit
identity across report-local IDs. Compact evidence and report hashes are retained
in [`python-branches-results.json`](python-branches-results.json).

Local audit root: `/tmp/mutie-python-branches-audit`. Reproduction:

```sh
AUDIT=/tmp/mutie-python-branches-audit
"$AUDIT/mutie" /tmp/mutie-python-betting-math-original \
  --lang python --python /tmp/mutie-pytest-audit-env/bin/python \
  --include src/betting_math_kit/simulation.py \
  --operator branch_force_true,branch_force_false --continue-on-error 40 \
  --timeout 30 --baseline-timeout 60 --ui plain --output-dir "$AUDIT/original-reports"
# Original run exits 1 because it retains candidate errors.
"$AUDIT/mutie" "$AUDIT/stronger" \
  --lang python --python /tmp/mutie-pytest-audit-env/bin/python \
  --rerun "$AUDIT/original-reports/run-1792723035-noFPOX" --id 15,16,22 \
  --timeout 30 --baseline-timeout 60 --ui plain --output-dir "$AUDIT/stronger-reports"
python3 benchmarks/fixtures/python-branches/verify.py "$AUDIT" \
  benchmarks/python-branches-results.json
```
