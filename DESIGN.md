# mutie design

Status: working design, 2026-10-08. This document records the direction for the
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
complete mutation inventories, saved reports, missed-only reruns, and coordinated
cancellation with retained command diagnostics.

The fixture and tests exercise killed and missed mutations, compilation failures,
timeouts, failed baselines, workspace restoration, and preservation of partial
results. MVP validation on `godotenv` v1.5.1 also demonstrates actionable test gaps
and practical runtime on a small library. It does not yet establish useful
mutation coverage, acceptable noise, or runtime on a large real project.

The prototype supports rerunning missed outcomes from a saved run; selecting
individual saved IDs or operators and interactive result browsing remain planned.
Rust appears in the CLI language enum but is not implemented.

## Minimal MVP

The initial deliverable is a useful local Go tool with a small, predictable scope:

- One self-contained Go module, initially targeting Linux/Unix execution.
- `go test ./...` for the baseline and every mutation; no custom test runner yet.
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
MVP reports counts without enforcing a mutation-score threshold. Completed local
runs can succeed with missed mutations; baseline/tool failures exit unsuccessfully.
CI score definitions and exit policies are later work.

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

These describe the small initial catalog, much of which already exists. Each
mutation makes one change. Discovery order and operator names should be
predictable, with tests for exact edits and source locations. Exclude test files,
generated code, and dependencies by default. Let the language tool determine
which files belong to the active build.

Tree-sitter supplies syntax, not a complete type system. Some syntactically valid
edits will be unbuildable, such as changing string addition to subtraction in Go.
Keep those results visible and separate; improve noisy operators based on evidence.
Do not attempt general equivalent-mutation detection in the MVP.

Later candidates include dropping standalone function calls, replacing return
values, and changing branches or loop behavior. These need language-specific
eligibility rules: removing a standalone call differs from removing a call whose
value is used, and can change side effects or make code invalid. Add operator
selection and named profiles as the catalog grows. Favor understandable findings
over generating the largest possible number of mutations.

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
`--clear-exclude` remove the respective configured patterns. `--clear-timeout`
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
handling. Full logs are still read and saved after process-group cleanup.

Keep noisy test output in artifacts rather than interleaving it with screen
updates. After completion, restore the terminal and print a durable plain summary
and report path. Verbose captured output is printed only in plain mode. The live
view leaves input in normal mode between cursor-position queries and uses the
existing signal-driven cancellation path. A terminal write failure cancels and
drains execution before finalizing the report; terminal cleanup also runs on
errors and panics. A later view can support selecting a result, scrolling its diff
and logs, filtering missed mutations, and requesting focused reruns.

The UI renders coordinator state; it does not own workers or decide outcomes.
Typed events feed the report writer, plain output, and a compact run-state
projection. A watch channel shares the latest snapshot with the renderer; it
retains counts and bounds recent display history without dropping persisted
results. Report saving works independently of presentation.

## Saved reports

