# Experimental Node test-runner slice

Recorded 2026-10-18 on macOS. This is JavaScript-only acceptance for a narrow first
adapter, not general Node/TypeScript support. The assertion-only attribution limit
below is historical; [the follow-up audit](node-body-attribution.md) records its
replacement with exact body evidence and additional late-failure checks.
Implementation is exposed as
`mutie PATH --lang node`; no package/schema/catalog version was changed.

## Scope and selected project

- Runtime: Node **24.22.0**, resolved from PATH once and revalidated through its
  pinned executable for every command. The local audited binary was
  `/path/to/home/.vite-plus/js_runtime/node/24.22.0/bin/node`.
- One ESM package (`type: module`), no installed `node_modules` or runtime
  dependencies, `.js` runtime files under `src/` and `test/`, literal relative
  imports within those directories or `node:` built-ins. Unused build-only
  devDependencies are accepted; no install/build/package script is run.
- Exact package test script: `node --test test/*.test.js`. The adapter expands
  this immediate-directory set and runs serial test files in isolated processes.
  Custom loaders/flags, CommonJS, workspaces, runtime TypeScript and package
  dependencies are rejected. Native test-file parallelism and Mutie workers are
  restricted to one; successful nested hooks and the audited in-file concurrent
  mixed-failure case have regression coverage, not a general concurrency claim.
- Mutation coverage: `boolean_flip` only. Full suite, fresh command copies,
  separate owned reporter/observer/temp directories, cleared NODE_OPTIONS and
  NODE_PATH. Source validation checks the original root before snapshotting,
  including workspace ancestry, and checks copied imports before execution.
