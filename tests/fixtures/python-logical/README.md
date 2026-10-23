# Python binary logical-swap fixture

The pinned CPython 3.14.7 / pytest 9.0.2 workflow runs five original tests against
six swaps: binary conjunction/disjunction, two nested sites, and two deliberately
weak same-value tests. Expected outcomes are four kills and two survivors.
Two stronger tests distinguish the operand values; the seven-test baseline passes
and both survivors are killed in a focused rerun.

```sh
MUTIE_PYTHON=/path/to/pinned/env/bin/python cargo test --test python_workflow logical_swaps_ -- --ignored --test-threads=1
```

The workflow copies inputs to temporary directories, checks source/report
preservation and rejects corrupted saved operator labels and keyword replacements.
A separate scored case proves that a newly evaluated right operand raising
ValueError remains an error. Native scope tests cover raw values, stateful
truthiness, grouping and explicit flat-chain exclusions.
