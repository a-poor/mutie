# Usage and configuration

[Documentation index](README.md) · [Project overview](../README.md)

## Try it

Building Mutie requires Rust/Cargo. Testing a Go project also requires Go 1.24+
on `PATH`. The prototype is tested on Linux and macOS; the initial Rust workflow
has been validated on macOS.

```sh
cargo run -- /path/to/go/module
cargo run -- /path/to/go/module --jobs 4
cargo run -- /path/to/go/module --test-scope affected --jobs 4
cargo run -- /path/to/go/module --fail-on-missed
cargo run -- /path/to/go/module --min-score 80
cargo run -- /path/to/go/module --list
cargo run -- /path/to/go/module --include 'internal/**/*.go' --exclude '**/generated_*'
cargo run -- /path/to/go/module --timeout 30 --baseline-timeout 300 --verbose
cargo run -- /path/to/go/module --output-dir ./mutation-reports
cargo run -- /path/to/go/module --ui plain
cargo run -- /path/to/go/module --rerun-missed /path/to/saved/run
cargo run -- /path/to/go/module --rerun /path/to/saved/run --id 9,11
cargo run -- report /path/to/saved/run
cargo run -- runs /path/to/go/module/.mutie
cargo run -- prune /path/to/go/module/.mutie --keep-last 5
cargo run -- dismiss --module /path/to/go/module list
```

The path defaults to the current directory. A Go project must contain `go.mod`;
a Rust project must contain `Cargo.toml`. Language detection rejects roots with
both manifests unless `--lang go` or `--lang rust` is explicit. Glob
patterns are relative to that directory; quote them so your shell does not expand
them. Repeat `--include` or `--exclude` to supply multiple patterns. Exclusions
win. These options control which files are mutated; test selection is controlled
separately by `--test-scope`. `--list` discovers mutations without running tests.

The bundled Go fixture deliberately produces all four outcomes:

```sh
cargo run -- tests/fixtures/go --jobs 2 --timeout 5
```

## Repository configuration

Create a starting configuration with `mutie init PATH`. It infers Go, Rust,
Python or a recognized Node/Bun/Vitest test recipe from project manifests. If
multiple adapters are found, a terminal session offers a choice; automation must
supply `--lang`. Package-manager metadata is reported separately and never used
to assume that Bun installed dependencies means Bun runs the tests.

```sh
mutie init PATH --non-interactive
mutie init PATH --lang node --non-interactive
mutie init PATH --lang python --python /path/to/venv/bin/python
mutie init PATH --lang rust --dry-run
```

`--dry-run` prints TOML without writing it. Existing `mutie.toml` files (including
symlinks) are never overwritten; preview and merge changes manually. Init does
not execute project scripts, install dependencies, run tests or certify project
compatibility. It reports required runtime versions and known package-manager
restrictions. Run discovery and an ordinary run afterward to validate the full
adapter scope. Relative `--python` paths are resolved from the invocation directory;
the virtual-environment launcher symlink is preserved.

Source include/exclude patterns select mutation locations, not which tests run.
The current `operators` setting is an inclusion list; omit operators to exclude
them from that explicit list. A dedicated exclusion setting and additional runner
flags are not implemented. Arbitrary test commands/flags cannot bypass adapter
inventory or failure-attribution checks.

Mutie reads an optional `mutie.toml` beside the selected project's manifest.
Settings apply in this order: built-in defaults, then the file, then explicitly
supplied CLI arguments. Parent directories are not searched.

```toml
lang = "go"
jobs = 2
test_scope = "full" # or "affected" (opt-in)
include = ["**/*.go"]
exclude = ["internal/legacy/**"]
operators = [] # all operators; or ["arithmetic_replace", "call_remove"]
timeout = 30
# max_log_bytes = 65536 # optional retained bytes per log artifact
# max_capture_bytes = 16777216 # optional combined stdout/stderr bytes per command
baseline_timeout = 300
fail_on_missed = false # enable to fail on undismissed missed mutations
# min_score = 80.5 # optional minimum raw mutation score (0-100)
output_dir = ".mutie"
verbose = false
ui = "auto"
```

The configuration example above is for Go. Use `lang = "rust"` (or automatic
detection) and Rust file globs for a Cargo package. Rust accepts only
`test_scope = "full"` and the `boolean_flip`, `comparison_invert`, `comparison_boundary`,
`logical_swap`, `negation_remove`, `arithmetic_replace`, `branch_force_true`,
`branch_force_false`, and `loop_skip` operators.
`rust_packages = []` selects all Rust workspace members for mutation; a list of
package names narrows mutations while tests still cover all members.
`rust_build_cache = "fresh"` (default) or `"worker"` applies only to Rust;
specifying it for Go is rejected.

