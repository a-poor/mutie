# mutie

A CLI for performing mutation testing.

The first prototype supports Go modules. It runs a passing baseline, finds small
source mutations with Tree-sitter, and runs the full test suite for each mutation
in an isolated temporary workspace. Rust and other languages are planned.

## Try it

Requires Rust/Cargo and Go 1.24+ on `PATH`. The prototype is tested on Linux.

```sh
cargo run -- /path/to/go/module
cargo run -- /path/to/go/module --jobs 4
cargo run -- /path/to/go/module --list
cargo run -- /path/to/go/module --include 'internal/**/*.go' --exclude '**/generated_*'
cargo run -- /path/to/go/module --timeout 30 --baseline-timeout 300 --verbose
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
  `--verbose` shows captured test and build output.
- There is one worker by default. Each worker restores its entire workspace from
  a pristine snapshot between mutations, including files changed or created by
  tests. Copies use [reflink-copy](https://docs.rs/reflink-copy/latest/reflink_copy/)
  where supported, with ordinary copies as a fallback. This costs extra copying
  compared with reverting just the mutated file, but avoids test-state leakage.
- The snapshot includes current local edits and untracked files; `.git` is
  omitted. Input files are not edited. Internal symlinks are kept within the
  copied tree; dangling or external symlinks are rejected. Temporary workspaces
  are removed on completion. On Unix, timeouts and interruption kill the child
  process group, including test binaries.
- A completed run exits successfully even if mutations survive. Baseline and
  tool failures exit unsuccessfully. CI thresholds and machine-readable reports
  are future work; result types are separate from terminal rendering.

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

The Go integration test checks all four outcomes, configurable scope, baseline
failure and timeout, workspace resets, and preservation of input files. It needs
Go on `PATH` and a writable Go build cache, and is opt-in because it runs real
builds and deliberately waits for a timeout.

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
