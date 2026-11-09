# Reports and review

[Documentation index](README.md) · [Project overview](../README.md)

## Live terminal status

Interactive terminals show a full-window Ratatui display on the alternate screen.
Overall status stays at the top, numbered worker rows stay in place below it,
and completed mutation results fill the remaining space. For example:

```text
 | mutie mutations  2.0s
 Mutations 3/8  |  Workers 2/2
 2 killed | 1 missed | 0 timeout | 0 build failed | 0 errors
 | worker 1 building  #000004 (2.0s) calc.go:8:3: + -> -
 | worker 2 testing   #000005 (2.0s) ready.go:4:2: true -> false
 Results · newest at bottom
   killed       #000001 (0.2s) add.go:3:2: + -> -
   missed       #000002 (0.3s) ready.go:7:2: true -> false
   killed       #000003 (0.2s) calc.go:5:3: < -> <=

 Ctrl-C to stop  |  Test output saved in report logs
```

The view refreshes up to ten times per second, including timers while commands
run. Each result represents a mutation's test run, rather than an individual test
case. It includes the outcome, report-local ID, command duration and source change.
Worker timers include workspace preparation; result durations cover the command.

Output is selected by CLI flags:

- No flag: use the live view when stdin and stdout are terminals, `TERM` is not
  `dumb`, and `CI` is absent. Fall back to plain text if initialization fails.
- `--plain`: print progress lines without starting the live display.
- `--json`: stream JSON events with a final summary, including in terminals.

`--plain` and `--json` are mutually exclusive. Presentation is not configured in
`mutie.toml`. See [output modes](output.md) for the JSON contract.

