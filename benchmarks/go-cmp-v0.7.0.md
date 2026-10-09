# go-cmp v0.7.0 validation

Measured on 2026-10-09 with Mutie commit `32b0b562a40490586ff5858d393a62b38f8c240f`,
built using `cargo build --release --locked`. The full inventory completed and
passed the report/source audit. See the [measurement data](go-cmp-v0.7.0.json)
for exact durations, copy counts, commands, environment, and source identity.

## Workload and method

- Project: `github.com/google/go-cmp` v0.7.0, upstream commit
  `9b12f366a942ebc7254abc7f32ca05068b455fb7`, copied from the local module cache.
- Size: 48 files, 470,126 logical bytes, 42 Go files and 12,650 Go lines including
  tests; 31 active implementation files containing 6,510 lines.
- Scope: all 1,450 discovered mutations across 10 packages, five with tests.
  Every mutation ran the full module test suite.
- Host: macOS, arm64; Go 1.27.2. Source, workspaces,
  reports, and cache were on the same APFS volume.
- Execution: four workers, plain UI, 30-second mutation deadline, 120-second
  baseline/discovery deadline, `GOMAXPROCS=8`, `CGO_ENABLED=0`, `GOENV=off`,
  `GOTOOLCHAIN=local`, `GOWORK=off`, empty `GOFLAGS`, and `GOPROXY=off`.
- A dedicated shared Go build cache was warmed by a passing preflight and
  calibration runs. Mutie disabled test-result caching with `-count=1`.
  This was not a cold-cache benchmark. No other Go/Cargo validation commands
  overlapped the timed runs; unrelated host load was not isolated.
- `TMPDIR` pointed outside the source tree. SHA-256 manifests verified that both
  the writable source copy and cached original were preserved.

An initial calibration used `GOMAXPROCS=2` and a ten-second mutation deadline.
It produced premature timeouts while ordinary packages were still progressing,
so it was interrupted and excluded from timing comparisons. Its finalized report
retained 133 completed, four interrupted, and 1,313 pending mutations, matching
checkpoints, the complete inventory/diffs, and active command diagnostics.
Rechecking its first four timed-out mutations with the final settings produced
three killed and one missed, with no timeouts.

## Full inventory

| Measurement | Result |
| --- | ---: |
| Run duration | 1,833.51 s (30m 34s) |
| Baseline command | 3.22 s |
| Package discovery command | 0.11 s |
| Mutation commands, cumulative | 7,059.62 s |
| Median mutation command | 4.34 s |
| Workspace operations, cumulative | 22.34 s |
| Killed / missed / timeout / build failed / tool errors | 1,111 / 262 / 27 / 50 / 0 |
| Report files | 5,815 |
| Report logical bytes | 1,345,815,146 |
| Log logical bytes | 1,343,894,963 (99.86% of the report) |

| Workspace stage | Operations | Cumulative seconds | Reflinked files |
| --- | ---: | ---: | ---: |
| Snapshot | 1 | 0.0108 | 48 |
| Baseline setup | 1 | 0.0070 | 48 |
| Discovery reset | 1 | 0.0116 | 48 |
| Worker setup | 4 | 0.0442 | 192 |
| Worker reset | 1,446 | 22.2694 | 69,408 |

All **69,744** file transfers used explicit reflinks; no fallback copies or
filesystem-operation failures were recorded. Repeated transfers of the same
source file count separately. These counts do not measure physical space saved.

Workspace operations account for about **0.315%** of measured workspace-plus-Go
command time. These are cumulative elapsed durations across overlapping workers,
not additive phases of wall time or CPU profiles. The longest worker command chain
was 1,766.74 seconds, leaving a 66.77-second gap to total run duration. That gap
includes baseline/discovery, setup, scheduling, mutation application, persistence,
and cleanup; it is not a measurement of report-writing cost.

The audit checked finalized states, matching checkpoints, inventory/outcome IDs
and counts, every saved source fingerprint and original byte range, diff/log
existence, outcome-list counts, and workspace transfer totals. All Mutie-owned
workspaces were removed. Raw reports remain outside version control under
`/tmp/mutie-go-cmp-s598wu91`; only compact evidence is kept here.

## Repeated worker comparison

The 32 sample IDs were chosen before the full run completed, evenly spaced across
its deterministic inventory: `1 + floor(i * 1449 / 31)` for `i = 0..31`. This
sample spans 17 files and all six operators. The same selection ran in 1/4/4/1
worker order against the shared warm build cache, with a fresh baseline and
full-module tests each time.

| Order | Workers | Run seconds | Cumulative mutation-command seconds | Killed / missed / timeout / build failed |
| --- | ---: | ---: | ---: | --- |
| A1 | 1 | 81.91 | 73.96 | 27 / 4 / 0 / 1 |
| B1 | 4 | 43.90 | 149.10 | 27 / 4 / 0 / 1 |
| B2 | 4 | 43.77 | 151.34 | 26 / 4 / 1 / 1 |
| A2 | 1 | 83.50 | 75.18 | 27 / 4 / 0 / 1 |