- Real project: [sri-dynamic-loader](https://github.com/subresource-integrity/sri-dynamic-loader)
  at `370652c304d6fee0c59cfb51bf097e72f6ebf13a`. Its original four test files pass
  60 tests without installing its build-only tooling. No production source or
  original tests were adapted. It uses a local DOM stub, not a browser/network
  test environment. This evidence does not validate browser execution.

The original source contains literal NUL bytes in comments and a template string.
Tree-sitter treated those as end-of-input. Shared `javascript.rs` now masks those
bytes only for parsing and requires each masked location to be literal text,
never executable code or a template substitution. Byte offsets, fingerprints,
mutation writes and saved diffs still use the original source. Regression checks
include booleans after NULs and rejection of NULs in code.

## Completion and failure audit

Reference: [Node's test runner documentation](https://nodejs.org/docs/latest-v24.x/api/test.html).
The observations below come from direct probes and the embedded implementation
of the pinned runtime, not an assumption that exit status identifies test failures.

The owned reporter collects bounded completion evidence. Each selected file must
have one summary and one worker lifecycle record; global counts must match file
sums, and the stream must finish. Exit/result disagreement is an error. Empty or
all-skipped files, todo/cancelled results, retry and expected-failure modes are
unsupported. Saved-log truncation does not truncate this classification evidence.

Node's native implementation suppresses afterEach/after errors once the body has
already failed. A reporter-only implementation therefore misattributes some mixed
failures. The observer watches native TestHook resources through async_hooks,
wraps their run method, and checks completion/error before the parent suppresses
it. The internal `internal/test_runner/test` SHA-256 must be exactly:

```text
6329a64a8825bf217d60526f7a53fd89af6fc5107ad9f15d41b825b19a587fa2
```

The adapter and each worker verify that hash. Observer/reporter bytes are part of
saved execution identity. There is no uninstrumented fallback. This depends on
private internals, so changing Node versions requires a new audit.

Only `testCodeFailure` with an `ERR_ASSERTION` cause is accepted as a kill. Parent
`subtestsFailed` events are structural evidence, not additional mutant kills.
A complete successful run is a survivor. Generic body exceptions are currently
errors, even when manual inspection establishes that a mutation caused them.
Hook errors and mixed errors cannot earn kills. Process timeout, cancellation and
capture-overflow precedence is shared with the other adapters.

`tests/node_workflow.rs` exercises 20 mutation-triggered cases after passing
baselines: plain/nested assertion kills; before/after/beforeEach/afterEach failures;
suppressed body-plus-cleanup failures; async and callback cleanup; successful
callback cleanup; nested and in-file concurrent mixtures; plan failures; generic
body errors; empty/all-skipped files; zero exit and cleanup exit; late errors and
import-time errors. Failed cases persist errors and withhold scores, including
when a minimum-score policy is requested.

The opt-in adapter cleanup test separately verifies timeout, cancellation and
capture overflow with a spawned descendant. It checks descendant termination,
removal of owned work/temp directories, preserved source, and retained classifier
precedence. This is process-group cleanup evidence, not a security sandbox claim.

## Original inventory and findings

[Machine-readable results](node-sri-results.json) preserve source IDs, outcomes,
run identifiers/times, recipe identity and all 32 original file hashes.

All **21** discovered candidates were attempted across nine reports. The original
full run stopped at its first execution error; saved-ID reruns completed the rest.
Combined accounting is **8 killed, 8 missed, 4 errors, 1 timeout**. This is not a
whole-project mutation score. Failed reports keep scores unavailable; successful
subsets retain their rerun scope. The initial run took 38.92 seconds; aggregate
elapsed time across the nine reports was 96.14 seconds, including repeated
baselines. These runs overlapped verification work and are not a performance
comparison.

| Source IDs | Original result | Inspection |
| --- | --- | --- |
| 1, 3–7, 14, 17 | Killed | Existing assertions detect these changes |
| 2 | Missed | Invalid-integrity wrapper's false result was unasserted |
| 9 | Missed | Cleanup's settled flag; may be redundant with listener/timer removal, not proven equivalent |
| 11, 12 | Missed | Missing stylesheet collection and matching collection entry were not exercised |
| 18, 19 | Missed | Boolean crossorigin aliases were not tested directly |
| 20, 21 | Missed | No-base and invalid-base origin fallback paths were unasserted |
| 10, 13, 15, 16 | Error | Non-assertion failures: stylesheet timeout, TypeError, registry replacement rejection, unverified-load rejection |
| 8 | Timeout | Starting already settled prevents load completion; exceeded the 5-second command budget |

The four errors expose an adapter limitation, not four demonstrated missing tests.
Do not convert them into kills merely because Node exits unsuccessfully. Broader
body-error attribution is the next implementation gate.

## Stronger-test review and rerun

[Five retained boundary tests](fixtures/node-sri/mutie-stronger.test.js) were copied
into a separate review checkout's `test/` directory. Original production source,
original tests, manifests and all other original files remained byte-identical.
The resulting 65-test baseline passes. The test additions assert invalid-integrity
results, crossorigin aliases, origin fallbacks and both stylesheet polling paths.
Expected successful calls use explicit doesNotThrow/doesNotReject assertions so
this narrow adapter has assertion evidence when a mutation makes them throw.

A saved selection of all eight original survivors completed in **16.51 seconds**:
**7 killed, 1 missed, 0 errors/timeouts**. The seven kills are IDs
2, 11, 12, 18, 19, 20 and 21. ID 9 remains for separate review. The reported 87.50%
is for this eight-candidate rerun only; it is not an improved whole-project score.
An earlier test draft left a thrown normalization error unwrapped; that rerun
correctly failed without a score. The retained version makes that expectation
explicit. No production behavior was changed to obtain kills.

The real reports live locally under `/tmp/mutie-node-sri-reports` and
`/tmp/mutie-node-sri-stronger-reports`; their identities are retained in the
JSON record. A final saved-ID rerun confirms all 271 captured report-file hashes are unchanged.
The automated fixture also verifies report preservation, reader rejection of bad
recipe identity, test-only reruns, changed-manifest rejection, dismissal CRUD/check
without Node on PATH, and separate raw-score/findings policy semantics.

## Reproduction and checks

With the audited runtime on PATH and a checkout of the recorded commit:

```sh
mutie /path/to/sri-dynamic-loader --lang node --ui plain --timeout 5
mutie /path/to/sri-dynamic-loader --lang node --list
# Add the retained test file to a review copy, then select original source IDs:
mutie /path/to/review --lang node --rerun ORIGINAL_RUN --id 2,9,11,12,18,19,20,21
cargo test --test node_workflow -- --include-ignored --test-threads=1
cargo test --bin mutie node::tests::timeout_cancellation_capture_and_owned_files -- --ignored
cargo test
cargo fmt --check
cargo clippy --all-targets -- -D warnings
```

Default suite: 242 passed, 45 ignored at the broad verification checkpoint.
The final Node workflow run passes all three tests (including unsupported-scope
preflight); its mutation-attribution test covers the 20 cases above. Cleanup and
Node project/protocol unit checks pass separately. Formatting and strict Clippy
pass. Real Node tests stay ignored by default to avoid recursive self-mutation.

Keep JavaScript **Experimental** until non-assertion attribution and broader
project evidence improve. TypeScript, package dependencies, loader/config variants
and wider runtime versions need their own scope decisions and acceptance.
