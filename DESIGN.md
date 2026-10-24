# mutie design

Status: working design, 2026-10-11. This document records the direction for the
project, including work that is not implemented yet. [README.md](README.md)
documents the current CLI and behavior; [TODO.md](TODO.md) tracks implementation
progress. Update this reference as decisions change.

[SUPPORT.md](SUPPORT.md) is the target support ledger: it defines maturity levels
and tracks verified scope separately for Go/go test, Rust/Cargo, and the
JavaScript/TypeScript runners (Vitest, Node and Bun) and Python/pytest. New runner
support requires its own scope and validation evidence even when language parsing
or shared reporting can be reused. The ledger is not an implementation ordering.

## Public release direction (2026-10-22)

The [release scope and ordered goals](TODO.md#public-release-scope-and-goal-sequence-2026-10-22)
are the current planning authority over historical next-step notes below. The
public release may remain `0.x`; all existing languages/runners are first-class
within explicit tested scopes on macOS/Linux. Expand operator coverage, including
numeric/string literals, ordinary Rust workspaces and JS/TS package-manager
compatibility, alongside usable setup and unchanged-project acceptance.

`mutie init` now distinguishes language syntax, test runner, package manager
and runtime version, infers only unambiguous choices, and supports explicit and
noninteractive configuration. TOML/CLI overrides retain existing precedence.
Runner flags require validated semantics and saved execution identity; arbitrary
commands or flags must not bypass complete test inventory or failure attribution.
Source filters and test selection are separate concepts. See the
[onboarding contract](benchmarks/project-init.md) for implemented setup and future
runner-option/operator-exclusion semantics. Exact-byte observers
mean runner upgrades require attribution/regression validation, not just matching
exit codes. The [package-manager audit](benchmarks/package-managers.md) separates
npm/pnpm/Yarn/Bun installation validation from test-runner selection. Vitest now
resolves its exact-byte worker through owned dependency ancestry and binds installed
content to snapshots/reruns. Node/Bun retain declared development installations
without admitting runtime dependency imports. Vite+ installation delegation is
separate from its unsupported bundled `vp test` runner; alternative linkers and
workspaces still need their own acceptance.

Ordinary Rust workspaces now use root-relative identities and per-member production
inventories. `rust_packages` selects mutation members; execution explicitly tests
all workspace members. Worker caches clean every local package to prevent stale
internal-dependency results. Root/member manifests, lockfile, selection and toolchain
bind focused reruns. [Workspace acceptance](benchmarks/rust-workspaces.md) supersedes
historical workspace exclusions below; build scripts, proc macros, external paths
and custom dev/test profiles remain outside this scope.

## Goal

Mutie is a Rust CLI for finding gaps in tests by making small changes to source
code and checking whether the tests detect them. The intended local workflow is:

1. Run mutation testing on a project or selected source files.
2. Inspect a missed mutation and understand the behavior the tests do not check.
3. Improve the tests.
4. Rerun the relevant mutations and verify that the tests now catch them.

The Go MVP and basic CI policies are implemented. Rust is the next language,
chosen because the maintainer has real Rust projects to use for validation. Use
`cargo-mutants` as a reference for understandable terminal output and inspectable
report artifacts, without requiring compatibility with its CLI or file formats.

## Current position: Go MVP complete

The established Go implementation supports one module, selected through its `go.mod`.
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
Rust now has an initial binary-package path through metadata validation, a passing
baseline, boolean-flip discovery, isolated Cargo execution and saved reports.
Focused Rust reruns now reuse shared source matching and ID provenance, with
Cargo execution-scope checks before the baseline. Rust dismissals now support
CRUD, pristine-source rediscovery, saved review and independent CI policies.

The project is a usable Go-focused alpha. The execution, inspection, and focused
rerun workflow is established; further Go operators and presentation refinements
are optional improvements rather than prerequisites for the next milestone.
Broad adoption and large-repository scaling remain unproven.

## Completed milestone: initial Rust support

The accepted initial milestone delivers the same test-improvement workflow on
the maintainer's supported Rust binary package: passing baseline, mutations in
isolated copies, live progress, saved findings, review, stronger tests, and focused reruns. Adding a
parser or recognizing `Cargo.toml` alone would not satisfy this milestone.
The [acceptance review](benchmarks/rust-milestone-acceptance.md) maps each criterion
to evidence and records the remaining limits.

### Scope and sequence

1. Select a small Rust fixture and a representative maintainer-owned project.
   Inspect their Cargo package/workspace layouts, test targets, features,
   conditional compilation, build scripts, and local dependencies. Record the
   initial support contract and unsupported cases before broad implementation.
   Start the first end-to-end slice with one Cargo package; include workspace
   member selection in this milestone if the chosen real project requires it.
   General workspace support is not implied by recognizing a member package.
2. Extract the smallest shared language boundary needed by Go and Rust while
   building that slice. Separate project/file discovery, mutation generation,
   command construction, and result interpretation from scheduling, workspaces,
   reports, and presentation. Route one Rust operator through discovery,
   baseline, isolated execution, and a saved report before expanding the catalog.
   Keep the existing Go behavior covered throughout; avoid a separate framework
   refactor or plugin architecture ahead of the working Rust path.
3. Establish reliable Cargo build/test execution and source selection. Define
   how compilation failures, test failures, tool errors, timeouts, and
   interruption map to existing outcomes. Verify that repeated and parallel
   mutations execute the intended tests without stale builds or shared artifact
   collisions. Specify target-directory/cache ownership and cleanup separately
   from source snapshots; preserve the input project's files and build artifacts.
   Define treatment of test-only code, generated sources, macros, and inactive
   conditional code, making unsupported cases explicit.
4. Add a small useful Rust operator set, driven by syntax fixtures and actual
   findings rather than parity with every Go operator. Reuse configuration,
   scope filters, live/plain output, saved reports, scores, operator totals,
   dismissals, and focused reruns. Make project language and execution settings
   unambiguous in saved runs and rerun validation. Keep existing Go reports
   readable; do not bump package, report, or catalog versions for ordinary
   development changes.
5. Validate the complete workflow on the fixture and the selected Rust project.
   Cover failed baselines, empty selections, all outcomes, cancellation,
   source preservation, saved-report inspection, and a stronger-test rerun.
   Measure time to an actionable finding, full/focused run time, build and
   workspace costs, and the usefulness of reviewed survivors. Record equivalent
   mutations and build-failure noise as well as successes. Use ordinary Mutie
   commands for the user workflow, and document evidence and limitations.

### Initial Rust support contract

The first real validation project is **Mutie itself**, selected by the maintainer.
The small companion package is [`tests/fixtures/rust`](tests/fixtures/rust/README.md).
Both have one ordinary binary and standard unit/integration tests. Mutie has no
package features, local path dependencies, root build script, or library target;
workspace-member selection is not required for this milestone's selected project.
The [scope audit and Cargo probes](benchmarks/rust-initial-scope.md) record the
inspected revision and planning evidence. The first execution slice now implements
the contract below; see the fixture README and `tests/rust_workflow.rs` for verified
behavior. Acceptance is limited to this initial contract.
The [first-slice validation record](benchmarks/rust-first-slice.md) covers actual
CLI runs, regression checks and observed self-testing costs/limitations.
The [focused rerun record](benchmarks/rust-focused-reruns.md) extends that evidence
with scope validation, browser export and the test-improvement workflow.

#### Package and test execution

Start with a package-root `Cargo.toml`, a checked-in usable `Cargo.lock`, exactly
one ordinary binary target, and the standard Rust test harness on a Unix host.
Use the host toolchain, default package features, and the normal test profile.
Reject multi-package/virtual workspaces, membership in an enclosing workspace,
path dependencies/overrides, library/proc-macro targets, custom harnesses, and
project build scripts for this first slice. An explicit single-package workspace
root is acceptable, as used by the fixture. Registry dependency build scripts and
derive macros remain necessary to compile Mutie; dependencies are never mutated.

Discover package/target roots using `cargo metadata --format-version 1 --no-deps
--locked`; validate the dependency sources and layout before executing tests.
Metadata describes targets, not a complete list of compiled Rust source files.
[Cargo's metadata reference](https://doc.rust-lang.org/cargo/commands/cargo-metadata.html)
documents the fields to inspect. Require package identity and all selected source
paths to remain inside the snapshot, with no references back to the input tree.

The test scope is `cargo test --locked --tests --no-fail-fast`, launched from the
isolated package root. This includes the binary's unit tests and its integration
tests, which must exercise that copy's binary through `CARGO_BIN_EXE_<name>`.
It excludes ignored tests, doctests, examples, and benchmarks in the supported
layout. Reject nonstandard target declarations that would silently widen that
scope. No test-name filters, feature selectors, cross-compilation, alternative
profiles, or custom runners are part of the first slice. Cargo documents the
[`--tests` selection and binary environment](https://doc.rust-lang.org/cargo/commands/cargo-test.html#target-selection).
Do not translate Go's affected-package option into a partial Rust test run;
reject it for Rust and record the actual Cargo selection in saved reports.

Compile the selected tests first with the same scope plus `--no-run
--message-format=json`, then execute through Cargo. Use compiler diagnostics and
build completion evidence to separate compilation failures from test failures;
Cargo's exit code alone is insufficient. Missing tool/protocol evidence is an
error, not a kill. Preserve both phases' diagnostics and use one overall deadline
per baseline or mutation. Compilation/test/tool evidence, compilation/test
timeouts, baseline cancellation, and saved failure diagnostics are covered.
Real-Cargo regressions cancel baseline compilation and interrupt parallel mutation
builds or serial/parallel mutation tests after one result has completed:
completed results/logs survive, active work becomes interrupted, queued work stays
pending, and the partial report passes the shared reader. Both capture paths
retain active diagnostics; Cargo, test runners and helper descendants stop, owned
workspaces/build/test directories are removed, and source/caller artifacts stay
unchanged. A baseline compilation timeout fails the run; a mutation compilation
timeout records `timeout`. Neither starts test execution. A controlled delay
after a real build verifies that testing receives only the remaining deadline;
resetting the test-phase budget makes that regression fail. Test-only PATH
proxies provide these checkpoints without enabling user compiler overrides or
project build scripts. Fault-injection regressions around real Cargo commands
cover startup failure (including after a completed build), unexpected exit codes
and signals, missing/malformed completion evidence, invalid metadata, and raw
capture overflow. They verify partial reports, peer shutdown, pending work,
unevaluated policies, and input/artifact preservation. Metadata capture failures
are reported from the underlying error independently of retained log excerpts.
Cargo's [machine-readable messages](https://doc.rust-lang.org/cargo/reference/external-tools.html#json-messages)
provide build evidence, not stable structured libtest result events.

#### Source eligibility and Rust operators

Rust supports `boolean_flip` (`true` ↔ `false`), `comparison_invert`,
`negation_remove`, `logical_swap` (`&&` ↔ `||`), `arithmetic_replace`
(binary `+ ↔ -`, `* ↔ /`, `% → *`), and `comparison_boundary`
(`<` ↔ `<=`, `>` ↔ `>=`) in ordinary function/method bodies, including closures
and match guards. The first slice began with boolean flips; the boundary operator
reuses the shared catalog name, filters, identities, reports and focused reruns.
Only the parsed binary-expression operator token changes. Operands, precedence
and borrowing remain intact; generic and overloaded comparisons are eligible.
Rust's [comparison operator reference](https://doc.rust-lang.org/reference/expressions/operator-expr.html#comparison-operators)
defines the corresponding `PartialOrd` methods. Custom implementations can make
the semantic difference broader than primitive equality; discovery does not
perform type analysis or prove equivalence. Equality/inversion, shifts, ranges
and generic angle brackets are not comparison-boundary candidates.

Logical swaps replace the parsed boolean binary expression, preserving trivia
and adding only the parentheses required by the new precedence. Each operand
appears once and evaluation order is retained; which right-hand operands execute
changes according to Rust's [lazy boolean operators](https://doc.rust-lang.org/reference/expressions/operator-expr.html#lazy-boolean-operators).
Do not mutate `let_chain` connector tokens: `||` cannot replace these bindings.
Ordinary boolean operands remain eligible, with a swapped disjunction grouped
inside a chain as required by the [condition grammar](https://doc.rust-lang.org/reference/expressions/if-expr.html).
Bitwise operators, references, closures' delimiters and excluded source remain
outside this operator. Overlapping nested expressions remain independent mutations.

Walk the selected binary's ordinary external and inline module declarations;
do not glob every `.rs` file in the repository. Deduplicate canonical source
locations and apply the existing file filters after determining eligibility.
Exclude integration-test targets, test functions/modules, dependencies, nested
packages, unreferenced files, and files with an `@generated` header.

For the first slice, conservatively skip subtrees with `cfg` or `cfg_attr`, even
when a branch happens to be active on this host. Carry exclusions through module
declarations and file-level attributes. This intentionally excludes active
platform-specific code as well as test-only/inactive code; report that limitation
instead of counting inactive changes as missed tests. Conditional attributes can
occur below item level, so statement/expression exclusions also need coverage.
The [Rust conditional compilation reference](https://doc.rust-lang.org/reference/conditional-compilation.html)
defines the syntax; full cfg evaluation is deferred.

Skip macro definitions, invocation token trees, expanded code, and attribute
arguments. Ordinary source bodies annotated with macros (for example Mutie's
`#[tokio::main]`) can still be inspected; mutation discovery does not inspect their
expansions. `include!`-generated modules, `#[path]` module remapping, and generated
sources are outside the initial discovery contract and must be surfaced as
excluded/unsupported scope. Constants/statics, type-level expressions, array
repeat lengths and constant patterns are deferred. Match guards are executable
expressions, even though Tree-sitter nests them under the pattern field; only their condition is
visited. These rules yield the original fixture's three literals and the separate
`rust-boundaries` fixture's five comparisons. The boundary fixture covers all
four replacements, signed/unsigned integers, borrowed generic operands, floats
including NaN/signed zero, and operand side effects. Its missing equality case
survives until a stronger integration test is added, then fails a focused rerun.

#### Build artifacts, configuration, and preservation

Snapshot source, lockfile, tests, and required assets. Exclude `.git`, reports,
the package's `target/`, and any resolved Cargo target/build directory; refuse
overlapping source/artifact layouts rather than dropping source inputs. The shared
workspace copier does not automatically exclude `target/`; Rust setup supplies
these exclusions explicitly. In-project artifact paths outside `target/` are
rejected so a configured build directory cannot hide source or test inputs.
Resolve layout before copying, and run Cargo from the isolated package, never
the caller's source directory.

By default, give the baseline and **each mutation** fresh owned target/build
directories outside the resettable source copy. Never share
compiled artifacts between concurrent workers or reuse the user's outputs.
Set both final and intermediate output destinations explicitly, including child
environment overrides for nested Cargo invocations. Cargo has separate
[`build.target-dir` and `build.build-dir`](https://doc.rust-lang.org/cargo/reference/config.html#buildbuild-dir)
settings; overriding only one is insufficient when both are configured. Give
child commands owned `TMPDIR`/`TMP`/`TEMP` directories, and clean up only owned
paths after all processes have exited, including timeout/interruption paths.

The opt-in `--rust-build-cache worker` mode retains dependency artifacts in a
`BuildCache` owned by each worker across jobs. The baseline remains fresh. Before
every mutation build, invoke `cargo clean --locked --package PACKAGE` against
that worker's owned directories from the restored workspace. Cargo removes the
package outputs, including binary and test artifacts; registry dependencies stay
available. No internal Cargo fingerprint paths or source timestamp assumptions
are used. Incremental compilation stays disabled. Source workspaces still reset
completely from the pristine snapshot before each mutation. Caches never cross
worker or run boundaries and drop only after active work has drained.

Cleanup, compilation and execution share a deadline, with one raw capture budget
per invocation. Failed cleanup is a tool error, timed-out cleanup is a timeout,
and neither starts compilation/tests. Preserve cleanup diagnostics alongside
later phase output, including a compiler startup failure. Record the selected
mode in effective configuration and the text summary. Reruns may select a
different mode without changing mutation identity; `fresh` remains the reference
and default. This is dependency artifact reuse, not reuse of test results.

Retain the normal Cargo registry/download cache; do not run `cargo clean` against
the source project or delete its caches. Require locked dependency resolution;
the audit runs offline from the existing cache, but offline-only operation is not
a new public CLI requirement. Report toolchain, package/target/features/profile,
lockfile identity and relevant execution settings so reruns can validate scope.
Existing Go reports must remain readable; no version bump is needed.

Focused Rust reruns compare package/binary/source/edition, full manifest and
lockfile fingerprints, default features, test selection, profile, and actual
rustc/Cargo version output against the captured current input. Language mismatches
and missing saved identity fail before tools start; scope changes fail after
metadata/version validation and before baseline execution. Early Rust reports
without manifest identity remain inspectable, but require a fresh scan to rerun.
Whole-file mutation identity still rejects any edit to selected production files,
including inline tests. Separate test files can change, and fresh discovery
diagnostics need not be identical. Current timeouts, workers, filters, limits,
and policies apply. Browser exports explicitly select Rust without executing it.

Project/ancestor Cargo configuration, compiler wrappers, custom rustflags/cfg,
toolchain override files and externally redirected sources receive explicit validation;
unsupported semantic overrides must fail with an explanation rather than be
silently discarded or read differently after copying. The audited projects do
not require these overrides. Artifact-location settings are overridden to owned
paths, not honored as write destinations. Network/terminal config sections are
allowed; other semantic settings are rejected. Inherited `RUSTUP_TOOLCHAIN` is
preserved (Cargo/rustup also sets it when launching Mutie), and actual compiler
and Cargo versions are saved. Do not claim general Cargo-config or custom-build
support from these two examples.

Fresh builds provide the reference for validating dependency reuse. The cache
regression backdates source and test files before compilation, checks local
targets rebuild while dependencies become fresh, and verifies restoration,
parallel isolation, build-failure recovery and stronger-test reruns. Disabling
package cleanup makes the regression fail with an incorrect kill. Cleanup
failure, timeout and cancellation checks cover partial reports and owned paths.
See `benchmarks/rust-build-cache.md` for the controlled Mutie comparison.
The first Mutie sample filters to
`src/score.rs` (two eligible boolean literals at the audited revision) while
running the complete selected test scope. Broaden only after inspecting its
outcomes and costs. Opt-in real-Go tests remain separate regression checks; future
nested Cargo fixture tests must not introduce recursive default self-tests.

### Completion and priorities

The initial milestone is complete for the contract above. The selected project
has demonstrated run → inspect → improve tests → rerun through the CLI, including
review, policies, strict source identities, reliable outcomes and source preservation.
Three operators satisfy the scoped catalog requirement. The acceptance review
records the full real-Cargo suite, Go compatibility evidence, historical failures
and remaining limits. General libraries/workspaces are separate future scope.

The first feedback-time objective followed Mutie samples taking roughly four to
five minutes for a one-finding rerun despite much faster warm compilation. Profile
full-suite costs before choosing an optimization, and validate changes against
unchanged saved selections and exact failure sets. Retain package rebuilds,
full tests, deadlines, cancellation behavior and source/artifact isolation. Test
selection or persistent reuse needs a separate design decision; neither is implied
by this objective. Follow explicit user priorities and fix demonstrated blockers.
Additional operators, languages and UI polish remain driven by concrete needs.

The [full-suite feedback profile](benchmarks/rust-feedback-profile.md) separates
adapter build/cleanup, test execution and individual harness durations across
cold/warm serial and parallel runs. Warm execution is dominated by test harnesses,
particularly dismissal, terminal and log-limit workflows. The measured residual
does not identify Cargo startup alone. The subsequent
[runtime experiment](benchmarks/rust-runtime-startup.md) found comparable warm CLI
startup and only a small dismissal-harness median difference with fewer Tokio
threads, so runtime defaults remain unchanged. The
[workflow probe](benchmarks/rust-workflow-costs.md) identified substantial report
persistence costs in focused and full dismissal tests, with measurable diagnostic
overhead. The report writer now compares derived text with
the actual published regular file and skips identical contents. Missing or edited
files are repaired; authoritative JSON checkpoints still publish on every update.
The [controlled comparison](benchmarks/derived-report-writes.md) records fewer
writes and lower prebuilt-suite latency. The
[saved-selection comparison](benchmarks/rust-report-write-reruns.md) completes
this first optimization's acceptance: the six-mutation self-run falls from 564
to 393 seconds in one pair, with identical failure sets, required rebuilds and
source/artifact preservation. This does not establish a universal speedup.
The ordinary Rust library-package milestone is complete within its scope. The
[scope audit](benchmarks/rust-library-scope.md) selects library-only and
library-plus-one-binary layouts, retaining unit/integration tests and excluding
doctests. Direct Cargo fixture probes and Marathon's 182-test isolated baseline
establish validation candidates. Target identity, library acceptance and multi-root
discovery are implemented. [Marathon validation](benchmarks/rust-library-marathon.md)
completes real-project acceptance: 15 initial mutations yield four survivors;
two stronger tests kill three in matching fresh/cached reruns, while an equivalent
survivor remains reviewed. This promotes the library slice to Validated, not
Established; the evidence is one combined library/binary project on macOS.
Workspaces and local dependencies remain separate objectives.

The first implementation step replaces the saved single-binary fields with a
sorted production-target inventory: kind, name, source and edition. Readers
normalize legacy binary identities; reports missing manifest/toolchain evidence
remain inspectable but cannot rerun. Mixed identity formats, malformed paths,
empty inventories and duplicate target kinds are rejected. Reruns compare the
complete target inventory, and current binary metadata must match its selected
identity and crate type. Manifest/metadata validation now accepts ordinary libraries
alone or alongside one binary, including explicit names/paths and `lib`/`rlib`
outputs. Discovery traverses all production roots and deduplicates shared source
mutations in stable file/range order. A shared file used both as a crate root and
a module is traversed in both module-resolution contexts. Standard harnesses
and enabled tests are required; doctests remain outside the execution scope.

## Original Go MVP — completed

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
- Branch conditions: force an `if` condition true or false (`branch_force_true`,
  `branch_force_false`).
- Conditional loops: skip all iterations of a `for` loop (`loop_skip`).

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

Branch forcing selects the complete `condition` field of each `if_statement`,
including `else if`, labeled statements, initializers, and nested function bodies.
After unwrapping parentheses and ignoring comments, a bare `true`/`false` is
excluded to avoid overlap with `boolean_flip`. This is syntactic eligibility;
shadowed bare boolean names are also excluded, and other constant expressions
are not evaluated. Loop and switch conditions are outside this operator's scope.

Replace `condition` with `(0 == 0) || (condition)` or `(0 != 0) && (condition)`.
Go's [short-circuit rules](https://go.dev/ref/spec#Logical_operators) skip the
original expression, including calls and receives, while its syntactic references
keep local variables and imports used. An `if` initializer still executes and both
branch bodies remain intact. The comparison guards produce
[untyped boolean constants](https://go.dev/ref/spec#Constant_expressions), compatible
with named boolean types and generic `~bool` operands, without referring to
shadowable `true`/`false` identifiers. This avoids unused-name repair edits and
does not claim general buildability or equivalent-mutation detection.

Loop skipping uses the same false guard for an explicit `for` condition.
Tree-sitter Go represents a single-condition loop with an expression child and
a three-part loop with `for_clause.condition`; select only those expressions.
Exclude range clauses, missing conditions (`for {}` or `for init; ; post {}`),
and bare `true`/`false` after unwrapping parentheses/comments. Like branch forcing,
this is syntax-only eligibility without name resolution or constant folding.

Replacing only the condition with `(0 != 0) && (condition)` preserves the init
and post statements and the body in the source. Under Go's
[for-clause semantics](https://go.dev/ref/spec#For_statements), the initializer
still runs once, but short-circuiting skips the original condition, all iterations,
and the post statement. References in the condition remain used, including locals
declared in the initializer. Labeled loops remain valid because the loop itself
is retained. Continue visiting the original tree so nested loops and other
operators remain independent, even inside an excluded outer loop.

Replacement text can now be owned for source-dependent edits; existing token and
call-removal replacements remain borrowed. Saved identities and inventories keep
the same string representation and exact original condition range. Discovery visits
the original tree only, so operators inside a condition remain separate mutations
and synthesized guards never create additional candidates.

All operators are enabled by default. Resolve repository `operators`, CLI
`--operator`, and `--clear-operators` before execution, normalize duplicates, and
save the effective list (empty means all). Match dismissals against the complete
discovered catalog before selecting operators, then remap only included matched
decisions to final IDs. `dismiss check` ignores operator restrictions so valid
decisions outside a run's selection remain checkable. Reruns apply the same merged
operator selection to the source inventory before strict current-source matching.

The [fzf branch sample](benchmarks/fzf-v0.65.2-branches.md) validated 24 mutations
at 12 conditions in both scopes: one killed, 23 missed, no build failures. Added
tests killed four survivors in both scopes; review also identified an equivalent
ASCII decoding shortcut. This is evidence for useful findings, not a general
noise-rate or killability claim.

The [fzf loop sample](benchmarks/fzf-v0.65.2-loops.md) validated 16 of 107
`loop_skip` mutations, with matching full/affected outcomes: five killed, 11
missed, no build failures. Added tests killed two survivors in both scopes,
covering partial tail chunks and display-width trimming. Debug-only code and an
ASCII fast path also survived; the sample does not make every miss a test gap.

Return-value, further loop/branch mutations, and named operator profiles remain
future work. Favor understandable findings over generating the largest possible number
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

Rust applies the configured raw budget independently to each Cargo metadata,
compilation and test invocation. Baseline/mutation reports merge the latter two
captures, including separators, so their combined raw retention totals can be
larger than one command's budget (at most two budgets plus separators). This is
independent of the single shared compilation/test timeout. Real-Cargo fault
regressions cover stdout, stderr and mixed overflow, discovery/listing and
baseline failures, parallel peer cleanup, and tiny retained logs. Unit checks
also require capture errors to override valid compiler/test failure evidence.

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

`t` opens the operator breakdown. `operator_summary.rs` groups inventory and
execution records by operator in catalog order, using the same formatter for
`summary.txt` and the browser. The reader joins records by saved ID before
aggregation; there are no new persisted fields. Counts cover the entire saved
selection, including pending and interrupted work, independently of filters,
marks, and dismissal metadata. Each operator shows planned/completed counts,
all outcomes, unfinished states, and cumulative mutation command time. Time
includes recorded interrupted commands, excludes pending work and non-mutation
commands/workspaces, and can exceed wall time when workers overlap. Missing,
negative, or non-finite timings count as unavailable, not zero; recorded zero
remains distinguishable from no timings, and sum overflow is labeled unavailable.
Unpublished inventories show an unavailable message; a published empty selection
shows no mutations. The derived view also works for older reports without scores
or dismissal snapshots, without rewriting their saved files.

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

Command spawning holds a shared mutex across pipe creation and child startup.
On platforms with non-atomic pipe close-on-exec setup, concurrent spawning can
otherwise leak a capture writer into an unrelated child and delay EOF beyond
the original group's lifetime. Execution and capture draining remain concurrent.

After a child exits, Unix process-group cleanup retries SIGKILL every 10 ms until
its group disappears, bounded to one second. A single group signal can miss a
descendant forking during delivery. Retire the group identifier on ESRCH; retain
a best-effort Drop signal otherwise. This applies to both piped and direct-to-file
captures. Orphan zombies can await an external reaper, so group cleanup cannot
wait indefinitely. Limited captures then retain their separate one-second EOF
drain bound and explicit incomplete-capture error. See
`benchmarks/process-reliability.md` for reproduction and checks. The subsequent
`benchmarks/rust-parallel-score-reruns.md` comparison verifies six intended kills
with matching failure sets in fresh and worker-cache modes after both capture
fixes; it records preservation checks and the limits of this sample.

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

Derived `summary.txt` and outcome lists are compared with their existing regular
files before publication. Identical contents need no replacement or sync; missing,
changed or symlinked files go through the existing atomic, synced publication path.
Read/metadata errors other than missing files remain errors. There is no in-memory
publication cache that could hide deletion or edits. Authoritative `run.json` and
`outcomes.json` are always rewritten with matching checkpoint numbers.

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
The owning guard explicitly unlocks before closing its descriptor: an inherited
or duplicated descriptor must not extend the writer/inspector's lock lifetime.
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

`language.rs` dispatches Go and Cargo test/discovery commands, while `command.rs`
shares captures, classification and progress phases. Go syntax remains in
`mutation.rs`; Rust project validation, module traversal and Cargo execution live
under `rust`. Workers, execution state and reports remain shared. Extend this
boundary only as the two implementations need it.

## Work after the MVP

The sections below retain completed follow-ups and deferred design options; their
order is not the current work queue. The focused-feedback objective above sets
the next priority.

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
require `go.mod` or `Cargo.toml` but no language tools or configuration loading,
and label source validity unchecked. They never alter the original report.
Manifest detection is shared with run configuration. `dismiss --lang go|rust`
disambiguates roots containing both manifests, and adding rejects a report from
a different language. CRUD does not enforce the complete Cargo execution contract;
nonempty Rust checks do. This keeps metadata editable without installed tools.

`identity.rs` shares exact source matching with focused reruns: module-relative
path, full-file SHA-256 fingerprint, operator, byte range, coordinates, and
original/replacement text must all agree. `dismiss check` loads repository
configuration, snapshots the project, runs Go package discovery or Cargo metadata
in a separate copy, and discovers mutations from pristine source. It runs no baseline or
mutation tests and creates no reports. Rust checks use the same project validation,
artifact exclusions and module discovery as ordinary runs; metadata side effects
cannot change identities or decisions. They honor timeout/capture limits and
cancellation with owned workspace/descendant cleanup. Empty stores skip discovery.
Test changes in separate files and moved checkouts preserve identity; edits
anywhere in a mutation's source file invalidate it. Missing candidates (including excluded/inactive files),
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

Each dismissal stores its language. Missing language defaults to Go for older
metadata/reports; `auto` is invalid in saved decisions. Matching also requires
the current language, and mismatches are retained as `language_mismatch` review
issues rather than applied. Saved-report readers validate this relationship
against the run's language. A dismissal records source judgment, not an execution
scope: Cargo manifest/lockfile/toolchain changes alone do not invalidate it.
Focused reruns continue to enforce their independent, stricter scope contract.

New runs load dismissal metadata from the pristine snapshot before language tools
start; invalid metadata fails the run at that point. Source-only `--list` does not load it.
After discovery, `DismissalSnapshot` partitions decisions into uniquely matched
entries and entries needing review. Matching happens before rerun filtering, so
valid decisions outside the selection are omitted without being called missing.
Selected matches are then mapped to the new inventory IDs. The plan event carries
the review to the report writer, which publishes it with the immutable inventory
before starting workers. Command side effects and later checkout changes cannot
alter the captured decisions.

`mutants.json` has an optional `dismissals` object with `matched` and `needs_review`
arrays. Matched records pair a run-local `mutation_id` with a full dismissal
(independent ID, language, reason, optional note, source identity). Review issues
pair a status with the same record. Readers validate required text, identities, unique
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

Rust is the second supported language within the accepted initial binary-package
scope described above. Further Go operators remain driven by observed gaps.
JavaScript/TypeScript and Python are later options; select either only after
evaluating routine Go/Rust use and a concrete project need.

Each language needs more than a Tree-sitter grammar: project/file discovery, useful
mutation rules, build/test integration, timeout behavior, and reliable outcome
classification. Add one language at a time using the same lifecycle and fixture
expectations. Decide multi-module and test-framework support separately from simply
recognizing a language.

#### Proposed operator expansion

The [current support matrix](README.md#mutation-operators) distinguishes implemented
operators by language. The following are proposals, not support commitments or an
automatic sequence of work:

- Extend comparison inversion and negation removal to additional languages to
  expose reversed validation/filtering decisions, beyond equality-at-a-cutoff gaps.
- Rust now has [scoped arithmetic replacement](benchmarks/rust-arithmetic.md) for formulas, counters and indexing.
  Python now has a [validated scoped binary slice](benchmarks/python-arithmetic.md);
  broader Python operators need separate semantic and real-project evidence.
- Consider a small numeric-literal replacement set (`0`, `1`, `-1`, or adjacent
  values) for defaults, limits and retry counts. Bound candidate growth and avoid
  duplicate or unchanged replacements.
- Consider return-value replacement with type-appropriate zero, empty string,
  empty collection or null values, and targeted string-literal replacement.
- Extend call removal or add carefully scoped statement/block removal to test
  persistence, notification and cleanup behavior. Measure build-failure noise and
  preserve lifecycle-error classification before broadening eligibility.

Select each extension using observed test gaps and runtime cost. Acceptance needs
language-specific semantic controls (operand evaluation, grouping, short-circuiting,
overloaded operations and error behavior as applicable), exact discovery/rerun
identity, and a useful unchanged-project finding killed by stronger tests in a
separate copy. Preserve original inputs, reports and scores; build/runtime/lifecycle
errors must not be relabeled as test kills. A larger operator catalog alone is not
milestone progress.

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
10. Extend the Go workflow with CI policies, affected-package testing, additional
    operators, report retention/output limits, and per-operator summaries —
    completed; details and validation are tracked in `TODO.md`.
11. Deliver usable Rust support through the scope, implementation, and real-project
    validation steps above — completed for the initial binary-package scope;
    see the acceptance review.

The initial Rust contract fixes Cargo scope, source eligibility and artifact
ownership, with execution reliability and three operators verified for that scope.
The first measured Rust feedback-time optimization and scoped ordinary-library
milestone are accepted. The next objective is the Vitest slice described below.
Dismissal editing in the browser, report
archival, run-wide resource budgets, and mutation matching across source edits remain deferred
choices; they do not block acceptance of the initial Rust scope.

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

## Scoped Vitest support and remaining work

The [scope record](benchmarks/vitest-initial-scope.md) defines the planned first
JavaScript/TypeScript runner: one ESM package, Node environment, pnpm dependencies,
plain JS/TS and boolean flips. It selects a mixed-syntax fixture and Pathe as the
real TypeScript project; Hyparquet later supplies independent JavaScript evidence.
Direct runner baselines and twelve probes are scope evidence, not an adapter.

Preserve the existing built-in language boundary while introducing explicit runner
and syntax identity in project/report data. Reuse execution, workspace, review and
report machinery only where its invariants hold. In particular, a successful exit
cannot become a miss without validated nonempty completion evidence, and pnpm
links must remain inside independent workspace dependency copies. A small pinned
reporter protocol must distinguish test failures from transform and infrastructure
errors; test its failure modes before integrating outcome classification.

Project validation and deterministic JS/TS discovery now have a read-only
`vitest-list` command, separate from the run/report pipeline. The
[execution boundary](benchmarks/vitest-execution.md) now adds private bounded
completion evidence, fresh isolated command workspaces and `vitest-check` baseline
diagnostics. Validated completion outcomes/errors now pass through the shared
command type and classifier, which requires evidence even after exit zero;
shared baselines also use outcome classification. Adapter fixture probes cover
mutation outcomes and cleanup through this boundary;
explicit `--lang vitest` now connects serial full-suite mutation runs to the shared
coordinator and saved reports. Runner/tool/config identity is persisted and checked
before each command; package arguments are empty because the entire suite runs.
See [CLI evidence](benchmarks/vitest-shared-runs.md). The next steps are
worker-side lifecycle attribution and real-project usefulness/timing. The
[lifecycle audit](benchmarks/vitest-lifecycle-attribution.md) demonstrates that
reporter hook events alone miss fixture, around-hook and completion-callback
failures; all can carry ordinary assertion errors. The isolated copy of the exact audited runner records per-test body-entry and
failure-phase evidence in transported task metadata. Fixture resolution precedes
body entry; cleanup and completion callbacks record separate lifecycle failures.
Only a body failure without a lifecycle failure can count as a kill. Pending async
assertions at body entry and post-body assertion-count failures remain ambiguous
and are rejected. Passed and failed executed tests require an instrumentation
marker. The reporter fingerprint covers both reporter and instrumentation source,
so older saved runs require a fresh scan. Runner source mismatch aborts execution.
File/worker fixture setup and teardown, nested hooks and concurrent body/cleanup
mixtures now have adapter evidence, with scoped fixture CLI score checks.
Cross-file fixture reuse, scoped teardown cancellation and parallel workers still
need acceptance evidence. Vitest dismissal
CRUD/check and CI policies now reuse shared review snapshots and score decisions;
check performs source rediscovery without invoking Node/Vitest. Review never
changes outcomes or raw scores. See [review evidence](benchmarks/vitest-dismissals.md).
Focused Vitest reruns
now reuse saved-ID/missed selection and exact fresh source matching, with saved
package/lock/tool/reporter/config identity checked before the baseline. Tests may
change independently; original reports are preserved. Browser exports include
`--lang vitest`. See [rerun evidence](benchmarks/vitest-focused-reruns.md).
The initial Pathe audit missed its coverage-only Vite config. Static recognition
now accepts that original config without executing or merging it: coverage stays
disabled in the generated private config. Only an import of `defineConfig` from
`vitest/config` and an exported object containing `test.coverage.reporter` literal
strings are accepted. Multiple configs, extra settings and executable expressions
are rejected. Accepted files are fingerprinted in saved execution identity and
checked across snapshot copying; edits invalidate reruns. See
[config acceptance](benchmarks/vitest-coverage-config.md).
No plugin abstraction, affected-test selection or persistent transform cache is
part of this milestone. The [Pathe survivor review](benchmarks/vitest-pathe-survivors.md)
completes the scoped TypeScript run/inspect/improve-tests/rerun milestone: both
survivors expose observable differences and are killed by stronger tests.
SUPPORT.md records both TypeScript and JavaScript as Validated within this scope.
Hyparquet now supplies JavaScript discovery and isolated-baseline evidence with
its original JavaScript-only NodeNext type-checking config; runtime TypeScript
under NodeNext remains unsupported. The [JavaScript findings review](benchmarks/vitest-hyparquet-findings.md)
adds 19 sampled mutations and two survivors killed by stronger tests. The
[subsequent full inventory](benchmarks/vitest-hyparquet-full.md) attempts all 133
candidates across explicit continuations: 89 kills, 41 survivors, two unhandled
callback errors and one timeout. Failed reports retain unavailable scores; the
combined inventory is not a synthesized full-project score. A test-only review
kills 36 survivors and both error candidates in a 43-candidate rerun; four
redundant scan settings and one unknown-operator policy case remain. Callback
assertions are moved into awaited test bodies, preserving Mutie’s unhandled-error
precedence. Node is now
resolved through PATH once at startup (including focused reruns), then its saved
absolute path is invoked for version/path revalidation before each command. Project
and runner/config identity are still reloaded and compared; there is no fallback
to PATH if the pinned executable fails. See [validation](benchmarks/vitest-node-reuse.md).
Test-only phase instrumentation does not change the production runner or reports; broader
execution and configuration scopes remain separate work. Existing Go/Rust compatibility and report reading remain required.


## Initial Node test-runner contract

The validated `--lang node` path supports one dependency-free ESM JavaScript
package on audited Node 24.22.0. The package has the exact test script
`node --test test/*.test.js`; the adapter expands that immediate-directory file
set and runs it with serial process isolation. Only `.js` runtime files under
`src/` and `test/`, static literal local/`node:` imports, full-suite tests, one worker and
boolean flips are accepted. Unused build-only devDependencies need not be installed.
Workspaces, installed/runtime dependencies, CommonJS, TypeScript, custom loaders,
retry and expected-failure modes are outside this contract.

`javascript.rs` owns concrete boolean discovery shared with Vitest. Literal NUL
bytes inside strings/comments/templates are masked only while parsing, at the
same offsets, and their syntactic location is checked; executable NULs are rejected.
Fingerprints, diffs and mutation writes always use original source bytes.
`node/project.rs` validates the original root before snapshotting (including its
workspace ancestry), then validates copies and static import resolution.

The Node command uses a fresh workspace and separate owned reporter, hook observer
and temporary directory. The event stream must end; every selected file must have
one matching summary and worker record; global counts must equal file totals.
Exit status alone is insufficient. Unknown failures, empty/all-skipped files,
todo/cancelled results, and missing completion are errors. Known interruption,
capture overflow and timeout retain shared classifier precedence.

Node suppresses an afterEach/after error when a body has already failed. Therefore
reporter-only attribution is unsafe. An async-hooks observer wraps the native
TestHook resource's run method, counts completion and records its error before
Node suppresses it. This uses an exact hash of `internal/test_runner/test`, checked
both before execution and in each worker. Observer/reporter bytes are part of the
saved recipe identity. There is no uninstrumented fallback. Successful hooks can
accompany kills; any failed/incomplete observed hook prevents a kill.

The body observer wraps the test function with an arity-preserving Proxy. It
records values escaping synchronous calls, native Promise rejections and error-first
callbacks. After native execution it requires the identical value in the native
`testCodeFailure` cause, then exports the file-local test ID. Reporter failures
must consume exactly one matching file/ID entry; plans, suite definition failures
and hooks cannot borrow another body's proof. Parent `subtestsFailed` events are
structural evidence, not extra mutants. Non-native thenable rejection remains
unclassified. Dynamic imports are rejected because their evaluation exceptions
can otherwise resemble body exceptions; static module failures still require
file completion and remain errors.

Hook fail() observation records errors before another invocation can reset shared
hook state. Process event observation records unhandled exceptions/rejections and
errors thrown by exit listeners: Node can reduce a late failure to a diagnostic
when another test already failed. The observer delegates process.emit unchanged,
including return/throw behavior, without adding error listeners. It persists after
exit listeners and on unhandled events so an already-written completion file
cannot hide a later error. Any such signal prevents both success and a kill.
Instrumentation bytes remain part of recipe identity; older Node recipes require
a fresh scan. See [the expanded audit](benchmarks/node-body-attribution.md).

Saved Node identity includes package manifest fingerprint, Node version/resolved
executable and runner recipe. Each command revalidates the pinned executable;
focused reruns validate this identity plus the shared source identity while allowing
test edits. Shared reporting, review, scores and browser command exports retain
language explicitly. JavaScript results do not establish native TypeScript support.

The [SRI follow-up](benchmarks/node-sri-final-review.md) preserves a timeout even
when a bounded standalone test exposes the pending promise: a diagnostic subset
is not full-suite completion. Cleanup-guard survival after additional late-event
tests is evidence of an unresolved survivor, not proof of semantic equivalence.

[Second-project evidence](benchmarks/node-openclaw.md) exercises the same contract
on unchanged `openclaw-cc`: 39 candidates and a three-finding stronger-test rerun.
Neither test-only review changes nor diagnostic subsets replace original scores.

The [storage/profile review and milestone audit](benchmarks/node-storage-profiles.md)
closes five more useful findings and establishes Validated maturity for the
existing Node JavaScript scope. This does not add package layouts, dependencies,
native TypeScript or runtime versions. Seven other `openclaw-cc` survivors and the
SRI limits remain explicit; no combined improved whole-project score is inferred.


## Initial native TypeScript on Node

The following describes the first explicit-glob slice; the default-discovery
extension and subsequent Validated milestone are recorded in the next section.

The native TS extension uses the same pinned Node 24.22.0 runner and fail-closed
observer/protocol. `.ts` source and tests join `.js` under the audited `src/` and
`test/` directories. The manifest must declare exactly `node --test test/*.test.ts`
or the existing JS script. The selected immediate test-file set runs serially;
default recursive discovery is not supported. Mixed static `.js`/`.ts` imports
are allowed only within the audited set, with explicit extensions. Even erased
local type imports must resolve to an audited file; bare dependencies and dynamic
imports remain rejected.

`node/typescript.rs` rejects syntax outside this stripping-only contract in both
production and test/helper files, including enums, all namespaces, parameter
properties, angle assertions, decorators and ambient declarations. `.d.ts`, TSX,
MTS and CTS files are excluded from the supported project scope. Shared discovery
continues to skip type-only booleans without changing Vitest's TS contract.

An optional plain-JSON tsconfig accepts only scoped NodeNext/type-checking settings.
Node never applies it, and Mutie neither runs `tsc` nor transforms code. Includes
and excludes do not change the declared test suite or mutation selection. Runtime
booleans can violate a literal type after mutation; that alone is not a kill.
A missing `import type` can instead cause module-linking failure, which remains
an error under the existing completion evidence contract.

Saved native TS identity adds the selected test glob and optional tsconfig hash;
absence is also significant. Config is revalidated in each copied command tree.
A distinct TS recipe prevents a JS-only saved identity from claiming TS mutations,
while legacy JavaScript records still deserialize and retain their original recipe.
Saved command descriptions use the selected glob; text summaries state that no
type checking occurs. Test-only edits still permit focused reruns.

[Independent evidence](benchmarks/node-typescript-scope.md) records the typed fixture,
42 attribution cases, cleanup and review/policy checks, and a 23-candidate Bragkit
run with three useful stronger-test kills. Bragkit's default test script required
one explicit-glob adaptation, despite matching direct test inventories. Native TS
therefore remains Experimental pending unchanged-project findings acceptance;
JavaScript's Validated status and its unresolved survivors are unaffected.


## Audited native default test discovery

Exact `scripts.test: node --test` is now accepted for JS and native TS. A bounded
whole-project scan applies the audited default naming/extension rules, ignoring
dot components and `node_modules`. It rejects visible symlinks and any selected
file outside the audited `test/` `.js`/`.ts` set. Helpers and nested files count as
entries, even without a `.test` suffix; empty/all-skipped entries still fail the
completion contract. Existing explicit-glob recipes keep their original identities.

For default mode, each Node identity check uses the pinned runtime's native glob
to verify the complete sorted selection without executing project code. Exact
hashes pin `internal/test_runner/runner`, `utils` and `internal/fs/glob`. The
pattern, implementation hashes and discovery mode join the recipe. TS scope also
records `native-default`; conflicting saved modes are rejected. Original and
snapshot selections must match so output/report exclusions cannot remove tests.
The final command copy is checked again, then the runner is invoked without file
arguments, using its own native default discovery. Per-file completion must match
the audited selection. Test additions on a focused rerun remain allowed.

[Unchanged-project acceptance](benchmarks/node-default-discovery.md) completes the
native TS milestone: Bragkit's original package/config/source/tests produce the
same 23 outcomes, and the retained HTML-list tests kill the same three survivors.
Together with the independent TS failure, cleanup, review and identity evidence,
this meets Validated within the declared scope. The
[remaining findings review](benchmarks/node-bragkit-survivors.md) kills all 18 other
survivors with a 76-test baseline in 53.39 seconds. Original sources/reports and
full-scan scores remain unchanged. Test layouts beyond the audited test directory,
dependencies and loaders remain separate scope decisions.


## Scoped Bun runner: attributed scoring

The [initial audit](benchmarks/bun-initial-scope.md) selects pinned Bun
1.3.11+af24e281e, separate dependency-free JS/TS fixtures and GramIO Composer as
an unchanged real TS project. JUnit and native inspector reports cannot alone
separate body failures from beforeEach/afterEach and unhandled process failures.
They do expose IDs, incomplete selection and timeout status. A future adapter
must combine complete execution accounting with positive failure provenance;
nonzero exit, JUnit AssertionError and inspector fail status are insufficient.

Retained probes cover 24 scenarios per syntax, including mixed cleanup failures,
early exit, only/skip/todo, imports and pending promises. Composer passes 272 tests;
a manual name-configurability flip passes until a stronger test is added (273-test
baseline). Direct probes establish feasibility and a useful finding, not scored
support. Observer semantics, package/config validation, command cleanup and the
shared report/rerun/policy workflow were the initial acceptance gates.


The [attribution follow-up](benchmarks/bun-body-attribution.md) validates a bounded
file-backed observer and classifier across 112 paired JS/TS cases and Composer's
272/273-test controls. Callback registration/invocation IDs and sequenced phase
records provide positive body evidence; complete JUnit structure/counts provide
native test completion. The native unhandled-error diagnostic is negative evidence
only. Caught body failures remain provisional until an event-loop checkpoint,
which exposes pending rejection mixtures otherwise lost on synchronous failure.
Successful callbacks are not delayed. Hook failures, unsupported APIs, mismatched
or incomplete evidence remain errors. Native Promise observation and failure-path
scheduling remain explicit instrumentation boundaries.

Inspector transports were rejected as an execution authority after final events
were lost at shutdown. The Rust reader now drives a baseline-only `bun-check` diagnostic through fresh
workspaces, a private config/path/to/home directory, independent expected-file
inventory, bounded captures and Unix regular-file limits. It requires the pinned
runtime and compares executable and observer bytes around execution. Input files
and test inventory are compared before executing the snapshot. The scope accepts the audited plain-JSON NodeNext/ES2022 configuration and
fingerprints ancillary examples/release scripts without making them importable.
It rejects aliases/inheritance, custom Bun config, installed dependencies and
nonlocal imports other than node: builtins and bun:test. It does not execute package scripts.
Unchanged Composer baseline/finding controls pass; the scored workflow follow-up is described below.

Bun now exposes read-only `bun-list` through shared boolean discovery with a
Bun-only erased ambient-declaration allowance. The default Node/Vitest path keeps
its existing rejection. `bun-check --json` emits a validated diagnostic identity
covering runtime, package/config, test inventory, recipe and limits, plus a separate
input fingerprint. The diagnostic records are separate from saved mutation reports. Composer's full preview has 12 runtime candidates; see
[discovery evidence](benchmarks/bun-discovery-identity.md).

Explicit `--lang bun` now connects discovery and execution to shared reports and
focused reruns. Saved identity validates the pinned executable bytes, manifest,
config and exact observer/runner recipe. Test additions, executable relocation and
budget changes remain compatible; the test inventory must match every execution
within a run. Original and snapshot inputs are compared before discovery so report
exclusions cannot hide executable files. Baselines and each mutation use fresh
owned copies and bounded captures; only complete positive body evidence can kill.
Independent JS/TS fixtures and Composer established the initial Experimental scope; see
[shared runs](benchmarks/bun-shared-runs.md). Independent JS/TS [scored cleanup checks](benchmarks/bun-scored-cleanup.md)
cover resource limits, timeout/SIGINT and descendant shutdown. [Independent JS/TS review/policy checks](benchmarks/bun-dismissals.md) now pass,
completing scoped TypeScript validation with Composer.
[Independent JavaScript validation](benchmarks/bun-javascript-pp-is.md) follows on
an explicitly adapted pp-is ESM root, preserving all source/test bytes. Seven
stronger tests kill seven former survivors; three remain open. Original package
layout support was separate from that adaptation. The
[unchanged-layout follow-up](benchmarks/bun-unchanged-layout.md) now admits static
JS ESM without a package type plus opaque ancillary packaging and root/dist
`.d.ts` files. Runtime imports still resolve only to audited source/test `.js`/`.ts`
files; hidden packaging tests and CommonJS runtime files remain errors.

Bun [execution profiling](benchmarks/bun-feedback-profile.md) attributes most
adapter overhead to full executable SHA-256 verification before and after each
command. A release build and a temporary sha2-only compiler experiment preserve
identical outcomes and identities while greatly reducing elapsed time. Test-only
phase timers are compiled out of production; no cache or reduced validation is
introduced. Real-run timings should distinguish build mode and test-command time
from complete feedback time before motivating further execution changes.

The measured sha2 optimization is now a dependency-specific development-profile
override (`opt-level = 3`), also inherited by the test profile. Application code
keeps its normal development settings; release settings and complete executable
fingerprints remain unchanged. This benefits all shared SHA-256 callers without
introducing runner-specific caching.

The [Composer findings review](benchmarks/bun-composer-survivors.md) validates the
remaining feedback loop: instance overrides, prototype redefinition, enumeration
and default macro options kill four survivors. Queue tests assert the active
precondition before waiting for idle, turning a dispatch-blocking mutation from
a full-suite timeout into a body-failure kill. This changes only project tests;
the original timeout stays excluded from its saved score. All five selected
candidates are killed with complete full-suite evidence and preserved source
and report hashes.

## Python / pytest feasibility boundary

The [initial native audit](benchmarks/python-initial-scope.md) selects CPython
3.14.7 / pytest 9.0.2 and retains fixtures plus an adapted Inflection runtime
recipe. It does not add a language adapter. Start from a dependency-free flat
package, explicit pytest tests and serial full-suite execution; pin owned
environments and reject unsupported plugins/config/import layouts.

Pytest setup/call/teardown reports are useful evidence but insufficient on their
own: unittest setup can appear as a call failure, and atexit failures can follow
successful pytest return with process exit zero. The prototype retains both as
negative controls. Process-lifetime failure observation must precede scores.
Similarly, disabling bytecode writes does not disable reads: a same-size,
backdated source mutation loads stale bytecode unless the command owns a fresh
cache prefix. The native controls establish this requirement without claiming
general cache/import isolation.

Inflection's 467 selected runtime tests include package doctests, while excluding
the upstream mypy/packaging/documentation recipe. A hand-authored Unicode-digit
regex mutation survives and is killed by a stronger test. This demonstrates
finding usefulness, not automatic operator support, unchanged-project acceptance
or readiness to broaden Python beyond the audited scope.

The [process-lifetime follow-up](benchmarks/python-process-lifetime.md) keeps a
raw evidence descriptor alive through shutdown and combines pytest phase reports
with return/atexit checkpoints, hook ownership, strict sequence/PID/nonce checks,
uncaptured stderr and actual process exit. Atexit is not the final boundary:
module-destructor errors can follow it. Any cleanup error dominates a body failure.
That initial experiment rejected unittest items and thread starts; pytest can otherwise
retain late thread exceptions after its last warning drain. The experiment
establishes these conservative boundaries without adding a Python score or
substituting for static scope validation and Rust process integration.

The [baseline CLI integration](benchmarks/python-baseline-integration.md) uses a
trusted, non-importing AST helper under `-I -S` for scope/discovery and a strict
Rust lifecycle reader for isolated collection and baseline execution. Runtime
identity includes the venv, executable, shared library, stdlib and installed
package bytes. Fresh external cache prefixes prevent stale bytecode reads.
Production process controls now reject a remaining process group after direct
child exit and cover cancellation, deadlines and output/file bounds. Typed
execution identity binds the recipe and runtime, permitting stronger tests but
rejecting runtime/configuration/recipe drift. Explicit Python runs save pristine node IDs and
require exact candidate selection. Shared scoring, reports, focused reruns and
review/policies pass the fixture workflow in
[Python shared runs](benchmarks/python-shared-runs.md). Scored cleanup now passes;
[Scrubify acceptance](benchmarks/python-scrubify-findings.md) completes the scoped
real-project loop with a blank-query survivor killed by stronger tests. Original
source and old reports are preserved. That milestone validates the initial
flat layout, pinned runtime and serial recipe; the native src follow-up below
extends only its audited layout/metadata boundary.

The [feedback profile](benchmarks/python-feedback-profile.md) separates isolated
pytest startup, repeated runtime/scope validation and workspace costs. Runtime
fingerprinting uses a directory-entry walk that prunes already-excluded trees,
preserves component ordering and hashes every included file on every check.
It does not cache identity by size/mtime or share project bytecode. Phase timers
are test-only; execution recipe identity still includes validator changes.


## Native Python src layout

[Unchanged Scrubify acceptance](benchmarks/python-src-layout.md) validates one
ordinary package in either the project root or `src/`, with full tests under
`tests/`. Static scope selects exactly one import root; all Python files are
validated before report filters, and original/snapshot comparisons include the
root, executable file identities and complete test set. Relative imports are
resolved from that root while discovery retains project-relative source paths.

An optional `source_root` execution-identity field defaults to the flat root for
older records and admits only empty or `src`. Saved mutation paths and reruns
validate the root alongside package, manifest/runtime/recipe fingerprints.
The driver inserts the isolated import root and verifies observed application
module origins against exact owned paths; it never installs the project.

The unchanged project's static setuptools src mapping, pytest-only test extra,
matching pytest pythonpath and quiet addopts are recognized declaratively.
Build hooks, dependency installation, arbitrary pytest/setuptools settings and
multiple/namespace packages remain excluded. Source-root changes invalidate
reruns; stronger tests can still establish a fresh pristine inventory. The
real-project acceptance retains original source/manifest hashes and old report
hashes across a stronger-test rerun in a separate complete project copy.

## Second unchanged Python project

[Cycler acceptance](benchmarks/python-cycler-unchanged.md) exercises 28 original
tests at the repository root. The validator now inventories both root
`test_*.py` and `tests/` files, before exclusions. Execution explicitly selects
every audited file; collection and candidate inventories must agree. Saved
identity and protocol readers share the same normal-path validator. Root tests
excluded by an explicit `testpaths=["tests"]` configuration are rejected.

The additional setuptools metadata is declarative: one flat package, `py.typed`,
and a dynamic-version declaration backed by a literal top-level string in its
initializer. Optional audited documentation/test tools are never resolved,
installed or loaded. Documentation configuration at `doc/source/conf.py` and
direct `.circleci/*.py` scripts stay fingerprinted and cannot be imported by
admitted executable code; tests/hooks in those files remain rejected. Native
fixture controls include raising ancillary scripts to prove they do not execute.

The original Cycler full run remains failed after a mixed assertion/KeyError
mutation. A selected rerun covers the remaining candidates. In a separate copy,
an added key-set assertion checks composition before rename and turns the error
into an attributable assertion kill. This improves test diagnostics without
weakening the classifier or rewriting the original report's score.

## Scoped unittest through pytest

[Pluralizer acceptance](benchmarks/python-unittest-pluralizer.md) extends the
pinned runner to direct synchronous TestCase subclasses. Pytest queues unittest
exceptions and can shift or suppress additional cleanup failures, so call-phase
assertion status alone is insufficient. Before project imports, the driver wraps
pinned TestCase setup/body/teardown/cleanup dispatchers. It retains the actual body
exception and requires identity with pytest's reported exception. The Rust reader
requires the ordered setup/body/teardown/cleanup trace, successful cleanup steps,
unchanged dispatch methods, all pytest phases and existing process-lifetime checks.
Multiple failures cannot become kills even when pytest reports only one of them.

Admission excludes custom dispatch/constructors, indirect inheritance,
metaclasses, decorators beyond class fixtures, subtests, skip/expected-failure
APIs and module-cleanup registration. Runtime checks independently reject changed
base/class/instance dispatch. The observed unittest node subset is bound to the
complete collected inventory. Missing framework inventory or per-call evidence is
an error. Driver, reader and static scope changes already participate in the
execution recipe hash, including focused reruns; no schema/version bump is needed.

Pluralizer's static Hatch/Ruff/development/resolver metadata is fingerprinted but
never executed or installed. Its original script-entry guard is retained because
pytest imports the test module under a non-main name. This does not enable general
unittest runners, plugins or dependencies.


## Scoped conftest fixture observation

The validated [fixture audit](benchmarks/python-conftest-fixtures.md) admits
root and direct `tests/conftest.py` modules containing audited imports, literal
data and synchronous pytest fixture declarations. General plugins, hooks and
nested conftests remain excluded. Pytest uses an explicit root `confcutdir`; the
observed loaded-conftest inventory must match static discovery. Snapshot checks
include each conftest, and execution identity hashes its bytes. Stronger ordinary
test files remain permitted in reruns; fixture configuration changes need a new run.

The driver observes pinned FixtureDef setup and finalization before importing
project code. Setup errors are independently recorded even when a body catches an
exception from `request.getfixturevalue`. Finalization errors dominate body errors.
Every instantiated fixture requires ordered started/ready/finished evidence, and
all fixture instances must be finalized at session finish. Repeated cached fixture
use and redundant finalization callbacks do not fabricate new lifetimes. Native
controls cover all four admitted scopes, dependency sharing, parametrization,
autouse/unittest interaction, yield teardown and multiple registered finalizers.
The unchanged Binary Tree Library validates seven original tests and all 11
mutations (five kills, six survivors). Separate-copy boundary tests kill all six
survivors. The validated scope remains limited to the audited fixture subset.
Plain root pytest.ini testpaths and an empty tests/__init__.py are also bound to
snapshot and execution identity; other settings or executable initializers are
rejected. Test-only random.sample and direct Exception subclass initialization
are bounded compatibility additions; Sphinx metadata remains inert.


## Larger Python fixture-project acceptance

[Route Mini](benchmarks/python-route-mini.md) exercises 68 unchanged tests and
38 boolean sites. Scoped compatibility permits synchronous ordinary conftest
helpers with literal defaults, direct unaliased helper imports from regular
tests packages, docstring-only test initializers, and inert example/static tool
metadata. Specific ABC, mapping proxy, time and in-memory test JSON APIs are
audited; broad modules, plugins, dynamic defaults and hook declarations remain
excluded. Existing conftest/initializer hashes cover helper execution identity.

Three failures require remaining-candidate runs. Report-local mutation IDs are
matched by original source identity for complete accounting, never summed into a
full score. Four extra test cases expose three incorrect-success audit
flags in a separate test-only copy; originals and earlier reports are preserved.
The remaining structural survivors and error/timeout cases stay open.

## Python comparison boundaries

[Scoped comparison discovery](benchmarks/python-comparison-boundaries.md) matches
single-comparison AST operand gaps to native operator tokens and changes only
`<`/`<=`/`>`/`>=`. UTF-8 byte coordinates are revalidated by Rust; saved operator
labels must match exact token pairs. Chains and annotations are excluded. Native
operand evaluation and rich-comparison dispatch are preserved; custom-dispatch
exceptions remain errors. The changed recipe requires fresh runs, without changing
the report schema or rewriting historical reports. Unchanged Route Mini exact-token-
capacity acceptance complements fixture evaluation-order and NaN controls.

## Python independent random generators and larger unchanged validation

The [betting-math-kit acceptance](benchmarks/python-betting-math.md) extends only
inert optional-tool minimum versions, selected Ruff/Mypy fields, and direct
`random.Random(...)` construction. The constructor may not escape as a value,
be rebound or mutate class methods; module global RNG APIs remain excluded.
Existing test-only `random.sample` remains supported. Generator seed errors stay
runtime errors. Every validator change joins recipe identity as before.

The unchanged 198-test suite supplies a new larger project. Ten selected metrics
boundaries yield six survivors and four errors across preserved failed reports.
Seven new cases in a separate 205-test copy kill the six survivors; full original
inputs and all test IDs remain intact. No combined/full score is inferred from
selected accounting, and observer/classification semantics remain unchanged.

The [complete betting-math-kit review](benchmarks/python-betting-math-full.md)
accounts for all 87 sites (19 kills, 59 survivors, nine errors). A separate
222-test copy kills 24 survivors and two endpoint-error candidates using explicit
expected-success assertions. Original inputs, reports and classification rules
remain intact. Domain-limited redundancy observations and unresolved numerical
boundaries are retained individually; combined accounting is not a full score.

## Python binary arithmetic

The [scoped arithmetic acceptance](benchmarks/python-arithmetic.md) adds binary
`+ ↔ -` and `* ↔ /` through native token replacement and the existing
`arithmetic_replace` operator. Equal-precedence replacements retain AST grouping
and operand order. No numeric type inference, desugaring, operand duplication,
coercion or exception conversion is introduced. Saved pairs and source spans are
validated; validator changes join existing recipe identity. Fixture/error controls
and unchanged betting-math-kit stronger-test findings establish the narrow scope.

The [complete arithmetic review](benchmarks/python-arithmetic-full.md) accounts
for all 194 sites on unchanged betting-math-kit: 61 kills, 97 survivors and
36 native errors. A separate 219-test copy kills 54 more survivors; with eight
prior kills, 35 remain open. Numerical questions are recorded per candidate. Preserve original report scores and the
source-identity mapping; combined accounting is not a full-run score.

## Bounded Python error continuation

[Candidate continuation](benchmarks/python-candidate-continuation.md) separates
classification from scheduling. Complete adapter validation may produce an
internal isolated-body-error marker, but the shared classifier still rejects the
candidate. An explicit invocation-only error budget lets the serial scheduler
continue these errors, then return failure after a complete inventory. Fatal
errors cancel immediately. All error reports remain failed/unscored and normal
cancellation/persistence precedence is retained. Eligibility starts with audited
Python type/value/arithmetic/lookup exceptions only; never infer continuation
from exit status, error strings or another runner's generic failure evidence.

## Python fixture data and explicit expected failures

[Tomli-W](benchmarks/python-tomli-w.md) drives scoped filesystem compatibility.
Static admission allows test paths derived from `__file__` and binary temporary
output. A process audit hook checks filesystem calls with project frames on the
stack, including their callees. Copied inputs and pinned-runtime/cache paths are
read-only; writes stay under the owned temporary directory. Rejected operations
emit fatal evidence before raising, so caught exceptions cannot conceal violations.
The hook supplements static scope, bounded processes and post-run hashes; it is
not a general Python sandbox. Non-Python files and directory membership participate
in snapshot and execution identity, preventing report exclusions or data drift
from silently changing fixtures. Optional build, formatter, coverage and tox
metadata remains inert and fingerprinted.

A project containing explicit literal-reason `pytest.xfail` calls gets a pristine
executed baseline when establishing its execution scope. The saved identity binds
the expected-failure node set, and every candidate must reproduce it. Positive
evidence requires the exact native XFailed exception in a skipped call phase,
not a same-named exception, assertion, setup failure or generic skip. Complete
fixture, teardown and process-lifetime evidence remains mandatory. These nodes
never contribute kills; all-expected-failure suites are rejected. Stronger-test
reruns establish a fresh pristine inventory and expected-failure set rather than
reusing historical body outcomes. Arbitrary marks/plugins and unittest expected
failures remain excluded.

## Scoped Python application dependency

The first [dependency slice](benchmarks/python-dependencies-tinycss2.md) is an
explicit audited allowlist, not a resolver. Static discovery admits only the
known webencodings requirement forms and named root-module imports. Runtime
identification, with site initialization disabled, verifies version 0.5.1,
pure-wheel metadata, no transitives/entry points and the exact package-tree hash.
The full environment is still fingerprinted before and after every command.
Snapshot and execution identity bind the manifest-derived dependency map; missing
or undeclared installed dependencies are rejected. Focused reruns require matching
runtime bytes and declaration, while allowing stronger tests as before.

Dependency calls use the unchanged body/fixture/process-lifetime observer. Runtime
exceptions remain errors and cleanup mixtures cannot become kills. No dependency
sources are mutated. The selected application is unchanged Tinycss2 1.2.1 with
its pinned data submodule; data-only Git pointer files are excluded consistently
with copying, while nested Git directories and hidden excluded tests are rejected.
Small compatibility additions admit literal Flit versions, inert docs/data
helpers, direct local-base initialization, class-method names that do not rebind
module imports, test decorator name reads, sys.maxunicode and narrow pprint/colorsys
APIs. They do not admit arbitrary reflection, plugin hooks or build execution.


### Shared JS/TS comparison mutations

Vitest, Node and Bun share parsed runtime comparison discovery in
`src/javascript.rs`: `comparison_invert` swaps strict/loose equality separately
and exchanges `< ↔ >=` / `<= ↔ >`; `comparison_boundary` swaps `< ↔ <=` /
`> ↔ >=`. Only the operator token changes. Operand expressions and comments are
retained, including native evaluation/coercion order and nested binary grouping.
The operator name describes the token pair, not universal Boolean complementation
(`NaN` is unordered). Type-only nodes, strings/comments, shifts, `in` and
`instanceof` are not comparison candidates. No inference or exception conversion
is introduced. Candidates at the same span sort by operator before report IDs
are assigned. Exact saved token pairs are checked before source rediscovery.

[Acceptance](benchmarks/javascript-comparisons.md) covers the six runner/syntax
combinations, native semantics, report corruption rejection, filtered selection,
stronger-test reruns and unchanged pp-is endpoint findings. Existing observer and
classification contracts remain intact. Bun's existing recipe fingerprint includes
discovery bytes, so older Bun recipes require fresh scans; Node/Vitest execution
recipes are unchanged and source identities select rediscovered candidates.


### Shared JS/TS logical expressions

`logical_swap` replaces a complete parsed `&&`/`||` expression with parenthesized
left and right operands and a parenthesized result. Inter-operand comments and
whitespace are retained. This prevents a changed precedence from regrouping a
nested expression or its parent. `negation_remove` replaces a parsed unary `!`
expression with its parenthesized operand and retained trivia. It removes the
boolean conversion as well as negation; no Boolean wrapper is introduced.
Native truthiness determines which operands are evaluated, and logical expressions
retain raw operand values. Each `!` in `!!x` remains an independent mutation.
TS non-null/definite-assignment syntax, `??`, logical assignments and bitwise
operators are outside this slice. Runtime type checking is not added.

Saved logical replacements are validated by parsing the saved original expression
in its JS/TS grammar and reconstructing the same recipe. Source rediscovery still
binds spans, fingerprints and replacement bytes before reruns. Nested same-start
candidates sort by operator and end offset. Existing boolean workflows use explicit
operator selection where their acceptance counts intentionally describe boolean
mutations. [Acceptance](benchmarks/javascript-logical.md) records semantic controls,
six runner/syntax fixtures, unchanged public inputs and a useful stronger-test rerun.


### Shared JS/TS arithmetic expressions

`arithmetic_replace` adds binary `+ ↔ -`, `* ↔ /` and `% → *` to shared JS/TS
discovery. The full-expression recipe used for logical swaps now also encloses
arithmetic operands/results, preserving their grouping and trivia. Native operator
semantics determine coercion hints, concatenation, Number/BigInt results and
exceptions; there is no numeric inference or custom arithmetic helper. Unary
signs, update/assignment forms, powers and type-only syntax remain excluded.
Readers reconstruct the exact arithmetic replacement before source-identity
matching. Existing logical replacement bytes remain the same after sharing the
binary renderer. Runtime recipes/classifiers are otherwise unchanged.

[Acceptance](benchmarks/javascript-arithmetic.md) covers strings, signed zero,
NaN/Infinity, BigInt truncation/zero/mixed-type exceptions, evaluation and coercion
order, six scored JS/TS workflows, and unchanged Bragkit weight findings killed by
separate-copy exact-score tests. Modulo/remainder has native and fixture evidence;
the selected public-project findings are division/multiplication replacements.

### Rust arithmetic expressions

Binary `arithmetic_replace` uses equal-precedence/associativity token edits for
`+ ↔ -`, `* ↔ /`, `% → *`. A new division token adjacent to a comment gets one
separating space to prevent accidental `//` lexing. Operands are neither cloned
nor borrowed by instrumentation; native trait resolution and ownership apply.
Saved readers validate exact token pairs and Rust extensions; reruns retain
source identity checks. Compilation errors, runtime panics and aborts continue
through the existing Cargo evidence classifier, without arithmetic exceptions.
[Acceptance](benchmarks/rust-arithmetic.md) records native move/borrow/numeric
controls, scored fixtures, unchanged UCD Generate and stronger-test reruns.


### Rust comparison inversion and Not removal

`comparison_invert` uses token edits for `== ↔ !=`, `< ↔ >=`, `<= ↔ >`,
preserving native implicit borrowing and exact overloaded method dispatch.
Unordered float cases need not be logical complements. Both inversion and boundary
candidates survive at a shared token span: ordering/deduplication includes operator
identity. Outer-before-inner order of existing logical edits is retained.

`negation_remove` replaces a complete unary `!operand` with `(operand,).0`, retaining
operand trivia. Tuple construction forces value evaluation; projection passes the
value on without an extra trait call, clone, cast or named binding. Plain `(operand)`
could become a borrow of the original place beneath `&!operand`, accidentally
changing move/drop behavior. This rule covers Boolean/integer/custom Not; removing
custom Not effects and changing its output type are intentional. Compile failures
stay build failures. Never types and macro punctuation are not unary expressions.
Saved readers parse the original and reconstruct exactly; reruns still rediscover
source identity. [Acceptance](benchmarks/rust-inversions.md) records controls and
unchanged public-project findings with separate stronger tests.


## Python comparison inversion and negation removal

The scope AST admits single `==`, `!=`, `<`, `<=`, `>`, `>=` comparisons, with
exact native token replacements; inversion and boundary candidates can share a
span. Chained comparisons and identity/membership operators are excluded. Rich
comparison/reflected dispatch, non-Boolean comparison returns and unordered floats
retain Python semantics. Inversion is not implemented as `not` of the old result.

A unary `not` candidate replaces its complete AST span with `(` plus original
bytes after the keyword plus `)`. This retains operand trivia/grouping, including
multiline and adjacent punctuation, and evaluates the operand once without a new
truthiness call. Nested candidates overlap independently. Rust-side admission
checks exact recipes, source bytes, UTF-8 byte coordinates and strict ordered keys
(file/start/reverse-end/replacement), rejecting duplicates while allowing shared
and nested spans. Readers match the same operator/recipe; focused reruns still
rediscover AST-authorized source sites. Validator bytes change execution identity.
No version bump or classification relaxation accompanies the extension.
See [acceptance](benchmarks/python-inversions.md).


## Python binary logical swaps

The AST scope admits `BoolOp` nodes with exactly two values and replaces their
single native `and`/`or` keyword. Existing parentheses and operand bytes remain
untouched. Flat three-or-more-value nodes are deliberately excluded: mutating one
join must not silently regroup such a chain or synthesize a fold with different
truthiness calls. Nested binary nodes remain independent candidates.

In the binary scope, a swapped node can merge with a neighboring same-operator
node in Python's AST. Pinned native controls compare the actual source edit with
the intended AST operator change, including operand and truthiness call traces,
raw object identity, stateful `__bool__`, exceptions, value/list/condition/negated
contexts and mixed nested expressions. This establishes native execution grouping
without introducing helpers, extra Boolean conversion or duplicate evaluations.
Exact keyword pairs are shared by discovery and saved-report readers; fresh
source and validator identity continue to bind reruns. Newly evaluated operand
failures retain existing classification. See [acceptance](benchmarks/python-logical.md).


## Python modulo replacement

Binary `% → *` joins `arithmetic_replace` through the existing AST/operator-token
path. Equal precedence and left associativity preserve operand/parent grouping;
no wrapper or coercion is added. Original operand bytes, comments, UTF-8 and CRLF
coordinates remain intact. `%=` assignments, annotations, tests, floor division
and powers remain excluded. Percent characters in strings and f-string format
specifiers are not operator tokens; expressions inside f-strings are eligible.

This is syntactic support, including string/bytes formatting and overloaded
operators. A numeric formatting RHS can become repetition; other operands can
raise TypeError. Native reflected/subclass dispatch, NotImplemented fallback,
raw results and operand exceptions are preserved. The existing classifier handles
these outcomes without converting runtime errors to kills. Exact saved token
pairs and source identity are required; discovery/validation bytes change the
execution recipe, so historical reports require fresh original runs.

[Acceptance](benchmarks/python-modulo.md) records native controls, scored fixture
and formatting/custom-dispatch errors, and unchanged betting-math-kit's odd-trial
median finding with a separate-copy stronger test through its public API.


## JS/TS branch forcing

The shared discovery visits `if_statement` conditions and emits independent
`branch_force_true`/`branch_force_false` candidates. The original condition span
includes its required parentheses; replacements are `(true || ORIGINAL)` and
`(false && ORIGINAL)`. This intentionally skips all condition evaluation, matching
the Go contract, while retaining source references and condition grouping/trivia.
Bodies and dangling-else relationships are untouched. Nested `else if` conditions
remain independent; syntactic Boolean literals behind parentheses/comments are
excluded to avoid duplicating Boolean flips. No constant folding/type inference
is attempted. Ternaries and loops remain outside branch forcing.

Saved JS/TS readers reconstruct an `if` statement from the original condition,
require its exact span and regenerate the exact branch recipe. They reject bare
constants, extra statements, opposite guards and arbitrary replacements. Source
rediscovery remains required. Bun's source-bound recipe changes with shared
discovery; its original acceptance must be fresh. No execution observer or failure
classifier changes. TypeScript narrowing does not guarantee the forced branch is
safe; existing adapters classify native failures under their normal body/lifecycle
rules. See [acceptance](benchmarks/javascript-branches.md).


## Rust branch forcing

Ordinary Boolean `if_expression` conditions receive independent true/false guards:
`true || (ORIGINAL)` and `false && (ORIGINAL)`. Preserve condition bytes and source
references while deliberately skipping all runtime evaluation, including moves,
borrows, effects and panics. Bodies remain unchanged. Parenthesized Boolean
literals are excluded. `if let`/let-chains are excluded because their bindings can
be required by the branch body; nested ordinary branches remain eligible.

Compiler checks remain authoritative for retained expressions and bodies; the
fixture preserves definite-initialization failures as build failures. Saved
readers parse an exact original condition and regenerate the exact guard, then
require normal source rediscovery. No macro-token/constant/type scope broadening
or exception reclassification. [Acceptance](benchmarks/rust-branches.md) records
native ownership/drop/borrowing controls and four unchanged UCD Parse field-loss
survivors killed in a separate 142-test workspace copy. Seven survivors remain open.


Scoped Python branch forcing is supported for ordinary `if`/`elif` conditions.
The exact `(True or (condition))` / `(False and (condition))` recipes skip all
condition effects and truthiness. Literal Boolean conditions and every condition
subtree containing `:=` are excluded; ternaries, loops, comprehension guards,
annotations and tests do not gain branch candidates. Other operators inside those
expressions retain their own eligibility. Runtime errors remain errors; exact
saved recipes, fresh discovery and execution identity stay enforced. See
`benchmarks/python-branches.md` for native controls, scored fixture/rerun checks
and unchanged-project evidence. Original scores and inputs remain separate from
stronger-test results.


`benchmarks/javascript-loops.md` records scoped JS/TS `loop_skip` across Vitest,
Node and Bun. Ordinary `while` and explicitly conditional `for` conditions use
`(false && (ORIGINAL))`; a `for` initializer still runs once. Preserve initializer
exceptions and lexical bindings while skipping condition/body/update effects.
Boolean literals, conditionless loops, iterator loops and do-while are excluded.
Saved edits reconstruct the exact expression and keep fresh source identity;
Bun's discovery recipe changes require fresh original reports. All six runner/
syntax workflows preserve lifecycle errors as errors. Unchanged OpenClaw's single
selected Telegram loop survives; a separate-copy chunk-content assertion kills
it with a 63-test baseline. Preserve all 63 original inputs, historical reports
and the original filtered 0% score; focused evidence is not a full-project score.


`benchmarks/rust-loops.md` records Rust `loop_skip` for ordinary Boolean `while`.
Preserve exact `false && (ORIGINAL)` guards, skipped condition/body effects and
native ownership/borrowing/drop checks. Literal Boolean conditions, while-let,
let-chains, iterator for, unconditional loop and existing constant/type/test/macro
exclusions remain. Definite-initialization failures stay build failures. Saved
loop edits require exact parsed spans/recipes and fresh source identity. Unchanged
Marathon has two selected TUI-loop survivors; a separate-copy bounded PTY test
kills the event-loop survivor by requiring document rendering before quit. The
search-scrolling survivor remains open. Preserve all original inputs, internal
symlink targets and original report scores; focused evidence is not a full score.


`benchmarks/python-loops.md` records Python `loop_skip` for ordinary `while`.
Preserve exact `(False and (ORIGINAL))` edits, skipped condition/truthiness/body
execution and native zero-iteration `while…else` behavior. Exclude literal Boolean
conditions, every assignment-expression condition subtree, iterator for, annotations
and tests. Discovery explicitly distinguishes loop context from false branches;
saved recipes and fresh source/operator identity remain mandatory. Runtime/phase
failures remain errors. Unchanged Binary Tree Library's AVL deletion-height
survivor is killed by a separate-copy test; preserve original inputs/reports and
keep combined inventory accounting distinct from a full score.
