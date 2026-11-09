# Python call removal

Two independent call sites exercise a checked list append and an unchecked
conditional append. The workflow expects one kill and one survivor; an added
positive-input test kills the survivor. The source is never modified in place.

Run with the pinned CPython 3.14.7 / pytest 9.0.2 environment:

```sh
MUTIE_PYTHON=/path/to/venv/bin/python \
  cargo test --test python_workflow calls_ -- --ignored --test-threads=1
```

The workflow also checks review policies, corrupt recipes, a forged non-call
identity, stale source and saved-report preservation. A separate test creates
runtime-initialization and mixed body/fixture-cleanup failures; both remain
errors with zero kills.
