# Scoped Python comparison-boundary mutations

The Python runner now discovers `comparison_boundary` alongside `boolean_flip`:
`<` ↔ `<=` and `>` ↔ `>=`. This uses the existing operator catalog and report
schema without a version bump. Select it with `--operator comparison_boundary`;
unfiltered Python discovery includes both supported operators.

## Semantics and limits

Only an AST comparison with one operator is admitted. Chained comparisons such as
`low < value <= high` are left untouched. Equality, inequality, identity and
membership operators are excluded. Test/conftest code and annotation subtrees
remain non-mutable, including lazy generic type-parameter bounds/defaults; ordinary
nested single comparisons are independently eligible.

Discovery matches the AST operand gap to exactly one native operator token.
It replaces only that token in the original UTF-8 bytes, preserving parentheses,
comments, operand order and single evaluation. Token columns are translated to
byte offsets, including Unicode prefixes, f-string expressions and LF/CRLF source.
Unicode line-separator characters inside strings do not create physical AST rows.
The Rust reader separately verifies source hashes, exact original/replacement
pairs, ordering, non-overlap and coordinates before exposing shared mutations.
Saved reports require matching operator/token pairs, preventing a comparison
from being relabeled as a boolean mutation or an unsupported replacement.

This is native operator substitution, not a claim of numeric type inference.
For ordinary numeric values it changes inclusion of the equality boundary.
NaN remains unordered. Custom objects can dispatch different methods for `<` and
`<=`, return non-booleans, or raise exceptions; no helper rewrites this behavior.
A custom-dispatch runtime exception remains an error, never a body kill.
Original expression evaluation and mutated semantics are deliberately distinct;
this feature does not prove comparisons pure or guarantee termination.

The scope and discovery implementations already participate in execution identity.
Old reports remain readable, but rerunning across this recipe change requires a
fresh run. Older source identities and scores are not rewritten.

## Fixture validation

The retained [fixture](../tests/fixtures/python-boundaries) contains all four
substitutions, equality assertions with operand-call traces, float NaN checks,
an untouched chained comparison and a budget survivor. Native scoped execution
kills four candidates and leaves the budget boundary alive. A stronger exact-budget
assertion kills that survivor in a focused rerun. The workflow checks original
source/report preservation and rejects a corrupted saved operator identity.

Static tests cover Unicode, comments, multiline operators, nested comparisons,
f-strings, annotation/chain/test exclusions, and native custom dispatch/operand
exceptions. The custom-dispatch scored negative control ensures an exception in
`__le__` remains an error even when the pristine `__lt__` result passes.

## Unchanged real-project finding

Route Mini is the unchanged checkout pinned in [initial acceptance](python-route-mini.md).
Discovery finds 38 existing boolean flips and seven new comparison sites.
The original 68-test suite does not exercise model capacity exactly equal to the
request's token limit. A fresh run selects only comparison boundaries in
`src/route_mini/router.py`: its one `>=` → `>` candidate survives. No upstream
source, test, fixture, configuration or metadata was adapted for that control.

A separate copy adds [test_exact_capacity.py](fixtures/python-comparison-boundaries/test_exact_capacity.py).
It requests exactly the advertised capacity and asserts a successful selected
model, response and provider call. Unexpected FallbackExhaustedError is explicitly
an assertion failure through `pytest.fail`. The native stronger baseline passes
69 tests and the focused rerun kills the candidate with no errors or timeouts.
This is one selected finding, not a full comparison inventory score.

The [driver](fixtures/python-comparison-boundaries/run.py) verifies original inputs,
all earlier acceptance/review reports, both prior stronger copies, its own reports,
the new stronger copy and the Mutie binary. Original Git remains clean. It starts
a fresh report under the new recipe and resolves the stronger rerun from that
report. Do not rebuild Mutie during the sequence.

```sh
cargo build --offline
python3 -B benchmarks/fixtures/python-comparison-boundaries/run.py \
  --previous /tmp/mutie-route-mini-acceptance/results.json \
  --review benchmarks/fixtures/python-route-mini-review/results.json \
  --output /tmp/mutie-route-mini-boundaries-new \
  --mutie target/debug/mutie
MUTIE_PYTHON=/path/to/audited-venv/bin/python \
  cargo test --offline --test python_workflow comparison_ -- --ignored --test-threads=1
```

## Recorded results (2026-10-21)

| Final-build run | Tests | Kills | Survivors | Errors/timeouts | Seconds |
| --- | ---: | ---: | ---: | ---: | ---: |
| Native original baseline | 68 | — | — | 0 | 0.98 |
| Unchanged router selection, including isolated baseline | 68 | 0 | 1 | 0 | 62.33 |
| Native stronger baseline | 69 | — | — | 0 | 0.61 |
| Stronger focused rerun, including isolated baseline | 69 | 1 | 0 | 0 | 22.63 |

[results.json](fixtures/python-comparison-boundaries/results.json) retains the
complete discovered inventory, selected source identity, pristine test nodes,
commands, outcomes and report/input hashes. Final discovery took 38.39 seconds;
startup varied in this batch. These are validation timings, not a controlled
performance benchmark. An earlier build produced the same survivor-to-kill result;
its two report hashes were checked again after final validation and retained.

During implementation, one fixture rerun correctly rejected execution identity
after a concurrent rebuild. It was repeated with a fixed binary; it is not counted
as a successful check. Final positional review also added Unicode line-separator
and generic-annotation exclusions before the final build and real-project repeat.

Final static scope checks pass **39 tests**. The scored comparison workflows pass
**2/2** in 129.68 seconds, covering the 10-test fixture, 11-test stronger baseline,
saved-token corruption and custom-dispatch error classification. The broader
**21/21** native Python workflow run passed in 776.01 seconds before the last
positional/annotation refinements; the final-build static and comparison checks
above specifically revalidate those refinements. The runtime identity is unchanged
between the preliminary and final real-project runs; the recipe fingerprint differs
as required. Formatting, whitespace and all-target Clippy checks pass.

The preliminary default run passed **276 tests**, with 93 opt-in tests ignored.
The final-build default run is **not clean**: 272 tests passed before three
`vitest_preview` failures, and the remaining `vitest_workflow` binary subsequently
passed its one default test (five ignored). All 12 terminal tests passed, including
the ten-size/cancellation-case test, which took longer in this run. The three
failures occur at `tests/vitest_preview.rs:75`: the freshly compiled native Node
fixture does not initialize within its unchanged ten-second bound, before any
Mutie/Vitest behavior assertion is exercised. A focused recheck reproduces all
three; [its log is retained](fixtures/python-comparison-boundaries/vitest-startup-recheck.log).
This is an outstanding test-fixture startup blocker, not evidence against Python
comparison semantics and not a passing full-suite claim. No Vitest fixture,
readiness limit or assertion was changed in this goal. The earlier successful
[readiness acceptance](process-readiness.md) remains historical evidence; it does
not establish that these later native-loader startup failures are resolved.

Final command records:

- `cargo test --offline`: final default failure described above;
  `/tmp/mutie-python-boundary-final-full.log`.
- `cargo test --offline --test vitest_preview --test vitest_workflow --no-fail-fast`:
  same three preview failures; remaining default workflow passes.
- `MUTIE_PYTHON=… cargo test --offline --test python_workflow comparison_ -- --ignored --test-threads=1`:
  final two comparison workflows pass.
- `cargo clippy --offline --all-targets -- -D warnings`, `cargo fmt --check`,
  `git diff --check`: pass.

The Python feature acceptance is complete; the independently reproduced native
fixture startup blocker is recorded for a separate reliability investigation.
