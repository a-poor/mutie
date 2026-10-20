# Cycler scoped execution control

Inspected [matplotlib/cycler](https://github.com/matplotlib/cycler) at commit
`d1737eb0a42bd451e8770cef245ebe60ebb36bfc` on 2026-10-20. The complete original
suite passes **28 tests in 0.04s** on CPython 3.14.7 / pytest 9.0.2, with plugin
autoload and pytest's cache provider disabled and a fresh bytecode prefix.

The retained [preparation script](fixtures/python-cycler/prepare.py) copies the
entire production package unchanged and moves the original `test_cycler.py`
unchanged to `tests/`. It replaces dynamic packaging metadata with the scoped
static manifest and records all input hashes. No test assertions are dropped.
The baseline motivated audited `copy.copy` and explicit `__getitem__` access;
copy internals, deep copy and other reflective attributes remain rejected.

The isolated scored baseline passes. Discovery finds three equality-related
boolean flips. The first, `cycler/__init__.py:390` (`False` → `True` for
comparison with a non-Cycler), changes internal comparisons as well as public
equality. The original suite produces 14 assertion/pytest.fail failures and one
`KeyError` failure. The reader conservatively records an **error**, stops with
two candidates pending, and publishes no full score. Runtime: **18.97s**.

This is an additional real mixed-failure classification control, **not** a
survivor or completed findings loop. The report is retained at
`/tmp/mutie-python-cycler-reports/run-1792524816-6HuAzj`.
The [Scrubify milestone](python-scrubify-findings.md) supplies the useful finding
and stronger-test rerun instead.
