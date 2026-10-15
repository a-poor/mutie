# Rust feedback: CLI startup and runtime concurrency

Measured on 2026-10-14 at `2e94598`, following the
[full-suite profile](rust-feedback-profile.md). No production code changed.
Decision: these samples do not justify changing Tokio's runtime defaults.

## Controls

The earlier profile identified dismissal, terminal and log-limit harnesses as the
dominant warm-suite costs. This experiment tests the narrower hypothesis that
CLI startup or excessive Tokio worker threads explain dismissal harness latency.
It does not yet attribute individual workflow phases or test the other harnesses.

The host is macOS arm64, Rust/Cargo 1.99.0 and the pinned
Tokio 1.53.1. `src/main.rs` constructs its runtime before parsing CLI arguments.
The dismissal harness launches the same prebuilt debug CLI for its fixture commands.
Tests use isolated temporary projects and shell Go shims; no real Cargo mutation
run or compilation occurs inside these timed samples.

Built with `cargo test --locked --test dismissals --no-run` before timing. Ran the
printed harness directly with no arguments: all 15 tests, no filtering, default
libtest concurrency and unchanged assertions/deadlines. The experiment removes
inherited `RUST_TEST_THREADS` and `TOKIO_WORKER_THREADS`, then either leaves Tokio's
setting absent or sets it to `1` or `2`. The variable is inherited by CLI children;
it does not change Mutie's mutation-worker setting or libtest concurrency.

Each phase uses three rounds with rotated orders:

| Round | Order |
| --- | --- |
| 0 | default, 1, 2 |
| 1 | 2, default, 1 |
| 2 | 1, 2, default |

First, each cell launches `target/debug/mutie --help` 20 times sequentially,
capturing output and checking every exit status. Wall time divided by 20 is the
cell's average, not a per-launch percentile. Second, each cell launches the entire
dismissal harness once, capturing its output and requiring a zero exit status.
Timing uses Python's monotonic `time.perf_counter()` around subprocess execution.
An outer 120-second watchdog bounds each invocation; it did not fire. No competing
benchmark/build was deliberately launched, but external host load is uncontrolled.

Raw captures and the measurement driver are under
`/tmp/mutie-runtime-startup-6l6ad67y` and
`/tmp/mutie-startup-probe.py`. The checked-in
[sample data](rust-runtime-startup.json) retains all 18 rows, harness pass counts
and executable hashes taken after measurement. The driver uses absolute local
binary paths; use the executable printed by your own build when reproducing.

Equivalent individual invocations from the repository root are:

```sh
cargo test --locked --test dismissals --no-run
env -u TOKIO_WORKER_THREADS -u RUST_TEST_THREADS target/debug/mutie --help
env -u RUST_TEST_THREADS TOKIO_WORKER_THREADS=1 target/debug/mutie --help
env -u RUST_TEST_THREADS TOKIO_WORKER_THREADS=2 target/debug/mutie --help
# Repeat the same environment variants with the printed dismissal executable,
# without test-name filters or --test-threads overrides, in the orders above.
```

## Results

Medians and ranges are across three cells per setting. The help measurements
include process launch, executable loading, runtime setup, parsing, formatting
and exit; they do not isolate runtime construction alone.

| Tokio workers | Help milliseconds per launch, median (range) | Dismissal seconds, median (range) |
| --- | --- | --- |
| Default | 10.44 (10.15–53.86) | 20.28 (19.61–20.67) |
| 1 | 10.88 (9.63–11.29) | 19.82 (19.68–20.84) |
| 2 | 10.83 (10.41–10.94) | 19.70 (19.17–19.74) |

The slower first default help batch is retained. There was no separate warmup,
so initial launch/cache effects and order are confounded with that batch; it is
not evidence of a repeatable default-runtime penalty. Later default batches are
about 10 ms, comparable to the reduced-thread variants.

All nine dismissal runs passed all 15 tests with no skips or filtering. One and
two threads reduce the observed median by roughly 2.3% and 2.9%, respectively,
but ranges overlap with the default and the sample is small. These results do not
establish a meaningful full-suite improvement, behavior under paired Cargo workers,
or safe runtime-default changes. No baseline/mutation source, execution scope,
cleanup protection or timeout was changed. The repository was clean after the
measurement, before recording these results.

## Next measurement

Keep the production runtime unchanged. Attribute time within the dismissal
workflow to command execution, report persistence and process cleanup using an
opt-in diagnostic on an isolated source copy. Start with the long
`score_gate_preserves_raw_outcomes_and_labels_filtered_and_rerun_scopes` scenario
and cross-check against the complete harness. These are candidate costs to measure,
not established bottlenecks; do not bypass durable writes or cleanup as an assumed
optimization. Extend the diagnosis to terminal/log-limit targets if the costs differ.

Only after identifying a candidate should it be compared on full serial/parallel
suites and the unchanged saved score selection, retaining exact failure sets,
cancellation, package rebuilds and source/artifact preservation. This experiment
rules out treating thread-count tuning as an obvious next optimization; it does
not complete the broader feedback-time objective.