All runs passed the artifact/source audit and had no tool errors. A1, B1, and A2
matched the full run by mutation identity and outcome. B2 differed only for source
ID **328**, `cmp/internal/diff/diff.go:240`, changing `<` to `>=`: it timed out after
30 seconds instead of being killed. All four missed findings remained identical.
The timings show the observed benefit of four workers for this sample; two trials
and a varying execution path do not establish a general speedup or fully stable
outcomes.

The upstream diff implementation seeds a random direction choice from the clock
at `diff.go:122`, then chooses forward or reverse search at line 223. A separate
disposable copy reproduced the difference by adding a same-package test `init`
that sets `randBool` to false or true, and applying only mutation 328 after checking
its source fingerprint and byte range. Running
`go test -count=1 -timeout=3s ./cmp/internal/diff` produced:

| Code | Forced initial direction | Result |
| --- | --- | --- |
| Original | Reverse | Passed |
| Original | Forward | Passed |
| Mutation 328 | Reverse | Index-out-of-range panic in `TestDifference/#03` |
| Mutation 328 | Forward | Timed out in `TestDifference/#01` |

The controlled probe ties the killed/timeout variation to the diff algorithm's
initial direction. It ran outside the timed comparisons and preserved their
source inputs. Keep this case in future comparisons and distinguish execution
outcomes from whether a mutation was detected.

## Finding review and improved tests

Source ID **33**, `cmp/cmpopts/equate.go:89`, changes the `EquateNaNs` filter from
`math.IsNaN(x) && math.IsNaN(y)` to `||`. It survived the original full suite. The
[added regression test](go-cmp-v0.7.0-nan_test.go.txt) checks that a single NaN is
not equal to a finite value, in both argument orders and for float32/float64.
In a separate source copy, the full baseline passed and a focused rerun of ID 33
changed from **missed to killed**, with the new test's expected failure message.
The original source files and cached module were preserved.

Not every survivor is a test gap. Source ID **133**, `cmp/cmpopts/sort.go:78`,
changes `start >= 0` to `start > 0`. It appears equivalent: `start` is initialized
to `-1` and only assigned a loop index starting at one, so zero is unreachable.
This is a reviewed example, not an equivalence analysis of all 262 missed results.

## Implications and limits

1. Fix Go temporary-build cleanup after forced termination. The full run's 27
   timeouts left 27 `go-build*` directories totaling 566,027,850 logical bytes in
   `TMPDIR`, separate from the successfully removed Mutie workspaces. The earlier
   calibration left another 45 directories (496,433,335 bytes) after 41 timeouts
   and four interrupted commands. Give temporary build output an owned lifetime;
   keep the reusable Go build cache separate.
2. Investigate affected-package execution, including reverse dependencies and test
   imports, against this full-module reference. For example, `cmp`'s external
   tests import `cmpopts`; selecting only the mutated package would omit relevant
   tests. Go command duration includes tooling, builds, and execution, which were
   not separately profiled here. Account for the demonstrated direction-dependent
   result when checking outcome consistency.
3. Use the 1.35 GB report as evidence for the existing retention/log-output work.
   The JSON metadata is small; captured logs dominate retained size. Pruning and
   output limits still need an explicit policy that preserves useful diagnostics.

This validates a larger mutation inventory and a multi-package library with a
compact source tree. It does not establish behavior for large applications,
vendored repositories, or large assets. `/tmp` is also APFS on this host; there
was no Linux tmpfs or ordinary-copy comparison. Workspace-copy optimization is
not justified by these timings alone. Warm-cache results on one shared host are
observations, not a general concurrency or scaling guarantee.

## Reproducing the workload

Start in the Mutie repository with Go 1.24+ and the pinned module already present
in the Go module cache. If necessary, download v0.7.0 before disabling network
resolution. Use the JSON record to match the measured toolchain and environment.

```sh
cargo build --release --locked
mutie_bin="$PWD/target/release/mutie"
bench_root=$(mktemp -d)
cp -R "$(go env GOMODCACHE)/github.com/google/go-cmp@v0.7.0" "$bench_root/module"
chmod -R u+w "$bench_root/module"
mkdir -p "$bench_root/cache" "$bench_root/workspaces" "$bench_root/reports"
(
    export GOCACHE="$bench_root/cache" TMPDIR="$bench_root/workspaces"
    export GOMAXPROCS=8 CGO_ENABLED=0 GOENV=off GOTOOLCHAIN=local
    export GOPROXY=off GOWORK=off GOFLAGS=''
    cd "$bench_root/module"
    go test -json -count=1 -timeout=120s ./... > "$bench_root/preflight.jsonl"
    "$mutie_bin" "$bench_root/module" --jobs 4 --timeout 30 \
        --baseline-timeout 120 --ui plain --output-dir "$bench_root/reports"
)
```

The preflight warms standard builds; the measured cache also had calibration
activity. Keep cache conditions explicit when comparing results. Preserve the
printed report directory for the focused reruns and inspect
`run.json.workspace_metrics` alongside `outcomes.json` command durations.

For worker comparisons, reuse that report with `--rerun RUN_DIR --id IDS`, using
the comma-separated `method.sample_ids` from the JSON record and `--jobs` in
1/4/4/1 order. Keep the same environment and cache. For the NaN regression,
create a second writable module copy, add the saved test as
`cmp/cmpopts/mutie_validation_test.go`, and rerun source ID 33 with the same
baseline and mutation deadlines.
