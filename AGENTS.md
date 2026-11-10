# Repository Guidelines

Project Cargo build scripts now run in owned copies/artifacts; see
`benchmarks/rust-build-scripts.md`. `src/rust/build_script.rs` binds script paths,
input files/modes, declared environment and generated output identity. Keep
scripts/generated code outside discovery; clean all local packages before cached
candidates. Stronger integration tests are allowed only with matching pristine
build evidence. Preserve error/compile/timeout distinctions and descendant cleanup.
Inactive release-profile chains and explicit non-host target flags are accepted;
active dev/test/compiler settings, proc-macro members and external paths remain
separate scopes. This supersedes historical build-script exclusions below.
Unchanged ripgrep has 1,209 passing tests and a bounded 16-candidate sample
(six kills, ten survivors); four separate-copy tests kill seven survivors with
1,213 tests. Preserve original inputs/scores/reports and the three unresolved
survivors. `.git` remains absent from copies; do not fabricate Git identity or
claim arbitrary host-dependent builds are reproducible.

## Current CLI and configuration

Bare `mutie` prints help. Use `mutie run`, `mutie list`, `mutie check` and
`mutie rerun RUN` (`--missed`, `--id`, `--dry-run`). Commands use the current
directory; global `--path DIR` changes the working directory for all relative
command paths. Project directories are not positional arguments. Runner-specific
`*-list`/`*-check` commands and root execution flags were removed in the
2026-11-07 redesign. `mutie.toml` has top-level execution defaults and `[go]`,
`[rust]`, `[js]`, `[python]` sections; `[js].runner` selects Vitest/Node/Bun.
Section presence enables a language, `enabled = false` disables it, and multiple
enabled languages require `--lang` (one project per invocation). Configured paths
are relative to the config file; source globs are relative to the selected project.
Presentation/verbosity are invocation-only: automatic live output, `--plain`, or
streaming `--json` for run/rerun. The two flags conflict; `--ui` was removed.
Source/operator/policy/runner settings are config
fields; jobs/timeout retain CLI overrides. See `docs/usage.md` for migration.
Historical command spellings below and in benchmarks describe retained evidence.
Adapter test fixtures materialize settings in external config files through
`tests/support/invocation.rs`; CLI/config contract tests invoke the binary directly.

## Project Structure & Module Organization

Mutie is a Rust 2024 CLI for mutation testing, supporting Go modules, validated
Rust packages and scoped Vitest packages, plus a validated scoped JavaScript/native TypeScript Node runner.

- `src/main.rs` coordinates runs and workers; `cli.rs` defines CLI arguments.
- `src/config.rs` loads repository settings and resolves explicit CLI overrides.
- `src/coordinator.rs` feeds execution events to reports and run-state snapshots;
  `src/status.rs` renders the live Ratatui view and manages terminal cleanup.
  `src/json_output.rs` writes flushed JSON records from the coordinator. Preserve
  stdout purity, report/score agreement, failure exit codes and worker cleanup
  after write errors; see `docs/output.md` and JSON controls in `tests/configuration.rs`.
- `src/go.rs` integrates Go tooling; `mutation.rs` discovers mutations with Tree-sitter.
- `src/language.rs` dispatches language commands; `command.rs` shares captures,
  classification and progress phases. `src/rust.rs` runs isolated Cargo commands;
  `rust/project.rs` validates the initial scope and `rust/discovery.rs` traverses
  Rust modules for boolean flips, comparison-boundary mutations, logical swaps, and binary arithmetic/remainder replacements.
- `src/bun.rs` implements scored `--lang bun`, read-only `bun-list` and baseline-only `bun-check` with the pinned Bun runtime,
  owned workspaces and bounded process/evidence captures. `bun/project.rs` validates
  the dependency-free JS/TS scope and independent test inventory. `bun/config.rs`
  admits the audited plain-JSON NodeNext/ES2022 settings; ancillary examples/scripts
  cannot be imported or contain tests. `benchmarks/bun-composer-execution.md` records
  unchanged Composer baseline and stronger-test acceptance.
  `bun/protocol.rs` requires complete XML/JSONL and positive body checkpoints;
  `tests/bun_protocol.rs` retains native replay controls. See
  `benchmarks/bun-isolated-execution.md`. Unix per-file limits apply to Bun and its
  descendants; this is not a total disk quota. `bun/identity.rs` records diagnostic
  runtime/config/recipe/limit identity in diagnostics and saved reports; focused
  reruns permit stronger tests but reject runtime/config/recipe drift. `benchmarks/bun-discovery-identity.md` records 12
  Composer flips and preserved inputs. Bun-only discovery skips erased ambient
  declarations without broadening Node/Vitest. JS/TS scored fixture and review workflows pass independently. TypeScript is
  Validated via Composer; JavaScript is independently Validated via pp-is.
  `benchmarks/bun-unchanged-layout.md` records unchanged checkout acceptance:
  missing package type requires static JS ESM, root/dist declarations and ancillary
  packaging stay fingerprinted/non-importable, and hidden tests remain rejected. See `benchmarks/bun-shared-runs.md` and `benchmarks/bun-dismissals.md`.
- `src/node.rs` implements the pinned Node 24.22.0 runner for one
  dependency-free ESM package: JavaScript and native TypeScript are independently Validated.
  `node/project.rs` validates the original root and copies; `node/protocol.rs` requires file/global completion and hook
  evidence. The exact-hash `node/observer.mjs` detects cleanup errors suppressed
  after body failures. Preserve fail-closed checks and include observer/reporter
  changes in execution identity. Body exceptions require exact worker evidence matched to file/test ID; hooks,
  plans, unhandled process errors and ambiguous failures remain errors. Dynamic
  imports are rejected; non-native thenable rejections remain unclassified.
  `benchmarks/node-body-attribution.md` records the expanded boundary. `tests/node_workflow.rs` and the ignored cleanup probe
  exercise real Node runs; `benchmarks/node-initial-scope.md` records scope and
  real-project findings. `node/typescript.rs` audits native erasable TS and optional
  plain-JSON NodeNext config without type checking or transforms. Config presence/bytes
  and the test glob join TS execution identity; legacy JS identity stays compatible.
  `benchmarks/node-typescript-scope.md` records the typed fixture, 42 attribution
  cases, cleanup and independent real-project evidence. `node/discovery.rs` audits
  native default selection across the whole tree, rejects tests outside the audited
  test directory, and verifies native selection plus implementation hashes.
  Preserve the original/snapshot test-set check so report exclusions cannot hide tests.
  `benchmarks/node-default-discovery.md` closes unchanged Bragkit acceptance; explicit
  JS/TS recipes remain compatible and default mode has a distinct identity.
- `src/javascript.rs` shares concrete boolean discovery with Vitest and Node.
  Literal NUL masking is parse-only, preserves byte offsets, and rejects NULs in
  executable code; source fingerprints and writes always use original bytes.
- `src/vitest.rs` exposes read-only `vitest-list`; `vitest/project.rs` validates
  pinned ESM/pnpm metadata and `vitest/discovery.rs` lists JS/TS boolean flips.
  `vitest/config.rs` statically recognizes coverage-reporter-only configs without
  executing them; fingerprint accepted files and reject all other config settings.
  `vitest/execution.rs` and `vitest/protocol.rs` implement isolated commands and
  strict completion evidence; `vitest-check` exposes baseline diagnostics. Explicit
  `--lang vitest` connects serial full-suite runs to shared reports and scores.
  Focused reruns validate saved source and execution identity; the browser exports
  explicit runner commands. Dismissal CRUD/check and CI policies use the shared
  source-identity and score machinery; `tests/support/vitest_dismissals.rs` and
  `benchmarks/vitest-dismissals.md` record JS/TS review and policy acceptance.
  `vitest/lifecycle.rs` instruments only an exact-hash isolated runner copy to
  distinguish body failures from otherwise unreported lifecycle failures; see
  `benchmarks/vitest-lifecycle-attribution.md`. Preserve the fail-closed byte
  check and include instrumentation changes in execution identity. Ignored scoped
  fixture regressions cover file/worker teardown, nested hooks and concurrent
  body/cleanup mixtures; they do not establish cross-file fixture reuse.
  Parallel workers remain unsupported. `benchmarks/vitest-focused-reruns.md`
  records rerun acceptance.
  `tests/vitest_workflow.rs` checks real scored CLI runs and saved failures; see
  `benchmarks/vitest-shared-runs.md`. Real adapter and CLI tests are
  ignored and require `MUTIE_VITEST_FIXTURE`; `benchmarks/vitest-execution.md`
  records outcome/cleanup evidence and conservative hook limits.
  `tests/vitest_preview.rs` covers the preview CLI;
  `benchmarks/vitest-initial-scope.md` distinguishes direct probes, adapted Pathe
  discovery and the remaining execution/configuration acceptance gates.
