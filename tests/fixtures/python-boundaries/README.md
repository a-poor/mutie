# Python comparison boundaries

This independent pinned-runtime fixture has ten tests and five mutations:
four equality-boundary kills and one budget survivor. Tests retain operand-call
order/single evaluation and native NaN behavior. Chained comparisons are excluded.
A separate stronger test at exact budget equality kills the survivor.

Run the opt-in production workflow from the repository root:

```sh
MUTIE_PYTHON=/path/to/audited-venv/bin/python cargo test --offline \
  --test python_workflow comparison_ -- --ignored --test-threads=1
```

The second workflow is a negative control: changing native rich-comparison
dispatch to a method that raises RuntimeError must produce an error, not a kill.
See [scope and real-project evidence](../../../benchmarks/python-comparison-boundaries.md).
