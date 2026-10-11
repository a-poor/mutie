# mutie design

Status: working design, 2026-10-11. This document records the direction for the
project, including work that is not implemented yet. [README.md](README.md)
documents the current CLI and behavior; [TODO.md](TODO.md) tracks implementation
progress. Update this reference as decisions change.

## Goal

Mutie is a Rust CLI for finding gaps in tests by making small changes to source
code and checking whether the tests detect them. The intended local workflow is:

1. Run mutation testing on a project or selected source files.
2. Inspect a missed mutation and understand the behavior the tests do not check.
3. Improve the tests.
4. Rerun the relevant mutations and verify that the tests now catch them.

Start with Go and local use. Support other languages and CI over time. Use
`cargo-mutants` as a reference for understandable terminal output and inspectable
report artifacts, without requiring compatibility with its CLI or file formats.

## What the prototype establishes

The current implementation supports one Go module, selected through its `go.mod`.
It runs a baseline, discovers active Go files, creates Tree-sitter mutations, and
executes them in isolated worker copies. It already has file filters, configurable
workers and timeouts, repository configuration, live and plain terminal output,
complete mutation inventories, saved reports, focused reruns, and coordinated
cancellation with retained command diagnostics.

The fixture and tests exercise killed and missed mutations, compilation failures,
timeouts, failed baselines, workspace restoration, and preservation of partial
results. MVP validation on `godotenv` v1.5.1 demonstrates actionable test gaps
and practical runtime on a small library. A larger `go-cmp` v0.7.0 run completed
1,450 mutations across ten packages and measured workspace and command costs.
A fixed 32-mutation application sample on `fzf` v0.65.2 matched full/affected
outcomes and demonstrated two actionable call-removal gaps in its Go tests.
These runs do not establish acceptable noise or runtime for large applications
or asset-heavy trees.

The prototype supports rerunning saved mutations by ID, operator, or missed
outcome, and inspecting finalized reports with `mutie report RUN_DIR`.
Rust appears in the CLI language enum but is not implemented.

## Minimal MVP

The initial deliverable is a useful local Go tool with a small, predictable scope:

- One self-contained Go module, initially targeting Linux/Unix execution.
- `go test ./...` for the baseline and, by default, every mutation; an opt-in
  affected-package mode narrows mutation tests. No custom test runner yet.
- Whole-module mutation discovery by default, with include/exclude file globs.
- A small catalog of arithmetic, comparison, boolean, and logical mutations.
- A passing baseline, independent worker workspaces, and command deadlines.
- A basic interactive status view by default in an interactive terminal, with
  plain output for redirected output and non-interactive environments.
- Repository settings in `mutie.toml`, overridable by explicit CLI arguments.
- A complete mutation inventory, incremental results, readable summaries, diffs,
  and captured output saved to disk.

An MVP user should be able to run the bundled fixture, see progress, and inspect
every outcome after the workspaces have been removed. They should also be able to
commit repeatable project settings and override them for a single invocation.
The same execution engine must work without an interactive terminal.

Missed-only reruns are the first implemented follow-up to this smaller MVP.
Rich report browsing, advanced mutations, coverage-driven execution, additional
languages, multi-module projects, and plugins are not MVP requirements.

## Execution model and guarantees

The normal run proceeds through configuration, snapshot creation, baseline tests,
discovery, inventory publication, worker execution, and final reporting. A
listing-only invocation performs discovery without running tests or saving a run.

1. Resolve and validate configuration before executing tests.
2. Snapshot the current working tree, including local edits and untracked inputs.
   Exclude Git metadata and mutie's report directories. All work uses this same
   snapshot, so edits made during the run do not change later mutations.
3. Run the unmodified tests in a separate copy. A failed or timed-out baseline
   stops mutation execution and retains its diagnostics.
4. Ask the language implementation for eligible source files and mutations. Apply
   the configured scope, assign deterministic IDs within the run, and save the
   complete inventory before starting mutation workers.
5. Each worker applies exactly one mutation, executes the tests, and reports the
   outcome. Restore the entire workspace before its next mutation, including
   files created or modified by tests. Use reflinks where available and ordinary
   copies otherwise.
6. Save completed results as they arrive. Finalize the run after all workers have
   finished or a run-level failure/cancellation has been handled.

The input source tree is never a mutation target. Temporary copies isolate
repository state, not arbitrary filesystem access or external services used by
tests. External dependencies, flaky tests, and shared services can still affect
results; a passing baseline is a prerequisite, not a proof of determinism.

For Go, retain `-json -count=1 -timeout=0`: mutation runs execute tests rather than
reuse successful test results, while Go can still use its build cache. Mutie owns
the whole-command timeout, including compilation. Start with the current defaults:
300 seconds for the baseline and `max(10 seconds, 5 * baseline duration)` for each
mutation, with explicit overrides. Scope filters select mutated files; they do
not narrow the tests that run.

`test_scope = "affected"` / `--test-scope affected` uses a separate package
selection policy. Discovery requests `go list -deps -test -json ./...` from the
pristine workspace. Ordinary packages matched by `./...` supply mutation files and
eligible test targets; dependency-only packages, test variants, and generated test
binaries supply graph edges without becoming mutation targets. Build tags, cgo,
and platform selection continue to follow Go's environment.

The graph resolves `ImportMap` edges and follows reverse imports starting from the
mutated package and every copy rebuilt for a test. A target is selected if its
ordinary package or generated test binary is reachable. This includes transitive
production dependencies, internal/external test imports, and paths through
dependencies outside the target set. Missing nodes, duplicate package ownership,
missing test binaries, and incomplete/error metadata stop execution before inventory
publication. Discovery cancellation and diagnostics use the existing command path.

The baseline stays full-module, and `full` remains the default and an explicit CLI
override. Selection is computed before workers start and sorted for reproducible
arguments. Each planned mutation records `test_packages`; `baseline_command`,
`discovery_command`, and `mutation_command_prefix` describe commands in `run.json`.
Reruns recompute selections from current source/tests and use current settings.
There is no result reuse or coverage selection. Dependencies through runtime file
reads, subprocesses, or dynamic loading are outside the import graph: affected mode
is opt-in and full-module results remain the correctness reference.

Cancellation stops scheduling, terminates active process groups on Unix, collects
completed results and active command diagnostics, finalizes an interrupted run,
and cleans up workspaces. Signal handlers are installed before snapshot creation;
copying and discovery check a shared stop request, and workspace copying and
source parsing run on blocking threads. Workers are drained rather than aborted so
command output can be returned and saved. Ctrl-C and Unix SIGTERM use this path.
Tool failures also stop peers but retain a failed run status and the original
error. Terminal restoration will be added with the interactive view. An abrupt
process kill can leave the last checkpoint incomplete; it must never be presented
as a successful completed run.

