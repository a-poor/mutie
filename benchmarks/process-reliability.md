# Process capture and report-lock reliability

Follow-up to `5b4ffa7` (worker-local Cargo dependency reuse). The paired self-run
exposed extra capture-EOF and retention-lock assertion failures alongside the
intended score failures. This investigation uses unmutated code first; the cache
mode remains opt-in and no report/package/catalog versions change.

## Reproduction and fixes

The original unit suite, with diagnostics added, reproduced the capture failure
in 3 of 12 runs (two test processes, 32 test threads each). Another 24-run sample
at 64 test threads reproduced capture EOF once and retention lock failures twice.
The capture error was `output capture did not reach EOF after command shutdown`;
in the diagnostic occurrence the recorded background descendant no longer
existed. Retention failed to reacquire the report lock even after its owner or
an intervening inspector had dropped its descriptor.

A subsequent isolated reproduction ruled out concurrent spawning as a sufficient
explanation: 9 of 40 runs failed with only the original capture test running in
each process (four test processes at once). At EOF timeout, `kill(-pgid, 0)` still
found the group. `lsof -g PGID -a -d 1,2` showed a surviving `sleep` process in that
same group holding both pipe writers. The background shell had died while its
forked child survived the one-shot group signal. An experimental spawn mutex did
not prevent this failure and was removed.

Process-group cleanup now retries SIGKILL every 10 ms until ESRCH or a one-second
bound, for both limited and unlimited captures. ESRCH retires the identifier so
it is never signaled again by that guard. The bound allows for orphan zombies
awaiting an external reaper; Drop retains a best-effort signal if the group has
not disappeared. Limited captures still have a separate one-second EOF drain
bound and explicitly report incomplete output. This does not add containment for
descendants that deliberately leave the process group.

The concurrent regression runs 64 captures on an eight-thread runtime, mixing
live commands and shells that exit while background helpers hold their pipes.
It checks exact output, clean EOF and successful status, and drains every task
even if one fails. The existing isolated regression failed 9/40 times before the
retry fix and passed 100/100 times afterward under the same four-process load.
These are stress results, not a guarantee of every possible scheduling interleaving.

Closing a file descriptor does not necessarily end a lock shared with duplicated
or inherited descriptors, as documented by [Rust's file-lock API](https://doc.rust-lang.org/std/fs/struct.File.html#method.try_lock).
Report writers and pruning now hold an owning guard that explicitly unlocks on
drop before closing its file. A deterministic regression duplicates the lock
descriptor, drops the owner and reacquires the lock while the duplicate remains
open. It also verifies that subsequently closing that old duplicate cannot
release the next owner's lock. This regression fails with the previous bare-file
ownership and passes with explicit unlock.

An intermediate 24-run sample hit a separate delayed-write assertion in the
blocked-observer shutdown test.
That test gave a helper one second before writing a marker; a busy host could
delay cancellation until after the write. The helper now waits for an explicit
release sent after command shutdown, and the test checks that its process is dead
before checking the marker. This preserves the descendant-cleanup requirement
without racing a wall-clock write. Linux zombie processes count as terminated.

Local reproduction logs (temporary, not committed) are under
`/tmp/mutie-reliability-repro`, `mutie-reliability-diagnostic`,
`mutie-eof-group` and `mutie-group-fixed`.

## Validation

- `cargo test --locked`: 198 passed; 22 opt-in tests ignored.
- Real Cargo suite: 14 passed, two failed in 457.44 seconds. The cache regression
  reported three timeouts instead of two kills/one survivor; the shared-deadline
  regression never reached its compilation checkpoint.
- `cargo fmt --check` and `cargo clippy --locked --all-targets -- -D warnings`: passed.
- Isolated original EOF regression: 100/100 passed, four concurrent test processes.
- Deterministic duplicated-descriptor lock regression fails without explicit
  unlock and passes with the owning guard.

The extended Cargo workflow validation encountered separate command-startup
delays. A one-second stack sample of the fixture Cargo proxy showed all 819
samples at `_dyld_start`, a 96 KiB footprint and no loaded binary image list;
this was before its Rust entry point. Keep these environment observations separate
from the reproduced process-group and lock failures; do not relax workflow
assertions or claim an extended-suite pass from the default suite.

The two failed Rust fixtures subsequently passed individually, unchanged:
`cache::worker_cache_rebuilds_backdated_sources_and_tests_with_isolated_dependencies`
in 22.56 seconds and `compilation::compilation_and_tests_share_one_deadline` in
25.11 seconds. The real Go workflow passed in 24.04 seconds. These focused
rechecks do not erase the failed full-suite result.

## Real Mutie self-run and remaining limitation

The prior temporary report was no longer present, so validation repeated the
six score-boundary mutations on an isolated copy instead of selecting two saved
IDs. It used `--include src/score.rs --operator comparison_boundary
--rust-build-cache worker --jobs 2 --baseline-timeout 300 --timeout 600 --ui plain`.
The baseline passed all 198 tests in 176.97 seconds. The run completed in
1,911.50 seconds with **two killed and four timeouts**, not the expected five
kills/one survivor. The validation script correctly rejected that outcome.

Neither the original capture EOF assertion nor the report-lock assertion recurred.
However, nested cancellation, log-limit and terminal tests failed or stalled;
even the previous survivor was killed by unrelated integration failures. Those
results cannot establish mutation usefulness or a clean real-project validation.
A loader stall was observed in the separate fixture investigation, but that does
not establish the cause of every self-run failure. Investigate these nested
parallel workflows next, retaining their deadlines and diagnostics rather than
broadening cache defaults or accepting the score as evidence of success.

Project hashes and caller target/build sentinels were unchanged. The owned scratch
directory was empty after completion, no remaining process groups referencing
that scratch root were found, and the strict retention preview accepted the
finalized report. An attempted interrupt arrived after the run had already ended;
the report is a completed, failed-validation run, not an interrupted one.

Local evidence (temporary, not committed):

- `/tmp/mutie-reliability-self-_pvnl_6r`, including `command.json`,
  `implementation.diff`, `inputs-before.json`, `run.log`, `results.json` and `read.log`.
- Saved run: `reports/run-1791942980-cTkOCu` beneath that directory.
- Suite logs: `/tmp/mutie-group-default.log`, `mutie-group-rust.log`,
  `mutie-group-cache-recheck.log`, `mutie-group-deadline-recheck.log` and
  `mutie-group-go.log`.
- Loader sample: `/tmp/mutie-cargo-sample-60993.txt`.

The subsequent [unmutated serial/parallel controls](rust-nested-self-tests.md)
pass the complete default suite in isolated Cargo workers and add startup and
checkpoint diagnostics. They narrow the investigation but do not resolve or
supersede the failed self-run above.


## Later descriptor-inheritance reproduction

The earlier 100-run pass did not eliminate every capture failure. The subsequent
[paired survivor isolation](rust-survivor-isolation.md) reproduced EOF in pristine
code and identified an unrelated command holding the failed capture's pipe after
its process group disappeared. Spawning is now serialized in addition to the
existing group retry. See that follow-up for descriptor evidence and validation;
the earlier failed mutex-only experiment does not rule out this second cause.
