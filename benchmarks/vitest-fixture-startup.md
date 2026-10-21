# Vitest preview fixture startup reliability

Investigation on 2026-10-21, starting from `1b0feb7`, follows the
[Python comparison-boundary regression failure](python-comparison-boundaries.md).
This change affects the default tests' identity-only Node stand-in. Production
Node resolution, Vitest execution, readiness limits and identity assertions are
unchanged.

## Reproduction and diagnosis

The unchanged preview suite failed all three tests at its ten-second fixture
readiness limit, both inside and outside the filesystem sandbox. The retained
[initial failure log](fixtures/vitest-fixture-startup/before.log) records the
sandboxed run. These failures occurred before product assertions.

A separate driver compiled `tests/support/mock_node.rs` with Rust 2024 and warnings
denied, then launched `--fixture-ready` with a ten-second deadline. On timeout it
sampled only its own child for one second, killed it and reaped it. The
[macOS sample](fixtures/vitest-fixture-startup/fresh.sample.txt) contains 881
samples at `_dyld_start`, before the helper's `main`, with a 96 KiB footprint and
no binary images available. A subsequent diagnostic launch of the same image
succeeded in 7.54 seconds. The [timing record](fixtures/vitest-fixture-startup/native-probe-results.json)
includes sampling/cleanup in the failed launch's 11.63 seconds; its readiness
limit was ten seconds. This second launch is diagnostic evidence, not a retry
introduced into the tests.

Two further pairs compared freshly linked helpers against helpers explicitly
ad-hoc signed with `codesign --force --sign - --timestamp=none`. All four launches
hit ten seconds: [signing control](fixtures/vitest-fixture-startup/signing-results.json).
The evidence establishes a fresh-image startup stall on this macOS host; it does
not identify the responsible OS service or establish behavior on every host.

## Fixture fix and preserved checks

On macOS the stand-in now uses the installed `/usr/bin/tclsh` with a private
`TCL_LIBRARY/init.tcl`. The script is ordinary data, avoiding a freshly linked
Mach-O helper for each fixture. The system interpreter is required explicitly;
missing Tcl fails clearly. Other Unix systems retain the existing native helper.
The Tcl environment is applied only to fixture commands, and `TCLLIBPATH` is
removed. No JavaScript runtime or Vitest execution is needed for these tests.

The initialization script accepts exactly the Node identity expression and two
arguments, or the separate readiness request. It reads the same response sidecar
through the invoked executable path, including PATH aliases and paths with
spaces. Errors explicitly exit unsuccessfully rather than falling back to normal
Tcl initialization. The optional counter still records identity reads only.

The readiness deadline remains **ten seconds**, with the original five-millisecond
polling and kill/reap on timeout. There are no retries or extended warmups.
Original source-preservation, static configuration rejection, exact version and
executable mismatch diagnostics, and single-launcher assertions remain intact.
A new protocol regression additionally checks empty readiness output, unchanged
counters on readiness/rejection, rejection of another expression, another command
and extra arguments, exact accepted JSON, and failure when the response is missing.

## Validation

The initial repaired three-test preview passed, followed by all four tests with
the new protocol regression. Ten default-parallel and ten serial repetitions then
passed: **80 test passes, zero failures**. Each repetition creates fresh fixtures.
[Timings](fixtures/vitest-fixture-startup/repetitions.json) measure whole Cargo
invocations, not readiness latency; [complete repeated logs](fixtures/vitest-fixture-startup/repetitions.log)
retain the individual results. Parallel runs took 14.73–25.26 seconds; serial runs
took 2.33–25.17 seconds. Some repetitions ran alongside the full suite.

The [initial full suite](fixtures/vitest-fixture-startup/full-initial.log) stopped
at the noisy-log interruption test's unchanged ten-second readiness assertion:
the CLI was still alive, stdout/stderr were empty and no fixture PID had been
recorded. This does not identify where that CLI was stalled. The unchanged
[log-limit recheck](fixtures/vitest-fixture-startup/log-limits-recheck.log) passed
all three tests in 9.40 seconds. No log-limit code or deadline was changed. This
intermittent observation remains a limitation of the broader host-startup
investigation; a later passing run does not erase it.

The subsequent unfiltered `cargo test --offline` completed all 20 targets:
**277 passed, zero failed, 93 existing opt-in ignores**. The preview target passed
all four tests in 0.76 seconds. [Full final log](fixtures/vitest-fixture-startup/full-final.log).
No new ignores or filtering were introduced. Formatting, all-target Clippy with
warnings denied, and whitespace checks pass. Non-macOS execution was not
revalidated on this host; its existing helper remains unchanged.