## Outcomes and identity

Keep mutation execution state separate from its test outcome. Pending and running
mutations have no final outcome. An interrupted mutation must not become missed or
killed merely because its execution stopped.

| Outcome | Meaning |
| --- | --- |
| `killed` | The tests fail after applying the mutation. |
| `missed` | The tests still pass. |
| `timeout` | The command exceeds its deadline. |
| `build_failed` | The mutation cannot compile or fails the language tool's build checks. |
| `error` | A tool or infrastructure failure prevents a trustworthy classification. |

Tool failures stop the run rather than inflate the killed count. A timeout is
reported separately, and an invalid mutation is not evidence of good tests. The
default reports counts and a raw score without enforcing a threshold. Completed
local runs can succeed with missed mutations; baseline/tool failures exit
unsuccessfully. Opt-in `--fail-on-missed` and `--min-score` policies exit `3` when
a completed, saved run fails either enabled gate. Execution status stays
`completed`, independently of the policy decision; see the CI section for score
definitions and exclusions.

Inventory records contain the run-local ID, relative source path, named operator,
byte range, line/column range, original text, replacement, and source fingerprint.
Fingerprints use SHA-256 over the complete original file, encoded as `sha256:`
plus lowercase hex. Operator catalog version 1 groups edits as arithmetic
replacement, comparison inversion/boundary changes, logical swaps, boolean flips,
and negation removal. The catalog version is recorded in run metadata and the
inventory. Execution records refer to these IDs and carry state, optional outcome,
duration, exit status, diagnostics, and artifact paths. Pending/running/interrupted
executions have no outcome. IDs do not depend on worker completion order.

Run-local IDs are not stable identities across edits. Reruns rediscover mutations
against the current snapshot and validate the match. The initial conservative
policy requires the same relative path, whole-file fingerprint, operator,
original/replacement text, byte range, and line/column range. Tests may change;
changes anywhere in a selected source file require a new full scan. Saved offsets
and replacements are never applied directly, and prior outcomes are never reused.

## Mutation catalog

Start with simple syntax-based edits, such as:

- Arithmetic: `+` to `-`, `-` to `+`, and selected multiplication/division changes.
- Comparison boundaries: `>` to `>=`, `<` to `<=`, and their counterparts.
- Comparison inversion: `==` to `!=`, or a comparison to its complement.
- Boolean and logical changes: `true` to `false`, `&&` to `||`, or removal of `!`.
- Standalone calls: remove a complete call statement (`call_remove`).

These describe the implemented catalog. Each
mutation makes one change. Discovery order and operator names should be
predictable, with tests for exact edits and source locations. Exclude test files,
generated code, and dependencies by default. Let the language tool determine
which files belong to the active build.

Tree-sitter supplies syntax, not a complete type system. Some syntactically valid
edits will be unbuildable, such as changing string addition to subtraction in Go.
Keep those results visible and separate; improve noisy operators based on evidence.
Do not attempt general equivalent-mutation detection in the MVP.

`call_remove` selects expression statements whose expression is a call (possibly
parenthesized) directly inside a statement list in a block or case body. This
follows Go's [expression statement syntax](https://go.dev/ref/spec#Expression_statements)
without needing type information. It handles methods, generic calls, built-ins,
and immediately invoked function literals. It excludes values used in assignments,
returns, or other expressions, including blank-identifier assignments; go/defer,
labeled statements, and control-flow headers are outside this first operator's
scope. Nested argument calls are not separate removal candidates; mutations in
their literals/operators and in function literal bodies remain independent.

The mutation replaces only the statement's byte range with empty text. Surrounding
whitespace, semicolons, and comments remain; comments inside the range disappear.
Do not repair unused imports/locals or missing returns after removing a terminating
panic: these are build failures, not killed mutations. Saved identity includes the
whole original statement and exact byte/line/column endpoints. Unified diffs now
support multiline edits, correct old/new hunk lengths, three context lines, CRLF,
Unicode, and missing final newlines.

All operators are enabled by default. Resolve repository `operators`, CLI
`--operator`, and `--clear-operators` before execution, normalize duplicates, and
save the effective list (empty means all). Match dismissals against the complete
discovered catalog before selecting operators, then remap only included matched
decisions to final IDs. `dismiss check` ignores operator restrictions so valid
decisions outside a run's selection remain checkable. Reruns apply the same merged
operator selection to the source inventory before strict current-source matching.

Return-value, branch, and loop mutations and named operator profiles remain future
work. Favor understandable findings over generating the largest possible number
of mutations. See [call-removal validation](benchmarks/godotenv-v1.5.1-call-remove.md)
for four actionable CLI gaps and four unbuildable removals on `godotenv` v1.5.1.

## Repository configuration

Mutie reads optional `mutie.toml` at the selected Go module root; parent-directory
inheritance and workspace configuration are deferred. Missing or empty files
retain the default workflow.

Precedence is built-in defaults, then repository config, then explicitly supplied
CLI arguments. The CLI parser preserves optional scalar arguments; `config.rs`
produces a resolved run configuration consumed by execution and reporting. Unknown
keys, malformed TOML, and incorrect types are rejected before merging. The merged
settings are validated before creating reports or invoking Go, including globs,
supported language, positive worker counts/timeouts, representable deadlines, and
output paths. `run.json` records every resolved setting in `effective_config`,
including absolute module/output paths and the optional config file path.

Supported configuration syntax (all keys are optional):

```toml
lang = "go"
jobs = 2
include = ["**/*.go"]
exclude = ["internal/legacy/**"]
operators = [] # all; or a list of named operators
timeout = 30
baseline_timeout = 300
output_dir = ".mutie"
verbose = false
ui = "auto"
```

Timeout values are positive integer seconds. File-selection globs are always
relative to the project root. Paths supplied in the repository config are relative
to that root; paths supplied through CLI arguments are relative to the invocation
directory. Absolute paths are supported. Output resolution follows existing
symlinks and normalizes `.`/`..` in order without creating directories; the module
root, its ancestors, files, and dangling symlinks are invalid output destinations.
Paths are not shell-expanded.

