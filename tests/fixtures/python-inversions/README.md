# Python comparison inversion and negation fixture

Use the pinned CPython 3.14.7 / pytest 9.0.2 environment. Five original tests
exercise six comparison inversions, one multiline negation and two independent
nested negations. A NaN-only comparison test and an ignored negation result leave
two deliberately weak contracts: 11 selected candidates, nine kills and two
survivors. Stronger ordered-value and Boolean-result assertions kill both.

Run the production workflow (it copies this fixture to a disposable directory):

```sh
MUTIE_PYTHON=/path/to/pinned/env/bin/python cargo test --test python_workflow inversions_ -- --ignored --test-threads=1
```

The workflow also rejects corrupted comparison labels and negation recipes,
preserves original source/report outcomes, and independently proves native custom
comparison and raw-value TypeError failures remain errors. Native discovery and
semantic controls live in `tests/support/python_scope.py`.
