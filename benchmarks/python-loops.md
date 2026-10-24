# Scoped Python while-loop skipping

Python `loop_skip` supports ordinary `while` conditions through the exact
`(False and (original))` recipe. It deliberately skips condition calls, truthiness,
exceptions and body execution. Native zero-iteration behavior remains: `else`
runs, and subsequent code executes normally. Retained syntax preserves generator
classification even when a condition contains a skipped yield.

Literal Boolean conditions belong to boolean flipping. Every condition subtree
containing `:=` is excluded, including nested lambdas/comprehensions. Iterator
`for` loops, annotation subtrees and tests do not gain loop sites. Nested ordinary
loops remain eligible. Existing operators inside excluded conditions retain their
own eligibility. Runtime errors, including uninitialized local access after a
skipped loop, remain errors rather than kills.

Discovery explicitly marks loop context because forced-false branches have the
same source recipe. Rust validates the exact false guard before assigning
`loop_skip`; saved readers validate the recipe and reruns match fresh source and
operator identities. Validator/recipe bytes join execution identity. No package,
report-schema or catalog version changed.

## Controls and fixture

The 69 static/native Python controls pass, including UTF-8/CRLF/comment spans,
native AST equivalence, literal/walrus/test exclusions, nested loops, skipped
calls/attributes/comparisons and failing `__bool__`/`__len__`, raw return identity,
`while…else`, generator classification and else exceptions.

The independent `tests/fixtures/python-loops` fixture produces one kill and one
survivor. An added positive-input test kills the survivor on focused rerun, with
fresh pristine collection. The workflow checks corrupt saved recipes/operator
labels and original source/report preservation. A separate scored control verifies
that skipping initialization produces one error, zero kills and a failed report.
Rust unit coverage verifies exact false guards and source identity.

```sh
/tmp/mutie-pytest-audit-env/bin/python -I -S -B tests/support/python_scope.py
MUTIE_PYTHON=/tmp/mutie-pytest-audit-env/bin/python \
  cargo test --test python_workflow loops_preserve -- --ignored --test-threads=1
MUTIE_PYTHON=/tmp/mutie-pytest-audit-env/bin/python \
  cargo test --test python_workflow loop_runtime -- --ignored --test-threads=1
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

Validation passes: 328 default-suite tests (160 ignored), both scored loop
workflows, 69 Python scope controls, Clippy, formatting and whitespace checks.

## Unchanged-project finding

The unchanged [Binary Tree Library](https://github.com/burpeesDaily/sphinx-sample-code)
checkout at `68d688fb4fe9c867c77a9d54f4fdb46a58dea34e` passes its original seven
tests under CPython 3.14.7 / pytest 9.0.2. This revisits a previously validated
project, rather than establishing a new independent compatibility milestone.

Ten ordinary while candidates are discovered. A preliminary run stops at its
first phase error; separate focused runs account for the other nine candidates.
Combined accounting is **five kills, one survivor and four errors**, with no
timeouts or build failures. This is not a completed full score. Phase errors
remain fail-closed and are not made continuable by the verified-body-error option.
All original failed reports are retained.

The survivor skips `_delete_fixup`'s ancestor walk in `avl_tree.py:346`, leaving
stale heights after deletion. The original suite checks traversal and search but
misses this height contract. The retained separate-copy
[test](fixtures/python-loops/test_delete_balance.py) inserts keys 2, 1 and 3,
deletes both leaves, then checks the remaining root and its height of zero.
The skipped cleanup leaves height one, producing an assertion failure.

After adding the Rust unit regression (which changes embedded recipe bytes), the
final executable generates a fresh original report and repeats the selected
survivor before the stronger-test rerun:

| Run | Pristine tests | Result | Seconds |
| --- | ---: | --- | ---: |
| Final original inventory run | 7 | first candidate error; nine pending | 8.92 |
| Final original focused ID 5 | 7 | survivor | 11.31 |
| Separate-copy focused ID 5 | 8 | killed, no errors/timeouts | 12.92 |

The full original report stays failed/unscored; the original focused score stays
0%. The stronger focused kill is not a full-project score. The preliminary
inventory review uses the earlier recipe and is recorded separately, not merged
into a fabricated final full run.

The [verifier](fixtures/python-loops/verify.py) checks all 33 tracked input bytes,
clean original Git state/commit, the sole stronger-test addition, current recipe
identity, source/operator mapping across report-local IDs, all captured original
report files and 275 historical outcome-file hashes. Historical preservation is
limited to those captured outcome files; it is not a claim that all historical
logs were rehashed. [Results](python-loops-results.json) retain report inventories,
outcomes, timings and hashes. Passing tests do not resolve the separately deferred
macOS startup issue.

Local evidence: `/tmp/mutie-python-loops-audit`. Recheck with:

```sh
python3 benchmarks/fixtures/python-loops/verify.py \
  /tmp/mutie-python-loops-audit benchmarks/python-loops-results.json
```
