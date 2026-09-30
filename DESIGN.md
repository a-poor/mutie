# mutie design

Status: working design, 2026-09-30. This document records the direction for the
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
workers and timeouts, repository configuration, plain terminal output, complete
mutation inventories, saved reports, and coordinated cancellation with retained
command diagnostics.

The fixture and tests exercise killed and missed mutations, compilation failures,
timeouts, failed baselines, workspace restoration, and preservation of partial
results. This establishes the basic mechanics. It does not yet establish useful
mutation coverage, acceptable noise, or runtime on a large real project.

The prototype does not yet provide an interactive terminal view or support
focused reruns. Rust appears in the CLI language enum but is not implemented.

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

Focused reruns complete the longer-term feedback loop, but are the first follow-up
to this smaller MVP. Rich report browsing, advanced mutations, coverage-driven
execution, additional languages, multi-module projects, and plugins are not MVP
requirements.

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

Run-local IDs are not stable identities across edits. Later reruns must rediscover
mutations against the current source and validate the match. Do not replay stale
byte offsets or silently reuse an old outcome after the source or tests change.

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
not. UI settings will be added with the terminal view. A separate environment
configuration layer is unnecessary for the MVP, although child tools still use
their normal language environment.

## Terminal experience

The default target is an interactive status view when the terminal supports it.
[Ratatui](https://ratatui.rs/) is the leading implementation candidate, not a
required engine dependency. Use plain output when output is redirected, in CI,
or when the user explicitly disables the UI. A proposed `ui` setting and matching
CLI override select `auto`, `interactive`, or `plain`.

The minimal view shows the current phase, elapsed time, completed/total mutations,
outcome counts, active worker locations, and recent results. Support small
terminals and resizing. Cancellation is the only necessary control initially;
pause/resume and changing the work queue interactively are later features.

Keep noisy test output in artifacts rather than interleaving it with screen
updates. After completion, restore the terminal and print a durable plain summary
and report path. A later view can support selecting a result, scrolling its diff
and logs, filtering missed mutations, and requesting focused reruns.

The UI renders coordinator state and sends control requests. It does not own
workers or decide outcomes. Plain output and the interactive view consume the
same state; report saving must work independently of either. Terminal refreshes
can be coalesced, but completed results cannot be dropped.

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
| Terminal presentation | Show state and forward user control requests. |

Introduce a small event/state boundary as the UI is added: phase changes, mutation
starts, mutation completions, and run termination. Keep process output in files and
avoid an unbounded backlog of log contents in the UI. Use typed domain records
rather than parsing human-readable progress messages.

Some Go-specific responsibilities currently live directly in `mutation.rs` and
`go.rs`. Keep a clear boundary now, and extract a shared language interface when
implementing a second language. Avoid speculative abstractions with only one
working implementation.

## Work after the MVP

### 1. Focused reruns and richer inspection

Select mutations by ID within a saved run, by file/operator, or by previous missed
outcomes. Rediscover them on the current checkout, run a fresh baseline, and reject
missing or ambiguous matches explicitly. A prior report supplies a selection, not
proof that an old result remains valid. Add interactive diff/log browsing on top of
the same report and run-state model.

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
   retaining plain mode and testing terminal restoration.
5. Validate the MVP's complete workflow, including config precedence, parallel
   execution, failed/empty runs, cancellation, output persistence, and source-tree
   preservation. Then begin focused reruns and real-project validation.

Open implementation choices include the precise UI layout and Ratatui backend,
report retention policy, cross-run mutation matching rules, and which language to
add next. Resolve these when working on the corresponding milestone and update
this document; they do not require expanding
the initial MVP.
