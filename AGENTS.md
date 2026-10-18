# Repository Guidelines

## Project Structure & Module Organization

Mutie is a Rust 2024 CLI for mutation testing, supporting Go modules, validated
Rust binary packages, and validated ordinary Rust library packages.

- `src/main.rs` coordinates runs and workers; `cli.rs` defines CLI arguments.
- `src/config.rs` loads repository settings and resolves explicit CLI overrides.
- `src/coordinator.rs` feeds execution events to reports and run-state snapshots;
  `src/status.rs` renders the live Ratatui view and manages terminal cleanup.
- `src/go.rs` integrates Go tooling; `mutation.rs` discovers mutations with Tree-sitter.
- `src/language.rs` dispatches language commands; `command.rs` shares captures,
  classification and progress phases. `src/rust.rs` runs isolated Cargo commands;
  `rust/project.rs` validates the initial scope and `rust/discovery.rs` traverses
  Rust modules for boolean flips, comparison-boundary mutations, and logical swaps.
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
- `tests/log_limits.rs` checks log retention, outcome preservation, complete
  discovery, reruns, noisy failures, signal cleanup, and source preservation.
- `tests/capture_limits.rs` checks raw output budgets, overflow errors, baseline
  and discovery failures, saved diagnostics, policy precedence, and reruns.
- `tests/dismissals.rs` checks persistence, invalidation, metadata edits, and
  preservation of execution outcomes and saved reports; it also exercises
  dismissal-based and minimum-score CI policies, configuration, and focused runs.
- `README.md` documents current behavior, `DESIGN.md` records architecture and plans, and `TODO.md` tracks progress. Keep these synchronized with verified changes; planned features are not necessarily implemented.
- `SUPPORT.md` tracks support levels, scope and validation evidence for each
  language/test runner. Update it when support changes; planned JavaScript/TypeScript
  and Python targets are not implemented CLI capabilities.

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
`benchmarks/fixtures/`. Reassess the next major scope objective.

Prioritize milestone progress, demonstrated correctness/reliability blockers,
and explicit user requests. Do not automatically propose another small Go feature,
operator, or UI refinement because it is an unchecked backlog item. Judge progress
by the run/inspect/improve-tests/rerun workflow on real projects, including runtime
and finding usefulness. Extract the language boundary as Go and Rust need it;
keep existing Go behavior covered and defer speculative plugin abstractions.
Distinguish planned Rust support from verified behavior in documentation and CLI
claims. Reassess the next major objective when the milestone is complete.

Mutie is still in local development and has no external users. Breaking changes are acceptable at this stage. Do not bump project versions (including the package, report schema, and operator catalog versions) unless explicitly requested; version bumps for ordinary development changes are unnecessary noise for now.

## Build, Test, and Development Commands

Use Rust/Cargo and Go 1.24+ on `PATH`. Real Go tests also require a writable Go build cache.

- `cargo build` — compile the CLI.
- `cargo run -- /path/to/go/module --list` — inspect mutations without running tests.
- `cargo run -- tests/fixtures/go --jobs 2 --timeout 5` — exercise the bundled fixture.
- `cargo test` — run unit and non-ignored integration tests.
- `cargo test --test go_workflow -- --ignored` — run the opt-in, real-Go workflow test.
- `cargo test --test rust_workflow -- --ignored --test-threads=1` — run real Cargo workflows.
- `cargo fmt --check` — check formatting; use `cargo fmt` to apply it.
- `cargo clippy --all-targets -- -D warnings` — lint all targets.

## Coding Style & Naming Conventions

Follow Rustfmt defaults, including four-space indentation. Use `snake_case` for functions/modules, `PascalCase` for types, and `UPPER_SNAKE_CASE` for constants. Format Go fixtures with `gofmt`. Keep language-specific behavior separate from execution and reporting, and add context to fallible operations using the existing `anyhow` conventions.

## Testing Guidelines

Place unit tests beside implementation in `#[cfg(test)]` modules; use `#[tokio::test]` for asynchronous behavior. Name tests descriptively in `snake_case`. Add regression tests for changed behavior, especially mutation boundaries, outcome classification, workspace restoration, timeout cleanup, and saved reports. There is no enforced coverage percentage. Tests must preserve input projects and use temporary workspaces.

## Commit & Pull Request Guidelines

History uses short descriptive subjects such as “First pass” and “Adds saved reports”; no Conventional Commits prefix is established. Keep commits focused. PRs should explain the problem, behavior changes, and validation commands/results; link relevant issues and include terminal examples or screenshots for presentation changes.

Do not commit generated `target/` or `.mutie/` directories.