Worker rows stay in numeric order, including `idle` workers. Busy rows show the
mutation and elapsed time since assignment, including workspace preparation.
`preparing` covers copying/resetting the workspace and applying the mutation.
`running` is the fallback while the command's activity is unknown. Live
[Go build events](https://pkg.go.dev/cmd/go#hdr-Build__json_encoding) can show
`building`; [package test events](https://pkg.go.dev/cmd/test2json#hdr-Output_Format)
show `testing` while an observed package is active. Quiet builds can remain
`running`, and testing takes precedence when build and test events overlap.

Worker rows are allocated from the configured job count, including before
mutation discovery. Very brief phases may pass between redraws. `mutie list`
uses plain text or `--json`, without opening the live view.

The log follows completions automatically, with the newest entry last. It shows
as many rows as fit and retains the latest 256 entries for resizing. All results
remain in the saved report. Extra workers collapse into a count, including how
many are active. Narrow terminals shorten labels; very short terminals prioritize
overall status. There is no keyboard scrolling in the live view.

The alternate screen preserves prior shell output and scrollback. Input stays in
normal mode, without cursor-position queries. Ctrl-C uses the same cancellation
and worker cleanup as plain mode; SIGTERM also works on Unix. Completion,
interruption and errors restore the original screen and cursor before printing
the final summary and report path. Interrupted summaries include pending and
interrupted counts.

Test output stays in saved logs while the live view is active, including with
`--verbose`. Use `--plain --verbose` to print captured test output during a run.
Use `mutie report RUN_DIR` to filter and inspect a finalized run.

The MVP has been validated on `godotenv` v1.5.1 as well as the bundled fixture.
Plain and live runs produced identical results for 77 mutations, and added tests
in an isolated copy caught four previously missed mutations. See the
[validation notes](../DESIGN.md#mvp-validation-evidence) for runtime, artifact sizes,
terminal checks, and the limits of this small-project validation.

A larger run on `go-cmp` v0.7.0 completed all 1,450 mutations across ten packages.
Workspace operations used 22.3 cumulative seconds versus 7,059.6 seconds in mutation
commands, with all file transfers reflinked on APFS. See the
[larger-project measurements](../benchmarks/go-cmp-v0.7.0.md) for the method, outcome
counts, report size, cleanup findings, and limits of these observations.

An application sample on `fzf` v0.65.2 matched full/affected outcomes for 32
mutations across all seven operators. Added tests killed two call-removal survivors
in both scopes. The [application measurements](../benchmarks/fzf-v0.65.2.md) cover
runtime, disk use, and test-created temporary files left outside managed workspaces.
The [cleanup follow-up](../benchmarks/fzf-v0.65.2.md#test-temporary-directory-cleanup-follow-up)
repeated both scopes with private test temporary directories: zero leftovers and
unchanged outcomes.
This covered fzf's Go tests; its separate Ruby/tmux suite and large-tree scaling
remain outside the validation.

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
| `m` | Inspect the saved mutation score, selection scope, and minimum-score decision |
| `t` | Inspect per-operator counts, unfinished work, and cumulative command time |
| `r` | Show a focused rerun command |
| `e` | Close the browser and print the rerun command to the shell |
| `?`, `q`/Esc | Show help; close the browser |

Filters intersect. Marks persist across filters, and the header counts hidden
marks. Rerun commands use **all marked saved IDs**, including hidden ones, or the
current row when nothing is marked. They use `mutie rerun`, so pending/interrupted or
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

`t` shows the operator breakdown for the entire saved run selection, independent
of browser filters, marks, or dismissals. Each operator has planned and completed
counts, all five outcomes, pending/running/interrupted counts, and recorded command
time. Time is cumulative across workers, includes interrupted commands with saved
durations, and excludes baseline, discovery, and workspace work; it can exceed the
run's wall time. Recorded and missing timing counts accompany each total. Missing
or invalid timings remain unknown; pending mutations do not count as missing.
Older reports use the same view, derived from their saved inventory and outcomes.
A run without a published inventory shows the breakdown as unavailable.

Review data reflects the run's source snapshot. The browser reads neither current
repository metadata nor source files, including with `--module` (which only changes
generated command paths). Later dismissal edits do not rewrite historical reports.
Older reports and runs that stopped before inventory publication show review as
**not recorded**; all their rows remain available under all/undismissed, with no
rows under dismissed. Missing diff/log files appear as an inline message;
other rows remain usable. Artifact previews read at most 256 KiB, replace terminal
control characters, and flag preview truncation. Full saved artifacts remain on
disk; if log retention omitted output, the browser separately shows byte counts
and explains that the omitted output is absent from the report. These notices
remain visible even when a log file is missing. Artifact paths must resolve inside
the report directory. JSON metadata is
limited to 64 MiB per file. Module directories named `report` or `dismiss` can
still be run as `mutie --path ./report run` or `mutie --path ./dismiss run`.

## Persistent dismissals

Record a review decision for a mutation from a finalized saved run:

```sh
cargo run -- dismiss --path /path/to/go/module add /path/to/saved/run \
  --id 9 --reason 'accepted test gap' --note 'Boundary is validated by the caller.'
cargo run -- dismiss --path /path/to/go/module list
cargo run -- dismiss --path /path/to/go/module check
cargo run -- dismiss --path /path/to/go/module update 1 --reason 'equivalent behavior'
cargo run -- dismiss --path /path/to/go/module update 1 --clear-note
cargo run -- dismiss --path /path/to/go/module remove 1
cargo run -- dismiss --path /path/to/rust/package add /path/to/saved/run \
  --id 3 --reason 'accepted test gap'
cargo run -- dismiss --path /path/to/rust/package check
```

The project defaults to the current directory. Use `dismiss --lang go|rust|js|python`
to select one enabled language when config contains several. Adding requires a saved report of the selected language.
`add --id` selects a **saved-run mutation ID**; `update` and `remove` use the
independent **dismissal ID** printed by `add` and `list`. Dismissal IDs are not
reused after removal. Reasons are required nonblank text; notes are optional.
`update --note TEXT` replaces a note, and `--clear-note` removes it. Updating a
reason or note preserves the stored mutation identity.

Decisions live in `mutie.dismissals.toml` beside the project manifest, separate
from generated reports and `mutie.toml`. Commit this file to version control. It
stores the reason, note, language, module-relative source path, SHA-256 source
fingerprint, operator, byte range, source coordinates, and original/replacement text. No absolute
checkout path or run-local ID is used for matching, so the metadata can move
with the repository. CLI edits validate the entire file and replace it atomically;
they reformat TOML and do not preserve hand-written comments. Malformed metadata,
unknown fields, incompatible versions, duplicate IDs, and duplicate additions
are rejected without overwriting the file.

`add`, `list`, `update`, and `remove` do not launch language tools or validate the
full execution scope. They resolve the same config and selected project as other
commands. Global `--path DIR` selects the working directory; `--config FILE` and
`--lang` select configuration and language.
Adding records the saved identity without checking current source; listing labels
it unchecked. `check` uses current Go package discovery or validated Rust Cargo
metadata/module discovery, plus the language, include/exclude patterns, discovery
timeout and output limits from `mutie.toml`. Explicit `dismiss --lang` selects an enabled language. It runs in disposable workspaces,
does not run tests, and creates no reports. With no dismissals, it skips discovery.
Rust checks enforce the initial Cargo scope, exclude caller build artifacts,
and discover identities from a pristine snapshot independently of Cargo side
effects. Timeout, cancellation and capture failures clean up owned directories.

New decisions record the execution adapter (`go`, `rust`, `vitest`, `node`, `bun`,
or `python`). Older decisions without a
language retain their Go meaning; they are never silently applied to Rust.
Language mismatches require review. Dismissals describe source judgments rather
than Cargo execution scope: manifest/lockfile/toolchain changes do not invalidate
an otherwise matching decision, though focused Rust reruns still require their
separate execution-scope checks.

Matching is strict, using the same source identity as focused reruns. Any edit to
the mutation's source file invalidates its fingerprint, even a comment or an
unrelated edit in that file. Test edits in separate files do not invalidate it;
edits to inline tests in the selected source file do. `check` reports
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
mutations. Invalid metadata fails a run before invoking language tools; `mutie list`
remains a source-only operation and does not load dismissal metadata.

`summary.txt` separates dismissed and undismissed missed counts and includes reasons,
notes, and unresolved entries. Execution counts in `outcomes.json` and all outcome
lists remain unchanged. To see a newly added or edited decision in the browser,
run Mutie again (a focused rerun is fine) and open the new report:

```sh
cargo run -- rerun /path/to/saved/run /path/to/go/module --id 9
cargo run -- report /path/to/new/run
```

Press `s` to filter by review status or `v` to inspect the saved dismissal review.

Concurrent CLI edits are guarded by `.mutie.dismissals.lock`, normally removed on
exit. If a process is forcibly stopped and leaves this file behind, confirm that
no dismissal command is running before removing the lock and retrying.

## Focused reruns

After improving tests, use a saved run directory to select just its completed
`missed` mutations. This works for Go modules and supported Rust packages:

```sh
mutie rerun /path/to/saved/run --missed
mutie rerun /path/to/saved/run --missed --dry-run
mutie rerun /path/to/saved/run --id 9,11
```

IDs refer to the **source run's** inventory, not a fresh scan or an earlier run
in a chain. They are positive integers; `--id` accepts repetition or comma-separated
values. Duplicate IDs select work once. `--missed` intersects with IDs and the
configured `operators` list. Unknown IDs are errors even when another filter would
exclude them. Set `operators = []` in config to allow all operators.
With no filters, `mutie rerun` selects the full saved inventory, including killed,
timed-out, build-failed, error, pending and interrupted mutations.

`RUN_DIR` is relative to the invocation directory, or absolute. It must contain
`run.json`, `mutants.json`, and `outcomes.json`; text lists, diffs, and logs are not
selection inputs. Finalized failed or interrupted runs also work when their
inventory is complete. `--missed` selects only completed missed outcomes;
`mutie rerun` can retry unfinished work. Running, incomplete, inconsistent, or
incompatible reports and invalid selectors are rejected before running tools or
creating a new report.

Rust reruns also require the saved language, Cargo package and production-target
inventory (kind, name, source and edition), default-feature/test/profile settings,
complete `Cargo.toml` and
`Cargo.lock` fingerprints, and compiler/Cargo versions to match the current
snapshot. Older single-binary report identities are normalized when read.
Scope mismatches fail before the baseline. Older Rust reports without
a manifest fingerprint remain browsable but require a fresh scan before rerunning.
Changing the manifest, lockfile, or toolchain requires a fresh scan too, including
comment-only manifest/lockfile edits. Worker counts, timeouts, file filters, output
limits, and policies use current settings. The browser exports Rust commands with
`--lang rust`, so a moved checkout with both manifests stays unambiguous.

Execution always runs a fresh baseline, rediscovers mutations from the current
snapshot, and validates every selected target before starting workers. Matching
requires the same module-relative source path, complete source-file fingerprint,
operator, original/replacement text, and source coordinates. Editing or adding
tests in separate files is supported; any change to a selected mutation's source
file, including comments or moved lines, requires a new full scan. Missing,
excluded, inactive, or ambiguous targets fail the entire selection rather than
silently being skipped.
Current include/exclude settings still apply; clear configured filters if needed.

The current module can be a moved or copied checkout. Current configuration,
tests, test scope, worker count, and timeouts apply; old outcomes are never reused. Newly
discovered mutations are not added to the saved selection. If the filters match
nothing, execution saves a completed empty run after baseline and discovery.
`--dry-run` previews and validates the selection without running tests or writing a
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

Reports keep publishing authoritative JSON checkpoints as a run progresses.
Derived text summaries and outcome lists are rewritten only when their contents
change or the file needs recreating. Changed files retain atomic publication and
disk synchronization; report contents and formats are unchanged.

By default, every test run saves a new directory under `<module>/.mutie/`. The CLI
prints its location when the run starts and the summary path when it finishes. Earlier runs
are preserved. Set `output_dir` in config to choose another parent directory;
relative paths are resolved from the config file. The output
directory cannot be the module root or an ancestor of it. `mutie list` writes no
reports; it still honors the configured report-directory exclusion.

Each run contains:

- `run.json`: versioned run metadata, effective configuration, commands, scope,
  timeouts, worker counts, workspace measurements, phase, status, findings-policy
  decision, and any run-level error.
- `mutants.json`: the complete mutation inventory, including IDs, named operators,
  source locations, original text, replacements, source fingerprints, `test_packages`, and diff
  paths. The inventory and every diff are published before workers start.
- `outcomes.json`: baseline and package-discovery results, mutation execution
  states and outcomes, durations, exit codes, log paths, retention byte counts, and
  summary counts.
- `summary.txt`: readable counts, per-operator outcomes and command time, workspace
  costs, unfinished work, missed mutations, and artifact references for every
  planned mutation.
- `diff/000001.diff`: one unified source diff per planned mutation.
- `logs/baseline.log`, `logs/discovery.log`, and `logs/000001.log`: readable command
  output, with separate `.stdout.jsonl` (Go) or `.stdout.log` (Rust) and
  `.stderr.log` captures alongside each log. Go stdout contains package JSON or
  test events. Rust discovery stdout contains Cargo metadata; test stdout mixes
  Cargo build events with standard test-harness output from both phases.
  Logs are saved even without `--verbose`. Limited logs
  contain head/tail excerpts and an omission marker; see the log limit below.
- `killed.txt`, `missed.txt`, `timeout.txt`, `build_failed.txt`, and `errors.txt`:
  outcome lists containing IDs and source descriptions, sorted by ID.
- `.mutie.lock`: an empty lock file held open for the report writer's lifetime,
  including its final writes. The file remains after the process exits.

New text summaries include the same operator breakdown as the browser's `t` view.
Totals cover only the run's selected mutations, including for filtered runs and
reruns. They are derived from the inventory and execution records; no additional
JSON fields are required, and browsing an older report does not rewrite its summary.

`run.json` includes `effective_config` with all merged settings, the resolved Go
language, absolute module/output paths, and the config file path (`null` when
absent). Its `timeout` is `null` when using the automatic policy; the actual
deadline, once determined, is in `mutation_timeout_seconds`. Configured `jobs`
and `requested_workers` describe the requested concurrency; `workers` records the
number used, capped by the mutation count. Configuration timeout values use
seconds, matching the TOML file.

`baseline_command` and `discovery_command` record their complete argument lists.
For a mutation, append its inventory `test_packages` to `mutation_command_prefix`.
Full scope records `["./..."]`; affected scope records sorted Go import paths.
These fields replace the earlier single `command` field; older saved reports
remain readable and eligible for reruns.

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
`comparison_invert`, `comparison_boundary`, `logical_swap`, `boolean_flip`,
`negation_remove`, `call_remove`, `branch_force_true`, `branch_force_false`, and
`loop_skip`.

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

### Limiting saved logs

Logs are unlimited by default. Set a positive byte limit per artifact with
`max_log_bytes` in `mutie.toml`:

```toml
max_log_bytes = 65536
```

Omit the setting for unlimited retained logs.

The limit applies independently to each readable `.log`, raw stdout, and raw
stderr artifact for baseline, discovery, and mutation commands. When output is
larger, Mutie retains roughly half from the beginning and half from the end
(the extra byte for odd limits goes to the beginning). A readable omission marker
is added outside the byte budget. Readable logs cut at UTF-8 boundaries and can
retain a few fewer bytes; raw captures preserve exact byte excerpts, which can
split characters or JSON records. A truncated `.stdout.jsonl` is an excerpt, not
a complete JSON stream.

Each `outcomes.json` log record includes `retention.text`, `retention.stdout`,
and `retention.stderr`, with `total_bytes`, `head_bytes`, `tail_bytes`, and
`omitted_bytes`. Counts exclude the marker. Complete logs have zero omitted and
tail bytes. Older reports without these fields remain readable. The browser
shows saved-log truncation separately from its own 256 KiB preview limit;
`--verbose` and command-failure diagnostics use the same limited readable output.

Classification and package discovery read the **complete temporary captures**
of commands that finish within their capture budget; saved-log truncation cannot
turn a tool/build failure into a killed or missed mutation.
Full stdout/stderr and rendered output remain file-backed until processed and
saved, then are removed. This setting limits retained artifacts and diagnostic
previews, **not temporary disk usage**. Parsing uses one Go output record at a
time; an individual oversized record and the package graph still need memory.
Interrupted commands save limited partial logs with the same metadata. Omitted
output cannot be recovered from the report; rerun with a larger limit or
an unset `max_log_bytes` config setting when more diagnostics are needed.

### Limiting command-output capture

`max_capture_bytes` in `mutie.toml` sets a positive
byte budget shared by raw stdout and stderr for **each Go or Cargo invocation**.
It defaults to unlimited. This applies to baseline tests, discovery (including
`mutie list` and configured `dismiss check`), and mutation tests. Each concurrent
command has its own budget. Reruns use current settings, not the source
report's budget.

For Rust, Cargo metadata, compilation (`--no-run`), test execution, and optional
worker-cache package cleanup each get a separate capture budget. Saved
baseline/mutation logs combine their phases, so combined raw byte counts can
reach twice the budget (three times for cached mutations), plus phase separators.
Cleanup, compilation and testing still share one timeout.

```toml
max_capture_bytes = 16777216
max_log_bytes = 65536
```

Omitting the capture limit restores the runner default. Bun/Python retain their
bounded default; Go/Rust/Vitest/Node default to unlimited.

With the limit enabled, Mutie drains both output pipes into temporary files and
never writes more than the combined byte budget to those raw captures. Output
ending exactly at the limit is allowed. Reading another byte stops the command's
process group and reports an explicit output-capture error with exit code `1`.
A baseline or discovery overflow prevents mutation execution; a mutation overflow
is a tool error and stops the run, including active peers. It is never classified
as killed, missed, or timed out, even if a test-failure event was already received.
Findings and minimum-score policies stay unevaluated.

The captured prefix is retained for diagnostics, with the saved-log limit applied
afterward. stdout/stderr are independent prefixes; their relative interleaving is
not recorded. Raw output can end within a UTF-8 character or JSON record. Readable
logs include an error notice, and the failure is saved in run/mutation errors or
the baseline/discovery record's `capture_error`, independently of log truncation.
Retention byte counts describe bytes captured, not all bytes the child attempted
to emit; the uncaptured amount is unknown. Increase or clear the capture limit
to rerun a command that needs more output.

This is a raw command-output budget, not a quota on all disk use. Rendered readable
logs, saved reports, workspaces, test-created files, and Go build/module caches
are separate. Readable rendering can expand invalid UTF-8, and JSON decoding still
needs memory for individual captured records. Deadlines and cancellation remain
active while draining. After process-group shutdown, Mutie allows up to one second
for buffered output to reach EOF; a pipe still held open is an explicit capture
error instead of an indefinite wait.

## Report retention

List saved runs with their status, age, logical file size, and any protection reason:

```sh
mutie runs                       # .mutie under the current directory
mutie runs /path/to/reports       # custom report root
mutie prune --keep-last 5         # preview keeping the newest five eligible runs
mutie prune --older-than 30       # preview runs finished more than 30 days ago
mutie prune --keep-last 5 --older-than 30
mutie prune --keep-last 5 --older-than 30 --apply
```

Both commands accept an optional report parent directory, defaulting to `.mutie`
relative to the current directory. They work without Go, the original checkout,
or `mutie.toml`; pass custom output directories explicitly. A missing report root
is an empty listing and is not created. Listing and pruning previews write nothing.

`prune` requires at least one rule and previews by default. `--apply` deletes the
selected run directories and all their report artifacts. `--keep-last N` preserves
the newest N eligible finalized runs, ordered by finish time (then name for ties).
Protected runs do not consume these slots; `--keep-last 0` selects all eligible
runs. `--older-than DAYS` accepts positive whole days of 24 hours and selects
runs strictly older than that threshold. With both rules, a run must be outside
the newest N **and** older than the age threshold to be selected. No automatic
retention or archival runs in the background.

Only direct `run-*` directories with supported, consistent finalized report metadata
and the expected artifact layout are eligible. Completed, failed, and interrupted
runs can qualify, including failed baselines with no inventory. Running reports
remain protected even after a forced kill. Locked reports, invalid metadata,
future/inconsistent timestamps, symlinks, special files, and unexpected files or
directories inside a run protect that entire run. Other root entries are ignored.
Sizes sum file lengths without reading logs; they are not physical storage usage
or a guarantee of space reclaimed on compressed or copy-on-write filesystems.

Apply acquires each selected report's lock and checks its metadata, identity, and
artifact list again before removing it. A changed or newly locked report is left
in place and reported as an error. Older finalized reports without a lock file
are supported; apply adds the lock before revalidation. Cancellation stops between
runs; deletion already underway finishes first. Filesystem failures can leave a
partially removed run and return an error with the number of fully deleted runs.

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
or operation. `mutie list` and `dismiss check` do not produce these reports or summaries.

The canonical temporary parent directory is recorded in
`workspace_metrics.temporary_root` and `summary.txt`. On Unix, set `TMPDIR` to an
existing, writable directory outside the source tree to choose workspace placement:

```sh
mkdir -p /path/on/project-filesystem/mutie-tmp
TMPDIR=/path/on/project-filesystem/mutie-tmp cargo run -- run /path/to/go/module --jobs 2
```

For copy-on-write comparisons, use a directory on the project's reflink-capable
filesystem and inspect the recorded counts. A different filesystem or a tmpfs can
prevent reflinks; support also depends on the OS and filesystem. Keep project inputs,
worker counts, timeouts, and Go cache conditions consistent when comparing runs.
The [go-cmp measurements](../benchmarks/go-cmp-v0.7.0.md) exercise a larger mutation
inventory on APFS. Comparisons against Linux tmpfs, ordinary copies, and projects
with large source trees or assets are still pending.

Each child Go command gets a private `mutie-go-*` directory for `GOTMPDIR`.
If the environment contains a nonempty `GOTMPDIR`, Mutie uses that existing,
writable directory as the parent; relative paths resolve from Mutie's starting
directory. Otherwise it uses the system temporary directory (`TMPDIR` on Unix).

Each Go command also gets a separate `mutie-test-*` directory under the system
temporary parent. Mutie sets `TMPDIR`, `TMP`, and `TEMP` to this absolute path
in the child, so test-created files and helper subprocesses use the same owned
directory. This applies to baseline tests, discovery, and mutation tests, with
separate directories for concurrent commands. The caller's environment is unchanged.

Both private directories and their contents are removed when the command ends,
including after startup errors, timeouts, or handled interruptions. Existing
parent contents and the reusable Go build cache (`GOCACHE`) are preserved.
Cleanup cannot run if Mutie itself is forcibly killed; files written to explicit
paths outside the private directories are not covered by this cleanup.
