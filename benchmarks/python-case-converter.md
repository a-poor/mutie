# Python case-converter execution audit

This is real-project execution evidence, not completion of the Python findings
milestone. The useful finding → stronger-test → focused-rerun gate remains open.

## Inputs and adaptation

Upstream is [chrisdoherty4/python-case-converter](https://github.com/chrisdoherty4/python-case-converter),
commit `4b625748db63f7f20353b5b0815e42122d739314`. On 2026-10-20 its
original complete suite passed **107 tests in 0.29 seconds** on the pinned
CPython 3.14.7 / pytest 9.0.2 environment, with plugin autoload disabled, a fresh
bytecode prefix and pytest's cache provider disabled. The checkout remained
clean.

The retained [preparation script](fixtures/python-case-converter/prepare.py)
requires that exact pristine commit and a new destination. It retains all 22
production/test Python files, copies the license and README, and records SHA-256
hashes before and after adaptation. Production files are byte-identical.
The ten colocated `*_test.py` files move to `tests/test_*.py`; their relative
package imports become absolute imports, with wildcard imports expanded to the
exports declared by the original package initializer. Test cases, parameters
and assertions are unchanged. A minimal static `pyproject.toml` replaces the
upstream setuptools packaging entry point; no build/install hook executes.
This does **not** establish support for the original colocated layout.

The audit motivated narrowly admitted `from io import StringIO`, ordinary
logging stream setup, and `super(...).__init__` access. Direct logging module
access is limited to `getLogger`, `StreamHandler`, `Formatter` and level
constants; logging submodules, file handlers and shutdown remain rejected.
Other reflective attributes remain rejected. A local function parameter named
`input` is accepted as a value, while the interactive builtin and direct calls
remain rejected. Defaults and nested function scopes do not inherit this
exception. Eleven static scope regression tests pass.

## CLI evidence

The isolated `python-check` baseline passes the same **107 tests** and reports
7.38 seconds. Discovery finds six concrete production boolean flips.

The initial scored run finishes as **failed**, with three kills, one error,
and two pending mutations in 60.13 seconds. Flips in alternating-case state at
lines 10, 18 and 21 are assertion kills. Changing the boundary override's
`return False` at line 27 to `True` invokes an abstract handler and raises
`NotImplementedError`; the conservative reader records an unsupported body
exception as an error. It does not count that exception as a kill or publish
a full score. The observed error is expected classification, not a passing
mutation.

A subsequent 26.02-second run excluding `caseconverter/alternating.py` completes both
previously pending mutations as kills: the punctuation-stripping default and
macro-case delimiter-only default. This is a **2/2 filtered score**, not a
full-project score. Combined inventory accounting is five kills and one error,
with no survivor established. A second acceptance project or a genuinely useful
finding is still required; adding assertions just to restate these constants
would not satisfy the findings gate.

The retained [results record](fixtures/python-case-converter/results.json)
contains input hashes, both report hashes and exact score/summary records.
Python's four non-ignored Rust unit tests, eleven static scope tests and
Clippy with warnings denied pass after the scope changes.

All 22 adapted input hashes match after the failed run. Reports and temporary
evidence for this audit are under `/tmp/mutie-python-caseconverter-*`;
the original checkout is `/tmp/mutie-python-caseconverter-original`.
The failed report is
`/tmp/mutie-python-caseconverter-reports/run-1792524329-YvfTaD`.
Do not treat combined candidate accounting across subsequent runs as the
failed run's score.
