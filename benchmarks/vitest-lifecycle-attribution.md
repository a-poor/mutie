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

## Conservative containment after the audit

The worker copy now records per-test lifecycle participation for aroundEach,
resolved fixtures (including automatic fixtures), and onFinished/onFailed callbacks.
The reporter includes that metadata and proof that the worker instrumentation ran.
Passed and failed executed tests require the proof; a failed participating test is
an infrastructure error. No message or stack matching is used for these APIs.
Traditional hooks retain their existing conservative rule. `test.fails` is now
rejected, since inversion can hide a lifecycle failure behind a passing result.

This is participation evidence, not precise failure attribution: a body assertion
with a successful fixture or callback is still conservatively rejected. Passing
fixture, aroundEach and completion-callback controls remain passing. The earlier
16-case diagnostic has become `real_lifecycle_failure_evidence`, with assertions
for all cases and four additional passing/inversion controls. Its successful-hook
body-failure expectation is now Error, explicitly preserving the conservative
policy rather than claiming support for that genuine kill.

The adapter instruments only the disposable dependency copy. It requires SHA-256
`c10a7a80fca65fb42e27eabf7655c5ce5ecf36e8aaeb1811499efd063df62556` for
`@vitest/runner@4.1.9/dist/chunk-artifact.js` before inserting three metadata writes.
Unknown bytes or ambiguous insertion points abort before Node starts. Replacement
uses a new inode, preserving any dependency-store hard links. This pinned-source
integration is deliberate: the public reporter API does not expose fixture
participation. Supporting another runner build requires a new source audit and
regression evidence, even when its package metadata claims the same version.

The existing saved `reporter_fingerprint` now covers the reporter and the worker
instrumentation source. Old saved identities therefore require a fresh scan;
report and operator versions are unchanged. Isolated dependency contents are
never reused between commands.

Validation commands:

```sh
MUTIE_VITEST_FIXTURE=/tmp/mutie-vitest-fixture-scope \
  cargo test --bin mutie real_lifecycle_failure_evidence -- --ignored --nocapture
MUTIE_VITEST_FIXTURE=/tmp/mutie-vitest-fixture-scope \
  cargo test --test vitest_workflow lifecycle_mutation_failures_do_not_improve_scores -- --ignored
MUTIE_VITEST_FIXTURE=/tmp/mutie-vitest-fixture-scope \
  cargo test --bin mutie real_fixture_outcomes_and_failure_evidence -- --ignored
cargo test
cargo fmt --check
cargo clippy --all-targets -- -D warnings
```

The 20-case worker regression passed in 54.02 seconds. The CLI regression uses
three separate passing-baseline runs: a single boolean flip triggers fixture setup,
completion-callback cleanup, or aroundEach teardown failure. It checks error exit
1 with both CI policies enabled, saved error outcomes with zero kills, and unchanged
input source, tests and runner bytes. This closes the audit's missing
mutation-triggered evidence without claiming broader lifecycle attribution.

The three CLI runs passed in 82.83 seconds; the existing outcome/failure matrix
passed in 60.26 seconds, preserving its ordinary kills, survivors, stronger-test
kills and transform/import distinctions. The default suite passed (232 passed,
38 ignored); formatting, diff whitespace checks and all-target Clippy passed.

## Body versus lifecycle failure evidence

The next implementation replaces participation-only rejection with per-test
failure-phase flags. The source hash requirement, isolated replacement and saved
execution identity remain unchanged. Within the audited runner, test-handler
fixture resolution marks body entry only immediately before calling the original
handler; hooks and fixture functions cannot set that marker. The existing callback
catch records body failure only after that entry. Other task failure catches mark
lifecycle failure, including returned cleanup, around hooks, fixture teardown and
completion callbacks. Mixed failures retain both flags and are rejected.

This avoids wrapping the user handler or changing its argument introspection,
fixture selection, execution order or error values. Successful traditional hook
events no longer veto an otherwise proven body kill. Suite/module errors and
unfinished hooks retain their existing rejection rules.

Two paths intentionally remain conservative. If Vitest already has queued async
assertions when the body is entered, a later callback failure cannot establish
that the body created it. Similarly, post-body assertion-count checks can be
configured from setup. These failures are errors, even if a particular project
could prove them to be body failures. Ordinary async body assertions remain kills.

