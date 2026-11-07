# Scoped Python fixture

One flat dependency-free package and two pytest tests exercise discovery,
isolated baselines and the scored workflow. Use CPython 3.14.7 in a dedicated venv
with the pinned [audit requirements](../../../benchmarks/fixtures/pytest-audit/requirements.txt).
Pass the venv's launcher with `--python`; do not resolve its symlink.

Discovery finds `enabled`'s `True` and `optional`'s `False`. The current tests
assert `enabled`'s result but only check that `optional` is callable. These are
the scored run's kill/survivor controls. The integration test adds a stronger
test and reruns the survivor, while checking original source/report preservation.

```sh
mutie init tests/fixtures/python --lang python --python /path/to/venv/bin/python
mutie run tests/fixtures/python
```

`python-list` and `python-check` remain read-only discovery/baseline commands
and do not create scored reports.

The root Cargo suite does not require Python. Opt in with:

```sh
MUTIE_PYTHON=/path/to/venv/bin/python \
  cargo test --test python_workflow -- --ignored --test-threads=1
```
