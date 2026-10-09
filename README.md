# mutie

A CLI for performing mutation testing.

The first prototype supports Go modules. It runs a passing baseline, finds small
source mutations with Tree-sitter, and runs the full test suite for each mutation
in an isolated temporary workspace. Rust and other languages are planned.

See [DESIGN.md](DESIGN.md) for the MVP scope, architecture, and roadmap, and
[TODO.md](TODO.md) for implementation progress. Features described as planned are
not necessarily available in the current CLI.

## Try it

Requires Rust/Cargo and Go 1.24+ on `PATH`. The prototype is tested on Linux and macOS.

```sh
cargo run -- /path/to/go/module
cargo run -- /path/to/go/module --jobs 4
cargo run -- /path/to/go/module --list
cargo run -- /path/to/go/module --include 'internal/**/*.go' --exclude '**/generated_*'
cargo run -- /path/to/go/module --timeout 30 --baseline-timeout 300 --verbose
cargo run -- /path/to/go/module --output-dir ./mutation-reports
cargo run -- /path/to/go/module --ui plain
cargo run -- /path/to/go/module --rerun-missed /path/to/saved/run
cargo run -- /path/to/go/module --rerun /path/to/saved/run --id 9,11
cargo run -- report /path/to/saved/run
cargo run -- dismiss --module /path/to/go/module list
```

The path defaults to the current directory and must contain `go.mod`. Glob
patterns are relative to that directory; quote them so your shell does not expand
them. Repeat `--include` or `--exclude` to supply multiple patterns. Exclusions
win. These options control which files are mutated; tests still run for the full
module. `--list` discovers mutations without running tests.

The bundled fixture deliberately produces all four outcomes:

```sh
cargo run -- tests/fixtures/go --jobs 2 --timeout 5
```

## Repository configuration

Mutie reads an optional `mutie.toml` beside the selected module's `go.mod`.
Settings apply in this order: built-in defaults, then the file, then explicitly
supplied CLI arguments. Parent directories are not searched.

```toml
lang = "go"
jobs = 2
include = ["**/*.go"]
exclude = ["internal/legacy/**"]
timeout = 30
baseline_timeout = 300
output_dir = ".mutie"
verbose = false
ui = "auto"
```

All keys are optional. Defaults are `lang = "auto"` (currently Go), one worker,
empty include/exclude lists, a 300-second baseline/discovery timeout, `.mutie`
under the module root for reports, `ui = "auto"`, and no verbose output. Omit
`timeout` for the automatic policy: `max(10 seconds, 5 * baseline duration)`. Worker counts and
timeouts must be positive integers; timeouts are in seconds. Unknown keys,
malformed TOML, and incorrect value types are rejected even if CLI options would
override them. The merged settings are validated before creating reports or
running Go, including globs, supported language, and output paths.

Globs are always module-relative. A configured `output_dir` is relative to the
module root; a CLI `--output-dir` is relative to the invocation directory.
Absolute paths are also accepted. Existing symlinks are resolved, and `.`/`..`
components are normalized before use. Paths are not shell-expanded.

Each CLI list replaces its configured counterpart. To clear a configured list,
use `--clear-include` (all eligible files) or `--clear-exclude` (no additional
exclusions). `--clear-timeout` restores the automatic timeout; `--no-verbose`
overrides `verbose = true`. Each reset flag conflicts with the corresponding
value flag. For example:

```sh
cargo run -- /path/to/go/module --include 'internal/payments/**' --jobs 1
cargo run -- /path/to/go/module --clear-include --clear-exclude --list
cargo run -- /path/to/go/module --clear-timeout --no-verbose
```

`--list`, `--rerun`, `--rerun-missed`, `--id`, and `--operator` are invocation-only;
they cannot be set in `mutie.toml`. Listing uses the same
configuration and excludes the configured report directory from its snapshot,
but creates no report directory or artifacts. An explicit `--output-dir` still
conflicts with `--list`.

## Live terminal status

