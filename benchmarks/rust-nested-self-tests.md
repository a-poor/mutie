# Unmutated serial and parallel self-test controls

Follow-up to `00dffcc` (process-group cleanup and explicit report-lock release).
The earlier six-mutation self-run had a clean baseline but four timeouts and an
unrelated kill of its previous survivor. Before attributing those failures to
mutation behavior, compare unmutated executions with one and two workers.

## Results

All controls below passed on the same macOS host. Test deadlines and assertions
were unchanged. These are diagnostic samples, not a performance benchmark or
proof that the intermittent failure is resolved.

| Control | Serial | Two concurrent workers |
| --- | --- | --- |
| Prebuilt cancellation binary, three rounds | 4.43–4.72 s | 7.21–7.71 s per worker |
| Same binary through Mutie's subprocess runner, three rounds | 4.43–4.85 s | 7.35–7.99 s per worker |
| Cancellation suite through Cargo, isolated fresh builds | 5.87 s | 9.18–9.80 s per worker |
| Repeat cancellation suite with existing artifacts | — | 9.03–9.27 s per worker |
| Complete default suite through Cargo | 87.69 s | 129.87–130.35 s per worker |

Each cancellation execution passed seven tests. Each complete execution passed
198 tests, with 23 ignored (including the new opt-in nested diagnostic). Direct
and nested prebuilt controls each ran nine suite executions: three serial, then
three pairs. The fresh Cargo cancellation builds took 36.84 seconds serially and
50.22–50.75 seconds in parallel; those build times are separate from execution.
Building the remaining full-suite targets took another 13.64 seconds serially and
15.77–15.82 seconds per parallel worker.

The Cargo controls used three separate source copies and artifact directories,
with `CARGO_INCREMENTAL=0` and private `TMPDIR`, `TMP` and `TEMP` directories.
Commands used `--locked --no-fail-fast --message-format=json`, first with
`--no-run`, then without it. Cancellation runs selected `--test cancellation`;
complete runs selected `--tests`. The full-suite input hashes were unchanged,
and every execution's scratch directory was empty afterward. No mutations were
applied and no test-thread limits were added.

## Reusable nested diagnostic

Build the cancellation integration test with `cargo test --locked --test
cancellation --no-run`; Cargo prints its executable path. Use that absolute path:

```sh
MUTIE_REPRO_TEST_BINARY=/absolute/path/to/cancellation-test-binary \
  cargo test --locked --bin mutie process::tests::nested_unmutated_test_binary \
  -- --ignored --exact --nocapture
```

This opt-in test invokes the prebuilt binary through `process::run_with_env`,
using the same direct-to-file capture mode as the earlier self-run. It executes
three serial rounds followed by three paired rounds, reports each status and
elapsed time, drains all tasks before returning an error, and checks temporary
cleanup. Its outer 120-second command bound does not change the integration
tests' own 15-second readiness/finish deadlines. Keep it ignored to avoid nested
self-testing in the default suite. It requires a test binary whose temporary
files are isolated and cleaned up; use the cancellation suite for this control.

The cancellation fixture now records an entry marker as soon as its Go shim
starts. Readiness timeouts include the requested checkpoint, available markers,
and CLI stdout/stderr. This distinguishes reaching the shim from reaching the
later test checkpoint; it does not infer why a process failed to start. These
changes preserve the existing deadlines and assertions. The updated cancellation
suite passed all seven tests in 5.13 seconds; formatting, Clippy with warnings
denied, and whitespace checks also passed.

## What remains unresolved

Parallel unmutated suites, fresh isolated Cargo artifacts, and the prebuilt
nested subprocess path all passed separately. The full Cargo controls ran from
a diagnostic driver, not through Mutie's complete Cargo adapter and mutation
worker scheduling. They do not establish that the previously observed loader
sample caused every failure, or that rebuilding different mutated executables
behaves identically. The failed historical self-run remains failed validation.

Next, use these controls and startup markers to compare the complete Cargo
adapter with fresh and reused worker artifacts, capturing the first stalled
process before its test fixture is dropped. Keep cache reuse opt-in and do not
use the earlier self-run's score or timings as evidence of finding quality.

Local evidence (temporary, not committed):

- Direct controls: `/tmp/mutie-unmutated-mznezvwo/results.json` and
  per-round `output.log` files.
- Nested runner: `/tmp/mutie-nested-unmutated.log`.
- Cargo controls: `/tmp/mutie-unmutated-cargo-452vdvko`, including
  `results.json`, `full-results.json`, `full-inputs-before.json` and per-worker logs.
- Drivers: `/tmp/mutie-unmutated-compare.py`,
  `/tmp/mutie-unmutated-cargo.py`, `/tmp/mutie-unmutated-full.py`.
