# Rust feedback: dismissal workflow costs

Measured on 2026-10-14 against `d93fdef`, following the
[runtime-thread experiment](rust-runtime-startup.md). Production code is unchanged.
The next optimization candidate is avoiding redundant publication of unchanged
derived report files while retaining checkpoint and durability guarantees.

## Method and controls

On the same macOS arm64 host, copy tracked sources into an isolated
temporary project and add timing probes to that copy only. The
[setup helper](profile-rust-workflow.py) reproduces those edits, asserting each
insertion matches exactly once. It does not build or execute tests. The probe
records per-process JSON lines outside fixture/source trees, guarded by a mutex
to prevent concurrent tasks from interleaving records.

Measure the prebuilt unchanged dismissal harness and the instrumented harness in
control/probe then probe/control order. Do this first for the exact
`score_gate_preserves_raw_outcomes_and_labels_filtered_and_rerun_scopes` test,
then for the complete 15-test dismissal harness. Clear inherited Tokio/libtest
thread overrides and retain default concurrency. Only the diagnostic focused
measurement filters tests; Mutie's production test selection is unchanged.
Every assertion, report sync, command deadline and process cleanup remains enabled.
An outer 120-second watchdog did not fire. External host load is uncontrolled.

Probe boundaries:

| Event | Measurement |
| --- | --- |
| `cli_main` | Async main body, including argument handling and workflow; excludes runtime construction/destruction and process launch |
| `report_flush` | Score/summary calculation and checkpoint publication in `SavedReport::flush` |
| `report_write` | Temporary file creation, write, sync and atomic publication for each report artifact |
| `report_sync` | The existing report-file `sync_all` call only |
| `command_to_exit` | Spawn/lock wait through observed direct-child exit |
| `group_shutdown` | Existing repeated process-group signaling and bounded wait |
| `command_total` | Existing command timer, including shutdown/capture drain and intervening probe emissions |

These timers overlap and nest; **do not add them together**. In the full harness,
CLI processes and commands also overlap across tests. Aggregates are cumulative
durations, not wall-time phases or CPU time. `report_write` also includes writes
outside `flush`. Timers omit some setup and final object destruction; this is not
an exhaustive cost accounting. The logger adds filesystem/locking work, included
in outer scopes and harness wall time; controls quantify its overall effect.

An initial unguarded logger produced interleaved JSON during a passing focused
test. That trace is retained locally but excluded from analysis. All results below
use the fixed logger, with a fresh control/probe sequence. No samples from that
sequence were discarded.

## Results

All eight fixed-logger/control runs passed: four focused tests and four complete
15-test harnesses. [Raw aggregate data](rust-workflow-costs.json) retains every
sample, event count, status and pass count.

| Scope | Control seconds, two samples | Probe seconds, two samples |
| --- | --- | --- |
| Focused score scenario | 6.94, 6.80 | 8.21, 7.41 |
| Complete dismissal harness | 19.44, 20.45 | 21.67, 23.87 |

The probe median is about 14% slower in both scopes. This is meaningful observer
overhead: these traces identify a candidate, not a predicted production speedup.

| Cumulative measurement | Focused probe range | Full-harness probe range |
| --- | --- | --- |
| CLI main bodies | 7.27–7.42 s | 111.51–123.95 s |
| Report flush | 5.63–5.67 s | 80.68–90.65 s |
| Report writes | 5.55–5.59 s | 68.73–76.83 s |
| Report sync calls | 4.39–4.41 s | 31.23–34.44 s |
| Command execution through exit | 0.72–0.78 s | 10.38–10.61 s |
| Command total | 0.75–0.82 s | 12.36–12.93 s |
| Process-group shutdown | 0.12–0.14 ms | 1.74–2.23 ms |

Both focused traces contain 12 CLI main spans, 127 flushes, 1,053 report writes/
syncs and 37 commands. Both complete traces contain 131 main spans, 524 flushes,
4,347 report writes/syncs and 182 commands. Report flush time is roughly 72–77%
of summed main-body time in these probes. This ratio is not a fraction of harness
wall time, nor a claim that all such work is avoidable.

Report publication is substantial even in the focused case, where the harness
runs only one test. Cleanup is negligible in these specific traces; this does
not justify weakening cleanup for descendant/fork/timeout cases. Cargo compilation
and startup are outside this experiment. Terminal/log-limit targets have not yet
received this phase instrumentation.

## Selected next candidate

Follow-up: [derived-report write validation](derived-report-writes.md) records
the implementation and controlled measurements for the candidate below.

`SavedReport::flush` currently republishes outcomes, metadata, summary and five
derived outcome lists at every checkpoint. Inspecting that path shows that lists
can remain unchanged across checkpoints, for example an empty build-failure list
throughout a successful workflow. The probe counts all writes; it does not measure
how many bytes or files would qualify for reuse.

Evaluate avoiding unchanged derived-file publication, with these constraints:

- Continue publishing authoritative outcomes/run metadata with matching checkpoint
  numbers; preserve partial/incomplete-run detection.
- Keep atomic replacement and `sync_all` for changed files. Preserve final text
  output, inventory, saved logs, error handling and cancellation behavior.
- Verify missing files, failed writes and unchanged-content cases explicitly;
  do not assume a cached publication succeeded or silently accept missing output.
- Measure the write reduction and runtime on the unchanged control, then full
  serial/parallel suites and saved score selections. Compare exact outcomes and
  source/artifact preservation. Do not infer the achievable saving from these sums.

No optimization has been implemented or accepted yet. Persistence is now the
measured next investigation target; runtime-thread tuning remains unchanged.

## Reproduction and evidence

Run the helper from a clean repository root, retain the printed absolute root,
and compile its copy before measuring. Use Cargo's printed executable path for
the instrumented harness and separately retain the unchanged control executable.

```sh
python3 benchmarks/profile-rust-workflow.py
# ROOT is the printed temporary root.
cargo test --locked --manifest-path "$ROOT/project/Cargo.toml" --test dismissals --no-run
# Create a fresh existing directory for each measured invocation.
mkdir "$ROOT/logs/sample"
env -u TOKIO_WORKER_THREADS -u RUST_TEST_THREADS \
  MUTIE_PROFILE_DIR="$ROOT/logs/sample" \
  "$ROOT/project/target/debug/deps/dismissals-<hash>" \
  --exact score_gate_preserves_raw_outcomes_and_labels_filtered_and_rerun_scopes
# Repeat without the filter for the complete harness; interleave the control.
```

Parse each per-PID JSON line and sum `seconds`/count by `phase`; keep scopes
separate and retain harness logs. Root for this run:
`/tmp/mutie-workflow-profile-c4lg0b0w`. `results-fixed.json` and
`logs/fixed-*` contain the valid matrix. The local run driver is
`/tmp/mutie-workflow-profile-run.py`. A second helper-generated copy was
compared byte-for-byte with all four instrumented source files to verify the
checked-in reproduction helper. The source checkout's production files remain
unchanged; fixture preservation assertions passed with the complete harnesses.
