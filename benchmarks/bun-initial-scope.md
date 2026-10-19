# First Bun scope and failure-attribution audit

Recorded 2026-10-18 on macOS arm64. Selected runtime: installed Bun
**1.3.11+af24e281e**, pinned by revision and executable SHA-256 in the
[runner evidence](bun-runner-audit-results.json). This starts the next runner
milestone after native Node TypeScript acceptance. **Bun remains Planned: no
`--lang bun`, automatic classification, saved mutation score or rerun support is
implemented.** The audit found a correctness gate before scored integration.

## Proposed first scope

One dependency-free ESM package, local JS or TS sources, native `bun:test`, serial
full-suite execution, no install/build step. Start with boolean flips using shared
JS/TS discovery. Runtime TS evidence must remain separate from JS evidence;
compile-time assertions and type-only literals are not runtime mutation sites.
Parallel tests/workers, retries, snapshots updates, focused-only selection,
external preload/plugins and arbitrary config are outside the proposed first slice.
Package imports, tsconfig semantics and complete discovery still require explicit
validation before this becomes an accepted adapter scope.

The two independent fixtures are [`bun-js`](../tests/fixtures/bun-js/README.md) and
[`bun-typescript`](../tests/fixtures/bun-typescript/README.md). Each has a covered
boolean and an uncovered empty-input fallback. On temporary copies, both syntaxes
pass the baseline, fail the covered flip, pass the fallback flip, then fail that
fallback flip after a stronger empty-input test. This is direct-run evidence,
not a Mutie result or promotion to Experimental.

## Native reporting gate

Bun's [reporter documentation](https://bun.sh/docs/test/reporters) describes JUnit
and the inspector's TestReporter/LifecycleReporter domains. The
[pinned protocol definitions](https://github.com/oven-sh/bun/blob/bun-v1.3.11/packages/bun-inspector-protocol/src/protocol/jsc/index.d.ts)
provide discovered test IDs, starts, terminal statuses and error details, but no
explicit body-versus-hook phase on those errors. Current documentation can include
features newer than the installed version; all observations below come from the
pinned executable, not assumptions based on current docs.

[`probe.py`](fixtures/bun-audit/probe.py) executes **24 scenarios independently in
JS and TS**, both directly with JUnit and through a separate inspector run:
**48 cases, 96 native invocations**. The inspector controller waits for attachment,
enables reporters before initialization, and drains the WebSocket after child exit.
A process exit is not a transport-drained completion signal. All retained cases
finish without the outer watchdog or transport-drain timeout.

| Case | Native evidence | Required Mutie treatment |
| --- | --- | --- |
| Passing body | ID found/start/end(pass), exit 0 | Pass only with complete file/global accounting |
| Body throw/rejection/assertion | JUnit AssertionError, inspector error between start and end(fail) | Potential kill only with positive body attribution |
| beforeEach/afterEach, including async cleanup | Same failure shape as body errors | Execution error, never a kill |
| Unhandled rejection or timer exception during test | Same failure shape as body errors | Execution error, never a kill |
| Body plus cleanup error | One JUnit failure, two inspector errors | Execution error |
| beforeAll/import error | Error without completed body | Execution error |
| afterAll error | Passing body end followed by error | Execution error |
| Empty/skip/todo | May exit 0 without a real passing test | Reject incomplete baseline |
| test.only | Two IDs found, only one completed | Reject incomplete selection |
| process.exit(0) inside body | Exit 0 after start without end | Execution error |
| Pending promise | Native TimeoutError / end(timeout), exit 1 | Timeout, excluded from score |
| Assertion plan mismatch | end(fail), no lifecycle error in probe | Error until separately attributable |
| Second-file import failure | First test passes, later import errors | Execution error |

The shared-error controls throw a preconstructed Error from a body or a hook,
showing why an error message/class is not phase evidence. A blanket `exit != 0`,
JUnit `AssertionError`, or `TestReporter.end(fail)` rule would score hook/process
failures as kills. Inspector ordering alone does not fix that ambiguity. Stack
text is not an authoritative phase marker either. The audit verifier locks down
these observations separately for both syntaxes.

**Decision:** do not connect a scored Bun runner to these signals alone. The next
implementation gate is an observer that positively records body, hook and process
failure provenance, plus complete discovery and execution accounting. It must be
validated for sync/async/nested and mixed body/cleanup failures without changing
suite semantics; observer changes must join execution identity. Until then,
ambiguous failures stay unscored. A report parser alone is not sufficient.

## Real TypeScript project: GramIO Composer

Selected [@gramio/composer](https://github.com/gramiojs/composer), commit
`3de15b846e3f5ef46cf8aebd5d136908baa43b54`: one ESM package, no runtime dependencies,
explicit local `.ts` imports, `bun:test`, and `"test": "bun test"`. No dependencies
were installed. The original **272 tests across 11 files** pass, including 13
snapshots. Its type assertions do not establish compiler correctness; no typecheck
or build was performed. This project supplies TS evidence only.

[Direct-run evidence](bun-composer-probe-results.json) records an isolated baseline
and three manually selected boolean changes, each restored before the next run:

- Queue initially inactive: exit 1 with eight failures, including native test
  timeouts. This must not become a clean mutation kill by exit-code inference.
- Queue stop leaves active true: exit 1 with four failures.
- Middleware name becomes nonconfigurable: original 272-test suite still passes.

A [retained stronger test](fixtures/bun-audit/composer-stronger.test.ts) names a
handler twice for different contexts. The unchanged source passes **273 tests**;
the previously passing flip now causes exactly one failure when the second name
cannot be defined. This demonstrates a useful observable finding, without calling
the manual probe an automatically classified kill or inventing a score.

All **35 original files** are hashed and unchanged. Only temporary copies receive
mutations or the stronger test. No upstream files, package/config, snapshots or
runtime dependencies are modified. Full mutation inventory, isolated adapter
execution, source/execution-identity reruns, policy integration, timeout/cancellation
and descendant/capture cleanup acceptance remain open.

## Reproduce

No package installation is needed. Use the recorded Composer commit. Inspector
probes require temporary loopback sockets; the controller uses Node 24's built-in
WebSocket. The scripts enforce the audited Bun revision.

```sh
python3 benchmarks/fixtures/bun-audit/probe.py \
  --bun /absolute/path/to/bun \
  --inspector-node /absolute/path/to/node24 --out /tmp/bun-audit.json
python3 benchmarks/fixtures/bun-audit/verify.py /tmp/bun-audit.json
python3 benchmarks/fixtures/bun-audit/real-project.py \
  --bun /absolute/path/to/bun --project /path/to/composer --out /tmp/bun-composer.json
```

The checked-in scripts/fixtures and results are the deliverable for this gate.
Existing Go, Rust, Vitest and Node runner code is unchanged; no version bump or
Bun support claim is made.


## Follow-up

The [body-attribution prototype](bun-body-attribution.md) now records 112 paired
JS/TS cases, 30 corrupted-evidence controls and matching Composer baseline/finding
results. It uses synchronous phase records and completed JUnit files after rejecting
inspector event loss at shutdown. This completes the bounded observer experiment;
production adapter, complete inventory and cleanup acceptance remain open.
