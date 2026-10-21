# Route Mini remaining-finding review

Reviewed 2026-10-21 against the unchanged commit and reports recorded in
[initial acceptance](python-route-mini.md). This review starts with the remaining
25 survivors, three errors and one timeout. Application code, original tests,
metadata, original report bytes and the preceding stronger-test copy remain
unchanged. All edits below exist only in a new separate review copy.

## Retry and fallback behavior

The upstream fallback documentation requires immediate fallthrough for provider
failures and fallthrough for unknown errors. The unknown-error test checked only
fallthrough, leaving `retry_same` untested. A new test checks both flags, error kind
and zero backoff at attempts 0, 1 and 5; it kills original survivor **14**.

Original **10** forces provider-down retries indefinitely. ScriptedProvider emits
another 503 after its finite script is exhausted, so waiting longer cannot repair
this mutation. Every router test now checks the no-retry precondition before
calling the router. This is a body assertion on the actual FallbackChain result,
not a skip, fixture failure, reduced timeout, or changed application behavior.
The mutation now fails promptly and is killed with the same 20-second limit.

Original **9** (network fallthrough), **11** (provider-down fallthrough), and
**20** (router loop entry) previously combined assertions with unexpected
FallbackExhaustedError. Expected-success routes now explicitly turn that specific
unexpected exception into `pytest.fail`, while preserving the original success
assertions. Expected-failure routes retain their original `pytest.raises` blocks.
The native baseline still executes every upstream test. These are stronger
expectations in tests, not relaxed runner classification: the classifier and
observer are unchanged. The final focused rerun kills all five selected findings,
with no errors or timeouts. Original failed reports remain failed and unscored.

An intermediate review still left the telemetry-success call unwrapped: four kills
and one error. That report remains intact in `/tmp/mutie-route-mini-review-accepted`.
The final patch covers that expected-success call too. An earlier static preview
rejected the helper parameter name `request` under fixture-request restrictions;
renaming it `route_request` required no scope change. That attempt's native
85-test baseline passed; its isolated baseline did not.

## Immutable records and slots

Twelve new tests assign to ordinary fields on AuditDecision, ProviderError,
FallbackDecision, ModelCapabilities, ScriptedSuccess, ProviderRollup, Provider,
Cost, Model, Request, Response and Decision. Each expects FrozenInstanceError.
These test the declared shallow immutability contract; they do not claim that
nested metadata dictionaries are deeply immutable.

The twelve `slots=True` candidates are a different concern. A separate **native,
unscored** diagnostic applies each mutation in a disposable copy. Eleven instances
gain a dictionary; ProviderError already inherits one from Exception, but loses its
class slot declaration. All twelve therefore have observable layout differences;
none is declared equivalent. These observations use reflective attributes excluded
from Mutie's scoped Python tests. No reflective API was admitted, no dismissal was
created, and no diagnostic result was counted as a scored kill. The reviewed
survivors remain open for a future concrete layout/memory contract, rather than
testing incidental exception text or expanding scope solely to eliminate them.

The retained [native diagnostic](fixtures/python-route-mini-review/probe_slots.py)
and [results](fixtures/python-route-mini-review/slots-results.json) distinguish
this evidence from supported scored execution.

## Reproduction and preservation

The [review driver](fixtures/python-route-mini-review/run.py) uses the initial
acceptance `results.json`, pinned Python launcher and original full inventory.
It verifies every original input/report hash and the earlier stronger copy before
and after commands. New report IDs are mapped back using source fingerprint,
file, operator, byte span, original and replacement. It also hashes the entire
new stronger project before and after runs. No original reports are rewritten.

```sh
python3 -B benchmarks/fixtures/python-route-mini-review/run.py \
  --previous /tmp/mutie-route-mini-acceptance/results.json \
  --output /tmp/mutie-route-mini-new-review \
  --mutie target/debug/mutie
python3 -B benchmarks/fixtures/python-route-mini-review/probe_slots.py \
  --previous /tmp/mutie-route-mini-acceptance/results.json \
  --output /tmp/mutie-route-mini-new-slots.json
```

The driver adds the previous four audit-boundary cases and the new thirteen cases,
for **85 tests** total. Both native and isolated baselines pass. The retained
[test file](fixtures/python-route-mini-review/test_review_contracts.py) and
[router patch](fixtures/python-route-mini-review/router-tests.patch) show all new
expectations. No upstream test is removed or deselected. No product source,
execution recipe, timeout, classifier, schema or support boundary changes.

## Final results

| Run | Tests | Kills | Survivors | Errors | Timeouts | Seconds |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Native stronger baseline | 85 | — | — | 0 | 0 | 0.53 |
| Isolated stronger baseline | 85 | — | — | 0 | 0 | 8.81 |
| Retry/fallback focused rerun | 85 | 5 | 0 | 0 | 0 | 46.43 |
| Dataclass focused rerun | 85 | 12 | 12 | 0 | 0 | 220.57 |

All 29 previously open findings are reviewed. Original IDs **9, 10, 11, 14, 20**
are killed in the first selection. Frozen-record IDs **1, 4, 7, 16, 18, 25, 27,
29, 31, 33, 35, 37** are killed in the second. Slots IDs **2, 5, 8, 17, 19, 26,
28, 30, 32, 34, 36, 38** remain survivors with the native observations described
above. These are two focused runs, not a new full 38-candidate score. The previous
three stronger audit-status kills and six original kills are separate evidence.

[results.json](fixtures/python-route-mini-review/results.json) retains final
commands, inventories, original-ID mappings, outcomes, scores and all report hashes.
[Intermediate results](fixtures/python-route-mini-review/intermediate-results.json)
retain the rejected preview and the four-kill/one-error attempt. Final verification
recomputed all original and current report hashes, original and both stronger-copy
input hashes, and confirmed the original Git checkout is clean. Every final local
outcome was checked against its original source identity and expected disposition.
Python syntax and repository whitespace checks pass. This change adds evidence,
review scripts and test-only artifacts; it changes no Rust implementation and does
not claim a new broad Rust-suite run. The preceding full-suite acceptance remains
recorded in [process readiness](process-readiness.md).