- `src/workspace.rs` manages isolated copies; `process.rs` handles subprocesses and timeouts.
  Its bounded process-group shutdown retries signals to catch racing forks;
  `benchmarks/process-reliability.md` records the regression evidence.
  Its ignored `nested_unmutated_test_binary` probe accepts a prebuilt integration
  test through `MUTIE_REPRO_TEST_BINARY`; `benchmarks/rust-nested-self-tests.md`
  records serial/parallel controls and the remaining self-run investigation.
  `tests/support/rust_adapter_probe.rs` adds an ignored complete Cargo-adapter
  comparison using two isolated projects under `MUTIE_REPRO_ROOT`, including
  fresh, cold-cache and warm-cache executions. The paired variant isolates a
  candidate from its pristine control; `benchmarks/rust-survivor-isolation.md`
  records score-boundary evidence and a second capture fix: serialize spawning
  to prevent cross-command pipe inheritance.
  `benchmarks/rust-parallel-score-reruns.md` records the subsequent successful
  fresh/cached six-mutation comparison and its limits.
- `src/capture.rs` owns file-backed output and head/tail retention; outcome
  classification reads complete captures independently of the saved-log limit.
- `src/workspace_metrics.rs` aggregates workspace timings, copy methods, and partial failures.
- `src/report.rs` defines outcomes; `saved_report.rs` persists reports, diffs, and logs.
- `src/operator_summary.rs` derives per-operator counts and cumulative command time
  for saved text summaries and the report browser.
- `src/score.rs` computes raw mutation scores, selection scope, and exact minimum-score
  decisions independently of dismissal-based findings policy.
- `src/report_reader.rs` validates saved runs for reruns and browsing;
  `src/browser.rs` owns the full-screen report browser and command export.
- `src/rerun.rs` selects saved findings through fresh source identities; Rust
  reruns also validate manifest/lockfile, target settings and toolchain identity.
- `src/report_retention.rs` lists and prunes finalized saved runs;
  `src/report_lock.rs` shares lifetime locks between writers and pruning, with
  explicit unlock-on-drop so inherited descriptors cannot extend ownership.
- `src/identity.rs` shares strict source matching; `dismissals.rs` stores review
  decisions, implements dismissal CRUD and rediscovery checks, and captures
  validated dismissal snapshots for saved reports and the browser.
- `tests/go_workflow.rs` covers the Go workflow; `configuration.rs` tests config behavior, `cancellation.rs` tests shutdown, and `terminal.rs` exercises the CLI in a pseudo-terminal. `tests/fixtures/go/` provides the sample Go module.
- `tests/fixtures/rust/` is an independent Cargo package for the initial Rust
  slice, with unit/CLI tests and known boolean-flip outcomes. It is not exercised
  by the root `cargo test`; see its README. Mutie itself is the selected real
  Rust validation project; `benchmarks/rust-initial-scope.md` records the audit
  and `benchmarks/rust-first-slice.md` records CLI validation and runtime limits.
  `benchmarks/rust-focused-reruns.md` records focused rerun validation.
  `benchmarks/rust-dismissals.md` records Rust review and policy validation.
- `tests/rust_workflow.rs` runs real Cargo workflows with `--ignored`; keep these
  opt-in so Mutie's default self-test suite cannot recursively run mutations.
  `tests/support/rust_cache.rs` checks opt-in worker-local Cargo dependency reuse
  against fresh builds, backdated source, restoration and stronger-test reruns.
  `benchmarks/rust-build-cache.md` records the controlled Mutie comparison.
- `tests/fixtures/rust-boundaries/` checks Rust ordering mutations, generic
  borrowing, floating-point cases, operand effects and a stronger-test rerun.
  `benchmarks/rust-comparison-boundaries.md` records fixture and self-run evidence.
- `tests/fixtures/rust-logical/` checks logical swaps, grouping, short-circuit
  effects, let-chain exclusions, and a stronger-test focused rerun.
  `benchmarks/rust-logical-swaps.md` records fixture and Mutie validation.
- `tests/fixtures/go-branches/` exercises branch forcing with Go type checking,
  unused-name preservation, short-circuit behavior, and focused reruns.
- `tests/fixtures/go-loops/` checks loop skipping, initializer/condition/body/post
  effects, type checking, and stronger-test reruns.
- `tests/report_browser.rs` checks the saved-report browser in a pseudo-terminal;
  `tests/snapshots/` holds its wide and compact renderer snapshots.
- `tests/report_retention.rs` exercises previews, explicit deletion, and active-run
  protection across processes without requiring Go for retention commands.
- `tests/support/shell_fixture.rs` installs test-only Go commands through the
  existing system shell and ordinary copied `test`/`list` input files. This avoids
  reproduced fresh-executable loader stalls without changing production spawning
  or cancellation assertions; see `benchmarks/process-readiness.md`. Remove the
  fixture's `go` symlink before replacing it for a startup-error control; never
  write through it to the system shell.
- `tests/support/mock_node.rs` is an identity-only native mock compiled by the
  Vitest preview fixtures. Preparation has a ten-second bound; response files
  vary separately from its immutable executable. Preserve exact probe arguments,
  pinned identity errors and the one-launcher-call assertion.
- `tests/log_limits.rs` checks log retention, outcome preservation, complete
  discovery, reruns, noisy failures, signal cleanup, and source preservation.
- `tests/capture_limits.rs` checks raw output budgets, overflow errors, baseline
  and discovery failures, saved diagnostics, policy precedence, and reruns.
- `tests/dismissals.rs` checks persistence, invalidation, metadata edits, and
  preservation of execution outcomes and saved reports; it also exercises
  dismissal-based and minimum-score CI policies, configuration, and focused runs.
- `README.md` is the short project overview and getting-started guide. Detailed usage,
  runners, operators, reports and development instructions live in `docs/`.
  `DESIGN.md` records architecture and plans, and `TODO.md` tracks progress.
  Keep these synchronized with verified changes; planned features are not necessarily implemented.
- Mutie's original code is dual-licensed MIT OR Apache-2.0. Retained upstream
  excerpts keep their own licenses; update `THIRD_PARTY_NOTICES.md` and
  `licenses/third-party/` when adding such material. Sanitize published benchmark
  paths and host metadata as described in `docs/benchmarks.md`; historical hashes
  still refer to the original audit bytes, not the sanitized publication copies.
- `SUPPORT.md` tracks support levels, scope and validation evidence for each
  language/test runner. Update it when support changes; Bun JS/TS are independently Validated within
  their scope; Python/pytest is Validated within its pinned serial flat/src-package scope.

## Development Status & Versioning