Interactive terminals get a live, inline Ratatui display inspired by the Vitest
CLI. The status region updates in place up to ten times per second, including
elapsed time and activity while commands run. It shows the current phase,
completed/total mutations, outcome counts, stable numbered worker rows, and a
results tail below the workers. For example, during mutation execution:

```text
 | mutie mutations  2.0s
 | worker 1 building  #000004 (2.0s) calc.go:8:3: + -> -
 | worker 2 testing   #000005 (2.0s) ready.go:4:2: true -> false

   killed       add.go:3:2: + -> - (0.2s)

 Mutations 3/8  |  Workers 2/2
 2 killed | 1 missed | 0 timeout | 0 build failed | 0 errors
 Ctrl-C to stop  |  Test output saved in report logs
```

Set `ui` in `mutie.toml` or override it with `--ui auto|interactive|plain`:

- `auto` (default): use the live view when stdin and stdout are terminals,
  `TERM` is not `dumb`, and the `CI` environment variable is absent. Fall back to
  plain output if terminal initialization fails.
- `interactive`: request the live view, including in CI. Requires terminal stdin
  and stdout and a `TERM` other than `dumb`; unsupported terminals produce an error.
- `plain`: print progress lines without terminal control sequences.

Worker rows stay in numeric order, including `idle` workers. Busy rows show the
mutation and elapsed time since assignment, including workspace preparation.
`preparing` covers copying/resetting the workspace and applying the mutation.
`running` is the fallback while the command's activity is unknown. Live
[Go build events](https://pkg.go.dev/cmd/go#hdr-Build__json_encoding) can show
`building`; [package test events](https://pkg.go.dev/cmd/test2json#hdr-Output_Format)
show `testing` while an observed package is active. Quiet builds can remain
`running`, and testing takes precedence when build and test events overlap.

`--list` always uses plain output. The live view adapts to resizing and small
terminals, showing as many fixed worker rows as fit and up to three recent results,
with newest completions at the bottom. An overflow row counts hidden workers and
how many are active. Space for the results tail is reserved before results arrive
so completions do not move the worker rows. Narrow terminals use shorter phase
labels (`prep`, `run`, `build`, `test`), worker prefixes, and outcome totals
(`kill`, `miss`, `time`, `build`, `err`; initials at smaller widths).
All results remain in the saved report. The UI uses the normal terminal screen
and allocates at most its height. Earlier output and the shell prompt remain on
screen or in terminal scrollback as space is needed. Resizing and exit cleanup
clear only the live status area. Input stays in normal mode between
cursor-position queries. Ctrl-C uses the same cancellation and cleanup
path as plain mode; SIGTERM also works on Unix. On completion or interruption,
the status area is cleared, the cursor restored, and a final plain summary and
report path are printed. Interrupted summaries include pending/interrupted counts.

Test output stays in saved logs while the live view is active, including with
`--verbose`. Use `--ui plain --verbose` to print captured test output during a run.
Use `mutie report RUN_DIR` to filter and inspect a finalized run.

The MVP has been validated on `godotenv` v1.5.1 as well as the bundled fixture.
Plain and live runs produced identical results for 77 mutations, and added tests
in an isolated copy caught four previously missed mutations. See the
[validation notes](DESIGN.md#mvp-validation-evidence) for runtime, artifact sizes,
terminal checks, and the limits of this small-project validation.

A larger run on `go-cmp` v0.7.0 completed all 1,450 mutations across ten packages.
Workspace operations used 22.3 cumulative seconds versus 7,059.6 seconds in mutation
commands, with all file transfers reflinked on APFS. See the
[larger-project measurements](benchmarks/go-cmp-v0.7.0.md) for the method, outcome
counts, report size, cleanup findings, and limits of these observations.

## Browsing a saved run

Open a finalized run in the full-screen Ratatui browser:

```sh
cargo run -- report /path/to/saved/run
cargo run -- report /path/to/saved/run --module /path/to/moved/checkout
```

Browsing needs an interactive terminal, but no Go installation, original checkout,
or `mutie.toml`. It reads the saved report without changing it. Completed, failed,
and interrupted runs are supported, including baseline failures without a mutation
inventory. Running or inconsistent reports are rejected before terminal setup.
Use `summary.txt` and the saved artifacts directly for non-interactive inspection.

The mutation list and detail pane appear side by side in wide terminals and stack
in narrower ones. The browser uses an alternate screen, restoring the prior shell
screen and input mode on exit, Ctrl-C, or SIGTERM on Unix. The live run view remains
inline. Terminals smaller than 24 columns by 12 rows show a resize hint.

| Key | Action |
| --- | --- |
| Up/Down or `j`/`k` | Move through mutations or scroll the focused detail pane |
| Tab | Switch list/detail focus |
| Page Up/Down, Home/End | Page or jump to the first/last row or detail line |
| Left/Right | Scroll the detail pane horizontally |
| `o`, `p` | Cycle outcome/state and operator filters |
| `s` | Cycle all / undismissed / dismissed review filters |
| `/`, Enter, Esc | Edit, apply, or cancel a case-insensitive file substring filter |
| `x` | Clear all filters |
| Space | Mark/unmark the current mutation |
| `a`, `c` | Mark/unmark all visible mutations; clear all marks |
| `d`, `l` | Inspect the selected mutation's saved diff or log |
| `b`, `g` | Inspect baseline or package-discovery logs |
| `v` | Inspect the run's saved dismissal review, including unresolved entries |
| `r` | Show a focused rerun command |
| `e` | Close the browser and print the rerun command to the shell |
| `?`, `q`/Esc | Show help; close the browser |

Filters intersect. Marks persist across filters, and the header counts hidden
marks. Rerun commands use **all marked saved IDs**, including hidden ones, or the
current row when nothing is marked. They use `--rerun`, so pending/interrupted or
previously killed mutations can also be selected. Commands are quoted for POSIX
shells and are never executed by the browser. An actual rerun still checks source
identity and requires a fresh baseline. Use `--module PATH` to change the module
path in the generated command when the checkout has moved.
Marks last only for the current browser session. Use [persistent dismissals](#persistent-dismissals)
to record review decisions across sessions.

The detail pane shows source location, operator, outcome/state, duration, and
available diagnostics. Dismissed rows have a `[D]` badge and show their saved reason
and note in the detail pane. The review filter intersects with outcome, operator,
and file filters; dismissed rows can still be marked and rerun, including when
hidden by a filter. The header's missed count always includes dismissed results.
`v` shows the undismissed/dismissed missed counts and any stale, missing, unmatched,
or ambiguous entries that were not applied.

Review data reflects the run's source snapshot. The browser reads neither current
repository metadata nor source files, including with `--module` (which only changes
generated command paths). Later dismissal edits do not rewrite historical reports.
Older reports and runs that stopped before inventory publication show review as
**not recorded**; all their rows remain available under all/undismissed, with no
rows under dismissed. Missing diff/log files appear as an inline message;
other rows remain usable. Artifact previews read at most 256 KiB, replace terminal
control characters, and flag truncation. The complete logs remain in the saved
files. Artifact paths must resolve inside the report directory. JSON metadata is
limited to 64 MiB per file. Module directories named `report` or `dismiss` can
still be run as `mutie ./report` or `mutie ./dismiss`.

## Persistent dismissals

Record a review decision for a mutation from a finalized saved run:

```sh
cargo run -- dismiss --module /path/to/go/module add /path/to/saved/run \
  --id 9 --reason 'accepted test gap' --note 'Boundary is validated by the caller.'
cargo run -- dismiss --module /path/to/go/module list
cargo run -- dismiss --module /path/to/go/module check
cargo run -- dismiss --module /path/to/go/module update 1 --reason 'equivalent behavior'
cargo run -- dismiss --module /path/to/go/module update 1 --clear-note
cargo run -- dismiss --module /path/to/go/module remove 1
```

The module defaults to the current directory and must contain `go.mod`.
`add --id` selects a **saved-run mutation ID**; `update` and `remove` use the
independent **dismissal ID** printed by `add` and `list`. Dismissal IDs are not
reused after removal. Reasons are required nonblank text; notes are optional.
`update --note TEXT` replaces a note, and `--clear-note` removes it. Updating a
reason or note preserves the stored mutation identity.

Decisions live in `mutie.dismissals.toml` beside `go.mod`, separate from generated
reports and `mutie.toml`. Commit this file to version control. It stores the
reason, note, module-relative source path, SHA-256 source fingerprint, operator,
byte range, source coordinates, and original/replacement text. No absolute
checkout path or run-local ID is used for matching, so the metadata can move
with the repository. CLI edits validate the entire file and replace it atomically;
they reformat TOML and do not preserve hand-written comments. Malformed metadata,
unknown fields, incompatible versions, duplicate IDs, and duplicate additions
are rejected without overwriting the file.

`add`, `list`, `update`, and `remove` need neither Go nor a valid `mutie.toml`.
Adding records the saved identity without checking current source; listing labels
it unchecked. `check` uses current Go package discovery and the include/exclude
patterns and discovery timeout from `mutie.toml`. It runs in disposable workspaces,
does not run tests, and creates no reports. With no dismissals, it skips discovery.

Matching is strict, using the same source identity as focused reruns. Any edit to
the mutation's source file invalidates its fingerprint, even a comment or an
unrelated edit in that file. Test-only edits do not invalidate it. `check` reports
matched, stale, missing (removed/excluded/inactive files or files without mutations),
unmatched identity, or ambiguous entries. Duplicate identities introduced by a
manual edit or merge are flagged as ambiguous when they match a current mutation.
It exits nonzero when anything needs review and leaves metadata unchanged. Review
such entries with `list`, remove obsolete decisions, and re-add from a fresh report
when appropriate. It never silently attaches a decision to a different mutation.

Dismissals record human judgment and **do not change execution outcomes or counts**.
Normal runs and focused reruns still execute dismissed mutations; a missed result
remains missed. Each new run captures review metadata from its pristine project
snapshot and matches it against discovery before applying rerun selection. Matching
decisions for selected mutations, with the new run's IDs, and unresolved entries
are stored in `mutants.json`. Valid decisions outside a rerun selection are omitted,
without being flagged as missing. Review issues do not fail execution or suppress
mutations. Invalid metadata fails a run before invoking Go; `--list` remains a
source-only operation and does not load dismissal metadata.

`summary.txt` separates dismissed and undismissed missed counts and includes reasons,
notes, and unresolved entries. Execution counts in `outcomes.json` and all outcome
lists remain unchanged. To see a newly added or edited decision in the browser,
run Mutie again (a focused rerun is fine) and open the new report:

```sh
cargo run -- /path/to/go/module --rerun /path/to/saved/run --id 9
cargo run -- report /path/to/new/run
```

Press `s` to filter by review status or `v` to inspect the saved dismissal review.

Concurrent CLI edits are guarded by `.mutie.dismissals.lock`, normally removed on
exit. If a process is forcibly stopped and leaves this file behind, confirm that
no dismissal command is running before removing the lock and retrying.

## Focused reruns

After improving tests, use a saved run directory to select just its completed
`missed` mutations:

```sh
cargo run -- /path/to/go/module --rerun-missed /path/to/saved/run
cargo run -- /path/to/go/module --rerun-missed /path/to/saved/run --list
```

Use `--rerun` to select mutations regardless of their previous outcome, and
`--id` or `--operator` to narrow either rerun mode:

```sh
cargo run -- /path/to/go/module --rerun /path/to/saved/run --id 9,11
cargo run -- /path/to/go/module --rerun /path/to/saved/run --operator arithmetic_replace,comparison_boundary --list
cargo run -- /path/to/go/module --rerun-missed /path/to/saved/run --id 9,11 --operator arithmetic_replace
```

IDs refer to the **source run's** inventory, not a fresh scan or an earlier run
in a chain of reruns. They must be positive integers. Both filters accept repeated
flags or comma-separated values; duplicate values select work only once.
Multiple IDs or operators select any listed value within that filter. Combining
IDs, operators, and missed-only selection takes their intersection. For example,
`--rerun-missed RUN_DIR --id 9,11 --operator arithmetic_replace` selects only IDs
9 or 11 that were missed arithmetic mutations. A valid ID excluded by another
filter selects nothing; an unknown ID is an error even if other filters would
exclude it. Operator names match the [saved operator catalog](#saved-reports).

`--rerun` and `--rerun-missed` conflict. `--id` and `--operator` require one of
them; operator filtering for a fresh scan is not implemented. With no filters,
`--rerun` selects the entire saved inventory, including previously killed,
timed-out, build-failed, error, pending, or interrupted mutations.

`RUN_DIR` is relative to the invocation directory, or absolute. It must contain
`run.json`, `mutants.json`, and `outcomes.json`; text lists, diffs, and logs are not
selection inputs. Finalized failed or interrupted runs also work when their
inventory is complete. `--rerun-missed` selects only completed missed outcomes;
`--rerun` can retry unfinished work. Running, incomplete, inconsistent, or
incompatible reports and invalid selectors are rejected before running Go or
creating a new report.

Execution always runs a fresh baseline, rediscovers mutations from the current
snapshot, and validates every selected target before starting workers. Matching
requires the same module-relative source path, complete source-file fingerprint,
operator, original/replacement text, and source coordinates. Editing or adding
tests is supported; any change to a selected mutation's source file, including
comments or moved lines, requires a new full scan. Missing, excluded, inactive,
or ambiguous targets fail the entire selection rather than silently being skipped.
Current include/exclude settings still apply; clear configured filters if needed.

The current module can be a moved or copied checkout. Current configuration,
tests, worker count, and timeouts apply; old outcomes are never reused. Newly
discovered mutations are not added to the saved selection. If the filters match
nothing, execution saves a completed empty run after baseline and discovery.
`--list` previews and validates the selection without running tests or writing a
report. Each preview row shows its new ID, source ID, operator, and source change;
an empty preview prints an explicit message.

Every execution creates a separate report. Once its inventory is published,
`run.json` contains a `rerun` object with `source_run`, `source_checkpoint`,
`source_status`, `selection` (`"all"` or `"missed"`), optional `source_ids` and
`operators` filters, and `mutations` mapping each new `id` to its `source_id`.
Filters are deduplicated and sorted; new IDs follow current discovery order.
`summary.txt` includes the filters and mapping too. `effective_config.rerun`
records the source path, selection mode, and filters even if the baseline or
matching fails. This replaces the earlier `effective_config.rerun_missed` path;
previously saved reports remain usable. The source report is preserved, excluded
from the snapshot, and cannot contain the new output directory.

## Saved reports

By default, every test run saves a new directory under `<module>/.mutie/`. The CLI
prints its location when the run starts and the summary path when it finishes. Earlier runs
are preserved. Use `--output-dir DIR` to choose the parent directory instead;
relative paths are resolved from your current working directory. The output
directory cannot be the module root or an ancestor of it. `--list` writes no
reports and cannot be combined with `--output-dir`.

Each run contains:

- `run.json`: versioned run metadata, effective configuration, command, scope,
  timeouts, worker counts, workspace measurements, phase, status, and any run-level error.
- `mutants.json`: the complete mutation inventory, including IDs, named operators,
  source locations, original text, replacements, source fingerprints, and diff
  paths. The inventory and every diff are published before workers start.
- `outcomes.json`: baseline and package-discovery results, mutation execution
  states and outcomes, durations, exit codes, log paths, and summary counts.
- `summary.txt`: readable counts, workspace costs, unfinished work, missed mutations,
  and artifact references for every planned mutation.
- `diff/000001.diff`: one unified source diff per planned mutation.
- `logs/baseline.log`, `logs/discovery.log`, and `logs/000001.log`: readable command
  output, with separate `.stdout.jsonl` and `.stderr.log` captures alongside each
  log. Discovery stdout contains Go package JSON; test stdout contains Go test
  events. Logs are saved even without `--verbose`.
- `killed.txt`, `missed.txt`, `timeout.txt`, `build_failed.txt`, and `errors.txt`:
  outcome lists containing IDs and source descriptions, sorted by ID.

`run.json` includes `effective_config` with all merged settings, the resolved Go
language, absolute module/output paths, and the config file path (`null` when
absent). Its `timeout` is `null` when using the automatic policy; the actual
deadline, once determined, is in `mutation_timeout_seconds`. Configured `jobs`
and `requested_workers` describe the requested concurrency; `workers` records the
number used, capped by the mutation count. Configuration timeout values use
seconds, matching the TOML file.

Mutation IDs are local to each run, assigned in deterministic discovery order.
Inventory and execution records are sorted by ID regardless of completion order;
join them on `id`. Source paths are module-relative and artifact paths are
report-relative. Lines and byte columns are one-based; `end_line`/`end_column` and
zero-based `byte_end` are exclusive. Durations are seconds and timestamps are
Unix seconds. `source_fingerprint` is `sha256:` followed by the lowercase SHA-256
hex digest of the complete original source file. It identifies source content,
not test content or a reusable result. IDs and offsets must not be replayed across
source edits without rediscovery and validation.

After inventory publication, `mutants.json` also contains `dismissals` with
`matched` and `needs_review` arrays. A matched entry pairs `mutation_id` with a
`dismissal` containing its independent `id`, `reason`, optional `note`, and full
`mutation` identity. An unresolved entry has the same dismissal record and a
`status` of `stale`, `missing`, `unmatched`, or `ambiguous`. Readers validate IDs
and matched identities before browsing or rerunning. This data is optional for
older reports; absence means review was not recorded. An empty captured review
explicitly records that no dismissal decisions applied. It remains usable when
the checkout or repository metadata is unavailable.

`schema_version` is `1` in all three JSON files, and `operator_catalog_version` is
`1` in run metadata and the inventory. Named operators are `arithmetic_replace`,
`comparison_invert`, `comparison_boundary`, `logical_swap`, `boolean_flip`, and
`negation_remove`.

Mutation execution `state` is `pending`, `running`, `completed`, or `interrupted`.
Only completed executions have an `outcome`; the other states have `null` outcomes
and do not contribute to outcome counts. Worker numbers are one-based and remain
`null` until scheduling. Diagnostics, exit codes, and durations can be `null` if
no command ran. The inventory starts with `complete: false`; a failed baseline or
interruption before inventory publication leaves it incomplete. A successful
empty discovery publishes `complete: true` with no mutations.

Run status is `running`, `completed`, `failed`, or `interrupted`. Ctrl-C (and
SIGTERM on Unix) stops scheduling, terminates active child process groups on Unix,
collects their output and completed results, finalizes the report, and cleans up
workspaces. Active mutations become interrupted; mutations never scheduled remain
pending. Interrupted runs exit unsuccessfully. Tool failures are recorded as
`error` and fail the run while also collecting active workers' diagnostics.
Baseline and discovery interruptions retain their own logs.

JSON files are replaced atomically at each checkpoint. `run.json` and
`outcomes.json` carry matching `checkpoint` numbers after a complete update;
readers can retry if these differ during a write. Files are individually atomic,
not a multi-file transaction. A forced kill can leave an incomplete checkpoint,
`running` states, and missing active output. The inventory still identifies all
planned work once published. Structured inventory and outcomes are authoritative;
text summaries and outcome lists are derived and can lag after a forced kill.
Report write failures are surfaced as errors.

The module's `.mutie` directory, the selected output directory, and the source
rerun directory are omitted from snapshots so saved reports do not affect tests
or discovery. Keep output directories separate from source files, and add `.mutie/`
(or your custom output directory) to your project's `.gitignore`.

### Workspace measurements

The final CLI summary and `summary.txt` show cumulative snapshot, setup, and reset
times, plus counts of reflinked and copied files. Setup combines baseline and worker
creation; reset combines the reset before discovery and resets between mutations.
These times can overlap across workers, so they are not the run's wall-clock time.

`run.json.workspace_metrics` keeps the breakdown under `snapshot`, `baseline_setup`,
`discovery_reset`, `worker_setup`, and `worker_reset`. Each contains:

- `operations`, `completed`, `failed`, and `interrupted`: filesystem operation counts.
- `duration_seconds`: cumulative elapsed time, including failed/interrupted operations.
- `reflink_files`: successful copy-on-write clones reported by the copy library.
- `copied_files` and `copied_bytes`: successful fallback copies and their logical byte
  counts, including empty files. These are not measurements of physical disk traffic
  or space saved; the OS may optimize fallback copies too.

Every reset counts its transfers again. Directories, symlinks, excluded files, and
failed file transfers are not counted. Successful transfers before a later failure
or interruption are retained. Timings cover directory creation, copying, permission
preservation, and reset deletion. They exclude the blocking-pool queue, applying
mutations, Go commands, and final workspace cleanup. Workspace completion describes
the filesystem operation even when the subsequent Go command fails or is interrupted.
Command durations remain in `outcomes.json`.

Measurements are saved at normal report checkpoints, with worker measurements
collected when the worker job returns. They do not add report writes per copied file
or operation. `--list` and `dismiss check` do not produce these reports or summaries.

The canonical temporary parent directory is recorded in
`workspace_metrics.temporary_root` and `summary.txt`. On Unix, set `TMPDIR` to an
existing, writable directory outside the source tree to choose workspace placement:

```sh
mkdir -p /path/on/project-filesystem/mutie-tmp
TMPDIR=/path/on/project-filesystem/mutie-tmp cargo run -- /path/to/go/module --jobs 2
```

For copy-on-write comparisons, use a directory on the project's reflink-capable
filesystem and inspect the recorded counts. A different filesystem or a tmpfs can
prevent reflinks; support also depends on the OS and filesystem. Keep project inputs,
worker counts, timeouts, and Go cache conditions consistent when comparing runs.
The [go-cmp measurements](benchmarks/go-cmp-v0.7.0.md) exercise a larger mutation
inventory on APFS. Comparisons against Linux tmpfs, ordinary copies, and projects
with large source trees or assets are still pending.

## Behavior

- The baseline and each mutation run `go test -json -count=1 -timeout=0 ./...`.
  Test-result caching is disabled; Go's build cache is still used. Mutie enforces
  a deadline for the entire command, including compilation. See the
  [Go test documentation](https://pkg.go.dev/cmd/go#hdr-Test_packages).
- A failed baseline stops the run. The default baseline deadline is 300 seconds.
  Each mutation gets five times the baseline duration, with a minimum of 10
  seconds, unless `--timeout` is supplied.
- `go list` selects source files active for the current build. Tests, generated
  files with Go's standard generated-code marker, vendored packages, and nested
  modules are not mutated. Build tags and platform selection follow the Go
  environment. The initial operators flip comparisons and their boundaries,
  swap arithmetic and logical operators, flip booleans, and remove logical `!`.
- Results are **killed** (tests failed), **missed** (tests passed), **timeout**, or
  **build failed** (the mutation could not compile or failed Go's build checks).
  Tool/setup failures abort the run instead of counting as killed mutations.
  `--verbose` shows captured test and build output in plain mode.
- There is one worker by default. Each worker restores its entire workspace from
  a pristine snapshot between mutations, including files changed or created by
  tests. Copies use [reflink-copy](https://docs.rs/reflink-copy/latest/reflink_copy/)
  where supported, with ordinary copies as a fallback. This costs extra copying
  compared with reverting just the mutated file, but avoids test-state leakage.
- The snapshot includes current local edits and untracked files; `.git` and
  report directories are omitted. Input source files are not edited. Internal symlinks are kept within the
  copied tree; dangling or external symlinks are rejected. Temporary workspaces
  are removed on completion. On Unix, timeouts and interruption kill the child
  process group, including test binaries. Cancellation is also checked during
  workspace copying and between discovery/publication steps.
- A completed run exits successfully even if mutations survive. Baseline and
  tool failures exit unsuccessfully. CI thresholds are future work; saved JSON
  reports can already be consumed by other tools.

The initial scope is one self-contained Go module. `GOWORK` is set to `off` for
child Go commands. Workspaces, local replacements outside the module, and tests
requiring Git metadata are not supported yet. Copies isolate repository files,
but tests still have normal access to external services, caches, and filesystem
paths. Windows descendant-process cleanup is not implemented yet.

## Development

```sh
cargo test
cargo test --test go_workflow -- --ignored
cargo clippy --all-targets -- -D warnings
```

The Go integration test checks all four outcomes, repository configuration and
scope, baseline failure and timeout, workspace resets, saved reports, and
preservation of input files. It needs Go on `PATH` and a writable Go build cache,
and is opt-in because it runs real builds and deliberately waits for a timeout.
Non-ignored Unix tests use a fake Go command to verify SIGINT/SIGTERM during baseline, discovery, and
parallel execution, preserved diagnostics, pending work, process cleanup, and
source preservation. They require an environment that delivers those signals.
Configuration tests also cover defaults, explicit overrides, list clearing,
invalid settings, path resolution, listing without reports, and saved effective
settings using a fake Go command.
Rerun tests cover saved-report validation, stale/missing/ambiguous targets, fresh
baselines, ID/operator/outcome intersections, invalid selectors, selection
previews, new ID mappings, empty selections, source report preservation, and
improved tests turning a previous missed result into killed. The opt-in real-Go
test also selects a killed and a dismissed missed mutation by ID and operator,
checks the dismissal against real discovery, and verifies that a test-only change
preserves its identity while the new tests kill the previously missed mutation.
Dismissal tests cover persistence, independent IDs, reason/note edits, removal,
strict rediscovery, moved checkouts, stale/missing/ambiguous entries, malformed
metadata, concurrent-writer exclusion, and preservation of reports, source files,
execution outcomes, counts, and rerun selection. Report tests cover frozen review
data, rerun ID remapping, omitted valid decisions, invalid snapshot rejection,
and compatibility with older reports. Browser snapshots and PTY tests cover review
filters, badges, reasons/notes, hidden marks, export, and checkout-free viewing.
Browser tests cover saved IDs, intersecting filters, marks hidden by filters,
quoted command export, missing/bounded artifacts, Unicode input, and frame
snapshots at wide and compact sizes. Unix PTY tests open reports with no Go or
checkout, exercise paste and resize, and verify shell/termios restoration after
quit, export, Ctrl-C, and SIGTERM without changing report files.
Renderer tests check complete frames, styles, clipping, elapsed-time updates, and
resizing, stable idle/busy rows, and results ordering. Progress tests cover partial
output lines, interleaved packages, stale events after worker reuse, full capture
preservation, and cancellation/deadlines during blocked progress observation.
Unix pseudo-terminal tests exercise live phases before command completion and
redraws during a blocked baseline, Ctrl-C, resizing, terminal initialization
failure, empty/failed runs, terminal restoration, and plain output in CI. A
terminal screen/scrollback model checks that pre-existing output and prompts
survive completion, cancellation, width/height changes, and oversized worker
views. A regression deliberately delivers frames prepared at the old size after
a resize, verifying history preservation and normal line wrapping afterward.
Temporary workspace paths are
canonicalized so macOS path aliases agree with Go discovery and internal symlinks.

## Original plan

The rough plan:

- Start by running the tests and capturing the output
- Use `tree-sitter` to find possible mutation locations
- Use `reflink_copy` to make `N` clones of the repo (for `N` workers)
- In each worker:
  - Pull a mutation from the list
  - Make the change
  - Run the tests
  - Compare results (report timeout/killed/missed)
  - Report the results
  - Revert the change and pull new work

Some refs:

- <https://docs.rs/reflink-copy/latest/reflink_copy>
- <https://github.com/tree-sitter/tree-sitter>
- <https://github.com/tree-sitter/tree-sitter-go>

I *think* for now, I may want to start with Go but we can move on to
supporting other languages in the future.