All keys are optional. Defaults are `lang = "auto"` (detect Go or Rust), one worker,
empty include/exclude/operator lists, a 300-second baseline/discovery timeout, `.mutie`
under the module root for reports, `ui = "auto"`, `test_scope = "full"`, and no verbose output. Omit
`timeout` for the automatic policy: `max(10 seconds, 5 * baseline duration)`. Worker counts and
timeouts must be positive integers; timeouts are in seconds. Unknown keys,
malformed TOML, and incorrect value types are rejected even if CLI options would
override them. The merged settings are validated before creating reports or
running language tools, including globs, supported language, and output paths.

Globs are always module-relative. A configured `output_dir` is relative to the
module root; a CLI `--output-dir` is relative to the invocation directory.
Absolute paths are also accepted. Existing symlinks are resolved, and `.`/`..`
components are normalized before use. Paths are not shell-expanded.

Each CLI list replaces its configured counterpart. To clear a configured list,
use `--clear-include` (all eligible files), `--clear-exclude` (no additional
exclusions), or `--clear-operators` (all operators). `--clear-timeout` restores the
automatic timeout; `--no-verbose`
overrides `verbose = true`; `--no-fail-on-missed` overrides `fail_on_missed = true`.
`--clear-max-log-bytes` restores unlimited logs.
`--clear-max-capture-bytes` disables a configured command-output capture limit.
`--clear-min-score` disables a configured score threshold.
Each reset flag conflicts with the corresponding
value flag. For example:

```sh
cargo run -- /path/to/go/module --include 'internal/payments/**' --jobs 1
cargo run -- /path/to/go/module --clear-include --clear-exclude --list
cargo run -- /path/to/go/module --clear-timeout --no-verbose
```

`--list`, `--rerun`, `--rerun-missed`, and `--id` are invocation-only;
they cannot be set in `mutie.toml`. Listing uses the same
configuration and excludes the configured report directory from its snapshot,
but creates no report directory or artifacts. An explicit `--output-dir` still
conflicts with `--list`.

## CI findings policy

By default, completed runs exit successfully even if mutations survive. Enable
`--fail-on-missed` (or `fail_on_missed = true` in `mutie.toml`) to fail a completed
run when at least one **undismissed missed mutation** remains:

```sh
mutie /path/to/module --fail-on-missed --ui plain
mutie /path/to/module --rerun-missed RUN_DIR --fail-on-missed
mutie /path/to/module --no-fail-on-missed # override a repository policy locally
```

The policy uses the current run's dismissal snapshot. Only exact, matched
dismissals exempt missed findings; stale, missing, unmatched, or ambiguous entries
do not. Dismissed mutations still execute and remain in the missed outcome count.
Focused reruns evaluate only their selected mutations using current configuration
and freshly matched dismissals. A successful empty selection passes. `--list`
does not evaluate the policy or write a report.

| Exit code | Meaning |
| --- | --- |
| `0` | Successful operation; for runs, each policy is disabled or passes. |
| `1` | Execution, configuration, reporting, or other operation failure, including handled interruption. |
| `2` | Invalid CLI arguments. |
| `3` | A completed, saved run fails `--fail-on-missed` or `--min-score`. |

A findings failure keeps `status: "completed"` and `error: null` in the saved
report. It changes the CLI exit code without changing mutation outcomes. The
final terminal summary, `summary.txt`, and `run.json.policy` record the decision.
Policy status is `disabled`, `not_evaluated`, `passed`, or `failed`; passed/failed
decisions also record `undismissed_missed` and `dismissed_missed` counts. Enabled
policies remain `not_evaluated` during execution and after failed/interrupted runs.
Execution and report-writing errors take precedence over a findings failure;
partial report files may survive a write error but never produce exit code `3`.

Killed, timed-out, and unbuildable mutations do not count as missed for this policy.
The policy is not a mutation score or a guarantee that all mutations were killed.

## Mutation scores and minimum thresholds

Every new run records a raw mutation score:

```text
score = 100 × killed / (killed + missed)
```

Timeouts and build failures are excluded and reported alongside the score; they
never count as kills. Matched dismissals remain in the killed/missed counts, so
dismissing a survivor does not improve the raw score. A failed or interrupted run
has no final percentage. A completed run with no killed or missed mutations also
has an unavailable score, rather than an invented zero or 100 percent.

Use a threshold for gradual adoption in CI:

```sh
mutie /path/to/module --min-score 80 --ui plain
mutie /path/to/module --min-score 66.67
mutie /path/to/module --clear-min-score # disable a repository threshold locally
```

