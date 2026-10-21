# Complete betting-math-kit arithmetic review

This continues the [initial arithmetic acceptance](python-arithmetic.md) on
unchanged [betting-math-kit](https://github.com/bene-art/betting-math-kit), commit
`0bd3c25e93df8974f432310835768d77a5e45d3e`, with its complete 198-test suite.
The pinned CPython 3.14.7 / pytest 9.0.2 environment, production binary,
validator, observer, classifier and execution recipe remain unchanged.

## Accounting and identity

All **194 arithmetic candidates** are accounted for: **61 kills, 97 survivors,
36 errors and no timeouts**. The previous 30-site selection contributed
21 kills/eight survivors/one error. The other 164 contributed 40 kills,
89 survivors and 35 errors over 35 stopped runs, taking 1,003.589 seconds in
aggregate. These are single observations, not controlled performance measurements.

The [error review](fixtures/python-arithmetic-full/error-review.json) retains all
36 original error logs and failed-test summaries. Thirteen candidates raise
`ValueError`, ten `TypeError`, four `ZeroDivisionError`, three a mixture of
`ValueError` and `ZeroDivisionError`, three `IndexError`, and three
`InvalidOddsError`. Assertion failures can coexist with these exceptions; the
conservative classifier still reports an error. None is rewritten as a kill.

The [results ledger](fixtures/python-arithmetic-full/results.json) contains the
194-site arithmetic inventory, commands, timings, report data and SHA-256 maps.
The [candidate review](fixtures/python-arithmetic-full/candidate-review.md)
records every disposition, including open numerical questions and limited
ordinary-path redundancy observations. None is an equivalence dismissal.

The first acceptance already executed 30 Kelly candidates. This review reuses
those outcomes by exact file, source fingerprint, operator, byte span and
replacement identity, then executes the other 164. An error stops a run; the
next focused run selects the remaining identities from the initial report.
Every execution still runs the complete pristine suite and full candidate suite.
The verifier checks that each new identity was completed exactly once.

Canonical IDs in this review are arithmetic-only source order. They are **not**
report-local IDs: the driver translates them through source identity, and the
ledger records both. Failed reports retain their original null score. Combined
accounting across stopped runs is not a full-run mutation score.

## Useful stronger-test evidence

A separate copy adds only
[`tests/test_numeric_contracts.py`](fixtures/python-arithmetic-full/test_numeric_contracts.py).
All upstream implementation, metadata and tests remain byte-for-byte unchanged.
Its 21 additional cases bring the full pristine suite to 219 passing tests.

The tests check independently observable software contracts:

- Original implied margin remains correct across devig methods, and unequal
  outcomes retain favorite ordering. Additive normalization handles valid skew.
- Calibration and edge bins return exact boundaries, counts, means and rates.
- Non-even odds produce the expected Kelly magnitude and scaling.
- Seeded one- and two-bet trajectories produce exact bankroll statistics,
  percentile interpolation, drawdown and growth. Fraction searches use the
  documented incrementing seeds.
- Valid log-loss endpoints are clamped to a finite result.

Expected-success assertions for valid additive/log-loss inputs turn an unexpected
`ValueError` into a test assertion. This is a test contract in the separate copy,
not exception conversion in Mutie. Other runtime failures remain errors. These
checks validate software behavior, not the quality of a financial model.

The focused 56-candidate run completes in **249.147 seconds** with **54 kills,
two survivors, no errors and no timeouts**, after a passing isolated 219-test
baseline. The native 219-test baseline takes 1.019 seconds. The exact
[selection](fixtures/python-arithmetic-full/selected-ids.json) and
[representative assertion logs](fixtures/python-arithmetic-full/stronger-logs)
are retained. Canonical IDs 4 and 46 (two- and multi-outcome power midpoint
addition) still survive; favorite ordering did not distinguish them.

Together with the prior eight kills, separate-copy evidence now kills **62 of
the original 97 survivors**. **35 remain unresolved**, and all 36 original errors
remain recorded. This is not a combined full stronger-test score.

The experimental driver initially expected every selected survivor to be killed;
its post-run assertion rejected the two surviving results after saving the
successful CLI report. The retained driver now requires complete execution and
records actual outcomes, while the independent verifier checks each against the
saved report. No tests, classifier or report outcomes were altered to turn those
two survivors into successes.

The prior eight Kelly stronger-test kills remain separate evidence under their
original 201-test copy. They are not rerun or combined into a full stronger score.
The new selection does not claim all remaining numerical behavior is correct.
Shin accuracy/search, convergence tolerances and fallback branches need their
own independent oracles or justified input cases. The candidate review keeps
those questions open.

## Reproduction and preservation

Use the retained [driver](fixtures/python-arithmetic-full/run.py) with the
previous arithmetic acceptance ledger and unchanged binary:

```sh
python3 -B benchmarks/fixtures/python-arithmetic-full/run.py \
  --project /tmp/mutie-python-betting-math-original \
  --python /tmp/mutie-pytest-audit-env/bin/python \
  --mutie target/debug/mutie \
  --output /tmp/new-python-arithmetic-full \
  --previous /tmp/mutie-python-arithmetic-acceptance/results.json \
  --stage original
```

Prepare the separate copy:

```sh
python3 -B benchmarks/fixtures/python-arithmetic-full/prepare_review.py \
  /tmp/mutie-python-betting-math-original \
  /tmp/new-python-arithmetic-full/review-project
```

Then repeat the driver with the same paths, `--stage review` and `--ids` containing the
canonical IDs listed in the recorded stronger selection. Run
[`verify.py`](fixtures/python-arithmetic-full/verify.py) and
[`review.py`](fixtures/python-arithmetic-full/review.py), each with positional
arguments for output directory, original project and previous ledger.

Preservation checks surround every command. The final verifier checks original
Git cleanliness and file hashes, the unchanged binary, all finalized report
bytes and actual JSON data, prior stronger copies, historical ledgers and reports,
and the new copy's sole added test file. Native runs disable plugin autoload and
cache providers and use separate bytecode prefixes. The adapter retains fresh
bytecode prefixes and complete pristine inventories. Candidate/baseline deadlines
remain 30/60 seconds; no readiness bound or assertion is weakened.

This is an evidence-only follow-up. No production code, support boundary or
classification rule changes, and no version bump. The full Cargo, Clippy and
formatting results from the initial arithmetic implementation remain its regression
evidence; this review validates actual isolated Python runs and preservation.
