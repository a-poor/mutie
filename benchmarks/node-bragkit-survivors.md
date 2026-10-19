# Bragkit remaining survivor review

Recorded 2026-10-18 with pinned Node **24.22.0** on macOS. This completes the
findings review of [unchanged Bragkit acceptance](node-default-discovery.md), at
commit `458d31127f1d3a6f3fd28afd2d7a4741ebc648a7` of
[Bragkit](https://github.com/artmsilva/bragkit).

The original full scan remains **2 killed, 21 missed**, score **8.70%**. Three
HTML-list survivors were killed in the preceding review. This separate focused
rerun kills all **18 remaining survivors**, with **no errors or timeouts**, in
**53.39 seconds**. Its 100% score applies only to that selection. All 21 original
survivors now have stronger-test kills across the two review runs; this is finding
accounting, not a new whole-project score.

## Stronger tests and observed failures

The review copy retains the previous HTML tests and adds
[`mutie-boundaries.test.ts`](fixtures/node-bragkit/mutie-boundaries.test.ts).
All **76 baseline tests** pass, both directly and through Mutie's isolated adapter.
The 13 new tests exercise public CLI/collector/PDF functions. Original source IDs:

| IDs | Boundary | Mutant behavior observed |
| --- | --- | --- |
| 1, 8, 9, 10 | Unexpected positional arguments for collect/export/list/runs | Invalid arguments silently accepted |
| 2 | Default collection output | JSON emitted without opt-in |
| 3, 5, 6 | Collector options passed by CLI | Enrichment/created/updated flags enabled by default |
| 4 | Quiet JSON mode | Progress text unexpectedly written to stderr |
| 7 | Positional report year | Valid year rejected |
| 11, 12 | Nested database/output paths | Missing ancestor directories cause ENOENT |
| 13, 15 | Confluence/Jira optional contributions | Extra updated/created achievements returned without opt-in |
| 14 | GitHub enrichment | Extra change statistics/description returned by default |
| 16, 17 | Slack response and transport errors | Authentication failure lost; empty successful-looking result returned |
| 23 | Missing browser fallback | Success reported despite only HTML being produced |

Collector tests use synthetic responses and credentials. Fetch, GitHub subprocesses,
and browser lookup are stubbed; no network service, account, credential manager or
browser is contacted. Explicit opt-in controls also exercise successful enrichment
and expanded contributions. Default config reads return an empty fixture config;
database/output writes use memory or owned temporary directories. Each test body
restores mocks, synchronized builtin bindings and environment variables in `finally`.

Saved logs identify the expected failing test for every mutant: 15 assertion
failures, one argument exception, and two missing-directory exceptions. These are
completed body failures accepted by the existing exact-worker attribution; no
observer, adapter, production source or classification changes were needed.

## Preservation and reproduction

[Machine-readable results](node-bragkit-survivors-results.json) include exact
source ranges, original-ID mapping, failures, runner identity, timings and report
hashes. All **57 original files** in both the untouched project and review copy
match the prior recorded hashes. All **105 original report files** remain unchanged.
The review only adds the two retained test files, with the original `node --test`
script and config intact.

```sh
# Copy both retained test files into test/ in a separate unchanged Bragkit checkout.
# Put audited Node 24.22.0 on PATH.
node --test --test-concurrency=1
mutie /path/to/review --lang node --ui plain --timeout 10 --baseline-timeout 15 \
  --output-dir /path/to/review-reports --rerun ORIGINAL_RUN \
  --id 1,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,23
```

The native-TypeScript milestone remains Validated within its existing scope. These
are deterministic mocked integration boundaries, not validation of live services,
browser rendering or arbitrary TypeScript packages. No compiler typecheck was run;
the evidence concerns native type stripping. Broader test layouts, runtime
dependencies, loaders and other runtimes remain separate scope decisions.
