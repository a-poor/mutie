# Rust full-suite feedback profile

Measured on 2026-10-14 after `1881dcf`, with the diagnostic changes described
below. This profiles unchanged Mutie suites through the production Cargo adapter;
it does not claim a production speedup or replace mutation-outcome comparisons.

## Method

The ignored `rust::adapter_probe::profile_cargo_adapter` runs one cold and two
warm executions per worker, first with one worker, then with two. Each stage uses
new worker-local dependency caches. Every execution cleans and rebuilds the local
package and runs the complete default `cargo test --tests` scope. There are nine
executions, with unchanged 300-second command deadlines. No tests are filtered,
assertions weakened, or local package artifacts reused.

The host is macOS arm64, and Rust/Cargo 1.99.0. The
serial stage precedes the parallel stage; this is a small observational sample,
not a randomized experiment. External host load is uncontrolled. A short Clippy
check overlapped the first serial cold build. A process watcher polls every two
seconds; its optional stack sampling is only triggered by long-lived CLI children.

The diagnostic records monotonic Building/Testing callbacks and the time at
adapter return. The offline [parser](profile-rust-adapter.py) combines these with
Cargo compilation summaries, compiler-artifact freshness, and ordered per-target
libtest summaries. The gap outside harness timings includes executable loading,
scheduling gaps, shutdown and capture preparation: **it is not a direct measure
of Cargo startup alone**. Build/cleanup includes local-package cleaning. Adapter
timing excludes initial workspace copies, cache construction, final cache-directory
removal and the diagnostic's copying of captures to retained files.

## Reproduction

Prepare an absolute temporary root with two identical isolated Mutie source copies
named `project-0` and `project-1`, plus empty `logs` and `owned` directories, as in
the [adapter diagnostic instructions](rust-nested-self-tests.md). Copy tracked and
nonignored source files, excluding generated artifacts. Record source hashes before
execution and compare afterward; retain a separate caller-artifact sentinel when
checking Cargo environment overrides. Do not run the probe against a working tree.

Build the diagnostic before starting the measured run, then execute its unit-test
binary directly to avoid including the outer Cargo build:

```sh
cargo test --locked --bin mutie --no-run
# Use the executable path Cargo prints; ROOT is the prepared absolute root.
MUTIE_REPRO_ROOT="$ROOT" TMPDIR="$ROOT/owned" TMP="$ROOT/owned" TEMP="$ROOT/owned" \
  target/debug/deps/mutie-<hash> --ignored --exact \
  rust::adapter_probe::profile_cargo_adapter --nocapture
python3 benchmarks/profile-rust-adapter.py "$ROOT" > "$ROOT/timings.json"
```

Use a new root for every matrix: reserved capture names intentionally prevent
overwriting earlier evidence. The parser can summarize completed samples during
a run, so check that the final matrix contains all nine successful samples.
It rejects failed executions, missing phases, unmatched harness summaries and
reused local targets. Raw captures and JSON remain available for independent checks.

Local evidence root: `/tmp/mutie-runtime-profile-full-3f7rf75h`.

## Results

All nine executions passed 200 tests, with 27 opt-in tests ignored, across 13
harnesses. Every execution rebuilt all 14 local targets. Cold builds reused zero
of 155 dependency artifact records; warm builds reused all 155. The complete probe
took 910.96 seconds. Source hashes matched afterward, caller-artifact sentinels
were preserved, and owned temporary storage was empty. There were no timeouts,
interruptions, capture errors or failed tests.

Seconds below are per-command medians, with command ranges in parentheses.
Individual phase medians need not sum exactly to the median total.

| Workers | Cache | Samples | Command | Build/cleanup | Harnesses | Outside harnesses in test phase |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | Cold | 1 | 158.29 | 51.81 | 101.13 | 5.34 |
| 1 | Warm | 2 | 113.67 (108.12–119.23) | 20.09 | 89.26 | 4.32 |
| 2 | Cold | 2 | 207.14 (207.07–207.20) | 69.76 | 130.32 | 7.06 |
| 2 | Warm | 4 | 158.53 (154.67–162.06) | 21.96 | 131.37 | 5.57 |

Cargo's own compilation medians were 50.86/19.56 seconds serial cold/warm and
68.50/21.54 seconds parallel cold/warm. These are contained within build/cleanup,
not additional costs. The last parallel pair triggered one process snapshot and
an attempted one-second stack sample; no stack-sample file was produced. Its
observation overhead is included, and those samples are retained rather than
discarded as slower outliers.

The largest harnesses, using warm medians:

| Target | One worker | Two workers |
| --- | --- | --- |
| `tests/dismissals.rs` | 21.09 | 33.66 |
| `tests/terminal.rs` | 20.52 | 28.80 |
| `tests/log_limits.rs` | 12.84 | 17.83 |
| `tests/rerun.rs` | 7.99 | 12.83 |

Harness execution represents about 79% of the serial warm median command and
83% of the parallel warm median command. The first three targets account for
about 61% of warm harness time in either configuration. Two workers increase
individual command latency; this does not imply lower overall throughput, since
they complete two commands concurrently. These unchanged passing suites also do
not measure early failure paths or substitute for saved-mutation comparisons.

## Decision and next experiment

Investigate subprocess startup and concurrency inside the dominant integration
targets before further cache work. Dismissal tests repeatedly launch Mutie and
shell-based Go shims; terminal and log-limit tests also exercise child processes.
The CLI currently constructs its Tokio runtime before argument handling. Runtime
thread creation is one hypothesis to measure, not an established cause or a reason
to change defaults yet.

Next, use interleaved repetitions of a prebuilt CLI and the dismissal harness to
separate process/runtime startup from workflow work, then compare controlled
runtime concurrency settings. Keep all assertions, command deadlines and test
scope unchanged. If a candidate improves this target, validate it on full suites,
the unchanged saved score selection, cancellation and source/artifact preservation.
Do not remove process cleanup or descriptor protections to reduce elapsed time.

The diagnostic and parser change no production behavior. Validation includes the
nine full suites, formatting and Clippy, parser checks against all retained captures,
and negative controls rejecting failed executions and missing harness evidence.
No production speedup is claimed by this profiling change.