Explicit CLI lists replace their configured counterparts. `--clear-include` and
`--clear-exclude` remove the respective configured patterns. `--operator` replaces
configured `operators`; `--clear-operators` selects all operators. `--clear-timeout`
restores the automatic timeout and `--no-verbose` disables configured verbosity.
Reset flags conflict with their corresponding value flags. Automatic language
selection currently resolves to Go; a missing mutation timeout remains `null` in
effective settings, with the actual duration recorded after the baseline.

`--list` is invocation-only. It uses the resolved settings and excludes both
`.mutie` and the selected report directory without creating reports. An explicit
CLI `--output-dir` conflicts with `--list`; a configured output directory does
not. `ui` selects `auto`, `interactive`, or `plain`, with an explicit `--ui`
override; `--list` always uses plain output. A separate environment
configuration layer is unnecessary for the MVP, although child tools still use
their normal language environment.

## Terminal experience

Use [Ratatui](https://ratatui.rs/) for the default live status view when the
terminal supports it. Take visual inspiration from the
[Vitest default CLI reporter](https://vitest.dev/guide/reporters.html#default-reporter):
a compact inline display with active work and a running summary. Redraw the status
region in place as work progresses; appending log lines alone does not satisfy
the interactive view. Keep Ratatui in the presentation layer, independent of the
execution engine. Use plain output when output is redirected, in CI, or when the
user explicitly disables the UI. The `ui` setting and matching `--ui` override
select `auto`, `interactive`, or `plain`. Auto mode requires terminal stdin and
stdout, a `TERM` other than `dumb`, and no `CI` environment variable; terminal
initialization failure falls back to plain output. Forced interactive mode
requires a compatible terminal even when running in CI.

The minimal view shows the current phase, elapsed time, completed/total mutations,
outcome counts, active worker locations, and recent results. Update elapsed time
and activity indicators while commands are running, even between completed
mutations. Support small terminals and resizing. Cancellation is the only
necessary control initially; pause/resume and changing the work queue
interactively are later features. The implementation uses Ratatui 0.30.1 with
Crossterm 0.29 and an application-managed inline area backed by a fixed Ratatui
viewport. Rendering runs at most ten times per second,
coalescing snapshots while keeping timers and activity indicators moving. Worker
rows stay ordered by worker number, including idle workers, and expand to fit the
terminal height. Hidden workers have an overflow count including the active count.
A bounded results tail below the workers shows up to three completions, newest
at the bottom. Its space is reserved independently of the number of results so
worker rows remain stable. Compact layouts shorten labels and prioritize totals;
text labels accompany semantic colors.

Allocate at most the terminal's height, using ordinary scrolling to preserve
earlier output in scrollback. Park the hidden cursor at the status area's start
between frames. On resize, query this terminal-maintained anchor and recreate
the fixed viewport buffers, clearing only from the anchor downward. This avoids
Ratatui 0.30.1's full-screen clear and origin reset on horizontal shrink. Exit
cleanup also clears from the parked cursor, without a stale absolute row or a
teardown cursor query. Frames and their cursor placement are buffered and emitted
together; this uses the pinned backend's `unstable-backend-writer` feature.

Buffering alone does not prevent a frame prepared before resize from arriving
after the terminal pulls history back onto the screen. The live writer translates
the pinned Crossterm backend's absolute cursor moves to relative row movement
from the parked cursor. Downward movement uses line feeds so a late frame taller
than the screen scrolls history safely; wrapping is disabled during each frame
so a late wide frame cannot unexpectedly advance rows. Wrapping is re-enabled at
the frame's end and during error/panic cleanup. Inline allocation also uses
relative movement after its cursor query. A PTY regression deliberately holds
old-size output until after growth/shrink, covering completion and cancellation;
emulator tests compare normal full/incremental frames with native backend output.

Worker assignment starts the elapsed timer and the `preparing` state. Launching
the Go command switches to `running`. Live structured build output can switch to
`building`; observed package start/run events switch to `testing`. Track active
packages because their events interleave: a finished test within a package does
not end that package, and build output cannot hide another package's active tests.
When no observed package remains active, use `running` until further evidence
arrives. Silent builds do not supply enough evidence for a more specific label.
Completion clears the assignment to idle without resetting the worker row.

Progress travels over a bounded channel to the coordinator, identified by worker
and mutation ID; delayed messages cannot modify a reused worker's new assignment.
Only phase changes are sent. These transient events update presentation state,
not persisted execution states/outcomes or checkpoints. Completion and failure
remain authoritative and take priority over queued progress.

The process runner retains file-backed stdout/stderr capture. An independent
stdout reader observes complete lines during execution, with bounded polling
batches and line buffering; oversized or invalid lines are skipped only for
progress. The reader is reopened so it cannot move the child's shared write
offset. Observation is cancelled with command completion, timeout, or
interruption, including when a progress callback is blocked. Reader failure stops
advisory observation; final capture and classification retain their normal error
handling. After process-group cleanup, capture readers freeze their endpoint so
later writes cannot extend processing indefinitely. Complete captures remain on
disk; `go.rs` processes events one record at a time on a blocking task, accumulates
classification evidence independently of retained logs, and renders readable
output into a separate temporary file. Discovery decodes the complete capture
through a reader, including when only a tiny excerpt will be saved.

The opt-in `max_capture_bytes` budget changes raw capture transport to pipes.
Mutie alone writes the capture files, draining stdout and stderr with a shared
per-command remaining-byte count and bounded buffers. Each write is capped before
reaching disk; a one-byte lookahead distinguishes exact-budget output from an
overflow. Unlimited commands retain direct file capture. Progress observation
still uses an independent reader and cannot block capture, deadlines, or signals.
On overflow or capture I/O failure, stop the process group and preserve captured
diagnostics as an execution error. A fast child exit still drains buffered bytes;
after group shutdown, a one-second drain deadline prevents inherited pipes from
hanging cleanup. Failure to reach EOF is explicit, never silently treated as
complete output.

Capture errors take precedence over test/build evidence and timeout classification;
handled cancellation remains interrupted. Do not decode incomplete discovery as a
package graph. Baseline/discovery records save `capture_error` and a failed outcome;
mutations save the existing error outcome and explanatory error field. Run errors
and browser error details remain available independently of saved-log retention.
The readable log also includes a capture-error notice. Retention counts cover only
captured bytes; do not claim a total for unobserved output. CLI/config overrides
and a clear flag follow existing settings rules, and reruns use current settings.

Every child Go command owns two private temporary directories. Build output uses
`GOTMPDIR`; its parent is the nonempty environment `GOTMPDIR`, or the system
temporary directory when unset/empty. General test files use a separate directory
under the system temporary parent, passed through the child's `TMPDIR`, `TMP`,
and `TEMP`. Canonicalization makes relative parents independent of the child's
workspace; the caller's environment is never modified. Discovery and baseline
commands use the same ownership rules as mutation tests.

Both directory guards outlive the child and process-group guards, removing files
left by forced command termination as well as ordinary exits and startup errors.
Each concurrent command has separate directories, so completion cannot remove
another command's active files. Existing parent contents, explicit external paths,
and `GOCACHE` are outside this ownership. Like workspace cleanup, removal cannot
run if Mutie itself is forcibly killed.

Keep noisy test output in artifacts rather than interleaving it with screen
updates. After completion, restore the terminal and print a durable plain summary
and report path. Verbose captured output is printed only in plain mode. The live
view leaves input in normal mode between cursor-position queries and uses the
existing signal-driven cancellation path. A terminal write failure cancels and
drains execution before finalizing the report; terminal cleanup also runs on
errors and panics. The separate saved-report browser supports selecting results,
scrolling diffs/logs, intersecting filters, and generating focused rerun commands.

The UI renders coordinator state; it does not own workers or decide outcomes.
Typed events feed the report writer, plain output, and a compact run-state
projection. A watch channel shares the latest snapshot with the renderer; it
retains counts and bounds recent display history without dropping persisted
results. Report saving works independently of presentation.

### Saved-report browser

`mutie report RUN_DIR` bypasses run configuration and Go discovery. The shared
`report_reader.rs` validates finalized metadata, schema/catalog versions,
checkpoints, inventory IDs, and execution states, then joins records by saved ID.
Both rerun selection and the browser use this validation. Failed/interrupted
runs without an inventory can be inspected; reruns still require a complete
inventory. Metadata reads are capped at 64 MiB per JSON file.

`browser.rs` owns selection, marks, outcome/operator/file/review filters, input editing,
detail scrolling, and cached artifact text. Rendering is a projection of that
state with no file I/O. Artifact changes load at most 256 KiB and sanitize terminal
controls; missing files show inline errors. Artifact references must resolve to
regular files inside the report directory. The browser never modifies reports.
Dismissal data comes only from the saved inventory, never current checkout files.
`s` cycles all/undismissed/dismissed; it intersects with the other filters and keeps
hidden marks. `[D]` badges and saved reasons/notes accompany actual outcomes, and
`v` opens a scrollable review summary with unresolved entries. Older reports and
pre-inventory failures show review as not recorded; their rows have no recorded
dismissal. `--module` only affects command generation. Rendering remains on pinned
Ratatui 0.30.1/Crossterm 0.29.0 with the existing lifecycle and event loop.

The synchronous event loop runs on a blocking task, leaving Tokio's cancellation
handlers responsive. It polls every 100 ms and redraws on input/resize. Ratatui's
initialization enables raw mode and the alternate screen; a guard balances those
modes, bracketed paste, colors, and cursor visibility. Cleanup attempts every
operation even after an output error, and runs on normal exit, error, panic, and
cancellation. This lifecycle is separate from the live inline status view.

Wide terminals show a list beside the detail pane; widths below 90 columns stack
them. Below 24x12, a resize hint replaces the layout. Keyboard navigation, Unicode
file filtering, and paste update state outside rendering. Marks persist when
filters hide rows, with hidden counts shown. `r` previews a POSIX-quoted command
using all marked IDs or the current row, and `e` restores the terminal before
printing it. `--module PATH` overrides the recorded checkout path. No execution
is initiated from the browser; the resulting rerun follows normal source
validation and baseline rules.

## Saved reports

Borrow the separation of mutation inventory, execution results, diffs, logs, and
outcome lists from the
[`cargo-mutants` output directory](https://mutants.rs/mutants-out.html).
Use a mutie-specific, versioned schema for now. Elements-compatible JSON, HTML,
and other interchange formats are possible future exporters.

Current contents of a single run directory (schema version 1):

```text
run.json           effective configuration, versions, command, timing, workspace costs, run state
mutants.json       complete inventory, written before mutation execution
outcomes.json      baseline result, mutation outcomes, summary counts
summary.txt        readable summary and artifact references
diff/<id>.diff     one source diff per planned mutation
logs/baseline.log  readable baseline output
logs/discovery.log readable package-discovery output
logs/<id>.log      readable output for a mutation
killed.txt         outcome lists containing IDs and source descriptions
missed.txt
timeout.txt
build_failed.txt
errors.txt
```

Detailed stdout/stderr captures can remain alongside the readable logs. Source
paths are project-relative and artifact paths are run-directory-relative. Keep
units and location conventions explicit in the schema. Preserve native byte
offsets and document the conversion if a future exporter uses another convention.

`--max-log-bytes` / repository `max_log_bytes` sets an optional positive byte
budget for each readable, stdout, and stderr artifact. The default is unlimited;
an explicit CLI value replaces config, and `--clear-max-log-bytes` restores the
default. The effective setting is saved with all other run configuration.
`capture.rs` retains a beginning/end excerpt, with the extra byte assigned to the
head for odd limits, and inserts an omission marker outside the payload budget.
Readable output is valid UTF-8, with cuts moved inward to character boundaries;
raw excerpts can split UTF-8 or JSON records and are not promised to be parseable.

Log records contain `retention.{text,stdout,stderr}` byte counts: original total,
retained head, retained tail, and omitted middle. The reader validates their sums
and accepts legacy records without retention metadata. Artifacts are streamed
into temporary files in the report directory and atomically replaced only after
writing and syncing. The browser shows retention notices even for missing files,
separately from its bounded artifact preview. No report/catalog version bump is
needed during local development.

Retention never supplies classification evidence or package-discovery input.
The cap bounds persisted log payloads and in-memory diagnostic previews, not
temporary disk usage or individual JSON-record size. Full command captures and
the rendered text are removed when the command result is released, including
failure, timeout, interruption, and report-write errors. The separate opt-in capture
budget bounds combined raw stdout/stderr per command and terminates on overflow;
it does not silently discard semantic events and then classify an incomplete run.
Rendered logs can expand invalid UTF-8, and reports, caches, workspace files, and
individual decoded records have separate disk/memory costs. Archival/compression
and run-wide resource quotas remain future policies.

Publish the inventory and diffs before workers begin. Inventory entries without
final outcomes remain visibly untested or interrupted. Update results and derived
summaries as execution progresses, using atomic replacement for JSON snapshots.
The structured inventory/results are authoritative; text summaries are derived.
Report failures must be surfaced instead of claiming that output was saved.

The implementation saves a new `.mutie/run-.../` directory every time using the
layout above. Each JSON file is individually atomic. `run.json` and `outcomes.json`
carry matching checkpoint numbers once an update finishes; a mismatch identifies
a partial update. There is no multi-file transaction. Text outputs are derived
and may lag the JSON snapshots after a forced kill. Before discovery/publication,
`mutants.json` has `complete: false`; a complete empty inventory is distinguishable
from discovery that never finished.

Workspace operations return their elapsed time and successful file-transfer counts,
including partial work before a failure or interruption. `workspace_metrics.rs`
aggregates snapshot creation, baseline setup, discovery reset, worker setup, and
worker reset independently. Counts distinguish successful explicit reflinks from
fallback copies using the copy library's return value; fallback bytes are logical
bytes, not physical I/O. Copy behavior and outcome classification are unchanged.

The coordinator receives typed workspace events and stores the aggregate in
`run.json.workspace_metrics` at the next normal checkpoint, without extra report
writes. Worker measurements arrive with job completion, including failed or
interrupted jobs. Final CLI and text summaries show compact combined totals.
Operation status describes filesystem work, independently of subsequent Go results.
Durations are cumulative across workers; they include reset deletion and copying,
but exclude blocking-pool queue time, mutation application, Go commands, and final
workspace cleanup. Existing command durations remain in `outcomes.json`.
The canonical temporary root is saved for placement comparisons; `TMPDIR` already
allows Unix callers to place workspaces outside the source tree on a chosen filesystem.

Retention is explicit and separate from report generation. `mutie runs [DIR]`
lists direct `run-*` children with status, age, logical size, and protection reasons.
`mutie prune [DIR]` requires `--keep-last N` and/or `--older-than DAYS`; both rules
intersect. The newest eligible finalized runs are ordered by finish time, with
name as a deterministic tie breaker. Protected runs do not consume keep slots.
The default root is `.mutie` relative to the invocation directory; these commands
do not load module configuration or invoke Go. Preview is read-only; `--apply`
is required to delete. Automatic retention and archival remain future work.

The retention reader reuses finalized-report validation and restricts artifacts
to the writer's layout. Unknown files, nested directories, symlinks, special files,
inconsistent/future timestamps, unsupported metadata, and unfinished runs are
protected. Abandoned `running` reports are never inferred to be safe to delete.
Size calculation uses file metadata, not log contents, and reports logical bytes.
Each new writer holds an exclusive `.mutie.lock` file lock until drop, including
the multi-file final flush. The persistent empty file does not itself mean active.
Pruning acquires the same lock, revalidates the selected snapshot and directory
identity, then holds it through removal. Legacy finalized runs gain the lock only
on apply. Deletion removes only validated artifact paths and uses nonrecursive
directory removal, preserving unexpected files added during deletion. The lock
file is removed last, after the report artifacts, to coordinate concurrent pruners.
Changed or locked selections fail without being deleted; filesystem deletion
errors report partial progress. Cancellation stops between removals.

## Component boundaries

Keep the implementation modular without building a plugin framework up front.

| Component | Responsibility |
| --- | --- |
| CLI and config | Resolve project, options, and presentation mode into one run configuration. |
| Language support | Discover active files, generate language-specific mutations, define test commands, and interpret results. |
| Coordinator | Baseline gate, mutation inventory, scheduling, cancellation, and authoritative run state. |
| Workspace and process runner | Snapshot/copy/reset, apply edits, execute commands, enforce deadlines, and capture output. |
| Report writer | Persist inventory, results, metadata, diffs, and logs. |
| Report reader | Validate and join finalized reports; read bounded saved artifacts. |
| Rerun selection | Intersect ID/operator/outcome filters and match selected targets to current discovery. |
| Terminal presentation | Show live coordinator state; browse saved results and generate rerun commands. |

`coordinator.rs` owns a small event/state boundary: phase changes, command results,
workspace measurements, inventory publication, mutation starts and completions,
and run termination.
`status.rs` owns presentation and terminal lifetime. The execution task processes
every event and persists reports; the renderer reads compact snapshots without
blocking on worker operations. Process output stays in artifacts, keeping it out
of the redraw channel. Domain records drive both modes without parsing progress
messages.

Some Go-specific responsibilities currently live directly in `mutation.rs` and
`go.rs`. Keep a clear boundary now, and extract a shared language interface when
implementing a second language. Avoid speculative abstractions with only one
working implementation.

## Work after the MVP

### 1. Focused reruns and richer inspection

`--rerun RUN_DIR` selects a finalized saved inventory, regardless of previous
outcome; `--rerun-missed RUN_DIR` restricts it to completed missed outcomes.
Repeatable or comma-separated `--id` and `--operator` filters narrow either mode.
IDs always belong to the immediate source run. Values within a filter form a
union; different filters intersect. Normalize and deduplicate filters, reject
unknown IDs before applying other filters, and reject unknown operators during
CLI parsing or configuration loading. The source options conflict; IDs require a
source. Source and ID settings are invocation-only; operator selection also works
on fresh runs and can be configured in `mutie.toml`.

The reader validates schema/catalog versions, final status, matching
checkpoints, a complete inventory, counts, IDs, source paths, and execution states
before creating a new run. Finalized failed/interrupted reports are usable when
they have a complete inventory; general reruns can retry pending/interrupted
executions, while missed-only reruns exclude them.
The prior module root need not match the current checkout's location.

Execution uses current settings and the ordinary snapshot, fresh baseline,
discovery, and worker lifecycle. Scope filters apply before matching; every saved
selected target must match uniquely or the run fails before any mutation executes.
New mutations are not implicitly added. `--list` validates and previews without a
baseline or report, showing new IDs, source IDs, operators, and descriptions.
Empty selections print an explicit message and produce a completed empty run
after baseline and discovery when executing.

New reports preserve the source run. After inventory publication, `run.json.rerun`
records the source path/checkpoint/status, selection mode (`all` or `missed`),
optional normalized `source_ids` and `operators` filters, and old-to-new ID
mapping. The text summary includes the filters and mapping. The full request is
recorded in `effective_config.rerun` even if execution fails before inventory
publication; it replaces the earlier `effective_config.rerun_missed` path field.
Reading prior reports does not depend on either effective configuration format.
Snapshot copying excludes the source run directory; output paths inside that run
are rejected. The report and operator
catalog versions remain unchanged during local development.

Interactive diff/log browsing now uses the shared saved-report reader. A prior
report supplies a selection, not proof that an old result remains valid. More flexible
matching across source edits needs an explicit policy and validation before it
can replace the current strict identity check.

Persistent dismissals now use `mutie dismiss --module PATH add|list|update|remove|check`.
`add RUN_DIR --id ID --reason TEXT [--note TEXT]` copies a finalized report's
mutation identity into module-root `mutie.dismissals.toml`. Reasons and supplied
notes must be nonblank. Entries have independent monotonic dismissal IDs for
inspection, reason/note updates, and removal. The file is repository-owned and
can be version-controlled independently of generated reports. CRUD commands
require `go.mod` but no Go installation or configuration loading, and label source
validity unchecked. They never alter the original report.

`identity.rs` shares exact source matching with focused reruns: module-relative
path, full-file SHA-256 fingerprint, operator, byte range, coordinates, and
original/replacement text must all agree. `dismiss check` loads repository
configuration, snapshots the project, runs Go package discovery in a separate
copy, and discovers mutations from pristine source. It runs no baseline or
mutation tests and creates no reports. Empty stores skip discovery. Test-only
changes and moved checkouts preserve identity; edits anywhere in a mutation's
source file invalidate it. Missing candidates (including excluded/inactive files),
changed fingerprints, unmatched identities, and ambiguous matches are printed
for review with a nonzero exit status. Multiple stored entries matching the same
current mutation are all ambiguous. Checks never rewrite metadata or retarget a
dismissal; reassessment uses removal and addition against a fresh report.

`dismissals.rs` validates format/catalog compatibility, unique positive IDs,
identity fields, reasons, and notes. The new metadata format starts at version 1;
existing report and catalog versions are unchanged. CLI edits use an exclusive
create-new lock during read/modify/write and atomic same-directory replacement.
The temporary lock is removed on ordinary exit, including errors; a forcibly
terminated writer can leave a lock requiring manual removal. Readers do not need
the lock. CLI rewrites normalize TOML and discard comments. Duplicate identities
are rejected on addition; manual/merge duplicates remain inspectable and removable
so `check` can flag their ambiguity.

New runs load dismissal metadata from the pristine snapshot before Go starts;
invalid metadata fails the run at that point. Source-only `--list` does not load it.
After discovery, `DismissalSnapshot` partitions decisions into uniquely matched
entries and entries needing review. Matching happens before rerun filtering, so
valid decisions outside the selection are omitted without being called missing.
Selected matches are then mapped to the new inventory IDs. The plan event carries
the review to the report writer, which publishes it with the immutable inventory
before starting workers. Go command side effects and later checkout changes cannot
alter the captured decisions.

`mutants.json` has an optional `dismissals` object with `matched` and `needs_review`
arrays. Matched records pair a run-local `mutation_id` with a full dismissal
(independent ID, reason, optional note, source identity). Review issues pair a
status with the same record. Readers validate required text, identities, unique
IDs/targets, and references to inventory mutations; issues cannot also be applied
as matches. Missing review data means not recorded for legacy or pre-inventory
reports; empty arrays mean an explicit empty review. No schema/catalog version
bump is needed during local development.

`summary.txt` shows separate dismissed/undismissed missed counts, reasons/notes,
and unresolved entries. The browser exposes these through badges, details, a review
filter, and a review summary. Saved decisions remain historical: adding a dismissal
after a run requires a new run or focused rerun to capture it for browsing. Browsing
requires neither Go nor current repository metadata. Outcome counts, outcome lists,
execution, and rerun selection continue to include dismissed mutations. A dismissed
missed mutation still has a missed outcome; a rerun can later kill it. A dismissal
records human judgment, not proof of equivalent behavior or a passing result.

### 2. Real-project validation and measured performance work

The [go-cmp v0.7.0 measurements](benchmarks/go-cmp-v0.7.0.md) extend validation to
1,450 mutations in a multi-package library. On this APFS host, workspace operations
took 22.34 cumulative seconds and mutation commands 7,059.62 seconds. All 69,744
file transfers were reflinked. Focus performance experiments on Go command work,
while preserving full-module results as the reference. Distinguish compilation
and test costs where the tool provides reliable measurements.

A controlled [affected-package comparison](benchmarks/go-cmp-v0.7.0.md#affected-package-follow-up)
uses the same 32 saved IDs with four workers and a fixed diff-search direction.
Full / affected / affected / full runs matched all outcomes; mean run duration
fell from 36.13 to 27.00 seconds. This is evidence for the opt-in mode on this
compact library, not a general speedup or proof of selection equivalence.

The [fzf v0.65.2 application sample](benchmarks/fzf-v0.65.2.md) covers 32 mutations
across all seven operators, including eight call removals. Affected / full / full /
affected trials matched every outcome (6 killed, 21 missed, 5 build failures);
mean run time fell from 59.76 to 49.91 seconds. New tests killed two survivors in
both scopes. Workspace operations took about 1.1 cumulative seconds per trial,
with all 4,550 transfers reflinked. Sampled temporary logical bytes peaked at
59.7–66.4 MB; reports used 1.55–2.34 MB with a 65,536-byte per-artifact log limit.
This evaluates the Go suite on a 1.34 MB source tree, not fzf's Ruby/tmux suite or
large-tree scaling.

The application tests initially left files directly in inherited `TMPDIR`: 220 files
(15,961 bytes) per full trial and 172 (12,571 bytes) per affected trial. Mutie's
workspaces, Go build directories, and captures were removed. General test temporary
files now use a per-command owned directory with cleanup on completion, timeout,
and cancellation, preserving unrelated caller files and reusable caches.
Repeating the same fzf sample with the change left both temporary parents empty,
with all 32 outcomes unchanged in each scope; see the benchmark's cleanup follow-up.

The earlier go-cmp run exposed Go build directories retained after forced
termination and a 1.35 GB report dominated by logs. Temporary Go builds now have
a per-command cleanup lifetime; the reusable Go build cache remains separate. Explicit retention
now lists report sizes and previews pruning by count and age. Per-artifact saved
log limits and per-command raw capture limits are implemented; archival policies
remain open. Larger applications, asset-heavy trees, and reflink-capable filesystem
versus Linux tmpfs comparisons still need measurements.

Full-module tests remain the correctness reference for the implemented opt-in
affected-package mode. Coverage-based selection and result reuse still need explicit
invalidation rules and separate validation. Consider workspace-copy optimizations
without giving up restoration of test side effects. Measure before choosing which
optimization to implement.

#### Future exploration: structural identities and result reuse

Deferred exploration, not the next implementation task. Investigate whether
Tree-sitter structural fingerprints and Merkle hashes could preserve mutation
identity across unrelated edits and avoid redundant mutation execution. Current
matching requires the whole-file fingerprint and exact source coordinates; no
change to that behavior is implied by this proposal.

Keep mutation identity separate from result validity. A syntax-node hash could
combine node kind, token contents, and ordered child hashes, excluding source
positions. Enclosing declarations and surrounding context could help relocate
mutations; repeated identical expressions must not produce ambiguous matches.
Normalization needs explicit rules, including preservation of meaningful Go
directives. An unchanged subtree establishes syntax identity, not unchanged
behavior through its callees, shared state, or tests.

Result reuse would additionally need fingerprints of relevant source dependencies,
tests and fixtures, dependency versions, toolchain, execution settings, and tracked
environment inputs. A Merkle dependency graph could propagate changes into cache
keys, but Tree-sitter alone does not discover all dependencies. Untracked external
inputs and nondeterminism require a conservative fallback. The fzf validation is a
concrete invalidation case: adding tests changed missed mutations to killed while
all implementation files remained unchanged.

Explore in stages: structural matching first, conservative file/package-level
result keys next, then finer reuse using per-test coverage and killing-test
information. Added or changed tests must invalidate relevant missed results.
Keep dismissals as separate review decisions with explicit staleness rules;
initially retry timeouts and tool errors. Validate reuse against fresh full-module
runs, retain a fresh baseline, expose reused-result provenance, and provide a way
to force execution. Measure avoided test execution before investing in finer hashes.

Useful references are [Stryker's incremental mutation testing](https://stryker-mutator.io/docs/stryker-js/incremental/),
including its untracked-input limitations, and
[Bazel's action-cache input model](https://bazel.build/remote/caching).

### 3. Broader mutations and language support

Expand the Go catalog based on observed gaps. Candidate additional languages are
Rust, JavaScript/TypeScript, and Python. Pick their order based on concrete projects
we want to test rather than promising all of them at once.

Each language needs more than a Tree-sitter grammar: project/file discovery, useful
mutation rules, build/test integration, timeout behavior, and reliable outcome
classification. Add one language at a time using the same lifecycle and fixture
expectations. Decide multi-module and test-framework support separately from simply
recognizing a language.

### 4. CI and additional report consumers

The first CI policy is implemented: `fail_on_missed` defaults to false, merges from
repository configuration, and supports explicit `--fail-on-missed` /
`--no-fail-on-missed` overrides. Evaluate it only after successful execution with a
complete inventory and completed outcomes, using the saved current-run dismissal
snapshot. Exact matched dismissals exempt missed findings from the policy without
changing execution or outcome totals. Unresolved dismissals never exempt findings.
Focused reruns use freshly matched dismissals and evaluate only their selection;
empty runs pass and listing never evaluates the policy.

`run.json.policy` distinguishes `disabled`, `not_evaluated`, `passed`, and `failed`.
Passed/failed decisions include undismissed/dismissed missed counts. Text summaries
show the same decision. The coordinator finishes execution and persists this
decision without converting a findings failure into an execution error. The CLI
returns `3` only after the final report flush succeeds; execution/persistence errors
return `1`, CLI argument errors use `2`, and success uses `0`. Terminal restoration
precedes the final summary and policy exit. Baseline/tool failures and interruption
leave enabled policies unevaluated. Policy fields are additive; older reports
remain readable, and reruns do not reuse their policy decision.

Raw mutation scores and an independent minimum-score policy are implemented in
`score.rs`. Score is `100 * killed / (killed + missed)`. Timeouts and build failures
are excluded and exposed as counts, not counted as kills. Review decisions never
alter this ratio; matched dismissals among scored outcomes have a separate count.
Only successfully completed inventories with no unfinished/error outcomes have a
percentage. Zero denominators have no score. Failed/interrupted runs retain counts
but never present a partial percentage as a final result.

`min_score` defaults to disabled and merges from repository configuration with
`--min-score` and `--clear-min-score` overrides. Accept 0 through 100 with at most
two decimal places, represented internally as integer hundredths. Compare counts
using widened integer cross-products, independent of floating-point serialization
and two-decimal display rounding. An enabled threshold fails a completed run with
no killed/missed outcomes, even at minimum zero. Errors and interruption leave it
unevaluated. Either enabled CI policy may fail a completed run; execution and
persistence errors still take precedence. Listing evaluates neither policy.

Every report checkpoint includes `run.json.score` and `score_policy`, with the
decision finalized only on successful completion. Score scope distinguishes all
discovered mutations, file/operator-filtered selections, and reruns. Reruns always
score only their fresh selection; full/affected test scope is a separate axis.
The report reader recomputes from validated outcomes and saved dismissal matches,
checks scope/configuration and gate decisions, and tolerates only tiny JSON
floating-point representation differences in the derived percentage. Counts are
authoritative. These additive fields need no schema bump; legacy reports show
score as not recorded.

The final plain summary and saved text expose the score, counts, exclusions, and
selection scope. Browser `m` opens a cached score detail view using the existing
Ratatui layout and terminal lifecycle. Browser filters/marks never change the
score. Regression coverage includes exact thresholds, empty and partial runs,
dismissals, combined policies, scope/reruns, report validation/persistence errors,
compact/wide frames, and terminal restoration.

Consider integration, HTML, or interchange exports when there is a concrete
consumer. Individual missed mutations remain the main local finding.

### 5. Optional language plugins

Much later, consider allowing new language implementations outside the main binary,
possibly through WASM or another plugin mechanism. First establish the shared
interface through multiple built-in languages.

A plugin might provide discovery, mutation candidates, command descriptions, and
result interpretation. The host should retain control of workspaces, process
execution, deadlines, and report persistence. Protocol/ABI versioning, filesystem
access, execution permissions, packaging, and failure handling need a separate
design. No runtime or plugin ABI is selected now.

## Implementation checkpoints

1. Preserve the tested prototype as the starting point — completed.
2. Establish the inventory/outcome model and reporting layout, including pending
   work and explicit interruption semantics — completed.
3. Add `mutie.toml` merging and effective-config reporting — completed.
4. Add the coordinator state boundary and minimal default terminal status view,
   retaining plain mode and testing terminal restoration — completed.
5. Validate the MVP's complete workflow, including config precedence, parallel
   execution, failed/empty runs, cancellation, output persistence, and source-tree
   preservation — completed on the fixture and `godotenv` v1.5.1.
6. Add missed-only saved-run selection, source validation, fresh execution, and
   report provenance — completed and verified with improved tests on the fixture
   and `godotenv`.
7. Add ID/operator selection and a saved-report browser with filters, diff/log
   inspection, and command export — completed with renderer and PTY coverage.
8. Add repository-owned dismissal storage, CLI review commands, and strict
   rediscovery checks — completed with persistence and execution-preservation coverage.
9. Capture dismissal decisions in saved reports and expose browser review filters,
   details, and unresolved entries — completed with reader validation, renderer,
   PTY, rerun-remapping, and real-Go coverage.

Open implementation choices include dismissal editing in the browser, report archival,
run-wide resource budgets, mutation matching across source edits, and which language
to add next. Resolve these when working on the corresponding milestone and update
this document; they do not require expanding
the initial MVP.

## MVP validation evidence

Validated on 2026-10-08 using the locally cached `github.com/joho/godotenv` v1.5.1
source, Go 1.27.2, and macOS arm64. The starting Mutie commit was `cc92822`; the
validation also produced the compact-layout fix described below. Runs used a
writable temporary copy, a dedicated Go build cache, a five-second mutation
deadline, and the full module test suite. Repository configuration set two
workers and plain output; explicit CLI overrides selected six workers and the
interactive view. The source copy and cached original retained matching SHA-256
file manifests after execution. No external project was edited.

| Run | Workers | Baseline | Total | Result |
| --- | ---: | ---: | ---: | --- |
| Configured plain mode | 2 | 9.85 s | 54.88 s | 77 completed |
| Interactive mode | 6 | 1.60 s | 25.45 s | 77 completed |
| Interactive after layout fix | 6 | 2.07 s | 29.15 s | 77 completed |
| Ctrl-C during parallel work | 6 | 1.38 s | 4.52 s | 2 completed, 6 interrupted, 69 pending |
| Empty include scope | 0 | 2.19 s | 3.08 s | Complete empty inventory |
| Deliberately failing baseline, separate copy | — | 2.00 s | 2.24 s | Failed run, incomplete inventory, retained logs |

All three full runs matched mutation-by-mutation: **53 killed, 18 missed, 3 timed
out, 3 build failed, 0 tool errors**. Runtime numbers are observations, not a
controlled concurrency benchmark: the first run used a cold build cache, and
other validation work also ran on this machine. Each complete report occupied
about 3.56 MB in 323 files; structured test output accounted for about 2.94 MB.
Report checks covered inventory fingerprints, diff/log existence, checkpoint
agreement, summary/outcome-list counts, and absence of running mutations in a
finalized run. Listing created no reports.

The interactive CLI ran in a pseudo-terminal with captured ANSI output and text
screen reconstructions. The capture included 100×24 and 48×7 sizes, real Go
workers, progress between completions, and actual Ctrl-C input. Terminal settings
were restored on exit. At 48 columns, the original view clipped the error count;
shorter outcome labels and worker prefixes fixed this. That revision also added
four worker rows plus an overflow count when space permits. Regression frame
tests and another complete real-project run verify both changes. This validates
the exercised terminal protocol and layouts, not every terminal emulator.

The subsequent live-view refinement was verified in another six-worker PTY run
at 100×24 and 48×7. All 77 outcomes matched the original run; total run time was
28.53 s. Captured frames showed all six numbered rows at normal size, live
preparing/running/testing states, idle rows retaining their positions, a results
tail below the workers, and compact overflow/totals after resizing. Terminal
settings were restored. Deterministic PTY tests additionally held build and test
events open to verify both labels before completion; unit tests cover interleaved
packages, stale progress after worker reuse, bounded line decoding, and
cancellation/deadlines while progress delivery is blocked.

Prompt-preservation regression tests seed a PTY with earlier output and a shell
prompt and interpret the CLI's ANSI stream with `vt100`, including cursor-query
replies and scrollback. They cover width shrink, height shrink/growth, completion,
Ctrl-C, prompts near the screen bottom, and views taller than the screen. Height
changes model scrolling to retain the cursor and bringing history back into view
on growth. Width changes do not model emulator-specific reflow. Narrow 14×2 and
wide 512×1 cases exercise small layouts separately because `vt100` cannot wrap
text on a one-row grid. These are protocol regressions, not validation of every
terminal emulator's resize behavior.

Inspecting saved missed-result lists, diffs, and logs yielded concrete test gaps:

| Surviving mutation | Missing behavior check | Verification in an isolated copy |
| --- | --- | --- |
| `godotenv.go:56`, invert `Load`'s early-return condition | Successful loading continues to the second file | Added two-file test kills the mutation |
| `godotenv.go:79`, invert `Overload`'s early-return condition | Both files override existing environment values | Added two-file test kills the mutation |
| `godotenv.go:147`, return early after successful serialization | `Write` actually creates the output file | Added file-content test kills the mutation |
| `godotenv.go:151`, return early after successful file creation | `Write` writes content into the created file | The same file-content test kills the mutation |

The full suite with those added tests passed against unchanged source. Each
selected mutation then caused the expected new test to fail. The initial replay
used a validation script that checked saved source fingerprints and byte ranges
before editing the temporary copy.

The implemented `--rerun-missed` command was subsequently verified against that
original saved run and the copy with improved tests. It selected all 18 missed
mutations, ran a fresh passing baseline, and produced **4 killed, 14 missed** with
no timeouts, build failures, or tool errors. Source IDs 9, 11, 14, and 15 were the
four newly killed mutations above. With six workers and the warm dedicated cache,
the baseline took 1.29 s and the run took 7.73 s; this is an observation, not a
controlled benchmark. SHA-256 manifests confirmed that the input project and all
original report files were unchanged. The new report contains only the selected
mutations and their old-to-new ID mapping.

Missed mutations still require interpretation. All six mutations in the CLI
package survived, and that package has no tests. The `parser.go:146` boundary
change from `i >= 0` to `i > 0` is equivalent: its loop body already gates changes
on `i > 0`. Build failures included two invalid string-subtraction mutations and
one contradictory condition rejected by Go vet. These observations support
focused reruns and clearer inspection before adding more mutation operators.

Missed-only reruns complete this tested feedback loop. Focused ID/operator reruns
are additionally covered by deterministic selection tests and the real-Go
fixture. Saved-result review is covered by deterministic frame/state tests and
PTY tests on macOS, independently of Go and the original checkout. Performance
tuning and report-retention decisions require workload-specific evidence. The
[subsequent go-cmp measurements](benchmarks/go-cmp-v0.7.0.md) cover a larger
inventory and report; neither library run establishes general scaling claims.
