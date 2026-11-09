# Cancellation when child exit wins the wait

Follow-up to the [statement-removal audit](statement-removal-audit.md),
2026-11-09. That audit recorded a PTY Ctrl-C run with one raw SIGINT worker
classified as an error and another interrupted, leaving a failed report.

## Reproduction

The existing terminal regression passed 60 repetitions before the fix, with four
test processes at once. Repetition alone did not establish the cause or a fix.

The new `user_interrupt_and_completed_signal_exit_are_reported_as_interrupted`
regression makes the competing events ready together:

1. Start two workers and retain an already completed survivor and a pending item.
2. Pause the owned CLI with SIGSTOP and verify its stopped state.
3. Wait for one worker to exec native `sleep`, signal it, and verify that it is a
   zombie awaiting the paused parent's wait. This retains a raw signal status,
   rather than a shell's numeric translation.
4. Queue the matching SIGINT/SIGTERM to the CLI, then resume it.

With the original production code, SIGINT produced the audit's failure:
`Go command failed without a test or build failure (signal: 2 (SIGINT))`, one
error, one interrupted worker and a `failed` report. The child-exit branch can
win before the async signal task publishes its stop request. This establishes
the ordering bug; it does not establish exactly how the original terminal signal
reached that worker.

## Change and boundary

`Cancellation` now records SIGINT and SIGTERM receipt in separate atomic flags
through `signal-hook`, already present transitively through the terminal backend.
The signal callbacks only store flags. Tokio's existing task still wakes blocked
work, and synchronous cancellation checks also see the flags. Registrations are
removed when the run's signal guard drops.

After process-group shutdown and capture draining, `process.rs` marks a raw
SIGINT/SIGTERM exit interrupted only when this run received that same signal and
the command has no timeout or capture error. The wait order, command deadlines,
group shutdown retries and capture EOF bound are unchanged.

Negative controls preserve errors for child-only SIGINT/SIGTERM, mismatched
parent/child signals, SIGKILL during user cancellation, a completed numeric tool
failure and capture overflow alongside a matching signal. Internal peer-stop
requests alone are not evidence of a user signal. Unit controls also reject
numeric exits 129/130/143 and SIGABRT. Completed outcomes remain intact; active
interruptions have no killed/missed outcome, queued work stays pending, and
interrupted/failed reports receive no passing score or evaluated policy.

The controlled regression checks explicit plain and JSON output, exit code 1,
finalized report status, streaming event results, source preservation and removal
of owned workers, descendants and temporary files. Existing PTY tests retain
their terminal-restoration assertions. No test deadline or assertion was relaxed.

## Validation

Local macOS results on the final production code:

| Check | Result |
| --- | --- |
| Default suite, `cargo test --locked -- --test-threads=2` | 348 passed; 174 opt-in tests ignored |
| Cancellation integration suite | 9 passed, including four signal/output combinations and seven error controls |
| Terminal / configuration suites | 14 / 17 passed |
| Shared process tests | 12 passed; one unrelated opt-in probe ignored |
| Real Python scored cancellation / resource limits | Both passed |
| Real Cargo mutation cancellation | Passed: serial SIGTERM with direct captures and parallel SIGINT with bounded captures; descendants cleaned |
| Original PTY Ctrl-C regression, four concurrent test processes | 60/60 passed |
| Controlled signal race, same concurrency | 20/20 passed, each covering four combinations |
| Error-control regression, same concurrency | 10/10 passed, each covering seven cases |
| Rustfmt, Clippy with warnings denied, whitespace check | Passed |

The native Cargo check initially failed to compile its generated test because a
`std::process::Command` import was missing. Restoring that fixture import made the
unchanged cleanup assertions executable; the native check then passed in 21.19
seconds. It is now included in the Rust Linux CI job. The new controlled
regressions are ordinary tests in the existing CI check job. This work has not yet
been pushed or verified on Linux; repeated local passes are not a guarantee of
every scheduling interleaving.

Temporary logs are under `/tmp/mutie-cancel-*`, including
`mutie-cancel-race-regression-before.log`, `mutie-cancel-full.log`,
`mutie-cancel-rust.log`, `mutie-cancel-rust-fixed.log`, and
`mutie-cancel-race-{before,after}/`. These are local diagnostics, not checked-in
artifacts. The original assignment-audit reports remain unchanged. Existing
execution fingerprints that include the shared process recipe or lockfile change
naturally; no package, catalog or report-schema version was bumped.

The separately deferred macOS pre-main startup issue remains unresolved.