The Go MVP and initial Rust binary-package milestone are complete for their
supported scopes. Rust supports boolean flips, comparison boundaries, logical
swaps, focused reruns and dismissal review. `benchmarks/rust-milestone-acceptance.md`
records the binary scope, evidence and limits. Ordinary library-only and
library-plus-one-binary packages now have validated support within their scope;
`benchmarks/rust-library-scope.md` tracks fixture acceptance and
`benchmarks/rust-library-marathon.md` records real-project findings, stronger-test
reruns, fresh/cached agreement and limits. Workspaces remain unsupported.
The first focused-feedback optimization is validated in
`benchmarks/rust-report-write-reruns.md`: one saved-selection self-run improved
from 564 to 393 seconds with matching failures and preserved sources/artifacts.
Follow the scoped Vitest milestone in `TODO.md`, `DESIGN.md` and
`benchmarks/vitest-initial-scope.md`: one ESM/pnpm package, separate JS/TS boolean
discovery, isolated execution and explicit completion/error evidence. Shared runs,
reports, focused reruns and review/policies are integrated. The first complete
Pathe run with its original coverage-only config produced 12 kills and two
survivors in 94 seconds; `benchmarks/vitest-coverage-config.md` records evidence.
Both survivors now have observable boundary cases killed by stronger tests in
an 18.79-second focused rerun; `benchmarks/vitest-pathe-survivors.md` records
Validated TypeScript milestone acceptance. Independent JavaScript acceptance is
recorded in `benchmarks/vitest-hyparquet-findings.md`: 19 sampled candidates, two
survivors and a stronger-test rerun killing both.
Track JS and TS validation independently. Consider Rust workspaces separately. Preserve the
existing binary workflow, package rebuilds and full test scope as support expands.

JavaScript validation now targets Hyparquet: `benchmarks/vitest-javascript-hyparquet.md`
records 133 discovered flips and a passing 724-test isolated baseline. NodeNext
tsconfig support is limited to JavaScript-only type checking; runtime TypeScript
remains ESNext-only. JavaScript is Validated within the pinned serial scope.
`benchmarks/vitest-feedback-profile.md` records repeated focused runs and test-only
phase timings. Node now resolves once per run and revalidates the pinned executable
without repeating the launcher; `benchmarks/vitest-node-reuse.md` records validation.
`benchmarks/vitest-hyparquet-full.md` records all 133 attempted candidates across
three runs (89 kills, 41 survivors, two callback errors, one timeout). A separate
test-only review rerun kills 36 survivors and both error candidates; five remain
(four redundant scan settings, one unknown-filter-operator policy case). Preserve
failed-report scores and distinguish combined inventory accounting from a full
score. Retained boundary tests and awaited-callback patch are under
`benchmarks/fixtures/`. The first Node test-runner slice is now Validated for
JavaScript only; `benchmarks/node-initial-scope.md` records the real-project loop.
Body-error attribution now matches exact worker evidence and includes process-level
late-failure checks; see `benchmarks/node-body-attribution.md`.
`benchmarks/node-sri-final-review.md` records five more cleanup tests and a bounded
pending-promise diagnostic: the survivor is not proven equivalent and the full-suite
timeout remains excluded from scores. `benchmarks/node-openclaw.md` records a
second unchanged project: 39 candidates (22 kills, 15 survivors, two timeouts, no
errors), plus three stronger-test kills and two timeout-to-kill reruns after
a test-only teardown fix. `benchmarks/node-storage-profiles.md` records five more
kills (storage locking and workspace profiles), source/report preservation and the
Validated milestone audit. Seven scheduling/delivery/entry-point survivors remain
open; SRI's cleanup survivor is unresolved and its semantic hang remains a timeout.
Existing explicit-glob JavaScript behavior remains compatible. Native TypeScript on Node is now
selected and independently Validated: dependency-atlas passes nine tests with
no boolean sites; Bragkit has 23 candidates (two kills, 21 survivors), and a separate
stronger-test rerun kills three HTML-list survivors. Bragkit's `node --test` script
was adapted to `node --test test/*.test.ts`; source/tests/config were unchanged.
`benchmarks/node-typescript-scope.md` and its results record preserve that initial
adaptation. `benchmarks/node-default-discovery.md` now records unchanged Bragkit
acceptance with matching full outcomes and three stronger-test kills. Native default
discovery includes nested/helper files, rejects selected files outside audited test/,
and preserves all original input/report hashes. `benchmarks/node-bragkit-survivors.md`
records all 18 remaining survivor kills with 13 stronger tests, a 76-test baseline
and unchanged source/report hashes. Original full scores remain unchanged.
Do not infer broader loaders/dependencies or layouts from this evidence.

Bun is the selected next runner. `benchmarks/bun-initial-scope.md` pins
1.3.11+af24e281e, records 24 native reporting cases per syntax and an unchanged
272-test Composer baseline with a manual stronger-test finding. JUnit and inspector
failure status conflate bodies, hooks and process errors. The file-backed phase
observer in `benchmarks/bun-body-attribution.md` passes 112 paired JS/TS cases and
30 evidence-rejection checks; Composer baseline/finding controls match. A failure-only
checkpoint exposes suppressed pending rejections without delaying successful
callbacks. Inspector terminal events can be lost at exit and cannot be guessed.
The Rust reader now supports scored serial runs, saved reports and focused reruns.
Independent JS/TS fixture workflows and Composer evidence establish scoped
TypeScript validation. JavaScript has independent pp-is acceptance. `benchmarks/bun-scored-cleanup.md` records independent JS/TS scored
resource limits, timeout/SIGINT, descendant shutdown and score boundaries.
`benchmarks/bun-dismissals.md` records independent JS/TS CRUD, staleness, raw-score
preservation and error precedence. `benchmarks/bun-javascript-pp-is.md` records
22 JavaScript candidates (12 kills, ten survivors) and seven stronger-test kills,
with preserved inputs/reports. Initial acceptance added explicit ESM and omitted
packaging files in a temporary copy only; source/tests were unchanged. The original pp-is
checkout now passes under the narrow layout extension recorded in
`benchmarks/bun-unchanged-layout.md`, matching the adapted inventory and outcomes.
Implicit ESM is JS-only with static imports/exports; declaration/packaging files
remain outside executable imports and hidden tests are rejected. See `benchmarks/bun-shared-runs.md`.
`benchmarks/bun-composer-survivors.md` closes the four remaining Composer survivors
and queue timeout with test-only changes: all five are killed in a 276-test
full-suite focused rerun. The queue mutation blocks dispatch; precondition
assertions avoid waiting for impossible idle. Preserve original full scores and
distinguish focused evidence from a full stronger-test score.
`benchmarks/bun-feedback-profile.md` records a debug/release comparison with
matching identities/outcomes and test-only adapter phases: executable SHA-256
checks dominate, not copying. Prefer release builds for real feedback. Development
and test profiles optimize only the sha2 dependency, preserving byte-verification
rules; `src/bun/profile.rs` remains an ignored opt-in measurement probe.

