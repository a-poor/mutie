# Python standalone-call removal

Python `call_remove` replaces a native `Expr(Call)` statement with `pass`.
It removes receiver/property lookup, argument evaluation and unpacking, and the
call body. Outer parentheses belong to the statement span; surrounding comments,
semicolons and neighboring statements remain. `pass` keeps empty suites valid
and prevents a following string expression from becoming a docstring.

Calls used as values, annotations and tests are excluded. Every call subtree
containing `:=`, `yield`, `yield from` or `await` is excluded, including nested
closures. This avoids changing bindings or generator classification. Ordinary
constructors and callable objects use the same syntax rule; there is no type
inference. Async support, general statement/block removal and runtime admission
are unchanged. These semantics follow Python's
[expression-statement and pass rules](https://docs.python.org/3.14/reference/simple_stmts.html)
and are checked on pinned CPython 3.14.7.

Rust validates the exact wire replacement, while the pinned native AST helper
admits source syntax. Focused reruns require fresh AST/operator/source identity:
a forged assignment span with a correct source fingerprint cannot run. Recipe
bytes join execution identity; no package, report or operator-catalog version
changed. New reports are required after the recipe change.

Validation also found a pre-existing discovery mismatch: native branch sites
emitted `loop_skip: false`, whereas Rust serialization omitted the optional flag.
The native helper now omits false flags too. Listing a branch-containing project
has a regression check; unchanged-project and drift checks remain strict.

## Controls

The 76 Python static/native controls pass, including six new call tests covering
UTF-8, CRLF, multiline comments, parenthesized/semicolon statements, byte spans,
native AST equivalence, receiver/descriptor/argument/unpacking effects, ignored
return truthiness, binding/suspension exclusions, generator kind, empty suites,
docstrings and surrounding exceptions.

The independent [fixture](../tests/fixtures/python-calls/README.md) produces one
kill and one survivor. A stronger test kills the survivor on focused rerun.
Its workflow checks listing, exact recipes, forged non-call identities, stale
source, review policy versus raw score, and preserved reports. Separate scored
controls keep an initialization `AttributeError` and a mixed body/fixture-cleanup
failure as errors with zero kills. Existing flat/src/root/unittest/conftest review
workflows, loop reruns, resource limits, cancellation and collection checks also
pass. Linux CI includes the new `calls_` tests in its Python operator partition.
Local validation passes: 343 default-suite tests (172 ignored), 13 selected
Python workflow checks including the 76 static/native controls, Clippy, Rustfmt
and whitespace checks. The edited Linux workflow has not been run remotely.

```sh
MUTIE_PYTHON=/path/to/pinned/venv/bin/python \
  cargo test --locked --test python_workflow -- --ignored --test-threads=1 \
  calls_ static_scope_regressions scored_reports_and_stronger_test_reruns \
  loops_preserve_native_semantics_and_rerun
MUTIE_PYTHON=/path/to/pinned/venv/bin/python \
  cargo test --locked --test python_workflow -- --ignored --test-threads=1 \
  scored_resource_limits scored_cancellation mutation_cannot_change_parametrized_inventory \
  conftest_cleanup_failure_is_saved_as_error
cargo test --locked
cargo clippy --all-targets --locked -- -D warnings
cargo fmt --check
```

## Unchanged Route Mini findings

The unchanged [Route Mini](https://github.com/JustinJLeopard/route-mini) checkout at
`35a3db157731ba0073d66f432b69b997ce2e9f01` runs its full 68-test suite. Selecting
`call_remove` in `src/route_mini/router.py` yields seven candidates. With one worker,
15-second mutation timeout, 120-second baseline timeout and continuation limit 10,
all seven finish in 29.94 seconds: one kill, four survivors and two errors.
The run remains failed and unscored.

| Source ID | Removed effect | Original result | Separate-copy result |
| --- | --- | --- | --- |
| 1 | Exception message initialization | Survived | Killed by exact message assertion |
| 2 | No-route telemetry record | Survived | Killed by exact decision-ledger assertion |
| 3 | Retry backoff sleep | Survived | Open; not declared equivalent |
| 4 | Failed-model tracking | Error (mixed assertion/IndexError) | Not rerun |
| 5 | Blocked-error telemetry record | Survived | Killed by exact decision-ledger assertion |
| 6 | Successful-route telemetry record | Killed | Not rerun |
| 7 | Exhausted-fallback telemetry record | Error (IndexError) | Not rerun |

Three new tests in a separate copy raise the pristine baseline to 71 tests and
kill IDs 1, 2 and 5 in a 16.79-second focused rerun, with no errors or timeouts.
The tests check the externally visible exception message and complete telemetry
tuple before any indexing. No classifier, original tests or source changed.
This focused result is not a full stronger-suite mutation score, and the original
error outcomes remain errors.

The [retained tests and verifier](fixtures/python-calls-route-mini/verify.py)
check all 25 original tracked files in both copies, clean original Git state and
commit, the sole stronger-test addition, exact current recipe, source identity
mapping across report-local IDs, and all 44 original saved-report files.
[Results](python-calls-results.json) retain hashes, inventories, outcomes and
timings using relative paths. No preservation claim is made for older unrelated
campaigns. The deferred macOS startup issue is unchanged.

Local evidence is under `/tmp/mutie-python-calls-audit`. Recheck with:

```sh
python3 benchmarks/fixtures/python-calls-route-mini/verify.py \
  /tmp/mutie-python-calls-audit benchmarks/python-calls-results.json
```
