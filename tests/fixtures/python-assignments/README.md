# Python assignment removal

Two item assignments exercise checked and unchecked updates. The scored workflow
expects one kill and one survivor, then kills the survivor with a stronger test.
It checks review policy, raw score, exact recipes, forged local-binding spans,
operator identity and preserved inputs/reports. Separate attribute-initialization
and mixed body/fixture-cleanup controls remain errors, never kills.

```sh
MUTIE_PYTHON=/path/to/pinned/venv/bin/python \
  cargo test --test python_workflow assignments_ -- --ignored --test-threads=1
```
