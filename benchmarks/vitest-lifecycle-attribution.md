# Vitest lifecycle attribution audit

Date: 2026-10-15. Adapter base: `49c74a3`. Node 24.22.0, pnpm 11.10.0,
Vitest 4.1.9, Vite 8.3.4, macOS arm64. Prepared mixed JS/TS fixture:
`/tmp/mutie-vitest-fixture-scope`.

## Result

The current reporter-only policy is insufficient: eight probe cases count
lifecycle failures as kills. This is a scoring correctness blocker, not acceptance
of these APIs. A passing baseline does not protect a mutation run whose changed
source first triggers a fixture or cleanup assertion. The audit directly injects
failures to isolate classification; it does not yet demonstrate those cases via
source mutations through the complete CLI.

| Probe | Required classification | Observed classification |
| --- | --- | --- |
| Ordinary test-body assertion | Killed | Killed |
| Body assertion with successful beforeEach | Killed | Error |
| beforeEach assertion | Error | Error |
| afterEach assertion | Error | Error |
| Returned beforeEach cleanup assertion | Error | Error |
| beforeAll assertion | Error | Error |
| afterAll assertion | Error | Error |
| aroundEach setup assertion | Error | **Killed** |
| aroundEach teardown assertion | Error | **Killed** |
| aroundAll teardown assertion | Error | Error |
| onTestFinished assertion | Error | **Killed** |
| Body assertion plus onTestFailed assertion | Error | **Killed** |
| Extended test fixture setup assertion | Error | **Killed** |
| Extended test fixture teardown assertion | Error | **Killed** |
| Automatic fixture teardown assertion | Error | **Killed** |
| Concurrent test-finished assertion alongside passing test | Error | **Killed** |

All 16 commands produced readable completion evidence. The unmodified fixture
baseline passed before the probes. Each command used the existing isolated
project/dependency copy, private evidence path and shared adapter settings.
The lifecycle failures use ordinary assertions intentionally: rejecting particular
error names or messages cannot distinguish them from body assertions.

## Why the reporter misses these failures

Inspected the installed, pinned source rather than assuming that hook events cover
all lifecycle work. Vitest's `ReportedHookContext` in its reporter declarations
only names beforeAll, afterAll, beforeEach and afterEach. In `@vitest/runner`'s
`dist/chunk-artifact.js`, `runTest` also executes around hooks, fixture resolution
and cleanup, and onFinished/onFailed callbacks. Their errors join test result
errors, so a final failed test and exit code 1 do not identify the failing phase.
The observed false-kill completion records have no reported hooks.

Traditional hooks remain conservatively rejected. In particular, returned cleanup
failures need that conservative check even after a successful beforeEach end event.
The aroundAll failure appears as a module error and is rejected by the existing
collection/import boundary. No classification behavior was changed by this audit.

## Reproduction and validation

```sh
MUTIE_VITEST_FIXTURE=/tmp/mutie-vitest-fixture-scope \
  cargo test --bin mutie real_lifecycle_attribution_audit -- --ignored --nocapture
cargo fmt --check
cargo clippy --all-targets -- -D warnings
```

The ignored diagnostic completed all 16 cases in 42.00 seconds. It prints expected
and observed outcomes plus completion evidence; it deliberately does not assert
that today's false kills must persist. Missing completion fails the probe.
Local output: `/tmp/mutie-vitest-lifecycle-audit.log`.
Formatting and all-target Clippy passed.

## Next acceptance gate

Add worker-side evidence that separates body failures from lifecycle failures,
including fixture resolution inside the test callback wrapper. Fail closed when
that distinction is unavailable. Do not infer attribution from error text, stack
shape, or reporter hook counts alone. Preserve concurrent task identities and
mixed body/cleanup failures. Then convert these diagnostic cases to acceptance
regressions and verify mutation-triggered lifecycle failures after a passing
baseline through saved outcomes and CI policy decisions. Only after that evidence
should successful traditional hooks stop preventing genuine assertion kills.

File/worker-scoped fixtures, nested around hooks, cancellation during lifecycle
work, and all combinations of concurrency remain outside this audit. This audit
does not promote Vitest support or change report/operator versions.
