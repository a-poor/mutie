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

The next control at that stage was the complete Cargo adapter with fresh and
reused artifacts; its results follow below. Keep cache reuse opt-in and do not
use the earlier self-run's score or timings as evidence of finding quality.

Local evidence (temporary, not committed):

- Direct controls: `/tmp/mutie-unmutated-mznezvwo/results.json` and
  per-round `output.log` files.
- Nested runner: `/tmp/mutie-nested-unmutated.log`.
- Cargo controls: `/tmp/mutie-unmutated-cargo-452vdvko`, including
  `results.json`, `full-results.json`, `full-inputs-before.json` and per-worker logs.
- Drivers: `/tmp/mutie-unmutated-compare.py`,
  `/tmp/mutie-unmutated-cargo.py`, `/tmp/mutie-unmutated-full.py`.

## Complete Cargo adapter control

An additional ignored probe, `rust::adapter_probe::unmutated_cargo_adapter`,
invokes the production `rust::test` adapter itself. It covers fresh artifacts,
an initially empty worker cache, and a second execution through that same cache,
first serially and then with two concurrent workers. This includes package
cleanup, compilation, test execution, merged captures and outcome classification.
The application source is unmutated. Only the temporary copies' Cargo manifests
select a smaller target set: disable automatic integration-test discovery, set
`test = false` on the `mutie` binary, and explicitly select `tests/cancellation.rs`.
This preserves all seven cancellation tests and their existing deadlines.

The probe expects an absolute `MUTIE_REPRO_ROOT` containing two isolated Mutie
source copies, `project-0` and `project-1`, plus an empty `logs` directory. Create
an `owned` directory there and set `TMPDIR`, `TMP` and `TEMP` to that directory
for the probe. Do not point it at the working checkout. Each execution has a
300-second outer command deadline. Captures and result JSON remain in `logs`;
reserved filenames prevent a repeat invocation from overwriting prior evidence.

```sh
MUTIE_REPRO_ROOT="$ROOT" TMPDIR="$ROOT/owned" TMP="$ROOT/owned" TEMP="$ROOT/owned" \
  cargo test --locked --bin mutie rust::adapter_probe::unmutated_cargo_adapter \
  -- --ignored --exact --nocapture
```

The temporary manifest selection used for this comparison is:

```toml
# Add within the existing [package] table:
autotests = false
autobenches = false
autoexamples = false

[[bin]]
name = "mutie"
path = "src/main.rs"
test = false

[[test]]
name = "cancellation"
path = "tests/cancellation.rs"
```

A separate diagnostic watcher inspected only subprocesses referencing this
probe's private artifact directory. It was configured to save process state,
fixture checkpoint files and a one-second macOS stack sample if a CLI remained
alive for eight seconds, before the fixture's 15-second readiness deadline.

All **nine executions passed**, each running all seven cancellation tests: 63
individual test passes, no capture errors, interruptions or timeouts. The watcher
never observed a CLI cross its sampling threshold, so no stack samples were
needed or collected. An eight-second threshold on CLI lifetime is not a general
measurement of loader latency; no loader-cause conclusion follows from this pass.

| Adapter mode | Serial, including build | Two workers, per-worker total |
| --- | ---: | ---: |
| Fresh | 42.39 s | 63.31–64.76 s |
| Worker cache, initially empty | 43.35 s | 61.81–62.07 s |
| Same worker cache, second execution | 15.41 s | 21.65–21.73 s |

Compiler JSON confirms zero reused dependency records on fresh/cold executions,
155 reused dependency records on each warm execution, and both local targets
(`mutie` and `cancellation`) rebuilt in every execution. The application source
hashes and caller artifact sentinel were unchanged, and owned temporary storage
was empty after completion. These are single-sample diagnostic timings.

The probe is ignored by default to prevent recursive self-testing. Formatting,
Clippy with warnings denied, and whitespace checks passed. No production execution
behavior or test deadline was changed for this investigation.

This rules out a deterministic failure in the tested unmutated cancellation
matrix; it does not resolve the historical full-suite self-run. Next isolate the
previously surviving `src/score.rs` saved-percentage comparison (`<=` to `<`),
first with the focused suite and then the full target set, preserving the same
source/capture checks and first-stall watcher. That separates changed executable
behavior from the interaction among the full set of tests without another broad
six-mutation run.

Local evidence: `/tmp/mutie-adapter-probe-xhiqe_hd` contains `driver.log`,
`command.json`, `inputs-before.json`, `result.json`, `analysis.json` and per-case
captures/result JSON under `logs/`. The empty `diagnostics/` directory reflects
that no sampling trigger occurred. The setup/watcher driver is
`/tmp/mutie-adapter-driver.py` (temporary, not committed).
