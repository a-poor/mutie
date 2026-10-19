# mutie

A CLI for performing mutation testing.

Mutie supports Go modules, scoped Rust packages, pinned Vitest packages, and a scoped Node test runner. It runs a
passing baseline, finds small source mutations with Tree-sitter, and by default
runs the full test suite for each mutation in an isolated temporary workspace.
Rust currently supports `boolean_flip`, `comparison_boundary`, and `logical_swap` in the
[initial Cargo scope](DESIGN.md#initial-rust-support-contract).
Go and Rust support focused reruns and dismissal review. The
[initial Rust milestone is accepted](benchmarks/rust-milestone-acceptance.md)
for this binary-package scope; general Cargo/workspace support remains outside it.

See [DESIGN.md](DESIGN.md) for the MVP scope, architecture, and roadmap, and
[TODO.md](TODO.md) for implementation progress. Features described as planned are
not necessarily available in the current CLI.

See [target support](SUPPORT.md) for Go/Rust support and validated scoped
JavaScript/TypeScript support through Vitest. The Node runner has a validated
JavaScript scope and a separately validated native TypeScript scope. Bun and pytest remain planned.
Run `mutie PATH --lang vitest --ui plain` against a prepared, pinned ESM/pnpm
package to discover boolean flips, run the full suite in isolated copies, and save
scored outcomes. This initial path supports one worker. A static coverage-reporter-only
Vite/Vitest config is accepted without executing it; coverage remains disabled.
Plain-JSON NodeNext type-checking configs are accepted only for JavaScript-only
projects with `allowJs`, `checkJs` and `noEmit` enabled; runtime TypeScript still
requires ESNext. See the [Hyparquet scope audit](benchmarks/vitest-javascript-hyparquet.md) and
[JavaScript findings and stronger-test reruns](benchmarks/vitest-hyparquet-findings.md).
The [full Hyparquet inventory review](benchmarks/vitest-hyparquet-full.md) records
all 133 attempted candidates, useful test gaps, and remaining errors/limits.
Vitest resolves Node through PATH once per run, then rechecks the pinned executable
version/path and project identity before each baseline or mutation command. See
the [Node reuse validation](benchmarks/vitest-node-reuse.md).
Other runner config settings remain unsupported. See
[config support](benchmarks/vitest-coverage-config.md). Worker evidence distinguishes body failures from fixture, hook and callback
failures: successful lifecycle work can accompany a genuine kill, while lifecycle
and mixed failures are errors. Pending async assertions from setup and post-body
assertion-count failures remain conservatively rejected.
Worker evidence requires the audited Vitest runner bytes; expected-failure tests
(`test.fails`) are unsupported. See the [lifecycle evidence](benchmarks/vitest-lifecycle-attribution.md). Dismissal review, `--fail-on-missed`, and `--min-score` are
available within this scope; see [review/policy validation](benchmarks/vitest-dismissals.md).
Use `mutie PATH --lang vitest --rerun-missed RUN_DIR` after improving tests;
`--rerun RUN_DIR --id ID` selects individual saved mutations. Reruns require
unchanged source identity and pinned package/runner/config identity; see
[rerun validation](benchmarks/vitest-focused-reruns.md).

`mutie vitest-list PATH --include 'src/**/*.ts'` remains a read-only preview;
`mutie vitest-check PATH` checks an isolated baseline with a deadline and bounded
output. These two diagnostic commands do not apply `mutie.toml` or save reports;
the scored run does. See [CLI validation](benchmarks/vitest-shared-runs.md) and the
[execution boundary](benchmarks/vitest-execution.md) for pinned dependencies,
setup and conservative failure attribution.

## Scoped Node test runner

```sh
mutie PATH --lang node --ui plain
mutie PATH --lang node --list
mutie PATH --lang node --rerun-missed RUN_DIR
```

The initial scope is one dependency-free ESM package using audited Node **24.22.0**
on PATH and `.js` files under `src/` and `test/`. The package test script may be
exactly `node --test` or `node --test test/*.test.js`. There must be no installed
`node_modules`; unused build-only devDependencies are allowed. Imports must be
static literal relative paths within those directories or `node:` built-ins.
CommonJS, custom loaders/runner flags, workspaces and runtime dependencies are
unsupported. Only `boolean_flip`, `--jobs 1` and full-suite testing are accepted.

The validated native TypeScript extension also accepts erasable `.ts` files
under `src/` and `test/`, using `node --test` or exactly `node --test test/*.test.ts`.
JavaScript tests may import `.ts` sources, and TypeScript tests may import `.js`
helpers. Local imports must include the real file extension, including `import type`.
Only runtime booleans are mutated. Node strips types without type-checking or
applying `tsconfig.json`; type errors alone cannot kill mutations. An optional
plain-JSON NodeNext type-checking config is audited and fingerprinted, including
its absence. Config edits require a fresh scan. Enums, namespaces, parameter
properties, angle-bracket assertions, decorators, declarations (`.d.ts`/`declare`),
TSX, MTS/CTS, path aliases and build/loader transforms remain unsupported. See the
[scope and independent TypeScript evidence](benchmarks/node-typescript-scope.md).

For `node --test`, Mutie audits the complete native default selection across the
project and checks it against the pinned Node implementation. All selected files
must be audited `.js`/`.ts` files under `test/`, including nested files and helpers.
Matching tests elsewhere cause rejection; they are never silently omitted. Every
selected file must complete real tests. Visible symlinks are unsupported, and
report-directory exclusions cannot remove tests. See [default discovery and
unchanged-project acceptance](benchmarks/node-default-discovery.md).

Baseline and mutations run in fresh copies, with serial test files, owned temporary
directories and bounded captures. Saved reports, browser command export, reruns,
dismissal review and score policies use the shared workflow. Reruns accept improved
tests but require unchanged mutation source and package/runner identity.

Completed body exceptions count as kills only when worker evidence matches the
exact thrown/rejected value to the same file and test ID. This covers assertions,
synchronous exceptions, native Promise rejections and error-first callbacks.
Import/parse failures, plan failures, lifecycle failures (including mixed body/cleanup
and late-process failures), incomplete runs and ambiguous evidence remain errors.
Dynamic imports are rejected because their evaluation errors can escape through
the body; rejected non-native thenables remain conservatively unclassified. Expected-failure
and retry modes, todo tests, and empty/all-skipped files are unsupported. The hook
observer requires the exact audited Node implementation; no fallback is attempted.

The [Node acceptance record](benchmarks/node-initial-scope.md) covers real-project
findings, stronger-test reruns, cleanup evidence and remaining attribution limits.
[Body-exception attribution](benchmarks/node-body-attribution.md) extends the
initial assertion-only boundary. Its observer changes execution identity; start
a fresh scan instead of rerunning an old Node recipe. Existing reports are preserved.
The [remaining SRI findings review](benchmarks/node-sri-final-review.md) explains the
pending-promise timeout and preserves the cleanup survivor without claiming equivalence.
[Second-project acceptance](benchmarks/node-openclaw.md) records an unchanged
39-candidate `openclaw-cc` scan and three stronger-test kills.
The [storage/profile review and milestone audit](benchmarks/node-storage-profiles.md)
adds five further kills and establishes Validated maturity within the same pinned scope.
That milestone is JavaScript-only. Native TypeScript's first audit required a
Bragkit script adaptation; [the follow-up](benchmarks/node-default-discovery.md)
now runs the unchanged package and kills the same three HTML-list survivors with
stronger tests, establishing separate Validated acceptance. The
[remaining Bragkit review](benchmarks/node-bragkit-survivors.md) kills all 18 other
survivors with tests for CLI defaults, collector behavior and browser fallback.

The next selected runner is Bun. Its [initial audit](benchmarks/bun-initial-scope.md)
records JS/TS fixtures and real-project probes. A
[bounded observer prototype](benchmarks/bun-body-attribution.md) now distinguishes
body failures from unsafe outcomes in paired probes. `mutie bun-check PATH` now
checks a baseline in a fresh copy using pinned Bun 1.3.11+af24e281e. This diagnostic
accepts dependency-free ESM packages with `scripts.test: "bun test"`, `.js`/`.ts`
source under `src/`, and `.test.js`/`.test.ts` tests under `test/` or `tests/`.
It currently rejects tsconfig, custom Bun configuration, installed dependencies,
symlinks and dynamic imports. See [execution evidence](benchmarks/bun-isolated-execution.md).
Bun remains Planned: `--lang bun`, mutation runs and saved reruns are not available.

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

## Initial Rust support

Rust supports one ordinary package with a binary, a library, or a library plus
one binary, with a usable `Cargo.lock`, default features, and standard
unit/integration tests on Unix. The ordinary-library scope is validated through
fixtures and [Marathon findings and focused reruns](benchmarks/rust-library-marathon.md);
Mutie remains the binary-only real-project control.
Try the [small fixture](tests/fixtures/rust/README.md):

```sh
cargo run -- tests/fixtures/rust --list
cargo run -- tests/fixtures/rust --jobs 2 --timeout 30
cargo run -- report /path/to/saved/run
```

The fixture produces three `boolean_flip` mutations: two killed and one missed.
File/operator filters, live/plain status, saved diffs/logs, scores, failure
policies, operator totals, and report listing/retention use the shared workflow.
Rust supports focused reruns by saved ID, operator, or missed outcome, including
browser command export. Rust dismissal commands use the same review, reporting,
and policy workflow as Go.
The [focused rerun validation](benchmarks/rust-focused-reruns.md) records the
fixture and Mutie self-test evidence, including strict matching and runtime costs.
The [dismissal validation](benchmarks/rust-dismissals.md) covers review commands,
saved decisions, policies and source/artifact preservation.
The [comparison-boundary fixture](tests/fixtures/rust-boundaries/README.md) adds
five ordering mutations: four killed and one missing an equality test. A stronger
integration test kills that survivor through a focused rerun.
See the [boundary validation record](benchmarks/rust-comparison-boundaries.md)
for regression checks and the Mutie self-run.
The [logical-swap fixture](tests/fixtures/rust-logical/README.md) covers grouping,
short-circuit effects and let-chains, including a saved survivor killed by a mixed-input
test. The [acceptance review](benchmarks/rust-milestone-acceptance.md) collects the
verified workflow, scope limits and runtime evidence. The
[first feedback-time improvement](benchmarks/rust-report-write-reruns.md) reduces
redundant report writes and preserves the saved-selection outcomes in a measured
Mutie self-run; tests still run in full. The [library fixture](tests/fixtures/rust-library/README.md)
extends discovery to all production roots and exercises fresh/cached execution
and stronger-test reruns. Workspaces remain future scope.

Cargo metadata validates the package before the baseline. Mutie compiles tests
with `cargo test --locked --tests --no-fail-fast --message-format=json --no-run`,
then runs the same command without `--no-run`. Both phases share one timeout;
captured diagnostics survive failures, timeout and cancellation. By default each
baseline and mutation gets fresh owned target/build directories. The input `target/`,
caller-selected build directories and source files are preserved; the Cargo
registry/download cache remains available. Fresh builds can be expensive.

Opt into per-worker dependency reuse with `--rust-build-cache worker` (or
`rust_build_cache = "worker"` in a Rust project's `mutie.toml`). The baseline
still builds fresh. Each worker retains its own dependency artifacts for that
run, but runs `cargo clean --locked --package PACKAGE` before every mutation's
compilation. This forces the package's binary, unit tests and integration tests
to rebuild even when source timestamps move backwards. Caches are never shared
between workers or runs and are removed when the run ends. Use
`--rust-build-cache fresh` to select the default behavior explicitly. Saved
settings and text summaries identify the mode; focused reruns use the current
mode. On the six-mutation Mutie sample, warm compilation took 22–46 seconds
versus 66–73 seconds for the same mutations with fresh artifacts. Whole-run
timing also varied between fresh baselines; see the
[cache validation record](benchmarks/rust-build-cache.md) for results and limits.

The [process reliability follow-up](benchmarks/process-reliability.md) records
parallel capture and report-lock regressions discovered while testing Mutie
itself, including the fixes and unmutated reproduction checks. A subsequent
[parallel score rerun](benchmarks/rust-parallel-score-reruns.md) kills all six
selected Mutie mutations with matching intended failures in fresh and cached
modes, including the strengthened saved-percentage tolerance test.

Package cleanup shares the mutation deadline with compilation and testing and
has its own output-capture budget. A failed cleanup stops the run as a tool error;
a cleanup timeout records `timeout` without starting compilation or tests.

Interrupting compilation or mutation tests with Ctrl-C or SIGTERM preserves
completed results and active diagnostics in a partial report. Active mutations
are marked interrupted; queued mutations stay pending, and score/finding policies remain
unevaluated. Serial and parallel real-Cargo regressions verify this behavior,
process cleanup and source/artifact preservation.

A baseline compilation timeout stops the run; a mutation compilation timeout
records `timeout`. Tests do not start after either compilation timeout, and time
spent compiling reduces the time remaining for tests.

Cargo startup failures, unexpected exits, missing build/test completion evidence,
and output-capture overflow stop the run as errors. Completed results and active
diagnostics survive, unfinished work keeps its interrupted/pending state, and
findings/score policies remain unevaluated.

Discovery follows ordinary Rust modules and mutates boolean literals, logical operators, and ordering
comparisons in function/method bodies, including closures and match guards.
`comparison_boundary` swaps `<` ↔ `<=` and `>` ↔ `>=` by editing only the operator
token. It leaves operand expressions intact and supports generic/overloaded
comparisons. It does not change `==`, `!=`, shifts, ranges, or generic brackets.
`logical_swap` exchanges `&&` and `||`, preserving operand grouping and comments
with parentheses where required. Operands retain their order and appear once;
the changed operator intentionally changes short-circuit evaluation. Let-chain
connectors are excluded, while ordinary boolean expressions inside the chain
remain eligible. Bitwise operators, reference syntax and closure delimiters are
not logical-swap candidates.
Test-only code, `cfg`/`cfg_attr` subtrees (including active platform code),
macro tokens/expansions, generated files, constants/type-level code and array repeat lengths are
excluded. The run prints and saves discovery exclusions. Libraries use ordinary
`lib` or `rlib` crate types; doctests and ignored tests are not run. Proc-macro
packages, other library crate types, multiple binaries, multi-package or enclosing
workspaces, local/git dependencies,
project build scripts, custom profiles/harnesses, feature selection, and custom
compiler/runner settings are outside this first scope. Only artifact-location,
network and terminal Cargo config sections are accepted; in-project artifact
directories must be under `target/`. Toolchain override files are rejected;
an inherited `RUSTUP_TOOLCHAIN` selection is preserved and actual tool versions
are recorded. See [the original binary contract](DESIGN.md#initial-rust-support-contract)
and [library scope](benchmarks/rust-library-scope.md) for detail.

The first self-testing sample uses two literals in Mutie's score code:

```sh
cargo run -- . --include src/score.rs --operator boolean_flip \
  --jobs 1 --timeout 600 --baseline-timeout 300
```

See [the validation record](benchmarks/rust-first-slice.md) for measured costs and
results. Fresh builds and the full test suite make self-testing a minutes-long run.

Real Cargo workflow regression tests are opt-in to prevent recursive self-tests:

```sh
cargo test --test rust_workflow -- --ignored --test-threads=1
```

## Repository configuration

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
`test_scope = "full"` and the `boolean_flip`, `comparison_boundary`, and `logical_swap` operators.
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

## Mutation operators

All ten Go operators run by default; Rust runs `boolean_flip`,
`comparison_boundary`, and `logical_swap`.
Use repeatable or comma-separated
`--operator` values to select a subset for a fresh run, a listing, or a rerun:

```sh
cargo run -- /path/to/go/module --operator call_remove --list
cargo run -- /path/to/go/module --operator arithmetic_replace,call_remove --jobs 2
cargo run -- /path/to/go/module --operator branch_force_true,branch_force_false --list
cargo run -- /path/to/go/module --operator loop_skip --list
```

The `operators` list in `mutie.toml` sets the repository default. CLI values
replace it; duplicates are removed, and unknown names are rejected. An empty
configured list or `--clear-operators` allows all operators. Operator selection
intersects with file scope and, for reruns, saved IDs and outcome selection.
An empty selection completes normally. IDs are assigned after selection, and
reports record the effective operator list.

`branch_force_true` and `branch_force_false` force an entire `if` or `else if`
condition to take or skip its branch. They preserve the initializer (if any)
and both bodies, but skip evaluation of the original condition, including its
side effects. For example, `if ready()` becomes `if (0 == 0) || (ready())` or
`if (0 != 0) && (ready())`. Keeping the original expression as a short-circuited
operand keeps its imports and local variables used, avoiding unused-name build
failures. The constant comparisons work even when code shadows `true` or `false`.

These operators include nested, labeled, multiline, and initializer-bearing
`if` statements. They exclude loop/switch conditions and conditions consisting
only of `true` or `false` (possibly parenthesized), which remain covered by
`boolean_flip`. This is a syntax rule, without name resolution or general constant
folding; named constants and equivalent branches can still produce unkillable
mutations. Other mutations inside the original condition remain independent.

The [fzf branch sample](benchmarks/fzf-v0.65.2-branches.md) matched all 24 outcomes
between full and affected tests with no build failures. Added tests killed four
survivors, covering ellipsis arguments and up-arrow modifiers. This sample does
not establish a general noise rate or make every survivor a test gap.

`loop_skip` makes a conditional `for` loop execute zero iterations. It supports
both `for condition { ... }` and `for init; condition; post { ... }`, replacing
only the condition with `(0 != 0) && (condition)`. The initializer still runs
once; the original condition, body, and post statement are not evaluated.
References remain in the source, keeping imports and local variables used.
Nested, labeled, parenthesized, and multiline conditions are supported.

Range loops and loops without a condition are excluded. Bare `true`/`false`
conditions (including parentheses/comments) remain with `boolean_flip`, following
the same syntax rule as branch forcing. Other operators in headers and bodies
remain separate candidates, including inside loops excluded from `loop_skip`.
Skipping an already empty loop can be equivalent; a missed result alone does not
prove that a test is missing.

The [fzf loop sample](benchmarks/fzf-v0.65.2-loops.md) matched all 16 outcomes
between full and affected tests with no build failures. Added tests killed two
survivors covering partial tail chunks and display-width trimming.

`call_remove` deletes an entire standalone function or method call, including
multiline and parenthesized calls, from a block or case body. Arguments are also
removed, so their side effects no longer execute. It excludes calls used as
values (including `_ = call()`), `go`/`defer` calls, labeled calls, and calls in
control-flow headers. Surrounding comments and separators are preserved; comments
inside the removed statement are removed with it. Other mutations inside a call
remain separate candidates.

Removal can leave unused imports or locals, or remove a required terminating
`panic`. These are recorded as build failures; Mutie does not make extra edits
to repair them. Dismissals still match against discovery before operator
selection, so valid decisions for omitted operators are not reported as stale.
`dismiss check` checks all operators within the configured file scope.

Validation on `godotenv` found four surviving CLI mutations. Added behavioral
tests in an isolated copy killed all four; see the
[call-removal validation](benchmarks/godotenv-v1.5.1-call-remove.md) for evidence
and the observed build-failure noise.

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

The [go-cmp sample comparison](benchmarks/go-cmp-v0.7.0.md#affected-package-follow-up)
matched all 32 outcomes and averaged 25.3% less run time with affected scope on
one warm-cache host. A [32-mutation fzf sample](benchmarks/fzf-v0.65.2.md) also
matched outcomes and averaged 16.5% less run time. Results depend on the project
and its dependency structure.

## Live terminal status

Interactive terminals get a live, inline Ratatui display inspired by the Vitest
CLI. The status region updates in place up to ten times per second, including
elapsed time and activity while commands run. It shows the current phase,
completed/total mutations, outcome counts, stable numbered worker rows, and a
results tail below the workers. For example, during mutation execution:

```text
 | mutie mutations  2.0s
 | worker 1 building  #000004 (2.0s) calc.go:8:3: + -> -
 | worker 2 testing   #000005 (2.0s) ready.go:4:2: true -> false

   killed       add.go:3:2: + -> - (0.2s)

 Mutations 3/8  |  Workers 2/2
 2 killed | 1 missed | 0 timeout | 0 build failed | 0 errors
 Ctrl-C to stop  |  Test output saved in report logs
```

Set `ui` in `mutie.toml` or override it with `--ui auto|interactive|plain`:

- `auto` (default): use the live view when stdin and stdout are terminals,
  `TERM` is not `dumb`, and the `CI` environment variable is absent. Fall back to
  plain output if terminal initialization fails.
- `interactive`: request the live view, including in CI. Requires terminal stdin
  and stdout and a `TERM` other than `dumb`; unsupported terminals produce an error.
- `plain`: print progress lines without terminal control sequences.

Worker rows stay in numeric order, including `idle` workers. Busy rows show the
mutation and elapsed time since assignment, including workspace preparation.
`preparing` covers copying/resetting the workspace and applying the mutation.
`running` is the fallback while the command's activity is unknown. Live
[Go build events](https://pkg.go.dev/cmd/go#hdr-Build__json_encoding) can show
`building`; [package test events](https://pkg.go.dev/cmd/test2json#hdr-Output_Format)
show `testing` while an observed package is active. Quiet builds can remain
`running`, and testing takes precedence when build and test events overlap.

`--list` always uses plain output. The live view adapts to resizing and small
terminals, showing as many fixed worker rows as fit and up to three recent results,
with newest completions at the bottom. An overflow row counts hidden workers and
how many are active. Space for the results tail is reserved before results arrive
so completions do not move the worker rows. Narrow terminals use shorter phase
labels (`prep`, `run`, `build`, `test`), worker prefixes, and outcome totals
(`kill`, `miss`, `time`, `build`, `err`; initials at smaller widths).
All results remain in the saved report. The UI uses the normal terminal screen
and allocates at most its height. Earlier output and the shell prompt remain on
screen or in terminal scrollback as space is needed. Resizing and exit cleanup
clear only the live status area. Input stays in normal mode between
cursor-position queries. Ctrl-C uses the same cancellation and cleanup
path as plain mode; SIGTERM also works on Unix. On completion or interruption,
the status area is cleared, the cursor restored, and a final plain summary and
report path are printed. Interrupted summaries include pending/interrupted counts.

Test output stays in saved logs while the live view is active, including with
`--verbose`. Use `--ui plain --verbose` to print captured test output during a run.
Use `mutie report RUN_DIR` to filter and inspect a finalized run.

The MVP has been validated on `godotenv` v1.5.1 as well as the bundled fixture.
Plain and live runs produced identical results for 77 mutations, and added tests
in an isolated copy caught four previously missed mutations. See the
[validation notes](DESIGN.md#mvp-validation-evidence) for runtime, artifact sizes,
terminal checks, and the limits of this small-project validation.

A larger run on `go-cmp` v0.7.0 completed all 1,450 mutations across ten packages.
Workspace operations used 22.3 cumulative seconds versus 7,059.6 seconds in mutation
commands, with all file transfers reflinked on APFS. See the
[larger-project measurements](benchmarks/go-cmp-v0.7.0.md) for the method, outcome
counts, report size, cleanup findings, and limits of these observations.

An application sample on `fzf` v0.65.2 matched full/affected outcomes for 32
mutations across all seven operators. Added tests killed two call-removal survivors
in both scopes. The [application measurements](benchmarks/fzf-v0.65.2.md) cover
runtime, disk use, and test-created temporary files left outside managed workspaces.
The [cleanup follow-up](benchmarks/fzf-v0.65.2.md#test-temporary-directory-cleanup-follow-up)
repeated both scopes with private test temporary directories: zero leftovers and
unchanged outcomes.
This covered fzf's Go tests; its separate Ruby/tmux suite and large-tree scaling
remain outside the validation.

## Browsing a saved run

Open a finalized run in the full-screen Ratatui browser:

```sh
cargo run -- report /path/to/saved/run
cargo run -- report /path/to/saved/run --module /path/to/moved/checkout
```

Browsing needs an interactive terminal, but no Go installation, original checkout,
or `mutie.toml`. It reads the saved report without changing it. Completed, failed,
and interrupted runs are supported, including baseline failures without a mutation
inventory. Running or inconsistent reports are rejected before terminal setup.
Use `summary.txt` and the saved artifacts directly for non-interactive inspection.

The mutation list and detail pane appear side by side in wide terminals and stack
in narrower ones. The browser uses an alternate screen, restoring the prior shell
screen and input mode on exit, Ctrl-C, or SIGTERM on Unix. The live run view remains
inline. Terminals smaller than 24 columns by 12 rows show a resize hint.

| Key | Action |
| --- | --- |
| Up/Down or `j`/`k` | Move through mutations or scroll the focused detail pane |
| Tab | Switch list/detail focus |
| Page Up/Down, Home/End | Page or jump to the first/last row or detail line |
| Left/Right | Scroll the detail pane horizontally |
| `o`, `p` | Cycle outcome/state and operator filters |
| `s` | Cycle all / undismissed / dismissed review filters |
| `/`, Enter, Esc | Edit, apply, or cancel a case-insensitive file substring filter |
| `x` | Clear all filters |
| Space | Mark/unmark the current mutation |
| `a`, `c` | Mark/unmark all visible mutations; clear all marks |
| `d`, `l` | Inspect the selected mutation's saved diff or log |
| `b`, `g` | Inspect baseline or package-discovery logs |
| `v` | Inspect the run's saved dismissal review, including unresolved entries |
| `m` | Inspect the saved mutation score, selection scope, and minimum-score decision |
| `t` | Inspect per-operator counts, unfinished work, and cumulative command time |
| `r` | Show a focused rerun command |
| `e` | Close the browser and print the rerun command to the shell |
| `?`, `q`/Esc | Show help; close the browser |

Filters intersect. Marks persist across filters, and the header counts hidden
marks. Rerun commands use **all marked saved IDs**, including hidden ones, or the
current row when nothing is marked. They use `--rerun`, so pending/interrupted or
previously killed mutations can also be selected. Commands are quoted for POSIX
shells and are never executed by the browser. An actual rerun still checks source
identity and requires a fresh baseline. Use `--module PATH` to change the module
path in the generated command when the checkout has moved.
Marks last only for the current browser session. Use [persistent dismissals](#persistent-dismissals)
to record review decisions across sessions.

The detail pane shows source location, operator, outcome/state, duration, and
available diagnostics. Dismissed rows have a `[D]` badge and show their saved reason
and note in the detail pane. The review filter intersects with outcome, operator,
and file filters; dismissed rows can still be marked and rerun, including when
hidden by a filter. The header's missed count always includes dismissed results.
`v` shows the undismissed/dismissed missed counts and any stale, missing, unmatched,
or ambiguous entries that were not applied.

`t` shows the operator breakdown for the entire saved run selection, independent
of browser filters, marks, or dismissals. Each operator has planned and completed
counts, all five outcomes, pending/running/interrupted counts, and recorded command
time. Time is cumulative across workers, includes interrupted commands with saved
durations, and excludes baseline, discovery, and workspace work; it can exceed the
run's wall time. Recorded and missing timing counts accompany each total. Missing
or invalid timings remain unknown; pending mutations do not count as missing.
Older reports use the same view, derived from their saved inventory and outcomes.
A run without a published inventory shows the breakdown as unavailable.

Review data reflects the run's source snapshot. The browser reads neither current
repository metadata nor source files, including with `--module` (which only changes
generated command paths). Later dismissal edits do not rewrite historical reports.
Older reports and runs that stopped before inventory publication show review as
**not recorded**; all their rows remain available under all/undismissed, with no
rows under dismissed. Missing diff/log files appear as an inline message;
other rows remain usable. Artifact previews read at most 256 KiB, replace terminal
control characters, and flag preview truncation. Full saved artifacts remain on
disk; if log retention omitted output, the browser separately shows byte counts
and explains that the omitted output is absent from the report. These notices
remain visible even when a log file is missing. Artifact paths must resolve inside
the report directory. JSON metadata is
limited to 64 MiB per file. Module directories named `report` or `dismiss` can
still be run as `mutie ./report` or `mutie ./dismiss`.

## Persistent dismissals

Record a review decision for a mutation from a finalized saved run:

```sh
cargo run -- dismiss --module /path/to/go/module add /path/to/saved/run \
  --id 9 --reason 'accepted test gap' --note 'Boundary is validated by the caller.'
cargo run -- dismiss --module /path/to/go/module list
cargo run -- dismiss --module /path/to/go/module check
cargo run -- dismiss --module /path/to/go/module update 1 --reason 'equivalent behavior'
cargo run -- dismiss --module /path/to/go/module update 1 --clear-note
cargo run -- dismiss --module /path/to/go/module remove 1
cargo run -- dismiss --module /path/to/rust/package add /path/to/saved/run \
  --id 3 --reason 'accepted test gap'
cargo run -- dismiss --module /path/to/rust/package check
```

The project defaults to the current directory and must contain `go.mod` or
`Cargo.toml`. Use `dismiss --lang go|rust` to select a language explicitly when
both exist. Adding requires a saved report of the selected language.
`add --id` selects a **saved-run mutation ID**; `update` and `remove` use the
independent **dismissal ID** printed by `add` and `list`. Dismissal IDs are not
reused after removal. Reasons are required nonblank text; notes are optional.
`update --note TEXT` replaces a note, and `--clear-note` removes it. Updating a
reason or note preserves the stored mutation identity.

Decisions live in `mutie.dismissals.toml` beside the project manifest, separate
from generated reports and `mutie.toml`. Commit this file to version control. It
stores the reason, note, language, module-relative source path, SHA-256 source
fingerprint, operator, byte range, source coordinates, and original/replacement text. No absolute
checkout path or run-local ID is used for matching, so the metadata can move
with the repository. CLI edits validate the entire file and replace it atomically;
they reformat TOML and do not preserve hand-written comments. Malformed metadata,
unknown fields, incompatible versions, duplicate IDs, and duplicate additions
are rejected without overwriting the file.

`add`, `list`, `update`, and `remove` need neither language tools nor a valid
`mutie.toml`; they use manifest detection or explicit `--lang`, without loading
configuration or validating the full Cargo execution scope.
Adding records the saved identity without checking current source; listing labels
it unchecked. `check` uses current Go package discovery or validated Rust Cargo
metadata/module discovery, plus the language, include/exclude patterns, discovery
timeout and output limits from `mutie.toml`. Explicit `dismiss --lang` overrides
the configured language. It runs in disposable workspaces,
does not run tests, and creates no reports. With no dismissals, it skips discovery.
Rust checks enforce the initial Cargo scope, exclude caller build artifacts,
and discover identities from a pristine snapshot independently of Cargo side
effects. Timeout, cancellation and capture failures clean up owned directories.

New decisions record `language = "go"` or `"rust"`. Older decisions without a
language retain their Go meaning; they are never silently applied to Rust.
Language mismatches require review. Dismissals describe source judgments rather
than Cargo execution scope: manifest/lockfile/toolchain changes do not invalidate
an otherwise matching decision, though focused Rust reruns still require their
separate execution-scope checks.

Matching is strict, using the same source identity as focused reruns. Any edit to
the mutation's source file invalidates its fingerprint, even a comment or an
unrelated edit in that file. Test edits in separate files do not invalidate it;
edits to inline tests in the selected source file do. `check` reports
matched, stale, missing (removed/excluded/inactive files or files without mutations),
unmatched identity, or ambiguous entries. Duplicate identities introduced by a
manual edit or merge are flagged as ambiguous when they match a current mutation.
It exits nonzero when anything needs review and leaves metadata unchanged. Review
such entries with `list`, remove obsolete decisions, and re-add from a fresh report
when appropriate. It never silently attaches a decision to a different mutation.

Dismissals record human judgment and **do not change execution outcomes or counts**.
Normal runs and focused reruns still execute dismissed mutations; a missed result
remains missed. Each new run captures review metadata from its pristine project
snapshot and matches it against discovery before applying rerun selection. Matching
decisions for selected mutations, with the new run's IDs, and unresolved entries
are stored in `mutants.json`. Valid decisions outside a rerun selection are omitted,
without being flagged as missing. Review issues do not fail execution or suppress
mutations. Invalid metadata fails a run before invoking language tools; `--list`
remains a source-only operation and does not load dismissal metadata.

`summary.txt` separates dismissed and undismissed missed counts and includes reasons,
notes, and unresolved entries. Execution counts in `outcomes.json` and all outcome
lists remain unchanged. To see a newly added or edited decision in the browser,
run Mutie again (a focused rerun is fine) and open the new report:

```sh
cargo run -- /path/to/go/module --rerun /path/to/saved/run --id 9
cargo run -- report /path/to/new/run
```

Press `s` to filter by review status or `v` to inspect the saved dismissal review.

Concurrent CLI edits are guarded by `.mutie.dismissals.lock`, normally removed on
exit. If a process is forcibly stopped and leaves this file behind, confirm that
no dismissal command is running before removing the lock and retrying.

## Focused reruns

After improving tests, use a saved run directory to select just its completed
`missed` mutations. This works for Go modules and supported Rust packages:

```sh
cargo run -- /path/to/go/module --rerun-missed /path/to/saved/run
cargo run -- /path/to/go/module --rerun-missed /path/to/saved/run --list
cargo run -- /path/to/rust/package --rerun-missed /path/to/saved/run --list
```

Use `--rerun` to select mutations regardless of their previous outcome, and
`--id` or `--operator` to narrow either rerun mode:

```sh
cargo run -- /path/to/go/module --rerun /path/to/saved/run --id 9,11
cargo run -- /path/to/go/module --rerun /path/to/saved/run --operator arithmetic_replace,comparison_boundary --list
cargo run -- /path/to/go/module --rerun-missed /path/to/saved/run --id 9,11 --operator arithmetic_replace
```

IDs refer to the **source run's** inventory, not a fresh scan or an earlier run
in a chain of reruns. They must be positive integers. Both filters accept repeated
flags or comma-separated values; duplicate values select work only once.
Multiple IDs or operators select any listed value within that filter. Combining
IDs, operators, and missed-only selection takes their intersection. For example,
`--rerun-missed RUN_DIR --id 9,11 --operator arithmetic_replace` selects only IDs
9 or 11 that were missed arithmetic mutations. A valid ID excluded by another
filter selects nothing; an unknown ID is an error even if other filters would
exclude it. Operator names match the [saved operator catalog](#saved-reports).

`--rerun` and `--rerun-missed` conflict. `--id` requires one of them;
`--operator` also works for fresh scans. Configured `operators` apply to reruns,
too; use `--clear-operators` to remove that restriction. With no filters,
`--rerun` selects the entire saved inventory, including previously killed,
timed-out, build-failed, error, pending, or interrupted mutations.

`RUN_DIR` is relative to the invocation directory, or absolute. It must contain
`run.json`, `mutants.json`, and `outcomes.json`; text lists, diffs, and logs are not
selection inputs. Finalized failed or interrupted runs also work when their
inventory is complete. `--rerun-missed` selects only completed missed outcomes;
`--rerun` can retry unfinished work. Running, incomplete, inconsistent, or
incompatible reports and invalid selectors are rejected before running tools or
creating a new report.

Rust reruns also require the saved language, Cargo package and production-target
inventory (kind, name, source and edition), default-feature/test/profile settings,
complete `Cargo.toml` and
`Cargo.lock` fingerprints, and compiler/Cargo versions to match the current
snapshot. Older single-binary report identities are normalized when read.
Scope mismatches fail before the baseline. Older Rust reports without
a manifest fingerprint remain browsable but require a fresh scan before rerunning.
Changing the manifest, lockfile, or toolchain requires a fresh scan too, including
comment-only manifest/lockfile edits. Worker counts, timeouts, file filters, output
limits, and policies use current settings. The browser exports Rust commands with
`--lang rust`, so a moved checkout with both manifests stays unambiguous.

Execution always runs a fresh baseline, rediscovers mutations from the current
snapshot, and validates every selected target before starting workers. Matching
requires the same module-relative source path, complete source-file fingerprint,
operator, original/replacement text, and source coordinates. Editing or adding
tests in separate files is supported; any change to a selected mutation's source
file, including comments or moved lines, requires a new full scan. Missing,
excluded, inactive, or ambiguous targets fail the entire selection rather than
silently being skipped.
Current include/exclude settings still apply; clear configured filters if needed.

The current module can be a moved or copied checkout. Current configuration,
tests, test scope, worker count, and timeouts apply; old outcomes are never reused. Newly
discovered mutations are not added to the saved selection. If the filters match
nothing, execution saves a completed empty run after baseline and discovery.
`--list` previews and validates the selection without running tests or writing a
report. Each preview row shows its new ID, source ID, operator, and source change;
an empty preview prints an explicit message.

Every execution creates a separate report. Once its inventory is published,
`run.json` contains a `rerun` object with `source_run`, `source_checkpoint`,
`source_status`, `selection` (`"all"` or `"missed"`), optional `source_ids` and
`operators` filters, and `mutations` mapping each new `id` to its `source_id`.
Filters are deduplicated and sorted; new IDs follow current discovery order.
`summary.txt` includes the filters and mapping too. `effective_config.rerun`
records the source path, selection mode, and filters even if the baseline or
matching fails. This replaces the earlier `effective_config.rerun_missed` path;
previously saved reports remain usable. The source report is preserved, excluded
from the snapshot, and cannot contain the new output directory.

## Saved reports

Reports keep publishing authoritative JSON checkpoints as a run progresses.
Derived text summaries and outcome lists are rewritten only when their contents
change or the file needs recreating. Changed files retain atomic publication and
disk synchronization; report contents and formats are unchanged.

By default, every test run saves a new directory under `<module>/.mutie/`. The CLI
prints its location when the run starts and the summary path when it finishes. Earlier runs
are preserved. Use `--output-dir DIR` to choose the parent directory instead;
relative paths are resolved from your current working directory. The output
directory cannot be the module root or an ancestor of it. `--list` writes no
reports and cannot be combined with `--output-dir`.

Each run contains:

- `run.json`: versioned run metadata, effective configuration, commands, scope,
  timeouts, worker counts, workspace measurements, phase, status, findings-policy
  decision, and any run-level error.
- `mutants.json`: the complete mutation inventory, including IDs, named operators,
  source locations, original text, replacements, source fingerprints, `test_packages`, and diff
  paths. The inventory and every diff are published before workers start.
- `outcomes.json`: baseline and package-discovery results, mutation execution
  states and outcomes, durations, exit codes, log paths, retention byte counts, and
  summary counts.
- `summary.txt`: readable counts, per-operator outcomes and command time, workspace
  costs, unfinished work, missed mutations, and artifact references for every
  planned mutation.
- `diff/000001.diff`: one unified source diff per planned mutation.
- `logs/baseline.log`, `logs/discovery.log`, and `logs/000001.log`: readable command
  output, with separate `.stdout.jsonl` (Go) or `.stdout.log` (Rust) and
  `.stderr.log` captures alongside each log. Go stdout contains package JSON or
  test events. Rust discovery stdout contains Cargo metadata; test stdout mixes
  Cargo build events with standard test-harness output from both phases.
  Logs are saved even without `--verbose`. Limited logs
  contain head/tail excerpts and an omission marker; see the log limit below.
- `killed.txt`, `missed.txt`, `timeout.txt`, `build_failed.txt`, and `errors.txt`:
  outcome lists containing IDs and source descriptions, sorted by ID.
- `.mutie.lock`: an empty lock file held open for the report writer's lifetime,
  including its final writes. The file remains after the process exits.

New text summaries include the same operator breakdown as the browser's `t` view.
Totals cover only the run's selected mutations, including for filtered runs and
reruns. They are derived from the inventory and execution records; no additional
JSON fields are required, and browsing an older report does not rewrite its summary.

`run.json` includes `effective_config` with all merged settings, the resolved Go
language, absolute module/output paths, and the config file path (`null` when
absent). Its `timeout` is `null` when using the automatic policy; the actual
deadline, once determined, is in `mutation_timeout_seconds`. Configured `jobs`
and `requested_workers` describe the requested concurrency; `workers` records the
number used, capped by the mutation count. Configuration timeout values use
seconds, matching the TOML file.

`baseline_command` and `discovery_command` record their complete argument lists.
For a mutation, append its inventory `test_packages` to `mutation_command_prefix`.
Full scope records `["./..."]`; affected scope records sorted Go import paths.
These fields replace the earlier single `command` field; older saved reports
remain readable and eligible for reruns.

Mutation IDs are local to each run, assigned in deterministic discovery order.
Inventory and execution records are sorted by ID regardless of completion order;
join them on `id`. Source paths are module-relative and artifact paths are
report-relative. Lines and byte columns are one-based; `end_line`/`end_column` and
zero-based `byte_end` are exclusive. Durations are seconds and timestamps are
Unix seconds. `source_fingerprint` is `sha256:` followed by the lowercase SHA-256
hex digest of the complete original source file. It identifies source content,
not test content or a reusable result. IDs and offsets must not be replayed across
source edits without rediscovery and validation.

After inventory publication, `mutants.json` also contains `dismissals` with
`matched` and `needs_review` arrays. A matched entry pairs `mutation_id` with a
`dismissal` containing its independent `id`, `reason`, optional `note`, and full
`mutation` identity. An unresolved entry has the same dismissal record and a
`status` of `stale`, `missing`, `unmatched`, or `ambiguous`. Readers validate IDs
and matched identities before browsing or rerunning. This data is optional for
older reports; absence means review was not recorded. An empty captured review
explicitly records that no dismissal decisions applied. It remains usable when
the checkout or repository metadata is unavailable.

`schema_version` is `1` in all three JSON files, and `operator_catalog_version` is
`1` in run metadata and the inventory. Named operators are `arithmetic_replace`,
`comparison_invert`, `comparison_boundary`, `logical_swap`, `boolean_flip`,
`negation_remove`, `call_remove`, `branch_force_true`, `branch_force_false`, and
`loop_skip`.

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

The module's `.mutie` directory, the selected output directory, and the source
rerun directory are omitted from snapshots so saved reports do not affect tests
or discovery. Keep output directories separate from source files, and add `.mutie/`
(or your custom output directory) to your project's `.gitignore`.

### Limiting saved logs

Logs are unlimited by default. Set a positive byte limit per artifact with
`--max-log-bytes`, or `max_log_bytes` in `mutie.toml`:

```sh
cargo run -- /path/to/go/module --max-log-bytes 65536
cargo run -- /path/to/go/module --clear-max-log-bytes
```

The limit applies independently to each readable `.log`, raw stdout, and raw
stderr artifact for baseline, discovery, and mutation commands. When output is
larger, Mutie retains roughly half from the beginning and half from the end
(the extra byte for odd limits goes to the beginning). A readable omission marker
is added outside the byte budget. Readable logs cut at UTF-8 boundaries and can
retain a few fewer bytes; raw captures preserve exact byte excerpts, which can
split characters or JSON records. A truncated `.stdout.jsonl` is an excerpt, not
a complete JSON stream.

Each `outcomes.json` log record includes `retention.text`, `retention.stdout`,
and `retention.stderr`, with `total_bytes`, `head_bytes`, `tail_bytes`, and
`omitted_bytes`. Counts exclude the marker. Complete logs have zero omitted and
tail bytes. Older reports without these fields remain readable. The browser
shows saved-log truncation separately from its own 256 KiB preview limit;
`--verbose` and command-failure diagnostics use the same limited readable output.

Classification and package discovery read the **complete temporary captures**
of commands that finish within their capture budget; saved-log truncation cannot
turn a tool/build failure into a killed or missed mutation.
Full stdout/stderr and rendered output remain file-backed until processed and
saved, then are removed. This setting limits retained artifacts and diagnostic
previews, **not temporary disk usage**. Parsing uses one Go output record at a
time; an individual oversized record and the package graph still need memory.
Interrupted commands save limited partial logs with the same metadata. Omitted
output cannot be recovered from the report; rerun with a larger limit or
`--clear-max-log-bytes` when more diagnostics are needed.

### Limiting command-output capture

`--max-capture-bytes BYTES` (or `max_capture_bytes` in `mutie.toml`) sets a positive
byte budget shared by raw stdout and stderr for **each Go or Cargo invocation**.
It defaults to unlimited. This applies to baseline tests, discovery (including
`--list` and configured `dismiss check`), and mutation tests. Each concurrent
command has its own budget. Reruns use current settings, not the source
report's budget.

For Rust, Cargo metadata, compilation (`--no-run`), test execution, and optional
worker-cache package cleanup each get a separate capture budget. Saved
baseline/mutation logs combine their phases, so combined raw byte counts can
reach twice the budget (three times for cached mutations), plus phase separators.
Cleanup, compilation and testing still share one timeout.

```sh
cargo run -- /path/to/go/module --max-capture-bytes 16777216 --max-log-bytes 65536
cargo run -- /path/to/go/module --clear-max-capture-bytes
```

With the limit enabled, Mutie drains both output pipes into temporary files and
never writes more than the combined byte budget to those raw captures. Output
ending exactly at the limit is allowed. Reading another byte stops the command's
process group and reports an explicit output-capture error with exit code `1`.
A baseline or discovery overflow prevents mutation execution; a mutation overflow
is a tool error and stops the run, including active peers. It is never classified
as killed, missed, or timed out, even if a test-failure event was already received.
Findings and minimum-score policies stay unevaluated.

The captured prefix is retained for diagnostics, with the saved-log limit applied
afterward. stdout/stderr are independent prefixes; their relative interleaving is
not recorded. Raw output can end within a UTF-8 character or JSON record. Readable
logs include an error notice, and the failure is saved in run/mutation errors or
the baseline/discovery record's `capture_error`, independently of log truncation.
Retention byte counts describe bytes captured, not all bytes the child attempted
to emit; the uncaptured amount is unknown. Increase or clear the capture limit
to rerun a command that needs more output.

This is a raw command-output budget, not a quota on all disk use. Rendered readable
logs, saved reports, workspaces, test-created files, and Go build/module caches
are separate. Readable rendering can expand invalid UTF-8, and JSON decoding still
needs memory for individual captured records. Deadlines and cancellation remain
active while draining. After process-group shutdown, Mutie allows up to one second
for buffered output to reach EOF; a pipe still held open is an explicit capture
error instead of an indefinite wait.

## Report retention

List saved runs with their status, age, logical file size, and any protection reason:

```sh
mutie runs                       # .mutie under the current directory
mutie runs /path/to/reports       # custom report root
mutie prune --keep-last 5         # preview keeping the newest five eligible runs
mutie prune --older-than 30       # preview runs finished more than 30 days ago
mutie prune --keep-last 5 --older-than 30
mutie prune --keep-last 5 --older-than 30 --apply
```

Both commands accept an optional report parent directory, defaulting to `.mutie`
relative to the current directory. They work without Go, the original checkout,
or `mutie.toml`; pass custom output directories explicitly. A missing report root
is an empty listing and is not created. Listing and pruning previews write nothing.

`prune` requires at least one rule and previews by default. `--apply` deletes the
selected run directories and all their report artifacts. `--keep-last N` preserves
the newest N eligible finalized runs, ordered by finish time (then name for ties).
Protected runs do not consume these slots; `--keep-last 0` selects all eligible
runs. `--older-than DAYS` accepts positive whole days of 24 hours and selects
runs strictly older than that threshold. With both rules, a run must be outside
the newest N **and** older than the age threshold to be selected. No automatic
retention or archival runs in the background.

Only direct `run-*` directories with supported, consistent finalized report metadata
and the expected artifact layout are eligible. Completed, failed, and interrupted
runs can qualify, including failed baselines with no inventory. Running reports
remain protected even after a forced kill. Locked reports, invalid metadata,
future/inconsistent timestamps, symlinks, special files, and unexpected files or
directories inside a run protect that entire run. Other root entries are ignored.
Sizes sum file lengths without reading logs; they are not physical storage usage
or a guarantee of space reclaimed on compressed or copy-on-write filesystems.

Apply acquires each selected report's lock and checks its metadata, identity, and
artifact list again before removing it. A changed or newly locked report is left
in place and reported as an error. Older finalized reports without a lock file
are supported; apply adds the lock before revalidation. Cancellation stops between
runs; deletion already underway finishes first. Filesystem failures can leave a
partially removed run and return an error with the number of fully deleted runs.

### Workspace measurements

The final CLI summary and `summary.txt` show cumulative snapshot, setup, and reset
times, plus counts of reflinked and copied files. Setup combines baseline and worker
creation; reset combines the reset before discovery and resets between mutations.
These times can overlap across workers, so they are not the run's wall-clock time.

`run.json.workspace_metrics` keeps the breakdown under `snapshot`, `baseline_setup`,
`discovery_reset`, `worker_setup`, and `worker_reset`. Each contains:

- `operations`, `completed`, `failed`, and `interrupted`: filesystem operation counts.
- `duration_seconds`: cumulative elapsed time, including failed/interrupted operations.
- `reflink_files`: successful copy-on-write clones reported by the copy library.
- `copied_files` and `copied_bytes`: successful fallback copies and their logical byte
  counts, including empty files. These are not measurements of physical disk traffic
  or space saved; the OS may optimize fallback copies too.

Every reset counts its transfers again. Directories, symlinks, excluded files, and
failed file transfers are not counted. Successful transfers before a later failure
or interruption are retained. Timings cover directory creation, copying, permission
preservation, and reset deletion. They exclude the blocking-pool queue, applying
mutations, Go commands, and final workspace cleanup. Workspace completion describes
the filesystem operation even when the subsequent Go command fails or is interrupted.
Command durations remain in `outcomes.json`.

Measurements are saved at normal report checkpoints, with worker measurements
collected when the worker job returns. They do not add report writes per copied file
or operation. `--list` and `dismiss check` do not produce these reports or summaries.

The canonical temporary parent directory is recorded in
`workspace_metrics.temporary_root` and `summary.txt`. On Unix, set `TMPDIR` to an
existing, writable directory outside the source tree to choose workspace placement:

```sh
mkdir -p /path/on/project-filesystem/mutie-tmp
TMPDIR=/path/on/project-filesystem/mutie-tmp cargo run -- /path/to/go/module --jobs 2
```

For copy-on-write comparisons, use a directory on the project's reflink-capable
filesystem and inspect the recorded counts. A different filesystem or a tmpfs can
prevent reflinks; support also depends on the OS and filesystem. Keep project inputs,
worker counts, timeouts, and Go cache conditions consistent when comparing runs.
The [go-cmp measurements](benchmarks/go-cmp-v0.7.0.md) exercise a larger mutation
inventory on APFS. Comparisons against Linux tmpfs, ordinary copies, and projects
with large source trees or assets are still pending.

Each child Go command gets a private `mutie-go-*` directory for `GOTMPDIR`.
If the environment contains a nonempty `GOTMPDIR`, Mutie uses that existing,
writable directory as the parent; relative paths resolve from Mutie's starting
directory. Otherwise it uses the system temporary directory (`TMPDIR` on Unix).

Each Go command also gets a separate `mutie-test-*` directory under the system
temporary parent. Mutie sets `TMPDIR`, `TMP`, and `TEMP` to this absolute path
in the child, so test-created files and helper subprocesses use the same owned
directory. This applies to baseline tests, discovery, and mutation tests, with
separate directories for concurrent commands. The caller's environment is unchanged.

Both private directories and their contents are removed when the command ends,
including after startup errors, timeouts, or handled interruptions. Existing
parent contents and the reusable Go build cache (`GOCACHE`) are preserved.
Cleanup cannot run if Mutie itself is forcibly killed; files written to explicit
paths outside the private directories are not covered by this cleanup.

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

## Development

```sh
cargo test
cargo test --test go_workflow -- --ignored
cargo test --test affected_packages -- --ignored
cargo clippy --all-targets -- -D warnings
```

The Go integration test checks all four outcomes, repository configuration and
scope, baseline failure and timeout, workspace resets, Go build-directory cleanup,
saved reports, and preservation of input files. It needs Go on `PATH` and a writable
Go build cache, and is opt-in because it runs real builds and deliberately waits
for a timeout. Non-ignored Unix tests use a fake Go command to verify SIGINT/SIGTERM
during baseline, discovery, and parallel execution, preserved diagnostics, pending
work, process and build-directory cleanup, and source preservation. They require
an environment that delivers those signals.
The additional real-Go call-removal test covers multiline and parenthesized calls,
killed/missed/build-failed results, dismissal identity, and a stronger test killing
a survivor on an affected-package rerun.
The opt-in affected-package fixture compares full/affected outcomes and verifies
transitive and internal/external test imports, unrelated-package omission, and
fresh selections after adding tests to a focused rerun.
Configuration tests also cover defaults, explicit overrides, list clearing,
invalid settings, path resolution, listing without reports, and saved effective
settings using a fake Go command.
Rerun tests cover saved-report validation, stale/missing/ambiguous targets, fresh
baselines, ID/operator/outcome intersections, invalid selectors, selection
previews, new ID mappings, empty selections, source report preservation, and
improved tests turning a previous missed result into killed. The opt-in real-Go
test also selects a killed and a dismissed missed mutation by ID and operator,
checks the dismissal against real discovery, and verifies that a test-only change
preserves its identity while the new tests kill the previously missed mutation.
The real-Go branch fixture covers locals/imports used only in conditions,
initializer and condition side effects, named/generic booleans, shadowed boolean
names, config/CLI selection, diffs, dismissals, and affected-package reruns after
stronger tests. Parser tests cover nested and multiline conditions, comments,
Unicode, CRLF, literal exclusion, and strict source matching.
The real-Go loop fixture verifies both header forms, unused-name preservation,
named/generic booleans, initializer/condition/body/post effects, saved selections,
dismissals, and a stronger-test rerun. Parser tests check excluded loop forms,
nested discovery, exact condition ranges, and multiline diffs.
Dismissal tests cover persistence, independent IDs, reason/note edits, removal,
strict rediscovery, moved checkouts, stale/missing/ambiguous entries, malformed
metadata, concurrent-writer exclusion, and preservation of reports, source files,
execution outcomes, counts, and rerun selection. Report tests cover frozen review
data, rerun ID remapping, omitted valid decisions, invalid snapshot rejection,
and compatibility with older reports. Browser snapshots and PTY tests cover review
filters, badges, reasons/notes, hidden marks, export, and checkout-free viewing.
Browser tests cover saved IDs, intersecting filters, marks hidden by filters,
quoted command export, missing/bounded artifacts, Unicode input, and frame
snapshots at wide and compact sizes. Unix PTY tests open reports with no Go or
checkout, exercise paste and resize, and verify shell/termios restoration after
quit, export, Ctrl-C, and SIGTERM without changing report files.
Renderer tests check complete frames, styles, clipping, elapsed-time updates, and
resizing, stable idle/busy rows, and results ordering. Progress tests cover partial
output lines, interleaved packages, stale events after worker reuse, full capture
preservation, and cancellation/deadlines during blocked progress observation.
Unix pseudo-terminal tests exercise live phases before command completion and
redraws during a blocked baseline, Ctrl-C, resizing, terminal initialization
failure, empty/failed runs, terminal restoration, and plain output in CI. A
terminal screen/scrollback model checks that pre-existing output and prompts
survive completion, cancellation, width/height changes, and oversized worker
views. A regression deliberately delivers frames prepared at the old size after
a resize, verifying history preservation and normal line wrapping afterward.
Temporary workspace paths are
canonicalized so macOS path aliases agree with Go discovery and internal symlinks.

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
