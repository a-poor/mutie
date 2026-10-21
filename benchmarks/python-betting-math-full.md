# Complete betting-math-kit candidate review

This continues [unchanged-project acceptance](python-betting-math.md) at
`0bd3c25e93df8974f432310835768d77a5e45d3e`, on the same pinned CPython 3.14.7 /
pytest 9.0.2 and unchanged Mutie binary. All original commands use the complete
198-test suite. There are 87 candidates: 20 boolean flips and 67 comparison
boundaries. No compatibility, discovery, classification, runtime or timeout rules
change in this review.

## Accounting and review

The ten prior metrics outcomes are reused by exact source identity. New commands
account for the other 77 candidates, continuing pending selections after errors.
The new commands take 571.60 seconds in total. Combined accounting is **19 kills,
59 survivors and 9 errors**, with no timeouts or unfinished candidates.
The full initial inventory supplies canonical IDs; report-local IDs are mapped
using file, source fingerprint, operator, byte span and original/replacement
text. This is complete inventory accounting across reports, not a combined full
mutation score. Failed reports retain their unavailable scores.

The [candidate-by-candidate review](fixtures/python-betting-math-full/candidate-review.md)
records every outcome and disposition. Numerical convergence tolerances remain
unresolved where no distinguishing representable input was demonstrated. Some
ordinary numeric/list paths are redundant because of earlier validation, length
checks or equal-value assignments; these limited observations do not establish
unrestricted equivalence or create dismissal records.

Errors are inspected rather than relabeled. Examples include rejection of valid
probability endpoints, rejection of two-outcome input, an edge-bin IndexError,
and division by zero after a validation guard is weakened. The decimal-odds
mutation produces both an expected-exception assertion failure and an unexpected
ZeroDivisionError; error precedence remains intact. Original error logs are
retained with canonical IDs under `fixtures/python-betting-math-full/error-logs/`.

## Stronger tests in a separate copy

The complete review copy adds 17 [public-contract tests](fixtures/python-betting-math-full/test_public_contracts.py)
and the seven previously validated metric cases. Two existing Kelly endpoint
tests retain their original value assertions and now explicitly fail if a valid
endpoint raises InvalidProbabilityError. The exact [test-only patch](fixtures/python-betting-math-full/kelly-tests.patch)
is retained. No application, configuration or dependency bytes change, and no
original test is deleted or deselected. The pristine review suite has 222 tests.

Useful contracts include exact pool and bankroll caps, exact minimum bet and
edge thresholds, observable constraint flags, full-takeout behavior, inclusive
ruin thresholds, and frozen result objects. These are software API contracts;
this work does not validate financial model accuracy or betting advice.

The 24-survivor focused rerun kills all 24 in 184.38 seconds: 18 new findings
and six reconfirmed metric findings. Its complete 222-test isolated baseline
passes, as does the native baseline (1.54 seconds).

The two original endpoint errors also become kills in a separate 31.45-second
focused rerun, with another passing 222-test baseline. This reflects the explicit
expected-success assertions in the test copy; the original error reports and
classification rules remain unchanged. Altogether, 35 original survivors and
seven original errors have no stronger kill in this review.

The selected stronger outcomes do not replace original outcomes or establish a
full stronger-suite score. The six earlier metrics findings are independently
reconfirmed in this larger copy. Numerical survivors and unselected errors
remain explicitly open in the review table.

## Reproduction and preservation

Use the same built CLI as the [previous acceptance](python-betting-math.md), a
clean original checkout, its previous results ledger, and a fresh output directory:

```sh
python3 -B benchmarks/fixtures/python-betting-math-full/run.py \
  --project /tmp/mutie-python-betting-math-original \
  --python /tmp/mutie-pytest-audit-env/bin/python \
  --mutie target/debug/mutie --output /tmp/new-betting-full \
  --previous /tmp/mutie-betting-math-final/results.json --stage original
python3 -B benchmarks/fixtures/python-betting-math-full/prepare_review.py \
  /tmp/mutie-python-betting-math-original /tmp/new-betting-full/review-project
# Repeat run.py with the same arguments and --stage review.
python3 -B benchmarks/fixtures/python-betting-math-full/verify.py \
  /tmp/new-betting-full /tmp/mutie-python-betting-math-original \
  /tmp/mutie-betting-math-final/results.json
python3 -B benchmarks/fixtures/python-betting-math-full/review.py /tmp/new-betting-full
```

The driver checks original input hashes, Git cleanliness, binary identity, prior
stronger-copy bytes and finalized report hashes before and after commands. The
verifier independently reconstructs the exact test changes, checks all 87
identities against their actual reports, and rechecks preliminary betting-math
and Route Mini report hashes. Native checks disable plugin autoload and cache
providers and use a separate bytecode prefix; the adapter retains its normal
fresh-bytecode and full test-inventory validation. Candidate and baseline limits
remain 30 and 60 seconds respectively.

The retained [ledger](fixtures/python-betting-math-full/results.json) contains
commands, individual timings, inventories, outcomes and preservation hashes.
Timings are single observations, not a controlled performance comparison. This
is an evidence/test-only change; the prior production regression results remain
recorded in the initial acceptance and were not rerun or claimed anew here.