Python/pytest's initial pinned, serial scope is Validated.
`benchmarks/python-initial-scope.md` pins CPython 3.14.7 / pytest 9.0.2 and records
39 native probes plus five evidence-rejection controls. Adapted Inflection runs
467 tests; a manual Unicode-digit regex survivor is killed with a 468-test
stronger baseline. This is not an implemented operator or scored run. A fresh
pycache prefix defeats a demonstrated stale-cache false result; `-B` alone does
not. The prototype intentionally retains two unsafe classifications: unittest
setup as a body failure and late atexit failure as passing. Do not reuse it for
scores. `benchmarks/python-process-lifetime.md` records a separate native lifetime
observer that initially rejected late cleanup, incomplete evidence, unittest and
thread-starting suites. Keep uncaptured stderr fail-closed and wait for process
exit before accepting checkpoints; destructor errors can follow atexit.
`src/python.rs` exposes static `python-list` and isolated `python-check`, using
the pinned explicit venv launcher without canonicalizing its symlink.
`src/python/scope.py` never imports project code; `runtime.py` checks the
environment with site initialization disabled. `python/protocol.rs` requires
complete passing phases and process-lifetime evidence bound to the actual PID.
`benchmarks/python-baseline-integration.md` records this narrow preview.
`benchmarks/python-process-integration.md` records 18 production observer cases,
including bounded captures/files and descendant shutdown on exit/timeout/cancel.
Python rejects the shared runner's `group_remaining` flag after interpreter exit.
Typed execution identity includes observer/validator/classifier/process recipes.
`python/project.rs` validates shared mutation spans and source hashes.
The runner returns shared command outputs, including failure captures; direct
fixture controls verify kills/survivors and reject runtime/collection errors.
Explicit `--lang python --python PATH` now connects shared runs/reports/reruns
and review/policies. Candidate test IDs must match isolated pristine collection;
stronger-test reruns collect a new pristine inventory. See
`benchmarks/python-shared-runs.md` for native workflow tests, preservation,
review/policy and corrupt-identity controls. Nested command futures are boxed to
fit ordinary Tokio worker stacks. Scored timeout/cancellation/capture/file-limit
probes pass. `benchmarks/python-scrubify-findings.md` records the real-project
loop: six tests, one kill/two survivors, then nine tests and a focused kill for
blank query preservation. Source/test bytes were unchanged in the documented
layout/metadata adaptation, and all 28 original report files remain unchanged.
The doseq survivor remains recorded. Additional 107-test case-converter evidence
preserves conservative runtime-error classification. Keep the standard-library
API whitelist narrow; basic logging/StringIO, shallow copy, index-protocol access,
and named fnmatch/urllib.parse string helpers do not imply filesystem/network or
general reflection support. Broader layouts, dependencies and runtimes require
new acceptance; do not infer unchanged-project compatibility from adapted inputs.
`benchmarks/python-feedback-profile.md` records runtime traversal profiling and
the serial Scrubify comparison. Preserve byte-for-byte runtime fingerprints,
every pre/post check and fresh bytecode prefixes when optimizing overhead.
`tests/support/python_runtime.py` covers traversal compatibility and drift;
the ignored `python::profile::real_feedback_phase_profile` measures real phases.
`benchmarks/python-src-layout.md` now records native `src/package/` support and
unchanged Scrubify layout/manifest acceptance. `source_root` binds snapshot,
source-path and rerun identity (empty for flat, `src` for native src). Keep exact
owned application import checks and all original/snapshot tests. Only static
setuptools src mapping, matching pythonpath, quiet addopts and a compatible
pytest-only test extra are admitted; do not execute builds or install project
extras. The separate stronger-test copy kills the blank-query survivor while
preserving the clean original checkout and prior report hashes.
`benchmarks/python-cycler-unchanged.md` records a second unchanged project with
28 root-level tests. Preserve the combined root/directory test inventory and
reject root tests hidden by configured testpaths. Flat setuptools literal-version
metadata and audited optional tool declarations never execute or install anything.
The narrowly retained documentation/CI scripts stay non-importable and cannot
hide tests/hooks. The original mixed assertion/KeyError remains an error; a
separate test-only key-set assertion turns it into a kill without changing the
classifier or original report scores.


Prioritize milestone progress, demonstrated correctness/reliability blockers,
and explicit user requests. Do not automatically propose another small Go feature,
operator, or UI refinement because it is an unchecked backlog item. Judge progress
by the run/inspect/improve-tests/rerun workflow on real projects, including runtime
and finding usefulness. Extract the language boundary as Go and Rust need it;
keep existing Go behavior covered and defer speculative plugin abstractions.
Distinguish planned Rust support from verified behavior in documentation and CLI
claims. Reassess the next major objective when the milestone is complete.

Mutie is still in local development and has no external users. Breaking changes are acceptable at this stage. Do not bump project versions (including the package, report schema, and operator catalog versions) unless explicitly requested; version bumps for ordinary development changes are unnecessary noise for now.

`benchmarks/python-unittest-pluralizer.md` records scoped synchronous unittest
support through pytest. Preserve the ordered dispatch trace, exact body-exception
identity, protected TestCase methods and strict per-node framework evidence.
Setup/teardown/cleanup mixtures remain errors. Direct subclasses only: no custom
dispatch/constructors, subtests, skips/expected failures, async cases, mock/runner
APIs or module cleanup registration. Narrow Hatch/Ruff/development metadata stays
inert. Original Pluralizer inputs/reports and the redundant membership-value
survivor must remain distinct from stronger-test evidence.

The scoped conftest fixture extension is Validated; see
`benchmarks/python-conftest-fixtures.md`. Root and direct tests/conftest.py fixture
modules are statically audited and included in snapshot/execution identity.
Preserve independent fixture setup/finalization errors and the complete lifetime
ledger: caught dynamic setup failures must not become body kills. Unchanged
Binary Tree Library acceptance accounts for 11 candidates (five kills, six
survivors); separate-copy stronger tests kill all six. Preserve original scores
and inputs. Plain pytest.ini testpaths and empty tests/__init__.py also join
snapshot/execution identity. General hooks, plugins and dependencies remain
excluded; do not trim upstream suites to force acceptance.

Unchanged Route Mini acceptance (`benchmarks/python-route-mini.md`) covers 68
tests, four shared fixtures and 38 candidates across four runs. Ordinary audited
conftest helpers and direct unaliased helper imports are supported; initializer
docstrings and inert examples/tool metadata do not broaden executable test scope.
Preserve source-identity matching across report-local IDs and failed report scores.
Three audit-status survivors are killed by separate-copy tests; 25 survivors,
three errors and one timeout remain open, with no equivalence claims.

`benchmarks/python-route-mini-review.md` records the subsequent separate-copy
review. Expected-success exhaustion is an explicit test assertion and provider-down
routing has a no-retry precondition; no classifier or timeout was relaxed. Preserve
original failed reports and keep native reflective slots diagnostics outside scored
Python support. Slots survivors are not declared equivalent or dismissed.

Python supports boolean flips, single-comparison `comparison_boundary`, and
binary `+ ↔ -` / `* ↔ /` `arithmetic_replace` mutations.
See `benchmarks/python-comparison-boundaries.md`: preserve native operator-token
replacement and operand evaluation; chains, annotation subtrees and tests remain
excluded. Custom rich-comparison exceptions are errors. Require exact saved
operator/token pairs and fresh execution identity across recipe changes. The
Route Mini exact-capacity finding uses an unchanged original and separate stronger
copy; preserve historical scores and inputs.

## Build, Test, and Development Commands

Use Rust/Cargo and Go 1.24+ on `PATH`. Real Go tests also require a writable Go build cache.

- `cargo build` — compile the CLI.
- `cargo run -- list /path/to/go/module` — inspect mutations without running tests.
- `cargo run -- run tests/fixtures/go --jobs 2 --timeout 5` — exercise the bundled fixture.
- `cargo test` — run unit and non-ignored integration tests.
- `cargo test --test go_workflow -- --ignored` — run the opt-in, real-Go workflow test.
- `cargo test --test rust_workflow -- --ignored --test-threads=1` — run real Cargo workflows.
- `cargo fmt --check` — check formatting; use `cargo fmt` to apply it.
- `cargo clippy --all-targets -- -D warnings` — lint all targets.

Vitest preview tests use macOS's `/usr/bin/tclsh` with private identity-only script
data to avoid observed fresh Mach-O helper startup stalls; other Unix platforms
retain the native Rust helper. The ten-second readiness limit and exact identity
assertions remain unchanged. See `benchmarks/vitest-fixture-startup.md`.
Cancellation/log-limit readiness failures now attempt bounded macOS stack
sampling of their owned, unreaped CLI after the original deadline fails. Sampling
denial does not suppress the failure; a deterministic negative control verifies
the deadline and child reaping. `benchmarks/cli-startup-readiness.md` records actual
CLI pre-main stalls and the still-unresolved host-level remedy. Passing reruns do
not establish that this intermittent issue is fixed. The user deferred the
investigation on 2026-10-21; keep it open and preserve deadlines/diagnostics.

## Coding Style & Naming Conventions

Follow Rustfmt defaults, including four-space indentation. Use `snake_case` for functions/modules, `PascalCase` for types, and `UPPER_SNAKE_CASE` for constants. Format Go fixtures with `gofmt`. Keep language-specific behavior separate from execution and reporting, and add context to fallible operations using the existing `anyhow` conventions.