The worker regression now includes body kills with successful fixtures, before/
after hooks, returned cleanup, aroundEach, onTestFinished and onTestFailed. It also
checks mixed body/cleanup failures, mixed fixture failures, concurrent body versus
callback failures, and async assertions originating in setup versus the body.
The complete CLI regression adds two passing-baseline mutation runs that produce
genuine kills with successful lifecycle work, alongside the three lifecycle-error
runs. Both policies are enabled, source/test/runner bytes remain unchanged, and
saved score counts distinguish errors from kills.

File/worker fixture lifetimes and nested lifecycle combinations remain next
acceptance work; this change does not claim all Vitest lifecycle configurations.

Validation: all 33 worker cases passed (112.19 seconds), the five-run CLI
regression passed, and the existing outcome/failure matrix passed (59.37 seconds).
The default suite passed (232 passed, 38 ignored), as did formatting, whitespace
checks and all-target Clippy. Local logs use `/tmp/mutie-vitest-phases-`
with `final.log`, `cli.log`, `existing.log`, `default.log` and `clippy.log`.

## Scoped fixtures and nested lifecycle acceptance

Using the same pinned installation and classifier, the new
`real_scoped_fixtures_and_nested_lifecycle` regression checks 18 cases:

| Cases | Result |
| --- | --- |
| File/worker fixtures: passing body, successful teardown | Missed |
| File/worker fixtures: body assertion, successful teardown | Killed |
| File/worker fixtures: setup assertion | Error |
| File/worker fixtures: teardown assertion, passing or failing body | Error |
| Nested aroundEach or aroundAll with body assertion | Killed |
| Nested aroundEach cleanup, including mixed body failure | Error |
| Nested aroundAll cleanup | Error |
| Inherited beforeEach returned cleanup failure | Error |
| Concurrent body failure alongside another test's cleanup failure | Error |
| Concurrent file-fixture body failure alongside a passing test | Killed |

All commands produced completion evidence. File-scoped teardown is recorded as a
module error; worker-scoped teardown is recorded as an unhandled error. Both are
rejected before a command can count as a kill, including mixed failures. Nested
suite teardown uses suite error evidence. No classifier or instrumentation change
was needed for these cases.

The CLI test `scoped_fixture_mutation_failures_preserve_scores` runs four separate
passing baselines. For each fixture scope, flipping `ready = true` either fails a
body assertion with successful teardown, or fails teardown after a passing body.
The former must produce one saved kill and exit 0; the latter must produce one
saved error, zero kills and exit 1. Both findings and minimum-score policies are
enabled. Every run checks unchanged source, tests and installed runner bytes.
The shared CLI assertion helper also serves the previous five lifecycle cases.

```sh
MUTIE_VITEST_FIXTURE=/tmp/mutie-vitest-fixture-scope \
  cargo test --bin mutie real_scoped_fixtures_and_nested_lifecycle -- --ignored --nocapture
MUTIE_VITEST_FIXTURE=/tmp/mutie-vitest-fixture-scope \
  cargo test --test vitest_workflow scoped_fixture_mutation_failures_preserve_scores -- --ignored
cargo test --bin mutie vitest::
cargo test --test vitest_workflow
cargo fmt --check
cargo clippy --all-targets -- -D warnings
```

The 18-case adapter matrix passed in 41.94 seconds. These probes exercise a scoped
fixture declared in one test module, with the existing serial, isolated-file
runner settings. They do not establish fixture reuse across files or processes,
parallel-worker support, cancellation during scoped teardown, or every nested
lifecycle combination. Async setup/assertion-count ambiguity remains conservative.
The next milestone gate is unchanged Pathe config support and useful real-project
mutation findings.

The four scoped-fixture CLI runs passed. The targeted default checks passed
(16 Vitest unit tests and one CLI option test), as did formatting, diff whitespace
checks and all-target Clippy. Local evidence is in
`/tmp/mutie-vitest-scoped.log`, `mutie-vitest-scoped-cli.log`,
`mutie-vitest-scoped-default.log` and `mutie-vitest-scoped-clippy.log`.