Repository `min_score = 80.5` accepts a number from 0 through 100 with at most
two decimal places. The default is disabled. CLI values replace configuration;
`--clear-min-score` conflicts with `--min-score`. Equality passes. Comparison uses
the exact count ratio, independently of the two-decimal display: two killed and
one missed display as `66.67%` but fail a `66.67` minimum. Even a zero threshold
fails a completed run with no scored mutations, including an empty selection or
a run containing only timeouts/build failures. `--list` never evaluates a gate.

Both policies can be enabled. Either failure produces exit `3` after the completed
report is saved. For example, dismissing all survivors can satisfy
`--fail-on-missed` while still failing `--min-score`. Execution, interruption, and
report-writing errors retain exit `1` and take precedence over policy failures.

The final summary, `summary.txt`, and the browser's `m` view show the percentage,
exact counts, exclusions, dismissals included, and scope. Scope is
`all_discovered`, `filtered`, or `rerun`; a filtered run or rerun describes **only
that selection**. Testing affected packages does not change mutation selection.
Even an unfiltered run measures only the mutations discovered for this module,
active files, and supported operators. Browser filters and marks never recompute
the saved score, and reruns calculate a fresh score using current configuration.

`run.json.score` stores the scope, `killed`, `missed`, `timeout`, `build_failed`,
`errors`, `unfinished`, `dismissed_in_score`, `percentage`, and
`unavailable_reason`. Counts are authoritative; the JSON percentage is a derived
floating-point value. An unavailable score has `percentage: null` and reason
`incomplete_run`, `execution_errors`, or `no_scored_mutations`.
`run.json.score_policy` stores `disabled`, `not_evaluated`, `passed`, or `failed`,
plus `minimum` whenever enabled. Readers validate the score and decision against
saved outcomes, review, and configuration. Older reports remain readable and
show the score as not recorded.

## Affected-package testing

`--test-scope affected` tests the mutated package and module packages that depend
on it, including transitive dependencies and internal/external test imports.
The baseline always tests the full module. The default `--test-scope full` runs
`./...` for every mutation and overrides a configured affected scope.

```sh
cargo run -- /path/to/go/module --test-scope affected --jobs 4
cargo run -- /path/to/go/module --test-scope affected --list
cargo run -- /path/to/go/module --test-scope affected --rerun RUN_DIR --id 9,11
```

Selection uses `go list -deps -test -json ./...` from a pristine workspace, including
Go's test-specific package variants and dependencies outside the set matched by
`./...`. See [Go's package metadata](https://pkg.go.dev/cmd/go#hdr-List_packages_or_modules).
Only module packages matched by `./...` are test targets. Include/exclude globs
restrict mutation locations; they do not remove dependent tests from selection.
An incomplete or ambiguous graph stops the run before mutations execute, retaining
discovery diagnostics. Test output, classification, deadlines, and workspace
restoration use the normal execution path.

Use this mode for projects whose test dependencies are represented by Go imports.
File reads, subprocesses, dynamically loaded code, and other relationships outside
the import graph can require tests this mode omits. Full-module runs remain the
correctness reference. Affected-package testing is not coverage-based selection
and does not reuse old results. Compare outcomes on your project before relying
on reduced scope.

`--list` shows each mutation's selected packages. Saved inventory entries record
the exact package arguments as `test_packages`, even for pending or interrupted
mutations. Reruns use current settings and recompute the graph from current source
and tests; they never reuse the source report's package selection.

The [go-cmp sample comparison](../benchmarks/go-cmp-v0.7.0.md#affected-package-follow-up)
matched all 32 outcomes and averaged 25.3% less run time with affected scope on
one warm-cache host. A [32-mutation fzf sample](../benchmarks/fzf-v0.65.2.md) also
matched outcomes and averaged 16.5% less run time. Results depend on the project
and its dependency structure.

## Behavior

- The baseline runs `go test -json -count=1 -timeout=0 ./...`. Mutations use the
  same flags with `./...` by default, or selected packages in affected scope.
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
  swap arithmetic and logical operators, flip booleans, remove logical `!`, and
  remove standalone calls, force `if` conditions true or false, and skip
  conditional `for` loops.
- Results are **killed** (tests failed), **missed** (tests passed), **timeout**, or
  **build failed** (the mutation could not compile or failed Go's build checks).
  Tool/setup failures abort the run instead of counting as killed mutations.
  `--verbose` shows captured test and build output in plain mode.
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
- A completed run exits successfully unless an enabled `--fail-on-missed` or
  `--min-score` policy fails (exit `3`). Baseline, tool, and report-writing failures
  exit `1`. Saved JSON reports can be consumed by other tools.

The initial scope is one self-contained Go module. `GOWORK` is set to `off` for
child Go commands. Workspaces, local replacements outside the module, and tests
requiring Git metadata are not supported yet. Copies isolate repository files,
but tests still have normal access to external services, caches, and filesystem
paths. Windows descendant-process cleanup is not implemented yet.