## Testing Guidelines

Place unit tests beside implementation in `#[cfg(test)]` modules; use `#[tokio::test]` for asynchronous behavior. Name tests descriptively in `snake_case`. Add regression tests for changed behavior, especially mutation boundaries, outcome classification, workspace restoration, timeout cleanup, and saved reports. There is no enforced coverage percentage. Tests must preserve input projects and use temporary workspaces.

## Commit & Pull Request Guidelines

History uses short descriptive subjects such as “First pass” and “Adds saved reports”; no Conventional Commits prefix is established. Keep commits focused. PRs should explain the problem, behavior changes, and validation commands/results; link relevant issues and include terminal examples or screenshots for presentation changes.

Do not commit generated `target/` or `.mutie/` directories.

`benchmarks/python-route-mini-all-boundaries.md` records all seven comparison
candidates across two original reports (four kills, two survivors, one error).
A separate 70-test copy kills the new network-retry ceiling survivor by checking
the exact retry decision and primary call count. Preserve the original constructor
error, prior capacity finding and report-local source-identity mapping; combined
accounting is not a full score. This revisits Route Mini, not a new third project.

`benchmarks/python-betting-math.md` validates a new unchanged 198-test project.
Only audited optional-tool minimum versions, bounded Ruff/Mypy fields and direct
`random.Random(...)` construction extend scope. Preserve rejection of module RNG
mutation/escape and constructor/class escape. Six selected metrics survivors are
killed by a separate 205-test suite; four original runtime errors remain errors.
Selected ten-candidate accounting is not a full 87-site project score. Keep
original inputs/reports and the exact source-identity mapping across reruns.

`benchmarks/python-betting-math-full.md` accounts for all 87 candidates by source
identity: 19 kills, 59 survivors, nine errors. A separate 222-test copy kills 24
survivors (including six prior metrics findings) and two original endpoint errors
through explicit expected-success assertions. Preserve original failed reports,
classification rules and numerical survivors without blanket equivalence claims.
Combined inventory accounting is not a full original or stronger-suite score.

`benchmarks/python-arithmetic.md` validates Python binary `+ ↔ -` and `* ↔ /`
under `arithmetic_replace`. Preserve native grouping/evaluation and exact token
pairs; annotations, unary/augmented operators, floor division and powers
remain excluded. Modulo was added later in `benchmarks/python-modulo.md`.
This is not numeric type inference: unsupported operands and
zero division stay errors. Unchanged betting-math-kit's 30-site Kelly selection
has 21 kills, eight survivors and one string-operand error; a separate 201-test
copy kills all eight survivors. Preserve prior inputs/reports and recipe drift
checks; selected evidence is not a full 194-arithmetic-site score.

The [complete arithmetic review](benchmarks/python-arithmetic-full.md) accounts
for all 194 sites on unchanged betting-math-kit: 61 kills, 97 survivors and
36 native errors. A separate 219-test copy kills 54 more survivors; with eight
prior kills, 35 remain open. Numerical questions are recorded per candidate. Preserve original report scores and the
source-identity mapping; combined accounting is not a full-run score.

`--continue-on-error MAX_ERRORS` is invocation-only and Python-only. It continues
only positively typed ordinary pytest body errors after complete lifecycle,
inventory, process cleanup and input/runtime checks. Reaching the positive limit
stops execution. Type/value/arithmetic/lookup subclasses qualify; resource/system,
unittest and lifecycle errors remain fatal. Runs containing errors stay failed,
exit 1 and unscored. Reruns must opt in again, and changed observer/protocol recipes
require fresh execution identities. See `benchmarks/python-candidate-continuation.md`;
never infer eligibility from error names, exit status or generic adapter errors.
The unchanged 194-candidate arithmetic batch matches all prior outcomes
(61 kills, 97 survivors, 36 errors) in one failed, unscored report, preserving
original inputs and historical reports. Native exception, lifecycle/resource,
limit and cancellation controls establish this narrow continuation boundary.

`benchmarks/python-tomli-w.md` records unchanged Tomli-W acceptance: 271 cases,
30 candidates (nine kills, two survivors, 19 errors), and a separate 272-case
stronger-test rerun killing the default newline-format survivor. Preserve failed,
unscored original reports and distinguish combined accounting from a full score.
Non-Python data hashes and directory membership join snapshot/rerun identity.
Test-only path access is runtime-guarded; denied accesses emit fatal evidence even
when caught. Input/runtime reads, owned temporary output and opening the null
device do not imply general filesystem or sandbox support. Explicit test xfail
calls require exact native evidence and the pristine expected-failure node set;
new/missing expected failures and cleanup errors remain errors. General skips,
marks/plugins and unittest expected failures remain excluded. The fixed profiler
iteration value and all observer/scope changes remain bound to execution identity.

`benchmarks/python-tomli-w-review.md` reviews all 19 errors and the remaining
TYPE_CHECKING survivor. Thirteen errors are operand incompatibilities within the
syntactic arithmetic contract, not demonstrated discovery defects. Four parser
errors become kills after a test-only output-validity assertion; three error
controls and the survivor retain their outcomes. Preserve original scores and do
not describe assertion attribution refinement as new test coverage or equivalence.

`benchmarks/python-dependencies-tinycss2.md` records the first application dependency:
exact-hash webencodings 0.5.1, declared explicitly and accessed through five named
imports. Reject missing/undeclared packages, byte drift, transitives, native
additions and entry points; preserve every pre/post identity check. Never resolve
or install dependencies inside Mutie. Unchanged Tinycss2 1.2.1 passes 480 tests;
14 selected candidates have four survivors/ten errors, and separate-copy tests
kill all four with 484 cases. Preserve original failed scores and distinguish the
92-site discovery inventory from the 14 executed candidates. Current Tinycss2's
17,120-test native control is not production acceptance. Retain lifecycle errors
and conservative exception classification across dependency calls.

`benchmarks/package-managers.md` records npm 11.19.1 lock-v3, Yarn 4.18.1
node-modules/classic, Bun 1.3.11 hoisted text-lock and original pnpm 11.10.0
isolated-store acceptance. Keep installer identity distinct from runner identity.
Vitest resolves its exact-byte worker through owned node_modules ancestry; complete
installation content joins snapshot and focused-rerun identity. Node/Bun preserve
and fingerprint declared development installations without admitting runtime
package imports. The 16-case JS/TS matrix and content-drift controls pass; unchanged
Bragkit with installed development tools retains all 23 outcomes and three stronger
HTML boundary kills. Preserve original inputs/reports. PnP, alternative linkers,
workspaces and Vite+'s bundled `vp test` need separate acceptance; incidental Vite+
devDependencies do not select a runner. No package manager is invoked by Mutie.

`benchmarks/rust-workspaces.md` supersedes earlier workspace exclusions. Ordinary
virtual/root-package workspaces support member selection (`--rust-package`,
`rust_packages`), inherited metadata/dependencies, and internal path dependencies.
Selection narrows mutations; every baseline/mutation tests all members with
`--workspace --tests`. Worker caches clean all local packages, including unselected
dependents. Bind every member manifest/target, root manifest/lock, toolchain and
selection to reruns; preserve standalone empty-workspace report compatibility.
Explicit member selection is a filtered score in both writer and reader. Non-test
auxiliary targets and release/bench-only profiles are inert under the test command.
Build scripts, workspace proc macros, external paths/git dependencies, custom
dev/test profiles and feature-gated test targets remain separate scope. UCD Generate
provides unchanged root-workspace evidence: 140 tests, fresh/cached agreement on
four boolean survivors, and three stronger-test kills with 142 tests. Preserve the
failed exploratory abort report, original inputs/reports and unresolved fourth
survivor; do not infer a full workspace mutation score from member/operator filtering.


