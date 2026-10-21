# Complete Route Mini comparison-boundary review

Validated 2026-10-21 on unchanged Route Mini commit
`35a3db157731ba0073d66f432b69b997ce2e9f01`, with pinned CPython 3.14.7 /
pytest 9.0.2 and CLI commit `a8806f5`. The original suite contains **68 tests**,
versus Cycler's 28. This revisits a known larger validation project; it is not a
new third project or evidence for large dependency-heavy applications.
The earlier [comparison acceptance](python-comparison-boundaries.md) selected
only router capacity. This run covers all seven supported comparison sites,
including six previously unscored retry and policy boundaries.

## Original run and complete accounting

The complete original suite passes both native and isolated pristine baselines.
The first candidate changes the constructor's `max_retries < 0` rejection to
`<= 0`. An existing router test legitimately uses zero retries; the mutation
raises ValueError. Mutie correctly records an error and stops, leaving six
pending candidates. The original failed report stays failed, with no full score.
A focused run of the six pending identities completes without errors/timeouts.

| Original ID | Location | Outcome |
| --- | --- | --- |
| 1 | fallback.py:41, zero retries rejected | Error |
| 2 | fallback.py:65, rate-limit retry ceiling | Killed |
| 3 | fallback.py:66, rate-limit fallthrough ceiling | Killed |
| 4 | fallback.py:72, network retry ceiling | Survived |
| 5 | policy.py:79, exact budget cap | Killed |
| 6 | policy.py:91, exact latency cap | Killed |
| 7 | router.py:115, exact capacity | Survived |

This accounts for all seven candidates across two reports: four kills, two
survivors, one error. It is not a combined full-project score. Candidate 7 is the
previously demonstrated capacity finding, whose earlier stronger-test report is
preserved. Candidate 4 is a distinct new useful finding.

## New finding: an extra call after the network retry budget is exhausted

The mutation changes `attempt < self.max_retries` to `attempt <= self.max_retries`.
The original router test checks successful fallback to the secondary provider
with a zero retry budget, but does not check the number of primary calls. The
mutated router makes an extra call to the failing primary before falling back;
the original assertions still pass. This can waste a request and delay fallback.

A separate complete project copy adds two tests. One checks the public fallback
decision at exactly one exhausted retry. The other checks the actual router
workflow at zero retries, asserting one primary call, one secondary call and two
total attempts, as well as the successful secondary response. All original tests,
application source and metadata stay byte-identical.

The native and isolated **70-test stronger baselines pass**. A focused rerun of
original candidate 4 completes as **one kill, zero survivors/errors/timeouts**.
Both new tests fail under the mutation: the decision permits another retry, and
the primary call count is two instead of one. See the retained
[mutation log](fixtures/python-route-mini-all-boundaries/stronger-mutation.log).
No classifier, observer, runtime, timeout, source semantics, support scope or
versions changed. The original constructor error remains unresolved in its failed
report; this work does not reclassify it or claim a full stronger-test score.

## Reproduction and preservation

The [driver](fixtures/python-route-mini-all-boundaries/run.py) takes the original
acceptance and review ledgers, the CLI, and a new output directory:

```sh
python3 -B benchmarks/fixtures/python-route-mini-all-boundaries/run.py \
  --previous /tmp/mutie-route-mini-acceptance/results.json \
  --review /tmp/mutie-route-mini-review-final/results.json \
  --output /tmp/mutie-route-mini-new-all-boundaries \
  --mutie target/debug/mutie
```

The retained execution used `--resume` to continue after inspecting the original
error and subsequent survivors. Neither failed outcomes nor pending candidates
were hidden. The [verification script](fixtures/python-route-mini-all-boundaries/verify.py)
accepts the output directory followed by prior result-ledger paths. It verifies
original inputs, stronger-copy changes, saved report hashes, complete candidate
accounting and original-ID mappings using source fingerprint, file, operator,
byte span and exact tokens, rather than report-local IDs.

[Results](fixtures/python-route-mini-all-boundaries/results.json) retain commands,
timings, identities, inventories, outcomes and hashes. All original input hashes,
original/review/earlier-comparison report hashes and new report hashes matched.
The original checkout remained clean. The only stronger-copy addition is
[the new test file](fixtures/python-route-mini-all-boundaries/test_retry_boundary.py).

| Command | Seconds |
| --- | ---: |
| Discovery | 2.35 |
| Native original baseline | 0.50 |
| Original full selection, stopped on error | 15.15 |
| Original remaining six candidates | 41.32 |
| Native stronger baseline | 0.46 |
| Stronger network focused rerun | 16.96 |

These are individual workflow timings, not controlled performance measurements.
Repository whitespace checks and the explicit preservation/identity verification
pass. No Rust implementation changed, so no new full Rust-suite run is claimed.

## Selection limits

The [initial candidate survey](fixtures/python-route-mini-all-boundaries/candidate-survey.json)
records current static-scope rejection reasons across existing checkouts. Larger
alternatives often require plugins, reflective APIs, dependencies or broader test
selection. An additional disposable diagnostic separated package/test code from
metadata to locate executable blockers; none of those adapted copies was accepted
or scored. Route Mini was selected to produce meaningful new evidence without
trimming an upstream suite or broadening scope solely to get a result.
