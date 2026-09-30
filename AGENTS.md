# Repository Guidelines

## Project Structure & Module Organization

Mutie is a Rust 2024 CLI for mutation testing, currently supporting Go modules.

- `src/main.rs` coordinates runs and workers; `cli.rs` defines CLI arguments.
- `src/go.rs` integrates Go tooling; `mutation.rs` discovers mutations with Tree-sitter.
- `src/workspace.rs` manages isolated copies; `process.rs` handles subprocesses and timeouts.
- `src/report.rs` defines outcomes; `saved_report.rs` persists reports, diffs, and logs.
- `tests/go_workflow.rs` contains integration tests; `tests/fixtures/go/` provides the sample Go module.
- `README.md` documents current behavior, `DESIGN.md` records architecture and plans, and `TODO.md` tracks progress. Keep these synchronized with verified changes; planned features are not necessarily implemented.

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
