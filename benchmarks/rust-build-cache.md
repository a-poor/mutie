# Rust worker build cache — 2026-10-13

This follows `f5cb198` (Rust comparison boundaries). The six score-boundary
mutations previously took 780.31 seconds under concurrent validation load.
Compilation was a substantial cost, so this prototype tests dependency reuse
before adding more operators. Package, report and catalog versions are unchanged.

## Ownership and rebuild contract

`--rust-build-cache fresh` remains the default and reference implementation.
`--rust-build-cache worker` (or `rust_build_cache = "worker"`) gives each worker
an owned temporary Cargo target/build directory for one run. The baseline always
builds in a separate fresh directory. Workers still restore their entire source
workspace before every mutation.

Before compiling each mutant, Mutie invokes
`cargo clean --locked --package PACKAGE` against that worker's owned directories.
This removes the local package's binary, unit-test and integration-test artifacts
while retaining registry dependency outputs. It uses Cargo's supported
[package cleanup](https://doc.rust-lang.org/cargo/commands/cargo-clean.html),
without manipulating internal fingerprint paths or trusting source mtimes.
Incremental compilation stays disabled. All tests execute for every mutant.

Caches are not shared between workers, with the baseline, or across runs. Caller
artifacts and sources are untouched. Worker caches drop after active subprocesses
have stopped, including failure and interruption paths. Cleanup, compilation and
tests share a mutation deadline; each invocation gets its own raw-output budget.
Cleanup failures are tool errors, never kills. A cleanup timeout records a
timeout without starting a build. Reports save the effective mode and retain
cleanup diagnostics alongside later phase output. Focused reruns may change mode.

## Correctness evidence

The ignored real-Cargo cache regression adds the already-locked `itoa` dependency
to a temporary fixture, then backdates its manifest, lockfile, source and tests
to 2000 before every compilation. It runs fresh/serial, cached/serial and
cached/parallel selections, expecting **two killed and one missed** each time.
Cargo JSON confirms every local target is rebuilt, dependencies become fresh
within each worker, baseline artifacts remain separate, and cache paths do not
cross worker/run boundaries. A deliberately failed local compilation is followed
by successful mutations on the same worker. A stronger separate integration
test then kills the saved survivor through a focused rerun.

The test includes a negative-control proxy switch. With
`MUTIE_SKIP_PACKAGE_CLEAN=yes`, the regression failed: stale binaries produced
**three killed and zero missed**. Restoring package cleanup passes. This
demonstrates that the backdated-source check detects a false kill, rather than
merely checking that an output directory was reused.

Four additional Cargo fault cases cover cleanup exits, signals, output overflow
and compiler startup failure after successful cleanup. They preserve completed
results, interrupt the parallel peer, leave queued work pending, retain bounded
diagnostics and keep score policies unevaluated. Controlled cleanup timeout and
cancellation cases check descendant termination, saved state and owned-directory
removal; the timeout case successfully reuses the worker for the next mutation.
Parallel compilation cancellation also runs in worker-cache mode. Original
source bytes, reports and caller artifact sentinels remain intact.

## Validation commands

```sh
cargo test --locked
cargo test --locked --test rust_workflow -- --ignored --test-threads=1
cargo test --locked --test go_workflow go_workflow -- --ignored --exact
cargo fmt --check
cargo clippy --all-targets --locked -- -D warnings
```

The default suite passed **196 tests** (132 unit and 64 integration), with 22
opt-in tests ignored. All 16 opt-in Rust workflows passed in 311.94 seconds.
The final cache regression, including explicit baseline isolation assertions,
also passed in 33.10 seconds. The real-Go workflow passed in 48.84 seconds under
concurrent validation load. Formatting and Clippy passed.

An initial default run timed out in an existing Go cancellation test; its focused
rerun and the final full suite passed. The exact saved-configuration assertion
also needed the new default `rust_build_cache` field. The negative control above
is expected to fail and is not part of normal validation.

## Controlled Mutie self-run

The comparison uses one frozen temporary copy of the current source, without
weakening tests. Fresh and worker modes run sequentially after other validation
jobs finish, using the same CLI binary, two workers, six score-boundary mutations
and the complete default test suite:

```sh
target/debug/mutie "$PROJECT" --include src/score.rs \
  --operator comparison_boundary --rust-build-cache fresh \
  --jobs 2 --timeout 600 --baseline-timeout 300 --ui plain \
  --output-dir "$FRESH"
target/debug/mutie "$PROJECT" --include src/score.rs \
  --operator comparison_boundary --rust-build-cache worker \
  --jobs 2 --timeout 600 --baseline-timeout 300 --ui plain \
  --output-dir "$WORKER"
```

Both runs override caller target/build paths to sentinel directories and place
owned scratch files under an isolated `TMPDIR`. Input SHA-256 hashes, sentinel
contents and an empty owned directory are checked after each run. Cargo stderr
supplies compile-phase time, and compiler-artifact JSON identifies fresh/rebuilt
dependencies and local targets. The host uses rustc/Cargo 1.99.0. This is one
sequential pair on one host, not a statistically controlled speedup estimate;
the earlier 780.31-second run had concurrent load and is context only.

Both runs produced **five killed and one missed**, with no build failures,
mutation timeouts or tool errors. Mutation inventories and outcomes match
exactly; ID 5 remains the narrow saved-percentage tolerance-endpoint survivor.
Both baselines and both survivor executions passed all **196 default tests**,
with 22 opt-in tests ignored. No tests were weakened or skipped for timing.

| Measurement | Fresh | Worker cache |
| --- | ---: | ---: |
| Whole run | 846.13 s | 651.97 s |
| Baseline command | 226.89 s | 131.39 s |
| Run time minus baseline command | 619.24 s | 520.58 s |
| Cumulative mutation compilation | about 436 s | about 274.79 s |
| Cumulative mutation command time | 1,226.62 s | 1,031.76 s |
| Cumulative workspace work | 0.276 s | 0.433 s |

Command and compilation totals overlap across the two workers. The remainder
after subtracting the baseline includes setup, discovery and cleanup as well as
mutation execution. Compilation timings come from Cargo's `Finished` lines;
minute-formatted values are rounded to whole seconds. Other command costs include
tests, cleanup and orchestration, so subtracting compilation is not a precise
test-only measurement.

| ID | Outcome | Fresh compile | Cached compile | Fresh command | Cached command |
| --- | --- | ---: | ---: | ---: | ---: |
| 1 | Killed | about 78 s | about 70 s (cold) | 229.03 s | 206.20 s |
| 2 | Killed | about 79 s | about 69 s (cold) | 228.61 s | 204.05 s |
| 3 | Killed | about 72 s | 45.81 s (warm) | 196.55 s | 163.95 s |
| 4 | Killed | about 73 s | 44.59 s (warm) | 196.81 s | 162.89 s |
| 5 | Missed | about 68 s | 22.97 s (warm) | 189.28 s | 148.88 s |
| 6 | Killed | about 66 s | 22.42 s (warm) | 186.34 s | 145.78 s |

Every compilation rebuilt all 14 local target artifact records. Both worker
caches started cold; all 155 dependency artifact records were reused in each of
the four warm builds. Fresh mode reused none. All **134 input-file hashes**,
caller sentinels and owned-directory cleanup checks passed after both runs.
The strict report reader accepted both finalized reports via retention preview.

The observed whole-run reduction is 194.15 seconds, but **95.50 seconds of that
difference comes from baselines that both built fresh**. This pair supports a
useful dependency-reuse prototype, not a stable 23% overall speedup claim. Warm
builds also varied substantially. Keep reuse opt-in and fresh builds as the
default; broader Cargo layouts and reuse between runs remain outside scope.

Self-testing exposed additional assertion failures alongside the intended score
failures: `limited_capture_does_not_wait_for_descendants_after_parent_exit`
failed its `capture_error.is_none()` check in fresh ID 1 and cached IDs 1–2.
Cached ID 1 also failed the retention writer-lock test's post-drop selection
assertion. These extra failures did not determine a kill: each affected mutation
independently failed score assertions in both modes. They did not occur in either
baseline or survivor. Their causes are not established; preserve the logs and
investigate the capture/lock reliability before broadening cache defaults.

Local evidence (temporary, not committed):

- Root: `/tmp/mutie-rust-cache-self-cqj097di`
- Fresh report: `fresh/run-1791902393-mYzFGN`
- Cached report: `worker/run-1791903239-bEPphO`
- Commands and logs: `fresh-command.json`, `worker-command.json`, `fresh.log`, `worker.log`
- Source/toolchain evidence: `inputs-before.json`, `implementation.diff`, `toolchain.txt`
- Metrics and preservation checks: `results.json`, `analysis.json`
- Read-only report validation: `fresh-read.log`, `worker-read.log`
