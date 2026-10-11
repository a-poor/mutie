# Repository Guidelines

## Project Structure & Module Organization

Mutie is a Rust 2024 CLI for mutation testing, currently supporting Go modules.

- `src/main.rs` coordinates runs and workers; `cli.rs` defines CLI arguments.
- `src/config.rs` loads repository settings and resolves explicit CLI overrides.
- `src/coordinator.rs` feeds execution events to reports and run-state snapshots;
  `src/status.rs` renders the live Ratatui view and manages terminal cleanup.
- `src/go.rs` integrates Go tooling; `mutation.rs` discovers mutations with Tree-sitter.
- `src/workspace.rs` manages isolated copies; `process.rs` handles subprocesses and timeouts.
- `src/capture.rs` owns file-backed output and head/tail retention; Go event
  classification reads complete captures independently of the saved-log limit.
- `src/workspace_metrics.rs` aggregates workspace timings, copy methods, and partial failures.
- `src/report.rs` defines outcomes; `saved_report.rs` persists reports, diffs, and logs.
- `src/score.rs` computes raw mutation scores, selection scope, and exact minimum-score
  decisions independently of dismissal-based findings policy.
- `src/report_reader.rs` validates saved runs for reruns and browsing;
  `src/browser.rs` owns the full-screen report browser and command export.
- `src/report_retention.rs` lists and prunes finalized saved runs;
  `src/report_lock.rs` shares lifetime locks between writers and pruning.
- `src/identity.rs` shares strict source matching; `dismissals.rs` stores review
  decisions, implements dismissal CRUD and rediscovery checks, and captures
  validated dismissal snapshots for saved reports and the browser.
- `tests/go_workflow.rs` covers the Go workflow; `configuration.rs` tests config behavior, `cancellation.rs` tests shutdown, and `terminal.rs` exercises the CLI in a pseudo-terminal. `tests/fixtures/go/` provides the sample Go module.
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

## Development Status & Versioning

Mutie is still in local development and has no external users. Breaking changes are acceptable at this stage. Do not bump project versions (including the package, report schema, and operator catalog versions) unless explicitly requested; version bumps for ordinary development changes are unnecessary noise for now.

## Build, Test, and Development Commands

Use Rust/Cargo and Go 1.24+ on `PATH`. Real Go tests also require a writable Go build cache.

- `cargo build` — compile the CLI.
- `cargo run -- /path/to/go/module --list` — inspect mutations without running tests.
- `cargo run -- tests/fixtures/go --jobs 2 --timeout 5` — exercise the bundled fixture.
- `cargo test` — run unit and non-ignored integration tests.
- `cargo test --test go_workflow -- --ignored` — run the opt-in, real-Go workflow test.
- `cargo fmt --check` — check formatting; use `cargo fmt` to apply it.
- `cargo clippy --all-targets -- -D warnings` — lint all targets.

## Coding Style & Naming Conventions

Follow Rustfmt defaults, including four-space indentation. Use `snake_case` for functions/modules, `PascalCase` for types, and `UPPER_SNAKE_CASE` for constants. Format Go fixtures with `gofmt`. Keep language-specific behavior separate from execution and reporting, and add context to fallible operations using the existing `anyhow` conventions.

## Testing Guidelines

Place unit tests beside implementation in `#[cfg(test)]` modules; use `#[tokio::test]` for asynchronous behavior. Name tests descriptively in `snake_case`. Add regression tests for changed behavior, especially mutation boundaries, outcome classification, workspace restoration, timeout cleanup, and saved reports. There is no enforced coverage percentage. Tests must preserve input projects and use temporary workspaces.

## Commit & Pull Request Guidelines

History uses short descriptive subjects such as “First pass” and “Adds saved reports”; no Conventional Commits prefix is established. Keep commits focused. PRs should explain the problem, behavior changes, and validation commands/results; link relevant issues and include terminal examples or screenshots for presentation changes.

Do not commit generated `target/` or `.mutie/` directories.
