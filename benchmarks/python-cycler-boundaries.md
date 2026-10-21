# Unchanged Cycler comparison-boundary validation

Recorded 2026-10-21 against the original Cycler checkout at
`d1737eb0a42bd451e8770cef245ebe60ebb36bfc`, using the pinned CPython 3.14.7 /
pytest 9.0.2 environment and current CLI. This follows the
[original boolean acceptance](python-cycler-unchanged.md) without changing any
project source, tests or metadata.

The comparison-only full run passes the original 28-test pristine baseline and
completes one candidate: **zero kills, one survivor, no errors or timeouts**,
with a raw selected-operator score of 0%. The run takes 17.97 seconds; discovery
takes 2.81 seconds. These are single observations, not performance benchmarks.

## Finding review

At `cycler/__init__.py:542`, `len(args) > 2` becomes `len(args) >= 2`.
Here `args` is the function's ordinary positional-argument tuple. The preceding
`elif len(args) == 2` branch returns `_cycler(*args)`. If that call raises, control
also leaves the function. Consequently, the changed comparison is never evaluated
when the tuple length is two. At every reachable length the original and mutated
comparisons agree. There is no custom comparison dispatch or mutable length here.

This is a justified equivalent survivor within the audited runtime semantics.
No stronger test can distinguish these predicates on a reachable path, so no
stronger-test copy or rerun was manufactured. No dismissal was written and the
saved raw score remains unchanged. This adds independent unchanged-project
operator evidence, but does not add a useful test-gap finding beyond the existing
[Route Mini boundary finding](python-comparison-boundaries.md).

## Reproduction and preservation

The [driver](fixtures/python-cycler-boundaries/run.py) accepts `--project`,
`--mutie`, `--python`, and a fresh `--output` directory. It verifies the pinned
commit and original input hashes against the prior acceptance ledger, checks Git
cleanliness and every prior saved report after commands, and checks that the CLI
hash stays unchanged. It runs discovery followed by a comparison-only scored run
with a 20-second candidate timeout and 30-second baseline timeout.

[Results](fixtures/python-cycler-boundaries/results.json) retain the command
records, input and CLI hashes, complete candidate/outcome identity, collected
test inventory, run metadata, score and new report hashes. Original inputs and
all prior report hashes matched. No implementation, classifier, deadlines,
assertions, support scope or versions changed. The real-project workflow passed;
repository whitespace checks passed. The earlier six passing default suites are
separate [startup investigation evidence](cli-startup-readiness.md), not a new
full-suite claim for this documentation-only acceptance step.
