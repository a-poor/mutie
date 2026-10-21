# Intermittent CLI startup/readiness investigation

Started 2026-10-21 from `26e09ff`, following the
[Vitest fixture startup work](vitest-fixture-startup.md). The objective is to
identify and fix the remaining intermittent CLI readiness failure without
weakening deadlines or assertions. **The CLI failure reproduced with pre-main loader samples. A host-level cause
is not yet established, and no root-cause fix is claimed.**

## Original observation and reproduction controls

The preserved [failed full run](fixtures/vitest-fixture-startup/full-initial.log)
shows a living CLI, empty stdout/stderr, and no Go fixture PID when the noisy-log
cancellation test's ten-second readiness deadline expired. That evidence alone
cannot distinguish loader delay, runtime initialization, application work, or a
wait before the first fixture marker. The separate native mock's `_dyld_start`
sample does not establish this CLI's cause.

Before changing tests, ten parallel repetitions of all seven cancellation and
three log-limit tests passed: **100 passes**, no filters or retries within tests.
Whole-invocation times were 11.83–32.00 seconds:
[focused results](fixtures/cli-startup-readiness/before-focused.json).

Additional controls launched the actual CLI with `--help` and a ten-second
outer deadline:

- Four fresh filesystem copies passed in 0.51–0.58 seconds:
  [copied CLI](fixtures/cli-startup-readiness/copied-cli.json).
- Four fresh copies with distinct explicit ad-hoc signing identifiers passed in
  0.56–0.67 seconds: [signed CLI](fixtures/cli-startup-readiness/signed-cli.json).
  Only temporary copies were signed; no system security settings changed.
- A fresh `cargo rustc --offline --bin mutie -- -C metadata=cli_readiness_probe`
  link passed on its first launch in 1.07 seconds, then three further launches in
  25–35 ms: [fresh-link record and hash](fixtures/cli-startup-readiness/relinked-cli.json).
  The later ordinary Cargo tests restore the normal build. This diagnostic build
  changes compiler symbol metadata, not source or project configuration.

Three full-suite runs with an external watcher also passed all 277 tests with
93 existing opt-in ignores per run: [watched full runs](fixtures/cli-startup-readiness/watched-full.json).
The watcher checked only descendants of its own Cargo child, sampled CLI children
still alive after four seconds, and bounded each sampler to three seconds.
Some captured samples had empty call graphs; others showed initialized Tokio
workers, report persistence and filesystem calls during successful runs. None
captured the historical readiness failure, so they do not explain it. Some runs
overlapped the focused repetitions. These controls
are observations, not a guarantee under all host conditions or proof of a fix.

## Reproduced CLI loader stall

After two further passing focused rounds, the third failed five of seven
cancellation tests. The failing CLI processes had empty output and no fixture
start markers. Automatic sampling inside the sandbox was denied; the new stderr
capture preserves that limitation in the [failure log](fixtures/cli-startup-readiness/reproduced-cancellation.log).

A second sandboxed run used an external sampler with permission to inspect only
its recorded Cargo child's descendants. At approximately eight seconds after
launch, all seven sampled CLIs were at `_dyld_start` with about 112 KiB mapped
and no binary images available. The [representative stack](fixtures/cli-startup-readiness/cli-loader.sample.txt)
identifies CLI PID 4303; [all seven samples](fixtures/cli-startup-readiness/loader-samples/)
are retained. The PIDs belong to the [same failed run](fixtures/cli-startup-readiness/reproduced-watched.log).
Four tests failed; some CLIs subsequently reached baseline before the deadline
expired, leaving insufficient time to reach later checkpoints. This establishes
pre-main delay in the actual CLI, independently of the earlier mock diagnosis.
It does not establish a lock or wait inside Mutie's Rust code.

An overlapping run outside the sandbox also failed four tests after two passing
rounds. Its [post-deadline samples](fixtures/cli-startup-readiness/reproduced-unsandboxed.log)
showed already-running code and report writes. Those later samples do not prove
where its earlier startup time went. Running outside the sandbox is therefore
not a verified fix. Concurrency coincided with reproduction but is not isolated
as the cause.