Borrow the separation of mutation inventory, execution results, diffs, logs, and
outcome lists from the
[`cargo-mutants` output directory](https://mutants.rs/mutants-out.html).
Use a mutie-specific, versioned schema for now. Elements-compatible JSON, HTML,
and other interchange formats are possible future exporters.

Current contents of a single run directory (schema version 1):

```text
run.json           effective configuration, versions, command, timing, run state
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

Retention is a separate policy from report format. For the minimal MVP, retain
isolated run directories as today. Bounded retention, such as latest plus previous
with explicit archival, is a candidate follow-up. Decide its naming, concurrency,
and pruning rules before enabling automatic deletion. Never prune active runs or
unrelated user directories.

## Component boundaries

Keep the implementation modular without building a plugin framework up front.

| Component | Responsibility |
| --- | --- |
| CLI and config | Resolve project, options, and presentation mode into one run configuration. |
| Language support | Discover active files, generate language-specific mutations, define test commands, and interpret results. |
| Coordinator | Baseline gate, mutation inventory, scheduling, cancellation, and authoritative run state. |
| Workspace and process runner | Snapshot/copy/reset, apply edits, execute commands, enforce deadlines, and capture output. |
| Report writer | Persist inventory, results, metadata, diffs, and logs. |
| Rerun selection | Validate saved reports and match missed outcomes to current discovery. |
| Terminal presentation | Show state and forward user control requests. |

`coordinator.rs` owns a small event/state boundary: phase changes, command results,
inventory publication, mutation starts and completions, and run termination.
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

`--rerun-missed RUN_DIR` now selects completed missed outcomes from a finalized
saved run. The reader validates schema/catalog versions, final status, matching
checkpoints, a complete inventory, counts, IDs, source paths, and execution states
before creating a new run. Finalized failed/interrupted reports are usable when
they have a complete inventory; pending/interrupted executions are not selected.
The prior module root need not match the current checkout's location.

Execution uses current settings and the ordinary snapshot, fresh baseline,
discovery, and worker lifecycle. Scope filters apply before matching; every saved
missed target must match uniquely or the run fails before any mutation executes.
New mutations are not implicitly added. `--list` validates and previews without a
baseline or report, as for normal listing. Empty selections produce a completed
empty run after baseline and discovery.

New reports preserve the source run. After inventory publication, `run.json.rerun`
records the source path/checkpoint/status and old-to-new ID mapping, also shown in
the text summary. The source path is recorded in effective configuration even if
execution fails before inventory publication. Snapshot copying excludes the source
run directory; output paths inside that run are rejected. The report and operator
catalog versions remain unchanged during local development.

Extend selection to individual saved IDs and operators, and add interactive
diff/log browsing on top of the same report and run-state model. A prior report
supplies a selection, not proof that an old result remains valid. More flexible
matching across source edits needs an explicit policy and validation before it
can replace the current strict identity check.

Add persistent dismissals for individual mutations as part of result review.
Require a reason, such as "equivalent behavior" or "accepted test gap", with room
for an explanatory note. Store these decisions in repository-owned metadata that
can be version-controlled, independently of generated run reports. Users must be
able to inspect, change, and remove dismissals.

A dismissal needs a source-aware identity, not just a run-local mutation ID or
line number. Reuse the rediscovery and matching rules for focused reruns, taking
the source path, operator, original/replacement text, and source context into
account. Revalidate matches after source changes. Surface stale, missing, or
ambiguous matches for review instead of silently applying the old decision to a
different mutation. Choose the exact storage format and matching policy when
implementing this feature.

Keep review disposition separate from execution state and outcome: a dismissed
missed mutation still has a missed outcome. Reports and result views should show
the dismissal and its reason, distinguish dismissed findings from outstanding
ones, and allow dismissed findings to be included when inspecting or selecting
reruns. A dismissal records a human judgment; it is not automatic proof of
equivalent behavior or a passing test result.

### 2. Real-project validation and measured performance work

Try a modest real Go project and measure runtime, invalid/equivalent-looking
mutations, and whether findings lead to useful tests. Separate snapshot/reset and
command timings; distinguish compilation and test costs where the tool provides
reliable measurements.

Full-module tests remain the correctness reference. Explore affected-package tests
only with reverse dependencies accounted for, then coverage-based selection and
result reuse with explicit invalidation rules. Consider workspace-copy optimizations
without giving up restoration of test side effects. Measure before choosing which
optimization to implement.

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

Add explicit failure policies and score definitions, predictable exit codes,
report retention controls, and integrations driven by saved results. Consider HTML
or interchange exports when there is a concrete consumer. Avoid making a score the
only useful output; individual missed mutations remain the main local finding.

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
   and `godotenv`. Broader selection and richer result review remain follow-ups.

Open implementation choices include richer result browsing, report retention
policy, mutation matching across source edits, and which language to
add next. Resolve these when working on the corresponding milestone and update
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

Missed-only reruns now complete this tested feedback loop. Richer result review
and selection remain workflow improvements. Performance tuning and
report-retention decisions need larger-project measurements; this validation
establishes the small-project workflow, not general scaling claims.
