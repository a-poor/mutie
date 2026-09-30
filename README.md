# mutie

A CLI for performing mutation testing.

The first prototype supports Go modules. It runs a passing baseline, finds small
source mutations with Tree-sitter, and runs the full test suite for each mutation
in an isolated temporary workspace. Rust and other languages are planned.

See [DESIGN.md](DESIGN.md) for the MVP scope, architecture, and roadmap, and
[TODO.md](TODO.md) for implementation progress. Features described as planned are
not necessarily available in the current CLI.

## Try it

Requires Rust/Cargo and Go 1.24+ on `PATH`. The prototype is tested on Linux.

```sh
cargo run -- /path/to/go/module
cargo run -- /path/to/go/module --jobs 4
cargo run -- /path/to/go/module --list
cargo run -- /path/to/go/module --include 'internal/**/*.go' --exclude '**/generated_*'
cargo run -- /path/to/go/module --timeout 30 --baseline-timeout 300 --verbose
cargo run -- /path/to/go/module --output-dir ./mutation-reports
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

## Saved reports

Every test run saves a new directory under `<module>/.mutie/`. The CLI prints its
location when the run starts and the summary path when it finishes. Earlier runs
are preserved. Use `--output-dir DIR` to choose the parent directory instead;
relative paths are resolved from your current working directory. The output
directory cannot be the module root or an ancestor of it. `--list` writes no
reports and cannot be combined with `--output-dir`.

Each run contains:

- `run.json`: versioned run metadata, command, scope, timeouts, worker counts,
  phase, status, and any run-level error.
- `mutants.json`: the complete mutation inventory, including IDs, named operators,
  source locations, original text, replacements, source fingerprints, and diff
  paths. The inventory and every diff are published before workers start.
- `outcomes.json`: baseline and package-discovery results, mutation execution
  states and outcomes, durations, exit codes, log paths, and summary counts.
- `summary.txt`: readable counts, unfinished work, missed mutations, and artifact
  references for every planned mutation.
- `diff/000001.diff`: one unified source diff per planned mutation.
- `logs/baseline.log`, `logs/discovery.log`, and `logs/000001.log`: readable command
  output, with separate `.stdout.jsonl` and `.stderr.log` captures alongside each
  log. Discovery stdout contains Go package JSON; test stdout contains Go test
  events. Logs are saved even without `--verbose`.
- `killed.txt`, `missed.txt`, `timeout.txt`, `build_failed.txt`, and `errors.txt`:
  outcome lists containing IDs and source descriptions, sorted by ID.

Mutation IDs are local to each run, assigned in deterministic discovery order.
Inventory and execution records are sorted by ID regardless of completion order;
join them on `id`. Source paths are module-relative and artifact paths are
report-relative. Lines and byte columns are one-based; `end_line`/`end_column` and
zero-based `byte_end` are exclusive. Durations are seconds and timestamps are
Unix seconds. `source_fingerprint` is `sha256:` followed by the lowercase SHA-256
hex digest of the complete original source file. It identifies source content,
not test content or a reusable result. IDs and offsets must not be replayed across
source edits without rediscovery and validation.

`schema_version` is `1` in all three JSON files, and `operator_catalog_version` is
`1` in run metadata and the inventory. Named operators are `arithmetic_replace`,
`comparison_invert`, `comparison_boundary`, `logical_swap`, `boolean_flip`, and
`negation_remove`.

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

The module's `.mutie` directory and the selected output directory are omitted
from snapshots so saved reports do not affect tests or discovery. Keep output
directories separate from source files, and add `.mutie/` (or your custom output
directory) to your project's `.gitignore`.

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
- The snapshot includes current local edits and untracked files; `.git` and
  report directories are omitted. Input source files are not edited. Internal symlinks are kept within the
  copied tree; dangling or external symlinks are rejected. Temporary workspaces
  are removed on completion. On Unix, timeouts and interruption kill the child
  process group, including test binaries. Cancellation is also checked during
  workspace copying and between discovery/publication steps.
- A completed run exits successfully even if mutations survive. Baseline and
  tool failures exit unsuccessfully. CI thresholds and focused reruns are future
  work; saved JSON reports can already be consumed by other tools.

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
failure and timeout, workspace resets, saved reports, and preservation of input
files. It needs Go on `PATH` and a writable Go build cache, and is opt-in because
it runs real builds and deliberately waits for a timeout. Non-ignored Unix tests
use a fake Go command to verify SIGINT/SIGTERM during baseline, discovery, and
parallel execution, preserved diagnostics, pending work, process cleanup, and
source preservation. They require an environment that delivers those signals.

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
