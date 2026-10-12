# Terminal self-test reliability — 2026-10-11

The first parallel Rust self-run reported a false kill: the mutation at
`src/score.rs:187` failed only
`findings_failure_restores_terminal_and_preserves_completed_report`, where the
child exited 1 instead of the expected findings-policy exit 3. The assertion
did not retain the child output or say which UI mode failed. A serial repeat
passed that test and exposed the mutation as a survivor. See
[`rust-first-slice.md`](rust-first-slice.md) for the saved runs and subsequent
score regressions.

The PTY harness now emits failure diagnostics before removing its temporary
files: session mode/UI/CI/size/worker count, elapsed time and child status,
cursor-query count and longest gap between PTY reads, visible screen,
the last 32 KiB of escaped PTY output, and up to 16 KiB per saved
run/outcome JSON or command log. Successful tests stay quiet. Cleanup still
terminates and drains unfinished children.

## Initial reproduction attempt

With diagnostics added and terminal behavior unchanged, ran six batches of two
concurrent `cargo test --locked --test terminal` processes. All **12 suites / 144
tests passed**. Each suite used Cargo's default test concurrency and completed
in 27–29 seconds on this machine. No failure was retried or discarded.

Local logs and per-command results:
`/tmp/mutie-stress-before-_lyue219`.

Increasing to four concurrent suites caused PTY allocation failures and
whole-workflow watchdog failures. This exceeded the useful load for this local
PTY environment; those failures are not evidence about a source mutation.
Logs: `/tmp/mutie-stress-before-four-muxgw7ax`.

## Reproduced harness problems

Two concurrent full `cargo test --locked --no-fail-fast` processes, with a fresh
`cargo test --locked --offline --no-run` compilation started when the terminal
tests began, exposed a narrower watchdog failure. One complete suite passed;
the other failed only
`failed_terminal_setup_restores_input_and_auto_falls_back_to_plain`. The child
completed successfully and saved a completed report, but became waitable just
after the harness's ten-second deadline (10.34 seconds including cleanup).
Cursor-query handling was active; the longest gap between PTY reads was 91 ms.

The ownership audit also found that both descriptors returned by `openpty`
were inheritable. A focused probe observed an extra terminal descriptor in the
fake Go process; marking the descriptors close-on-exec removed it. Adding this
check to the actual terminal fixture made the findings-policy test fail with
exit 1 and `inherited terminal descriptor 3` before the fix, then pass with the
expected exit 3 after it.

Changes:

- Both terminal harnesses mark their PTY descriptors `FD_CLOEXEC`. A short
  allocation-through-spawn lock closes the race where another test could launch
  a child before those flags are set. Sessions still execute concurrently.
- Every fake Go invocation in the terminal suite rejects inherited terminal
  descriptors, covering both baseline and mutation subprocesses.
- Terminal test watchdogs allow 30 seconds for the entire workflow under load.
  The CLI's ten-second command timeout, Crossterm's cursor-query timeout, and all
  outcome/restoration assertions remain unchanged.
- Failures emit the bounded diagnostics described above before temporary files
  are cleaned up. A separate deliberately invalid `GOTMPDIR` probe confirmed
  that the session mode and saved error survive in the test log.

These are independently reproduced test-harness defects. The original historical
exit-code-only failure still lacks enough evidence to attribute it to one cause.

Local evidence:

- `/tmp/mutie-stress-full-before-e7c6711e`
- `/tmp/mutie-terminal-build-load.log`
- `/tmp/mutie-terminal-inherited-fds-before.log`
- `/tmp/mutie-terminal-diagnostic-probe.log`

## Verification after the fix

The descriptor regression passed after setting `FD_CLOEXEC`. Then ran six
batches of two concurrent Cargo processes, each running:

```sh
cargo test --locked --test terminal --test report_browser
```

All **12 paired runs / 216 test executions passed**, without retries or discarded
failures. A fresh, isolated `cargo test --locked --offline --no-run` build began
when the first terminal suites started and overlapped the first two batches.
The per-command logs/results are in `/tmp/mutie-stress-after-6jtoavyl`;
the competing build log is `/tmp/mutie-terminal-build-load-after.log`.
Formatting and strict all-target Clippy also passed.

Finally repeated the original two-mutation sample on Mutie with two workers:

```sh
target/debug/mutie . --include src/score.rs --operator boolean_flip \
  --jobs 2 --timeout 600 --baseline-timeout 300 --ui plain \
  --output-dir /tmp/mutie-terminal-self-after
```

The fresh baseline passed **189 tests**, with 8 opt-in tests ignored. Both
mutations were killed, with zero timeouts, build failures, execution errors or
unfinished mutations. Every terminal/browser test passed in the baseline and
both mutations. Inspection of the complete logs confirmed:

- `score.rs:183`, `true` → `false`: five failures, all caused by saved-score
  validation rejecting reports under the mutated comparison. These include
  report-reader, report-retention, persistence, capture-limit and log-limit
  checks; none were terminal or timing failures.
- `score.rs:187`, `false` → `true`: exactly the two saved-percentage regressions
  failed, as in the prior serial stronger-test run.

| Phase | Whole command |
| --- | --- |
| Fresh baseline | 161.37 s |
| `score.rs:183` mutation | 184.77 s — killed |
| `score.rs:187` mutation | 189.05 s — killed |

The run completed in **354.04 seconds**, with the mutations running concurrently.
All 51 checked Rust source/test and Cargo input hashes were unchanged. Saved-run
listing accepted the completed report. These are local observations under
workstation load, not a controlled performance comparison or a guarantee against
arbitrary system resource exhaustion.

Local evidence:

- `/tmp/mutie-terminal-self-after/run-1791781033-9ysB91`
- `/tmp/mutie-terminal-self-after.log`
- `/tmp/mutie-terminal-self-inputs.json`
