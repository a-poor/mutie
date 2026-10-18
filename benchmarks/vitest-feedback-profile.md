# Hyparquet focused feedback-time investigation

Date: 2026-10-16. Follows [JavaScript acceptance](vitest-hyparquet-findings.md).
No production optimization, runner identity change or report schema change was
made. The new ignored test adds phase observations only to test builds.

## Repeat the same selection

Used the original prepared Hyparquet project, unchanged tests, and saved utils
survivors from `run-1792118088-p2pcbA`. Each run uses a full baseline and full test
suite for both candidates, fresh isolated copies, one worker and 120-second
baseline/mutation deadlines. Node 24.22.0's resolved binary directory was first
on PATH. Runs were sequential; no profiling or Cargo checks ran concurrently.

| Run | Report duration | Baseline command | Mutation commands summed | Shared workspace phases summed |
| --- | ---: | ---: | ---: | ---: |
| `run-1792161390-oPxxDP` | 26.60s | 5.76s | 10.81s | 4.65s |
| `run-1792161416-n74ZFE` | 25.77s | 5.37s | 11.13s | 3.96s |
| `run-1792161442-aPGQGY` | 26.42s | 5.41s | 11.41s | 4.08s |

All three completed with both survivors reproduced and no errors/timeouts.
External wall times were 27.44, 25.81 and 26.46 seconds. The prior identical
selection took 103.59 seconds, while a different twelve-candidate sample took
978.11 seconds. These are not before/after code improvements: source, test
selection, host conditions and launcher configuration must be distinguished.
The slow twelve-candidate sample is not a stable performance baseline.

Reports: `/tmp/mutie-hyparquet-feedback-reports/`.
Logs: `/tmp/mutie-hyparquet-feedback-{1,2,3}.log`.
External timings: `/tmp/mutie-hyparquet-feedback-repeats.json`.

## Actual adapter phase observations

The opt-in `real_feedback_phase_profile` test creates an outer workspace, measures
execution-identity validation and the additional project load, then invokes the
actual isolated adapter baseline. Test-only timers separate the adapter's copy,
validation, instrumentation/setup, process runner, evidence rendering and cleanup.
It verifies matching execution identity, passing classification and reports test
counts. Each of three repetitions passed all 724 tests, with zero failed/skipped.

This is a debug test-harness baseline profile, not a complete scored CLI run.
It omits coordinator/report writes, saved-selection rediscovery and worker reset
scheduling. Adapter total includes the nested phases; do not sum both columns.
Compilation and initial/final identity checks are outside per-repeat timings.

| Phase | Median across three baselines |
| --- | ---: |
| outer_workspace_copy | 0.700s |
| execution_identity | 0.122s |
| project_reload | 0.087s |
| adapter_total | 6.384s |
| outer_workspace_cleanup | 0.240s |
| adapter_copy | 0.714s |
| copied_project_validation | 0.093s |
| instrumentation_and_command_setup | 0.008s |
| process_runner | 5.331s |
| evidence_and_diagnostics | 0.003s |
| cleanup | 0.242s |
| Total measured baseline | 7.558s |

Project validation is small here, so removing safety checks is not justified.
Copies and cleanup are measurable, but the current profile does not explain the
previous run's much larger residual. No claim is made that copy cost caused the
entire earlier slowdown.

```sh
# Put the already-installed pinned Node binary directory first on PATH.
MUTIE_VITEST_PROFILE_PROJECT=/path/to/prepared/hyparquet \
  cargo test real_feedback_phase_profile -- --ignored --nocapture
```

Profile output: `/tmp/mutie-hyparquet-phase-profile.log`.
The test is ignored by default and requires an explicitly prepared external
project; it does not recursively run mutation campaigns.

## Node launcher versus resolved executable

Three serial, alternating-order pairs ran the identical Node identity expression
in the prepared project. Both paths resolved to the same Node 24.22.0 executable.
No Cargo checks or other task workloads ran concurrently with these confirmation
pairs. An earlier exploratory probe overlapped checks and is excluded here.

| Pair | Launcher | Direct binary |
| --- | ---: | ---: |
| 1 | 4.788s | 0.035s |
| 2 | 6.147s | 0.027s |
| 3 | 4.236s | 0.034s |

Launcher: `/path/to/home/.vite-plus/bin/node`.
Resolved binary: `/path/to/home/.vite-plus/js_runtime/node/24.22.0/bin/node`.
Results: `/tmp/mutie-hyparquet-node-probes-serial.json`.
The launcher is repeatedly invoked by execution-scope checks, while actual
Vitest commands already use the resolved executable. These observations do not
identify why the launcher is slow or prove the cause of the prior timeout.

A subsequent sequential CLI pair used the exact same saved survivors and project,
changing only the directory prepended to PATH. Both reproduced two survivors,
with matching execution identity, no errors/timeouts and no source edits:

| PATH choice | Report | Report duration | External wall | Baseline command | Mutation commands summed |
| --- | --- | ---: | ---: | ---: | ---: |
| Launcher | `run-1792161597-9JDQGQ` | 58.71s | 58.81s | 5.68s | 19.28s |
| Direct binary | `run-1792161656-SSaEhc` | 27.52s | 27.56s | 5.77s | 11.54s |

Reports: `/tmp/mutie-hyparquet-feedback-pair-reports/`.
External timings: `/tmp/mutie-hyparquet-feedback-pair.json`.
The paired result supports avoiding repeated launcher invocations, but one pair
cannot assign the entire difference to identity checks: mutation command times
also differed by 7.74 seconds. The adapter's Vitest child already uses the resolved
binary. Host/runner variation and PATH-sensitive child behavior are not isolated
by this pair. It also does not explain all of the earlier 978-second sample.

## Decision and verification

The next optimization candidate is to resolve Node once at run startup, then
revalidate using that resolved executable for baseline and mutation execution.
Preserve version/executable checks, package/config/runner-byte checks and saved
rerun identity. Verify replacement/missing executables and failures still fail
closed. Do not cache the entire project validation result or weaken source checks
on the basis of these timings. That optimization is not implemented here.

Until then, placing the installed pinned Node binary directory first on PATH
is a measured local workaround. Copy lifecycle changes are a separate candidate;
this evidence does not justify combining them with Node resolution changes.

All five new reports passed reader validation before the expected noninteractive
terminal error. SHA-256 checks preserved all 305 original project files and all
102 artifacts in the original two reports. Default Vitest-filtered tests passed
(21 unit tests; opt-in probes remained ignored), the phase profile passed all
three real baselines, and formatting/Clippy checks passed. The phase observations
are compiled only for tests, require explicit opt-in, and do not alter CLI output,
execution identity, report schema or package/operator versions.