`benchmarks/javascript-comparisons.md` records shared JS/TS comparison inversion
and boundary support across Vitest, Node and Bun. Replace only parsed operator
tokens; preserve strict versus loose equality, native operand/coercion order,
NaN behavior and binary grouping. Do not describe ordering inversions as Boolean
negation. Keep type syntax, text, shifts, in/instanceof and tests excluded. Saved
JS/TS reports require exact operator/token pairs; discovery order is source span
then operator. Six independent scored JS/TS workflows and native semantic probes
cover the extension. Unchanged pp-is IPv4 validation has four inversion kills and
two boundary survivors, killed by endpoint tests in a separate copy. Preserve
original inputs/reports and filtered scores; no classifier or lifecycle rule was
relaxed. Bun's source-bound recipe changes require fresh original scans.


`benchmarks/javascript-logical.md` records JS/TS logical swaps and negation removal
across Vitest, Node and Bun. Preserve parsed operand/parent grouping with the full
expression recipe, trivia and raw JS values. Changed short-circuit decisions are
intentional; duplicate evaluation and added coercion are not. Keep TS postfix
assertions, nullish/logical assignment and bitwise operators excluded. Saved
logical edits must reconstruct exactly from the parsed original, and reruns still
require source identity. Unchanged pp-is has eight selected candidates (six kills,
two survivors); separate-copy boxed-Boolean tests kill the outer swap while the
inner fast-path survivor remains open. Preserve original scores/inputs/reports.


`benchmarks/javascript-arithmetic.md` records JS/TS binary arithmetic and remainder
replacement (`+ ↔ -`, `* ↔ /`, `% → *`) across Vitest, Node and Bun. Preserve parsed
operand/parent grouping and trivia through the full-expression recipe. Native
coercion hints, concatenation, signed zero, NaN/Infinity, BigInt truncation and
exceptions apply; no type inference or error reclassification is introduced.
Unary signs, updates, augmented assignment and powers remain excluded. Saved
arithmetic edits must reconstruct exactly. Six scored runner/syntax workflows
cover both arithmetic and remainder; unchanged Bragkit contributes two arithmetic
weight survivors killed by separate-copy exact-score assertions. Keep original
61-test inputs and reports distinct from the 62-test stronger rerun.

`benchmarks/rust-arithmetic.md` records binary `+ ↔ -`, `* ↔ /`, `% → *`
support. Replace only parsed operator tokens; insert a separating space when a
new `/` immediately precedes a comment. Preserve native grouping, evaluation,
borrowing/moves and trait dispatch. Keep unary/dereference/compound assignment,
macro/test/constant/type exclusions. Exact saved pairs and source rediscovery bind
reruns. Missing traits remain build failures, test panics retain existing Cargo
classification, and unsafe-invariant aborts remain errors. UCD Generate contributes
two killed Hangul remainder sites and two hyphen-index survivors killed in a
separate stronger copy. Preserve the failed 16-site exploratory report (one abort,
two unfinished), the original 140-test inputs, and the distinct 141-test rerun.


`benchmarks/rust-inversions.md` records Rust comparison inversion and Not removal.
Keep exact comparison token pairs and native borrowing/trait dispatch; NaN ordering
inversions are not Boolean complements. Negation uses `(operand,).0` to preserve
value evaluation beneath borrows; Boolean, integer and custom Not are eligible,
with output-type changes classified by Cargo. Never types/macro bangs are excluded.
Readers reconstruct the parsed unary recipe; deduplication includes operator so
inversion and boundary edits at one span coexist. Preserve outer-before-inner
logical order. Unchanged UCD Parse has 13 selected sites (11 kills, two survivors);
separate display/round-trip tests kill both. Original 140-test inputs/reports remain
separate from the stronger 142-test copy. No classifier or deadline was relaxed.


`benchmarks/python-inversions.md` records scoped Python comparison inversion and
`not` removal. Preserve native rich-comparison/reflected dispatch and raw values;
ordering inversion is not Boolean negation for NaN/custom comparisons. Negation
uses the complete AST span and parenthesized original operand bytes, removing
truthiness without duplication. Chains, identity/membership, bitwise `~`, tests
and annotations stay excluded. Validate exact recipes and multiline coordinates;
allow independent nested/shared spans while rejecting duplicates. Keep fresh
source/execution identity on reruns. Unchanged Tomli-W and betting-math-kit reports
retain native errors and unavailable scores. Two separate-copy median tests kill
simulation ID 8; IDs 13–16 remain open. Preserve all original inputs and 260 report
files, and keep the deferred startup failures distinct from completed checks.


`benchmarks/python-logical.md` records Python binary `and ↔ or` swaps. Admit only
BoolOp nodes with two operands; flat longer chains stay excluded, while nested
binary nodes remain eligible. Preserve native keyword edits, grouping execution,
short-circuit traces and raw values. Stateful truthiness controls compare source
edits with the intended AST swap across value/condition contexts. Saved reports
require exact keyword pairs and fresh source/recipe identity. Unchanged Route Mini
has four selected sites across three original reports: two diagnostic survivors
and two constructor errors. Separate-copy tests kill both survivors with 72 tests;
preserve all 25 original inputs and 145 earlier report files. Combined accounting
is not a full score, and constructor errors are not reclassified. Python's
modulo row was still unfilled at that milestone; the subsequent acceptance below
closes it. Flat logical chains remain a separate scope gap.


`benchmarks/python-modulo.md` supersedes Python's earlier modulo exclusion.
Binary `% → *` uses `arithmetic_replace` and native token replacement. Preserve
AST grouping, evaluation, reflected/subclass dispatch and raw values. This includes
formatting operands: integer repetition may succeed; incompatible multiplication
remains an error. No numeric inference or classifier relaxation. `%=` assignments,
annotations, tests, floor division and powers remain excluded. Exact saved pairs,
source hashes and recipe checks apply. Unchanged betting-math-kit has one modulo
survivor among 25 selected simulation arithmetic sites (four kills, 18 survivors,
three errors). A separate 199-test copy kills it through `simulate_bankroll`'s
odd-trial median contract. Preserve the failed/unscored original report, 20 inputs
and 6,280 historical report files; the focused kill is not a full project score.


`benchmarks/javascript-branches.md` records JS/TS `if` branch forcing across
Vitest, Node and Bun. Conditions become `(true || ORIGINAL)` or
`(false && ORIGINAL)` with ORIGINAL retaining its parentheses. Intentionally skip
condition side effects, exceptions, await/yield and getters; leave branch bodies
unchanged. Exclude syntactic Boolean literals behind parentheses/comments, loops,
ternaries and types. Saved reports must reconstruct the exact condition recipe;
source identity and Bun recipe drift checks remain mandatory. Unchanged Bragkit's
16 selected branches have nine kills/seven survivors; separate-copy exact bonus
and closed-issue-count tests kill all seven. Preserve 57 original inputs, the
original 56.25% filtered score and 295 historical report files. Do not generalize
native TypeScript acceptance to type-checking builds or change lifecycle attribution.


`benchmarks/rust-branches.md` records ordinary Rust Boolean `if` forcing. Preserve
`true || (ORIGINAL)` / `false && (ORIGINAL)` recipes and deliberately skipped
condition execution. Exclude if-let/let-chains, literal conditions and existing
macro-token/constant/type/test scope; nested ordinary branches remain eligible.
Retained expressions/bodies still undergo Rust ownership/borrowing checks. The
fixture's definite-initialization failures stay build failures. Saved readers
reconstruct exact recipes and require fresh source identities. Unchanged UCD Parse
has 26 selected workspace candidates (15 kills, 11 survivors); a separate 142-test
copy kills field-loss IDs 4/6/10/14. Preserve 108 original inputs, 225 historical
report files, original 57.69% score and seven open survivors; no equivalence claims.


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


`benchmarks/javascript-calls.md` records scoped JS/TS `call_remove` across Vitest,
Node and Bun. Replace complete ordinary call statements with `;`, skipping receiver,
property, argument and invocation effects while preserving statement boundaries.
Exclude value-used calls, constructors, direct super/import, tagged templates and
call subtrees with await/yield. No return-type inference; unawaited promise returns
retain strict lifecycle classification. Saved recipes require exact parsed statement
spans and fresh source identity; Bun recipe changes require fresh reports.
Unchanged OpenClaw has six selected survivors; separate-copy chunk-content tests
kill IDs 1/2, with four other survivors open. Preserve original inputs/reports and
filtered scores; focused evidence is not a full stronger-project score.


