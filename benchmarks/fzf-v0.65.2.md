# fzf v0.65.2 application validation

Validated on 2026-10-11 with Mutie commit `d5419d4` and the release binary. A fixed
32-mutation sample produced identical outcomes with full-module and affected-package
tests: **6 killed, 21 missed, 5 build failures, no timeouts or tool errors**.
Affected scope averaged **49.91 seconds versus 59.76 seconds**, a 16.5% reduction
on this host. Added tests killed two selected survivors in both scopes.

The [measurement data](fzf-v0.65.2.json) includes source hashes, mutation identities,
per-trial outcomes and package targets, timings, workspace metrics, disk samples'
aggregates, report hashes, and the calibration run. This is a stratified sample
of fzf's **Go tests**, not a project-wide score or an evaluation of its separate
Ruby/tmux integration suite.

## Source and method

- Used the official [v0.65.2 source](https://github.com/junegunn/fzf/tree/v0.65.2),
  commit `416aff86e9d550d0a34af9d3cc12dfede25354c3`, in disposable copies.
  The archive SHA-256 is recorded in the data. The source has 130 files,
  **1,344,688 logical bytes**, and 74 Go files containing 25,561 lines, including
  tests. It is a real application but still a modest tree, about 2.9 times the
  earlier go-cmp tree's byte size.
- Used macOS arm64, APFS, Go 1.27.2,
  `GOMAXPROCS=8`, `CGO_ENABLED=0`, `GOWORK=off`, and `/bin/zsh` as `SHELL`.
  Dependencies were fetched before timing. Reused writable Go module/build
  caches, disabled module downloads during runs, and cleared CI and `FZF_*`
  environment overrides. Source, temporary work, and reports were on APFS.
- The plain Go preflight passed. There are six module packages: four with Go
  tests (`src`, `src/algo`, `src/tui`, `src/util`), plus the root and `src/protector`
  without tests. Two Windows-specific tests skipped on macOS. Mutie discovered
  6,757 mutations across the seven current operators.
- Published a complete inventory in a seed run, then interrupted it. Before
  inspecting outcomes, selected eight `call_remove` mutations and four from each
  other operator. Within each operator's source-ID order, selected index
  `floor(i * (N - 1) / (n - 1))` for `i = 0..n-1`. The resulting 32 mutations span
  ten files. This deliberately overweights call removal; it is not a random sample.
- Every trial reran the same seed IDs with two workers, a 30-second mutation
  timeout, a 120-second baseline/discovery timeout, plain output, and a 65,536-byte
  retained-payload limit per log artifact. Each trial ran a fresh full-module
  baseline; affected trials rediscovered the package graph.
- Ran a full-scope calibration first to warm the mutation variants, excluded it
  from the timing comparison, then ran **affected / full / full / affected**.
  The calibration exclusion and added full trial were decided before inspecting
  the first affected result. No other Go/Cargo builds or tests overlapped these
  runs. Disk sampling and lightweight source review did overlap; this was not an
  otherwise isolated benchmark host.

## Runtime and outcomes

| Trial | Scope | Run seconds | Baseline seconds | Cumulative mutation command seconds | Cumulative workspace seconds |
| --- | --- | ---: | ---: | ---: | ---: |
| affected-1 | Affected | 50.326 | 4.256 | 84.099 | 1.063 |
| full-2 | Full | 59.185 | 4.249 | 102.097 | 1.124 |
| full-3 | Full | 60.331 | 3.649 | 104.346 | 1.107 |
| affected-2 | Affected | 49.499 | 3.882 | 83.301 | 1.093 |

All four trials and the excluded 69.107-second calibration matched every selected
mutation's outcome. The raw sample score was `6 / (6 + 21) = 22.22%`, excluding the
five build failures. It describes these 32 selected mutations only.

Across the 32 commands, full scope targeted 192 package instances and affected
scope targeted 78. These counts include packages without tests; they are not
counts of executed tests. Affected selection includes transitive reverse
dependencies: for example, a `src/tui` mutation selected `src/tui`, `src`, and the
module root. This sample supports using the opt-in mode here, but does not prove
equivalence for all fzf mutations or dependencies outside the Go import graph.

Every original trial recorded 4,550 successful explicit reflink transfers and
zero fallback copies: 35 workspace operations over 130 source files. Workspace
work was small relative to Go commands on this tree. Cumulative durations overlap
across workers and cannot be added to derive elapsed run time.

## Disk use and cleanup

All values below are logical bytes. The temporary parent was scanned every
250 milliseconds **plus scan time**, with separate groups for workspaces,
`mutie-go-*` build directories, `.tmp*` capture files, and test-created/other files.
The scans are non-atomic; sampled peaks are lower bounds, and independently
measured group maxima need not coincide. Reflinks make these unsuitable as
physical-allocation estimates. They do not measure memory/RSS.

| Trial | Peak temporary total | Peak Go build files | Peak capture files | Final report | Final logs |
| --- | ---: | ---: | ---: | ---: | ---: |
| affected-1 | 63,220,228 | 59,128,708 | 82,877 | 1,548,725 | 1,476,883 |
| full-2 | 66,411,427 | 62,374,030 | 126,417 | 2,339,083 | 2,269,528 |
| full-3 | 66,411,433 | 62,374,030 | 134,808 | 2,339,060 | 2,269,533 |
| affected-2 | 59,653,112 | 55,537,303 | 82,994 | 1,548,749 | 1,476,904 |

Each trial's sampled workspace peak was 4,034,066 bytes. The 165–200 scans per
measured trial took 1.90–2.96 cumulative seconds. Source copies, saved reports,
the Go module cache, and the reusable build cache are outside the temporary
totals. Every original report has 144 files. With the configured log limit,
27 mutant stdout streams were truncated per full trial and four per affected
trial; baseline streams could also be truncated. Classification still used
complete captures. Truncation notices are additional to the retained-payload limit.

All Mutie-owned workspaces, Go build directories, and capture files were removed.
However, fzf's Go tests create files directly under inherited `TMPDIR`, including
`fzf-history*` and files produced through `src/functions.go`'s temporary-file helper.
They remained outside Mutie's managed workspaces: **220 files / 15,961 bytes** per
full trial and **172 files / 12,571 bytes** per affected trial. These files were
preserved in the per-trial temporary parents for inspection.

This suggests a concrete cleanup improvement: give each Go command an owned
general test temporary directory as well as its owned `GOTMPDIR`, and clean it
after normal completion, timeout, and cancellation. Preserve caller-owned files
and reusable caches. The observed leftovers are small; this is a lifetime and
isolation gap, not evidence of large disk consumption in this sample.

### Test temporary-directory cleanup follow-up

Implemented on the working tree based on `1424d9e`. Each Go command now receives
a separate owned `mutie-test-*` directory through child `TMPDIR`, `TMP`, and
`TEMP`. Its lifetime covers the command and process-group shutdown; `GOTMPDIR`
keeps its separate ownership and placement rules.

Repeated the original 32-ID sample with the release build, once per scope.
Both runs retained **6 killed, 21 missed, 5 build failures, and no timeouts or
tool errors**, matching every original outcome. Both per-trial temporary parents
were completely empty after exit, down from 220 leftover files for full scope
and 172 for affected scope. Source hashes remained unchanged and both saved
reports passed Mutie's reader. The [follow-up data](fzf-v0.65.2-temp-cleanup.json)
records commands, binary/source hashes, reports, and cleanup results; raw evidence
remains under the original temporary root as `cleanup-full` and `cleanup-affected`.
These checks make no new timing or peak-disk comparison.

Regression coverage also checks success, tool/startup errors, concurrent workers,
timeouts, SIGINT/SIGTERM, relative parent placement, and nested files/symlinks
without removing caller-owned files or reusable caches. The real-Go fixture leaves
temporary files before a deliberately hanging mutation and verifies their cleanup.
Removal still cannot run after a forced kill of Mutie itself, and tests that
explicitly write outside the supplied temporary directories remain outside this
ownership.

## Findings and build-failure noise

The eight selected call removals produced one killed, five missed, and two
unbuildable mutations. The two build failures left either `tempFiles` unused
(`src/terminal.go:4258`) or the `unix` import unused (`src/util/util_unix.go:92`).
The other three build failures replaced string concatenation with subtraction
in `main.go:36`, `src/options.go:3556`, and `src/util/util_unix.go:57`. No import,
local-variable, or other repair edits were applied. These outcomes remain separate
from evidence of tests catching behavioral changes.

Two call-removal survivors were selected for focused tests in a second copy:

| Seed ID / sample-run ID | Removal | Added check | Full / affected rerun |
| --- | --- | --- | --- |
| 1690 / 9 | `src/options.go:1715`, `appendAction(actBell)` | `--bind ctrl-x:bell` produces one bell action | Killed / killed |
| 5949 / 25 | `src/tui/light.go:94`, `fmt.Fprint(r.ttyout, sequence)` | Pass-through output reaches the supplied output file on refresh, without duplication on a second refresh | Killed / killed |

The [bell-binding test](fzf-v0.65.2-bell_test.go.txt) inspects parsed key bindings.
The [output test](fzf-v0.65.2-output_test.go.txt) uses a temporary output file and
does not require a real terminal. Both improved baselines passed. The focused
reruns failed in the expected new tests, with `got []` for the missing binding
and empty output for the missing write. All upstream implementation files retained
their original hashes. The two-mutation reruns are correctness checks, not timing
comparisons or a claim of complete feature coverage.

Other sampled survivors include debug printing, terminal border layout, and
preview scrolling. They were not all investigated for equivalence or covered by
new tests. fzf's separate Ruby/tmux suite was not run, so the demonstrated gaps
apply to the Go-test command Mutie currently executes. Eight stratified removals
cannot establish a general call-removal noise rate or justify broad source repair.

## Reproduce

Use the pinned source and Mutie commit above, isolated writable source copies,
and writable caches populated with the pinned dependencies. Match the environment
in the JSON, with `GOENV=off`, `GOTOOLCHAIN=local`, `GOPROXY=off`, `GOWORK=off`,
`GOMAXPROCS=8`, `CGO_ENABLED=0`, empty `GOFLAGS`, and no CI or `FZF_*` overrides.
Set absolute paths for `PROJECT`, `REPORTS`, and `TEMP_PARENT` outside each other.
Build Mutie with `cargo build --release --locked`.

First publish an inventory using this command. Interrupt with Ctrl-C after
`mutants.json` has `complete: true`, or let it finish; its outcomes are not used
to choose the sample. Set `SEED` to the printed run directory.

```sh
mkdir -p "$TEMP_PARENT/seed"
TMPDIR="$TEMP_PARENT/seed" GOTMPDIR="$TEMP_PARENT/seed" target/release/mutie "$PROJECT" \
  --jobs 2 --timeout 30 --baseline-timeout 120 --max-log-bytes 65536 \
  --ui plain --output-dir "$REPORTS/seed"
```

Confirm source hashes and inventory identities against the JSON before using the
saved IDs. From the Mutie repository, define and run the fixed sample:

```sh
IDS=1,2,8,9,14,25,108,1448,1690,2085,2153,2191,2225,2480,2487,3246,4009,4337,4373,4394,4464,4515,4898,4970,5949,6714,6723,6742,6748,6749,6756,6757
run_sample() {
  mkdir -p "$TEMP_PARENT/$1"
  TMPDIR="$TEMP_PARENT/$1" GOTMPDIR="$TEMP_PARENT/$1" target/release/mutie "$PROJECT" \
    --rerun "$SEED" --id "$IDS" --test-scope "$2" --jobs 2 \
    --timeout 30 --baseline-timeout 120 --max-log-bytes 65536 \
    --ui plain --output-dir "$REPORTS/$1"
}
run_sample full-1 full       # Excluded calibration
run_sample affected-1 affected
run_sample full-2 full
run_sample full-3 full
run_sample affected-2 affected
```

For the test improvement, copy the two supplied test files to
`src/mutie_bell_test.go` and `src/tui/mutie_output_test.go` in a separate source
copy, set `IMPROVED` to it, and `FULL_RUN` to the calibration's report directory.
Run this once per scope with separate output and temporary directories:

```sh
mkdir -p "$TEMP_PARENT/improved-$SCOPE"
TMPDIR="$TEMP_PARENT/improved-$SCOPE" GOTMPDIR="$TEMP_PARENT/improved-$SCOPE" \
  target/release/mutie "$IMPROVED" --rerun "$FULL_RUN" --id 9,25 \
  --test-scope "$SCOPE" --jobs 2 --timeout 30 --baseline-timeout 120 \
  --max-log-bytes 65536 --ui plain --output-dir "$REPORTS/improved-$SCOPE"
```

The measurement driver and disk samples remain with raw reports under
`/tmp/mutie-fzf-igp8pib1`, outside version control. All seven completed
reports passed Mutie's report reader, checkpoint/outcome checks, and retained-log
byte-count audits. All 32 unique saved diffs were applied to disposable files and
matched the recorded byte replacements; corresponding diffs matched across trials.
Original and improved copies were checked against the upstream SHA-256 manifest.

## Limits and next validation

Hugo v0.134.3 was tried first because its source tree was larger: 13.38 MB and 833
Go files. Its full Go preflight failed with the installed toolchain's `go vet`
printf-analyzer panics; some tests also required unavailable loopback/network
services. No checks were disabled to force a passing baseline, and Hugo was not
used for mutation performance measurements. Raw preflight evidence remains under
`/tmp/mutie-hugo-x1nkr4tj/artifacts`.

The fzf results extend validation to an application and identify useful Go-test
gaps. They do not settle performance on large applications or asset-heavy trees,
ordinary-copy filesystems, Linux/tmpfs, cold caches, or the complete inventory.
Keep those measurements open, along with larger samples for operator-noise review.
