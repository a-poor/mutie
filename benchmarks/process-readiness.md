# Go fixture startup and readiness investigation

Follow-up on 2026-10-21 to the failures recorded during
[Route Mini validation](python-route-mini.md#regression-checks), starting from
`d882822`. Changes cover fixture launching, diagnostics and extraction of unchanged
Node identity validation for test use. Production launch/validation semantics,
timeouts and process-group behavior remain unchanged.

## Final acceptance

The final ordinary `cargo test --offline` run exits successfully across all
20 test targets: **275 passed, zero failed, 91 existing opt-in ignores**, with
normal parallel scheduling and no filters. The final preview checks pass in
3.16 seconds. `cargo fmt --check`, explicit formatting of the standalone native
mock, `git diff --check`, and Clippy with all targets/warnings denied pass.
The 140 repeated cancellation checks and four native Python workflows below also
pass. No source/report version or production timeout was changed.

Final logs: `/tmp/mutie-readiness-acceptance.log` and
`/tmp/mutie-readiness-acceptance-clippy.log`. Earlier failed runs below
remain part of the evidence; they are not counted as successful acceptance.

## Reproduced cancellation/timeout race

The noisy-log interruption test used the fixture's one-second mutation timeout,
then waited up to ten seconds for a readiness marker before sending SIGINT or
SIGTERM. Slow work before the marker could let the mutation timeout first. The
CLI would exit normally with a timeout outcome, while the harness continued
waiting for a checkpoint that could no longer appear.

The retained fixture now deliberately spends two seconds before the interruption
checkpoint. With the old one-second mutation limit, this control fails: the saved
CLI output reports one timeout and zero interrupted mutations, then the readiness
check reports that the CLI exited. The negative control failed in 2.76 seconds.
With a cancellation-specific 60-second mutation limit (matching the cancellation
suite), both SIGINT and SIGTERM cases pass. This changes the event under test's
competing deadline, not the assertion or readiness deadline:

- The noisy-log readiness bound remains ten seconds.
- The cancellation suite's readiness and finish bounds remain fifteen seconds.
- All existing interrupted-state, retained-log and temporary-file assertions remain.
- The separate timeout case still uses one second and checks a timeout outcome.
- The delayed fixture must actually reach its marker before either signal is sent.

CLI output in the noisy-log test now goes to files so failure diagnostics can read
it without waiting for exit or risking an undrained output pipe. Readiness checks
fail immediately on early CLI exit, with PID/output diagnostics. A guard reaps the
CLI and kills the recorded fixture group on assertion failure. Normal signal
handling and cleanup remain the behavior under test. Post-signal completion now
also has a fifteen-second bound instead of an unbounded wait.

Cancellation tests similarly detect early CLI exit and include the CLI PID in
existing shim-marker/output diagnostics. Their deadlines and outcome assertions
are unchanged.

## Reproduced fresh-executable loader stall

Initial repetitions passed: ten unchanged serial cancellation suites and ten
with default parallel test scheduling (**140 individual test passes**). The
[retained timings](fixtures/process-readiness/repetitions.json) describe complete
suite wall time, not startup latency. Serial suites took 9.15–10.29 seconds;
parallel suites took 3.78–6.04 seconds. These initial passes did not resolve the
historical failures.

A further serial repetition ran alongside the four native Python conftest
workflows. Its seventh suite reproduced two readiness failures after six passing
suites: **5 passed, 2 failed in 123.89 seconds**. The failure output and
[shell stack sample](fixtures/process-readiness/shell-loader-sample.txt) are
retained. The shell had a 96 KiB footprint and every sample was at `_dyld_start`,
with no binary image list. It had not reached the first shim marker. Mutie was
waiting for its child, rather than executing a fixture body or holding an
application-level lock. The four Python workflows passed in 172.90 seconds;
concurrency correlated with reproduction here but is not established as its cause.

A separate Python parent launched a tiny fresh executable shell script outside
Mutie. That also stalled before its first output, and a three-second diagnostic
bound fired. Its [stack sample](fixtures/process-readiness/direct-script-loader-sample.txt)
matched the test's loader state. Explicit `/bin/sh script` controls completed in
8.7 and 23.3 milliseconds; another direct script took 1.18 seconds. The
[control results](fixtures/process-readiness/script-launch-results.json) retain
both successes and the timeout. This isolates an executable-loading delay outside
Mutie's process runner; it does not identify a particular macOS service or policy.

A temporary native proxy compiled per fixture did **not** solve the problem:
one cancellation test failed and the new native executable was also sampled at
`_dyld_start`. That approach was removed.

The retained test-only fix puts a symlink to the existing `/bin/sh` at the
fixture's `bin/go`, and writes the fixture body as ordinary `test` and `list`
files in its module. Mutie still launches `go test ...` and `go list ...` through
PATH, with its ordinary environment, process group, captures and temporary
directories. The shell reads the corresponding input file from the isolated
workspace. A prefix restores the command name as the fixture's first argument.
No fresh executable or interpreter shebang needs loading. The same shell body
still creates descendants, emits diagnostics, waits at checkpoints, floods
captures, and leaves temporary files for Mutie to clean.

The startup-error control explicitly removes the symlink before creating its
missing-interpreter executable; it never writes through the link to the system
shell. It continues to verify the real spawn-error path and private-directory
cleanup. No retries, skipped tests, conditional assertions or deadline increases
are introduced to hide loader stalls.

## Validation

The first unchanged full serial suite passed all **275 non-ignored tests**, with
91 opt-in tests still ignored. After the noisy-log fix, the ordinary parallel
`cargo test --offline` suite also passed all 275 tests with the same 91 ignores.
Those initial clean runs did not by themselves resolve or explain the failures;
the later concurrent reproduction above was necessary.

The delayed interruption test passes both signals. Restoring only its old
one-second command timeout makes it fail before readiness with the expected
saved timeout evidence. The new readiness diagnostic therefore distinguishes
this reproduced failure from an alive-but-stalled process.

Reproduce the ordinary checks with:

```sh
cargo test --offline --test cancellation --test log_limits -- --test-threads=1
cargo test --offline
cargo fmt --check
cargo clippy --offline --all-targets -- -D warnings
```

For repetitions, build `cargo test --offline --test cancellation --no-run` and
run the printed executable ten times with `--test-threads=1`, then ten times with
no scheduling override. Assertions, test selection and deadlines stay intact.

Local logs are `/tmp/mutie-readiness-{initial,full-before,full-after,
delayed-control,delayed-fixed,targeted-after,clippy}.log`; repetition logs and the
watcher outputs are under `/tmp/mutie-readiness-watch` and
`/tmp/mutie-readiness-parallel`.

The final system-shell fixtures pass all seven cancellation and three log-limit
tests together. Ten serial and ten parallel cancellation repetitions then pass
**140/140 tests** while native Python validation runs. The first serial suite
includes a sampled CLI loader delay and takes 24.10 seconds; it still meets every
original checkpoint assertion. Remaining serial suites take 6.27–6.74 seconds,
and parallel suites take 3.02–5.13 seconds. These are stress controls, not a
controlled speed benchmark or a claim that macOS cannot delay any executable.

A simultaneous full-suite attempt, then quiet parallel and serial attempts, fail
the existing Vitest fake-Node identity unit test (`could not identify Node for
Vitest`), with 197 other unit tests passing. That test also launches a fresh shell executable, but no sample of that
specific child was captured, so its cause remains unproven. Keep that failed
attempt distinct from clean full-suite results. A fixed script launcher also
failed, so it was not retained.

The Node test now invokes its response script explicitly through `/bin/sh` and
feeds the actual process output to `validate_node_identity`, extracted unchanged
from the production launcher. This preserves the ten-second bound, 4096-byte
capture limit, scrubbed Node environment and real exit/signal evidence. The
original mock never interpreted Node's `--eval` arguments; production invocation
still uses exactly those arguments. Non-executable and missing-path checks still
use the real production launcher, because an explicit interpreter would otherwise
read a non-executable script. Invalid metadata cases now require their specific
validation error, and a second valid response verifies recovery after all six
negative cases. There are no conditional skips or weaker identity assertions.
The first complete parallel unit suite with this extraction passes 198 tests.

The standalone loader control can be repeated with:

```sh
python3 benchmarks/fixtures/process-readiness/launch_probe.py --output /tmp/mutie-loader-control-new
```

It creates only owned temporary scripts, runs direct/explicit-shell pairs, and
samples its own stalled child on macOS. The three-second bound applies after
spawn; executable loading and sampling time are retained in the wall-time result.
Reproduction is intermittent, so a passing probe is not contrary evidence to the
retained failure samples.

The concurrent native Python control also passes all four conftest workflows
with the final Go fixtures in 167.65 seconds. No Python source, classifier or
validation assertion changes were needed.


## Preview integration fixtures

The next full run passes 271 tests, including all twelve terminal checks, then
fails all three Vitest preview checks at their generated Node launchers. Their
original identity/configuration assertions reject the startup failure rather
than accepting it as the expected mismatch. These failed results are retained
in `/tmp/mutie-readiness-final-all.log`.

Those CLI integration checks need a real executable mock, so they now compile a
small standard-library-only Rust stand-in during fixture preparation. A bounded
ten-second readiness invocation must succeed before testing; it consumes no
identity response and touches no launcher counter. The fixture then keeps that
executable immutable and uses separate response files for version/path changes.
Tool paths, including the pinned path with spaces, remain real PATH entries;
they live outside the copied project. All temporary tools are fixture-owned.
No JavaScript runtime, network dependency or production runner override is added.

The mock requires exactly the production `--eval` identity expression and rejects
any attempt to run Vitest. The three preview tests retain read-only listing,
filtering, untouched sources, rejected executable configuration, runtime mismatch,
missing pinned runtime and exactly-one-launcher-call assertions. Each scored
baseline scenario now requires its specific expected error, replacing the earlier
OR across all three errors. The native fixture's first targeted run passes all
three tests in 4.15 seconds. Fixture preparation is bounded independently; the
production ten-second identity limit is unchanged and no failed run is retried
inside a test.