`benchmarks/rust-calls.md` records scoped Rust standalone-call removal. Preserve
whole semicolon-terminated statement spans and the exact `{}` replacement; no-op
blocks avoid redundant-semicolon warnings without retaining receiver/argument
side effects. Native ownership/drop timing and type checks remain authoritative.
Calls used as values, semicolon-free tails and subtrees with macros, attributes,
await/try/yield or explicit return/break/continue are excluded. Saved readers must
reparse the recipe and reruns must match source identity. Separate-copy tests kill
two unchanged UCD Parse collection survivors; preserve original 60% filtered score
and input/report hashes. Native semantic and scored fixture tests remain opt-in
and are included in the Rust Linux CI job.

`benchmarks/python-calls.md` records Python standalone-call removal. Replace only
native Expr(Call) statement spans with exact `pass`; preserve empty suites,
parentheses/separators and docstring positions. Skip all receiver/argument/body
effects. Exclude value uses, annotations/tests and every call subtree containing
named expressions, yield/yield-from or await. Rust validates the wire recipe;
fresh AST/source/operator matching establishes syntax on rerun. Ordinary runtime
and lifecycle failures remain errors. Optional false loop markers must also be
omitted by the native helper to match Rust's reserialized discovery document.
Three separate-copy tests kill Route Mini's exception-message/no-route/blocked
telemetry survivors. Preserve the original failed/unscored report, its two errors,
the unresolved sleep survivor and all input/report hashes. Python operator CI
includes the new scored workflows; arbitrary statement/block removal remains out
of scope.

`benchmarks/statement-removal-audit.md` records Python `assignment_remove` for
single-target attribute/item Assign statements replaced by exact `pass`.
Preserve skipped RHS/target/index/store effects and independent fresh AST/operator
identity; the assignment context marker distinguishes its wire recipe from calls.
Exclude name bindings, unpacking/chained targets, annotations, augmented stores,
constructor (__init__/__new__) subtrees and binding/suspension subtrees. Constructor
deletions were 16/16 errors on unchanged Route Mini; do not infer other methods
cannot initialize state. Preserve all runtime/setup/cleanup errors and timeouts.
Two separate-copy tests kill Binary Tree Library rotation root/parent-link
survivors. Draft combined counts are not a full score, and final original reports
stay failed where applicable. Other languages and broader blocks remain separate.
The audit also records an intermittent PTY Ctrl-C classification failure.
`benchmarks/cancellation-exit-race.md` records its controlled reproduction and
fix: retain synchronous receipt of each user signal and reconcile only matching
raw child signal exits after capture/cleanup. Internal peer cancellation, numeric
exits, unrelated signals and capture failures must not become interruptions.
Keep the stopped-parent/completed-child regression, plain/JSON report checks and
native Cargo cleanup coverage. This is distinct from the deferred pre-main startup
investigation; no deadline or assertion was relaxed.

`benchmarks/javascript-assignments.md` records JS/TS `assignment_remove` across
Node, Bun and Vitest. Replace whole standalone property/index `=` statements with
exact `;`; skip receiver/key/RHS/store effects while retaining statement/directive
boundaries. Exclude bindings/destructuring, value uses, compound/chained/nested
assignments, updates, suspension, optional targets, super/private access and class
constructor/field/static-initialization ancestors (including nested callbacks).
Escaped method names are conservative exclusions; do not infer all constructor
functions or all initialization sites. Fresh source/operator discovery must reject
forged constructor spans even when the isolated statement recipe parses. Include
`javascript/assignment.rs` in Bun's execution recipe. Preserve lifecycle errors,
original filtered scores, and unchanged OpenClaw input/report hashes; separate-copy
tests kill session-reset persistence and timestamp-refresh survivors. Broader
block deletion remains separate.

`benchmarks/go-assignments.md` records Go selector/index `assignment_remove`.
Only standalone single-target `=` statements directly in statement lists qualify;
exclude names/declarations, multiple targets, compound assignments, updates,
pointer-only stores, headers, labels and select receive clauses. Use exact empty
replacement, skip all target/key/RHS/store effects, and retain genuine unused-name
build failures. Selection is syntactic (including package-qualified variables);
do not infer constructor or type semantics. Saved snippet validation and fresh
source/operator matching must reject forged headers and changed recipes. Preserve
existing Go classification, timeout/capture-error separation, original scores and
unchanged godotenv input/report hashes. Its three standard-stream survivors are
killed by one separate-copy public-API test; three other candidates do not compile.
Full/affected selection agrees. CI runs the native semantic pairs and scored
workflows; remote validation of the Go commit still requires a push.

`benchmarks/rust-assignments.md` records scoped Rust field/index assignment
removal with exact `{}` replacement. Require a complete semicolon-terminated
statement directly in a block; exclude bindings/destructuring, pointer-only
stores, compound/nested assignments, value uses and tails. Whole subtrees exclude
macros, attributes, await/try/yield and explicit return/break/continue, including
closures. Preserve existing cfg/test/constant/type/generated exclusions and
independent discovery of eligible nested statements. Native controls require
skipping RHS/place/index/deref/store effects and old-value destruction; remaining
values follow native drop rules. Partial moves, inference loss and denied warnings
remain build failures. Exact recipes plus fresh source/operator matching reject
forged test-only spans. Preserve genuine timeout/capture errors and original
filtered scores. Unchanged UCD Parse has ten eligible candidates (six kills, four
survivors); separate-copy tests kill all four, with legacy ISO comment preservation
identified as a compatibility boundary. All 108 original paths, 56 original report
files and the unchanged-copy focused control remain preserved.

`benchmarks/javascript-numbers.md` records the first `number_replace` slice:
JS/TS runtime Number/BigInt only, zero → one and nonzero → zero. Keep one
candidate per literal/direct sign, Number/BigInt category, parenthesized member
syntax, decimal underflow/overflow decisions and exact saved recipes. Exclude
types/enums, property declaration names (including computed names), module
syntax and direct unary plus on BigInt. Fresh source/context discovery must
reject forged property-key spans. Include `javascript/number.rs` in Bun's recipe
fingerprint. Six independent runner/syntax workflows preserve scores, review,
setup-error and timeout boundaries. Unchanged OpenClaw's five selected candidates
produce one kill/four survivors; three separate-copy tests kill all four. Preserve
all 63 input files, 33 original report files and the original 20% filtered score.
This revisits an existing project; broader numeric recipes, other languages and
string literals need separate acceptance.

`benchmarks/python-numbers.md` records Python `number_replace` for runtime
integer/float literals. Preserve exact numeric types, direct-sign ownership,
signed-zero and float rounding/underflow/overflow decisions, and parenthesized
replacement syntax. Booleans, imaginary literals, annotations/type parameters,
match patterns and tests are excluded. Parenthesized sign operands retain the
outer sign; dictionary keys/indexes and f-string expressions remain eligible.
AST admission and the independent Rust wire reader must agree; fresh discovery
rejects forged annotation spans. Fingerprint `python/number.rs` in execution
identity. Keep runtime/lifecycle errors and timeouts separate from kills.
Unchanged betting-math-kit has 42 selected sites: 20 kills, 20 survivors, two
errors across three reports. Three separate-copy default-contract tests kill
three survivors; 17 remain open. Preserve 20 original inputs, 287 report files,
failed scores and report-local ID matching. Combined accounting is not a full
score. String literals remain a separate roadmap slice; Go numeric acceptance is recorded below.