The normal CLI's existing ad-hoc linker signature verifies with
`codesign -v --strict`. Filtered [host launch logs](fixtures/cli-startup-readiness/host-launch.log)
mention the CLI under `syspolicyd`, including an event near the sampled launch
window; they do not establish that service's responsibility. During these initial probes, no host security policy or quarantine/provenance attribute was changed. Signing
experiments affected temporary copies only; the normal CLI retains its existing
linker signature, and no production source was changed.
Apple's [trusted-execution troubleshooting guidance](https://developer.apple.com/forums/thread/706379)
provides signature/log investigation steps; it does not identify this failure.
A possible further controlled experiment is the per-app Developer Tools permission.
Apple [documents its security effect](https://support.apple.com/en-is/guide/mac-help/mchlc5fb7f9c/mac):
it allows the selected app to run software outside the system's security policy.
The user subsequently authorized a temporary experiment and reported adding and
enabling Codex, which was originally absent. With the permission enabled, three
unfiltered suites passed **278 tests each** (93 ignored), and ten overlapping
focused repetitions passed **110 tests**. CLI and test hashes remained unchanged.
See [phase evidence](fixtures/cli-startup-readiness/developer-tools/enabled/summary.json)
and [permission state](fixtures/cli-startup-readiness/developer-tools/permission-state.md).
Afterward the user confirmed disabling the permission; the added entry remains
present, so the original absent-entry configuration was not exactly restored.
The matching disabled-permission comparison also passed three full suites
(**278 tests and 93 ignores each**) and ten focused repetitions (**110 passes**).
See [disabled-phase evidence](fixtures/cli-startup-readiness/developer-tools/restored/summary.json).
All selected hashes matched within and between phases. Enabled full runs took
64.80–83.41 seconds; disabled full runs took 68.99–71.32 seconds. Neither phase
reproduced the stall. This sequential comparison is **inconclusive**, with no
verified effective-state/restart observation and possible cache/history effects;
it does not establish a permission remedy or repository fix.

## Retained diagnostic change

`tests/support/startup_diagnostics.rs` collects a best-effort macOS stack sample
only after a readiness assertion has already failed. It targets the owned,
unreaped CLI child, so its PID cannot be reused during collection. Readiness is
not re-evaluated afterward: late readiness still fails. Other Unix platforms
report that automatic sampling is unavailable.

The original bounds remain ten seconds for noisy-log readiness and fifteen
seconds for cancellation readiness. Poll intervals, early-exit checks, mutation
limits, report assertions and cleanup assertions remain unchanged. Sampling is
limited to one second of requested samples and a three-second process deadline;
a stalled sampler is killed and reaped. Saved diagnostic output is capped at
64 KiB of sample text plus 4 KiB of sampler stderr. Failure or denial of sampling
preserves the readiness failure and its ordinary PID/output diagnostics. No
production runner, startup, timeout, signal, score or report behavior changes.

A deterministic negative control launches an owned sleeping child that never
writes a readiness marker. It must fail the original ten-second assertion,
finish diagnostic collection within fifteen seconds total, and leave no waitable
child after the cleanup guard runs. This is **injected missing readiness**, not a
reproduction of the historical intermittent failure.

The initial negative control passed inside the sandbox in 10.08 seconds, where
sampling was unavailable, and outside it in 12.12 seconds, where the stack showed
`nanosleep`: [sandboxed control](fixtures/cli-startup-readiness/negative-sandboxed.log)
and [sampled control](fixtures/cli-startup-readiness/negative-unsandboxed.log).
The helper subsequently retained sampler stderr as well, to make denied sampling
explicit. Successful readiness checks do not invoke the sampler.

## Final validation and remaining work

Three unfiltered full-suite runs of the diagnostic changes passed all **278 tests**
with **93 existing opt-in ignores** each. See [timings](fixtures/cli-startup-readiness/final-full.json)
and [per-run counts](fixtures/cli-startup-readiness/full-counts.json), with
[complete logs](fixtures/cli-startup-readiness/full-validation.log). These passes
coexist with the focused failures above and do not establish a startup fix.
After the overlapping runs ended, ten parallel and ten serial repetitions of
both targets passed: **220 individual test passes**, including the negative
control in every repetition. Parallel Cargo invocations took 14.39–15.32 seconds;
serial ones took 30.40–41.79 seconds. See [timings](fixtures/cli-startup-readiness/isolated-focused.json)
and [complete logs](fixtures/cli-startup-readiness/isolated-focused.log). These
isolated passes do not explain the overlapping failures. Formatting, whitespace
checks and all-target Clippy with warnings denied pass. No versions or existing
opt-in test selections change.

The remaining work is a justified host-level remedy followed by repeated
acceptance, not a speculative change to code that the stalled process has not
reached. Keep the goal open and preserve the failures; passing repetitions alone
do not close it. If sampling is denied, repeat the diagnostic
run in an environment that permits sampling its owned child, without changing
readiness limits or accepting a late checkpoint.

## Deferral decision

On 2026-10-21 the user chose to defer this investigation and resume milestone
work. The goal is paused, not complete. Preserve the diagnostics, failed evidence,
original deadlines and assertions. The user confirmed disabling the temporary
Developer Tools permission. Revisit if startup failures disrupt ordinary runs,
become frequent, or reproduce on another machine. No root-cause fix is claimed.
