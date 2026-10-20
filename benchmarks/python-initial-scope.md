# Python / pytest initial scope audit

Recorded 2026-10-20. This is a native feasibility audit, not an implemented Mutie
runner. Python remains **Planned**. No `--lang python`, Python score, saved-run
integration or support promotion is introduced.

## Selected environment and project

- CPython 3.14.7 on macOS arm64, in a dedicated temporary virtual environment.
- pytest 9.0.2; exact supporting package versions are retained in
  [requirements.txt](fixtures/pytest-audit/requirements.txt).
- [Inflection](https://github.com/jpvanhal/inflection) at
  `88eefaacf7d0caaa701af7c8ab2d0ab3f17086f1`: one pure-Python package with no runtime
  dependencies and one parametrized pytest module.
- The baseline selects `test_inflection.py inflection --doctest-modules`:
  455 ordinary/parametrized tests plus 12 package doctests.

This is an **adapted execution recipe**. Upstream tox adds a mypy plugin and its
pytest config enables doctests during broader discovery. The audit explicitly
selects the runtime test module and package with an owned empty pytest config;
it does not execute mypy, packaging or documentation modules. Original project
files are preserved, but this is not unchanged full-project acceptance.

## Proposed first adapter boundary

Start with one dependency-free pure-Python package in a flat repository layout,
stdlib imports, explicit pytest test paths and serial complete-suite execution.
Use ordinary pytest functions and parametrization; fixture setup/teardown must
have distinct passing evidence. Default-argument boolean constants need an
explicit discovery decision, because their effects can include recursion.

Initially exclude unittest classes, user plugins/conftest hooks, installed or
editable application dependencies, `src/` layouts, namespace packages, extension
modules, doctests, async frameworks, distributed workers and custom collection.
The audit exercises doctests to retain Inflection's runtime checks, but that
does not establish production doctest attribution or automatically admit it.
Before claiming this real project for the first slice, either audit its doctest
scope into the adapter or choose a real project matching the narrower boundary.
Skip/xfail and changed test inventories need explicit policy rather than silently
treating incomplete execution as passing.

The future validator must reject unsupported configuration and imports; the
experimental driver's command-line exclusions are not a substitute for such a
validator. Runtime, stdlib, pytest/dependencies, config, discovery and observer
recipe need an execution identity before focused saved-report reruns.

## Isolation and bytecode

Every invocation runs in a fresh owned copy, with an owned config, disabled
plugin autoload and no editable install. The interpreter uses `-I -B`; the driver
adds the copy's root to `sys.path`. Recorded application module origins point into
that copy. Poisoned `PYTHONPATH`/`PYTHONHOME`/`PYTHONPYCACHEPREFIX` controls do not
redirect those imports. Original source hashes remain unchanged.

**`-B` alone is insufficient.** A control compiles `FLAG = True `, changes it to
the same-size `FLAG = False`, and restores the original timestamp. With `-B` the
valid old bytecode still loads and the assertion for False fails. With a fresh
owned `-X pycache_prefix`, the same test passes from mutated source. Both runs
preserve the input, including its intentionally stale cache. This establishes a
concrete cache boundary for the next implementation, not general import safety.

Python documents [`-B` and cache-prefix controls](https://docs.python.org/3.14/using/cmdline.html);
pytest documents the distinction between
[import modes and application import paths](https://docs.pytest.org/en/stable/explanation/pythonpath.html).

## Failure evidence and blockers

The retained [driver and audit](fixtures/pytest-audit/README.md) consume
[`pytest_runtest_makereport`](https://docs.pytest.org/en/stable/reference/reference.html#pytest.hookspec.pytest_runtest_makereport)
phase evidence plus collection, finish and process exit status. Positive assertion
failures require complete setup/call/teardown evidence for every collected item.
Collection/import/syntax errors, fixture/finalizer failures, mixed body/cleanup
failures, warnings, skips/xfail, early exit, missing evidence and ambiguous runtime
exceptions are conservatively errors; the external deadline remains a timeout.

Two deliberately retained negative controls prevent promoting this prototype:

1. A `unittest.TestCase.setUp` assertion is reported in the call phase and looks
   like a body failure. Rejecting unittest suites is necessary for this boundary;
   phase names alone do not establish attribution.
2. A registered `atexit` callback raises after pytest returns. Pytest reports
   success, the process exits zero, and the observer currently classifies it as
   passed. The log contains the ignored cleanup exception. **Late cleanup is an
   open correctness blocker**, requiring explicit process-lifetime evidence.

Thread failures and unraisable destructor errors during tests produce warning
evidence and are rejected in this matrix. That does not prove shutdown-time,
background-thread, subprocess or asynchronous cleanup coverage.

## Real findings and stronger tests

Inflection's only runtime boolean literal is the default `True` in `camelize`.
Flipping it causes recursive calls to retain the lower-camel mode and eventually
raise recursion errors: six ordinary tests report `RecursionError` and two
doctests report `UnexpectedException`. The conservative
assertion-only prototype reports an error; it is not counted as a kill.

A manual regex probe removing digit matching from `underscore` is already caught
by the existing `Area51Controller` case. Narrowing Unicode `\d` to ASCII `0-9`,
however, survives the original selected suite. The retained
[stronger test](fixtures/pytest-audit/inflection-stronger.py) checks
`underscore("version٣HTTP") == "version٣_http"`. It passes on original source and
fails on the narrowed regex, demonstrating an observable Unicode boundary.
This is a hand-authored experiment, not automatic discovery or a new operator.
Synthetic boolean controls separately demonstrate baseline → survivor → passing
stronger baseline → body-failure kill.

The direct runs have no mutation score. They do not imply broad Unicode
equivalence, complete upstream test coverage or saved-report acceptance.

## Recorded validation

The final reproducible run passes 39 native probes and five evidence-rejection
controls, including the two deliberately expected misclassifications above.
The 467-test real baseline takes 1.56s; the 468-test stronger baseline takes
1.65s and the stronger mutation run takes 1.67s. Times include interpreter
startup and external wait, excluding copy preparation; these are single samples.
All 17 original project file hashes remain unchanged. The added stronger test
and both manual regex edits exist only in owned copies.
Original/mutated test-ID sets agree; the stronger selection adds exactly one ID.
Parametrization over Inflection's set of uncountable words can reorder IDs across
interpreter hash seeds, so cross-run inventory comparison must distinguish
membership from execution order.

[Compact results](python-initial-scope-results.json) retain environment versions,
probe hashes, source preservation, inventory hashes/counts, failure phases and
log/evidence hashes. Full JSONL streams, logs and copied projects remain under
the recorded temporary output path; the retained script regenerates them.
No Rust production code changed, so validation targets the native audit rather
than rerunning unrelated Rust workflows.

## Next gate

Build and audit a strict process-lifetime observer that detects the late-cleanup
negative control and requires positive complete evidence. Expand replay rejection
and cancellation/capture/descendant controls. Then choose a real project within
the exact admitted scope and implement read-only discovery plus isolated baseline
checking before connecting scored runs. Keep Python Planned until those gates
produce an actual usable adapter.