`benchmarks/rust-numbers.md` records Rust `number_replace` for runtime integers/
floats. Preserve literal category, explicit suffixes, direct negative ownership
and signed-zero handling. Skip unsuffixed floats whose f32/f64 zero decisions
differ. Tests/cfg/generated/macros, constants/types/patterns, array lengths and
tuple selectors remain excluded. Saved recipes and fresh source discovery must
agree; compiler failures remain non-kills. 41 native pairs and fresh/cached scored
fixtures cover boundaries, review and stronger reruns. Unchanged UCD Parse has
39 candidates (26 kills, 12 survivors, one timeout); four separate-copy tests kill
six selected survivors. The EOF mutation still hangs in existing library tests
before the new integration assertion; six other survivors remain open. Preserve
108 original paths, 172 report files and the original 68.42% selection score.
Go numeric acceptance is recorded below; string literals remain separate.


`benchmarks/go-numbers.md` records Go `number_replace`: one eligible literal or
its direct sign, exact zero to one/nonzero to zero, preserving integer/float
constant kinds. Go constants have no negative zero; keep exact mantissa checks
and conservative exclusions for nonzero floats that round to zero at f32.
Named constants, types/array lengths, keyed composite keys, switch case values,
rune/imaginary literals, tests and generated files remain excluded. Preserve
compiler failures, exact saved recipes and fresh source/context matching.
33 native pairs, exact-constant/compiler controls and full/affected scored
workflows cover review/reruns and timeout/capture boundaries. Unchanged godotenv
has 40 candidates (25 kills, 15 survivors), with full/affected agreement. Three
separate-copy tests kill CRLF normalization, EOF diagnostic and literal-dollar
survivors; 12 remain open. Preserve all 20 inputs, 176 original and 176 affected
report files and the original 62.50% selection score. Numeric literal parity is
bounded; string literals and broader recipes remain separate. Remote Linux CI
validation requires a push.

`benchmarks/javascript-strings.md` records JS/TS `string_replace` across Node,
Bun and Vitest. One ordinary quoted literal becomes `("x")` when its value is
empty and `("")` otherwise. Preserve strict escape validation, line-continuation
emptiness, NUL/Unicode/lone-surrogate behavior and original-byte identities.
Do not evaluate source. Exclude standalone string statements/directives, module
syntax, declaration/computed keys, types, templates/substitutions, tests and
generated files. Existing export-attribute parse rejection remains. Exact saved
recipes and fresh source-context discovery reject forged keys; Bun fingerprints
`javascript/string.rs`. Six independent runner/syntax workflows preserve review,
setup errors and timeout boundaries; 120 native pairs validate values. Unchanged
OpenClaw has 13 selected session-key candidates (two kills, 11 survivors), all
11 survivors killed by five separate-copy tests. Preserve all 63 input files,
65 original report files and the original 15.38% filtered score. Focused 100%
is not a full stronger-project score. Other languages' strings, templates and
broader recipes remain separate; remote Linux CI validation requires a push.

`benchmarks/python-strings.md` records Python `string_replace`: ordinary runtime
str literals change nonempty to empty or empty to "x". Native AST admission resolves
escapes and implicit concatenation; `python/string.rs` independently validates
literal forms, emptiness and exact recipes and joins execution identity. Preserve
raw/triple-quoted behavior, Unicode/NUL/surrogate values and original spans.
Exclude standalone strings/docstrings, annotations/types, patterns, bytes and
formatted/template subtrees. LF/CRLF coordinates are supported; existing CR-only
mismatches remain rejected. Native parsing supplies Unicode-name validation.
Ninety-two native pairs, CLI inventories and scored review/error/timeout workflows
pass. Unchanged Scrubify accounts for 34 candidates across two reports (eight
kills, 25 survivors, one error); separate-copy public-API tests kill all 19
implementation survivors. Preserve all nine original inputs, 170 report files,
failed scores and source-identity mappings. Six export/version survivors and the
mask IndexError remain open; focused 100% is not a full stronger-suite score.
Rust/Go strings and templates remain separate; remote CI requires a push.

`benchmarks/rust-strings.md` records Rust `string_replace` for ordinary/raw runtime
str literals: nonempty to empty, empty to "x". Preserve full-token validation,
decoded continuation/Unicode/NUL behavior, raw delimiters and original spans.
Constants/statics, macros, attributes, types/patterns, array lengths and cfg/test/
generated code remain excluded, as do bytes/C strings and characters. The pinned
parser still rejects Unicode escapes containing underscores; native controls
verify that compatibility limit. Exact saved recipes and fresh context/source
matching protect reruns. Sixty-four native pairs, grammar controls, fresh/cached
review/reruns and timeout/capture boundaries pass. Unchanged UCD Parse has 112
candidates (48 kills, 64 survivors); four separate-copy public-API tests kill 24
survivors. Forty remain open. Preserve all 108 input paths, 464 report files and
the original 42.86% selection score; focused 100% is not a full stronger score.
Go strings and broader recipes remain separate; remote CI requires a push.

`benchmarks/go-strings.md` records Go `string_replace`: interpreted/raw runtime
strings, nonempty to empty and empty to "x". Preserve full-token escape validation,
byte/Unicode values, raw CR removal and original source spans. Shared Go literal
contexts exclude imports, tags, named constants, types/array lengths, composite
keys and switch case values; runes/tests/generated files remain excluded. Exact
saved recipes and fresh source/context checks protect reruns. Compile/vet failures,
timeouts and capture errors stay outside kills. Sixty-four native pairs and
full/affected scored controls pass. Unchanged godotenv has 39 candidates (16 kills,
19 survivors, four build failures); four separate-copy tests kill four survivors
in both scopes. Fifteen remain open. Preserve all 20 inputs, 172 original and
172 affected report files and the original 45.71% selection score. Bounded numeric/
string parity is complete; prioritize routine compatibility and feedback next.
Templates and broader recipes remain separate; remote CI requires a push.

`benchmarks/compatibility-audit.md` records the first six-runner unchanged-project
sample after literal parity. SJSON completes init/check/run/review/rerun: 36
selected candidates yield 23 kills, 11 survivors and two vet/build failures;
five separate-copy tests kill five survivors. Six remain open. Preserve 1,239
tracked inputs across all six projects, 160 original SJSON report files and its
67.65% selection score. Ripgrep, Click, UFO, eslint-node-test and Elysia stop at
admission; do not report their unattempted mutations as passing. Ripgrep's native
workspace controls pass 1,209 tests with and without Git metadata; installed Click
passes 2,622 tests with its original skips/deselection/xfail configuration. Native
JS suites were not attempted. Prioritize ordinary Cargo build-script support and
inactive custom release profiles on unchanged ripgrep as the next bounded goal,
including build inputs/output identity, fresh/cached restoration and stronger-test
acceptance. This is planned, not implemented; broader compatibility remains open.

The subsequent Cargo build-script milestone is complete; see
`benchmarks/rust-build-scripts.md`. `benchmarks/python-click-installed.md` closes
unchanged Click admission within an additional installed Python scope: explicit
Flit module metadata, static dependency-free packaging, pinned Flit Core 3.12.0,
owned wheel/venv and exact package/runtime verification. Preserve selected node
IDs, ordered upstream deselection identity and pristine skip/xfail dispositions.
Joined threads have independent body/lifetime evidence; raw starts, fixture
failures and late cleanup remain errors. The I/O guard must retain `dir_fd` for
`os.open`, which its native audit event omits. Do not relax external/source-write
checks or treat this as an OS sandbox. The evidence/per-file limit is 16 MiB;
historical 4 MiB reports remain readable, with old recipes rejected for reruns.
The unchanged baseline has 2,622 passes, 25 skips, one xfail and 31,000 original
stress deselections. Three failed reports account for 16 candidates: five kills,
eight survivors, three errors. Four separate-copy tests kill four survivors.
Preserve all 179 original inputs and failed report scores; combined inventory
accounting and a focused 100% are not full scores. Other backends/dependencies,
async suites and general plugins remain unsupported. Linux CI is configured but
remote validation requires a push. Reassess the remaining compatibility-audit
blockers instead of automatically adding another Python feature or operator.
