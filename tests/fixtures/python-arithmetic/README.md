# Scoped Python arithmetic fixture

Five binary arithmetic sites exercise `+ ↔ -` and `* ↔ /`. The five original
pytest cases kill four sites and leave the zero-fee subtraction alive. Adding
`assert net(10, 2) == 8` kills that survivor in a six-test focused rerun.

Run through the pinned runtime with:

```sh
MUTIE_PYTHON=/path/to/pinned-venv/bin/python cargo test --test python_workflow arithmetic_ -- --ignored --test-threads=1
```

The workflow also checks exact operator labels, source/report preservation,
corrupt saved operator rejection, native custom-dispatch errors and division by
zero. Static scope tests separately check token offsets, grouping, annotation
and unsupported-operator exclusions, operand order, reflected dispatch, floating
point values and early exceptions. No custom arithmetic or exception rewriting
is injected into the project.
