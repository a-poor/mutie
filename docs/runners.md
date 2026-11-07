# Languages and test runners

[Documentation index](README.md) · [Project overview](../README.md)

## At a glance

| Language / runner | Toolchain | Starting point |
| --- | --- | --- |
| Go / go test | Go 1.24+ | One module; multiple packages are supported. |
| Rust / Cargo | Rust/Cargo; see the [validated scope](../SUPPORT.md#rust--cargo) | Ordinary packages and workspaces with standard tests. |
| JS/TS / Vitest | Node 24.22.0, Vitest 4.1.9, Vite 8.1.3 | One ESM package; audited package-manager installation. |
| JS/TS / Node | Node 24.22.0 | One ESM package without runtime dependencies. |
| JS/TS / Bun | Bun 1.3.11+af24e281e | One package without runtime dependencies. |
| Python / pytest | CPython 3.14.7, pytest 9.0.2 | One flat or src package; narrowly audited dependencies and fixtures. |

`mutie init` can identify supported tooling and write an initial configuration.
Discovery and a passing isolated baseline are still required. See the sections
below for layout, dependency and test-runner restrictions.

## Vitest and package managers

Package installation and test-runner selection are separate. Vitest accepts
`packageManager` selectors `pnpm@11.10.0` (isolated local store), `npm@11.19.1`
(lock v3), `yarn@4.18.1` (node-modules/classic), and `bun@1.3.11` (hoisted text
lock), with the audited Vitest 4.1.9 and Node 24.22.0 runtime. Install dependencies
before running Mutie; Mutie does not install packages or execute package-manager
scripts. Node/Bun tests still use their own pinned runtime regardless of installer.
PnP, external dependency links, workspaces, Yarn's pnpm linker and Bun's isolated
linker are outside this acceptance. Vite+ may manage installation, but `vp test`
uses a different bundled runner and is explicitly unsupported. See the
[package-manager audit and validation](../benchmarks/package-managers.md).

Run `mutie PATH --lang vitest --ui plain` against a prepared, pinned ESM
package to discover supported expression mutations, run the full suite in isolated copies, and save
scored outcomes. This initial path supports one worker. A static coverage-reporter-only
Vite/Vitest config is accepted without executing it; coverage remains disabled.
Plain-JSON NodeNext type-checking configs are accepted only for JavaScript-only
projects with `allowJs`, `checkJs` and `noEmit` enabled; runtime TypeScript still
requires ESNext. See the [Hyparquet scope audit](../benchmarks/vitest-javascript-hyparquet.md) and
[JavaScript findings and stronger-test reruns](../benchmarks/vitest-hyparquet-findings.md).
The [full Hyparquet inventory review](../benchmarks/vitest-hyparquet-full.md) records
all 133 attempted candidates, useful test gaps, and remaining errors/limits.
Vitest resolves Node through PATH once per run, then rechecks the pinned executable
version/path and project identity before each baseline or mutation command. See
the [Node reuse validation](../benchmarks/vitest-node-reuse.md).
Other runner config settings remain unsupported. See
[config support](../benchmarks/vitest-coverage-config.md). Worker evidence distinguishes body failures from fixture, hook and callback
failures: successful lifecycle work can accompany a genuine kill, while lifecycle
and mixed failures are errors. Pending async assertions from setup and post-body
assertion-count failures remain conservatively rejected.
Worker evidence requires the audited Vitest runner bytes; expected-failure tests
(`test.fails`) are unsupported. See the [lifecycle evidence](../benchmarks/vitest-lifecycle-attribution.md). Dismissal review, `--fail-on-missed`, and `--min-score` are
available within this scope; see [review/policy validation](../benchmarks/vitest-dismissals.md).
Use `mutie PATH --lang vitest --rerun-missed RUN_DIR` after improving tests;
`--rerun RUN_DIR --id ID` selects individual saved mutations. Reruns require
unchanged source identity and pinned package/runner/config identity; see
[rerun validation](../benchmarks/vitest-focused-reruns.md).

`mutie vitest-list PATH --include 'src/**/*.ts'` remains a read-only preview;
`mutie vitest-check PATH` checks an isolated baseline with a deadline and bounded
output. These two diagnostic commands do not apply `mutie.toml` or save reports;
the scored run does. See [CLI validation](../benchmarks/vitest-shared-runs.md) and the
[execution boundary](../benchmarks/vitest-execution.md) for pinned dependencies,
setup and conservative failure attribution.

## Scoped Node test runner

```sh
mutie PATH --lang node --ui plain
mutie PATH --lang node --list
mutie PATH --lang node --rerun-missed RUN_DIR
```

The initial scope is one dependency-free ESM package using audited Node **24.22.0**
on PATH and `.js` files under `src/` and `test/`. The package test script may be
exactly `node --test` or `node --test test/*.test.js`. Declared development
installations may be present: their complete owned `node_modules` tree and manager
metadata are fingerprinted and copied into isolated runs. Imports must be
static literal relative paths within those directories or `node:` built-ins.
CommonJS, custom loaders/runner flags, workspaces and runtime dependencies are
unsupported. The supported operators are `boolean_flip`, `comparison_invert`,
`comparison_boundary`, `logical_swap`, `negation_remove`, `arithmetic_replace`,
`branch_force_true`, `branch_force_false`, `loop_skip`, and `call_remove`, with `--jobs 1`
and full-suite testing.

The validated native TypeScript extension also accepts erasable `.ts` files
under `src/` and `test/`, using `node --test` or exactly `node --test test/*.test.ts`.
JavaScript tests may import `.ts` sources, and TypeScript tests may import `.js`
helpers. Local imports must include the real file extension, including `import type`.
Runtime booleans, comparisons, logical expressions, negations and arithmetic are mutated; type-only syntax is excluded. Node strips types without type-checking or
applying `tsconfig.json`; type errors alone cannot kill mutations. An optional
plain-JSON NodeNext type-checking config is audited and fingerprinted, including
its absence. Config edits require a fresh scan. Enums, namespaces, parameter
properties, angle-bracket assertions, decorators, declarations (`.d.ts`/`declare`),
TSX, MTS/CTS, path aliases and build/loader transforms remain unsupported. See the
[scope and independent TypeScript evidence](../benchmarks/node-typescript-scope.md).

For `node --test`, Mutie audits the complete native default selection across the
project and checks it against the pinned Node implementation. All selected files
must be audited `.js`/`.ts` files under `test/`, including nested files and helpers.
Matching tests elsewhere cause rejection; they are never silently omitted. Every
selected file must complete real tests. Visible symlinks are unsupported, and
report-directory exclusions cannot remove tests. See [default discovery and
unchanged-project acceptance](../benchmarks/node-default-discovery.md).

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

The [Node acceptance record](../benchmarks/node-initial-scope.md) covers real-project
findings, stronger-test reruns, cleanup evidence and remaining attribution limits.
[Body-exception attribution](../benchmarks/node-body-attribution.md) extends the
initial assertion-only boundary. Its observer changes execution identity; start
a fresh scan instead of rerunning an old Node recipe. Existing reports are preserved.
The [remaining SRI findings review](../benchmarks/node-sri-final-review.md) explains the
pending-promise timeout and preserves the cleanup survivor without claiming equivalence.
[Second-project acceptance](../benchmarks/node-openclaw.md) records an unchanged
39-candidate `openclaw-cc` scan and three stronger-test kills.
The [storage/profile review and milestone audit](../benchmarks/node-storage-profiles.md)
adds five further kills and establishes Validated maturity within the same pinned scope.
That milestone is JavaScript-only. Native TypeScript's first audit required a
Bragkit script adaptation; [the follow-up](../benchmarks/node-default-discovery.md)
now runs the unchanged package and kills the same three HTML-list survivors with
stronger tests, establishing separate Validated acceptance. The
[remaining Bragkit review](../benchmarks/node-bragkit-survivors.md) kills all 18 other
survivors with tests for CLI defaults, collector behavior and browser fallback.

## Bun

The Bun runner’s [initial audit](../benchmarks/bun-initial-scope.md)
records JS/TS fixtures and real-project probes. A
[bounded observer prototype](../benchmarks/bun-body-attribution.md) now distinguishes
body failures from unsafe outcomes in paired probes. `mutie bun-check PATH` now
checks a baseline in a fresh copy using pinned Bun 1.3.11+af24e281e. This diagnostic
accepts dependency-free ESM packages with `scripts.test: "bun test"`, `.js`/`.ts`
source under `src/`, and `.test.js`/`.test.ts` tests under `test/` or `tests/`.
A narrow plain-JSON NodeNext/ES2022 tsconfig is accepted; aliases, inherited config,
custom Bun configuration, runtime dependency imports, project symlinks and dynamic
imports remain unsupported. Declared development installations are retained and
fingerprinted, including owned links within `node_modules`. Examples/release scripts may be present but cannot be imported
by audited source/tests. The unchanged 272-test Composer baseline and stronger-test
finding now pass through this executor; see [acceptance](../benchmarks/bun-composer-execution.md).
`mutie bun-list PATH [--include GLOB] [--exclude GLOB] [--json]` previews runtime
supported expression mutations without launching Bun. `bun-check --json` emits baseline execution
identity; see [discovery and identity](../benchmarks/bun-discovery-identity.md).
`mutie PATH --lang bun` now runs serial, full-suite expression mutations with saved
reports and focused reruns (`--rerun RUN_DIR --id ID` or `--rerun-missed RUN_DIR`).
JavaScript and TypeScript are independently Validated within this scope. Raw captures have a mandatory
4 MiB default budget; Unix per-file limits bound evidence, not total disk usage.
Reruns allow stronger tests but reject runtime bytes, package/config or observer
recipe changes. See [scored workflow evidence](../benchmarks/bun-shared-runs.md).
The [Composer findings review](../benchmarks/bun-composer-survivors.md) kills its four
remaining survivors and turns the queue timeout into a body-failure kill through
test-only changes; original full-run scores remain unchanged.
Independent JS/TS [review and policy checks](../benchmarks/bun-dismissals.md) pass;
[JavaScript project acceptance](../benchmarks/bun-javascript-pp-is.md) records seven
stronger-test kills on an explicitly adapted pp-is copy. Its source/tests are
unchanged. [Unchanged-layout acceptance](../benchmarks/bun-unchanged-layout.md) now
supports that original checkout too: absent `type` requires static JavaScript
import/export syntax; root/dist declarations and ancillary packaging files are
fingerprinted but cannot be imported by audited runtime files. Hidden packaging
tests and CommonJS runtime files remain rejected.

For real Bun mutation runs, prefer `cargo build --release` and
`target/release/mutie PATH --lang bun`. The [execution profile](../benchmarks/bun-feedback-profile.md)
measured a 22-candidate run at 306.65s in fully unoptimized debug versus 19.76s
in release with matching outcomes. Development and test builds now optimize the
`sha2` dependency to reduce fingerprinting overhead while retaining every byte check.

## Python / pytest

Mutie's root `mutie.dismissals.toml` and `.mutie.dismissals.lock` are review metadata,
excluded from Python execution copies and application-data identity. Nested files
with those names remain ordinary tracked inputs. Reviewing findings therefore
does not invalidate a stronger-test rerun. Reports from before this fix still open,
but the changed execution recipe requires a fresh run before rerunning them.

Python/pytest is Validated within its pinned serial scope on macOS arm64.
[Scoped dependency acceptance](../benchmarks/python-dependencies-tinycss2.md) adds
one explicitly declared dependency: the byte-verified pure-Python `webencodings`
0.5.1 package and five audited imports. Unchanged Tinycss2 **1.2.1** runs 480 tests;
a separate 484-test copy kills four default-filtering and escaping survivors.
This does not establish support for arbitrary dependencies or current Tinycss2.
[Unchanged Tomli-W validation](../benchmarks/python-tomli-w.md) adds a non-numerical
271-case suite, copied fixture-data reads, confined temporary binary output and
explicit expected failures bound to a pristine executed baseline. Its 30 candidates
produce nine kills, two survivors and 19 errors across preserved runs; one extra
test in a separate copy kills a default newline-formatting survivor. Error reports
remain failed and unscored. Fixture data and directory changes require fresh runs;
general skip/xfail marks, plugins and unrestricted filesystem access remain excluded.
[Unchanged betting-math-kit validation](../benchmarks/python-betting-math.md) covers
198 original tests and six metric-boundary survivors killed by a separate
205-test stronger suite. Narrow inert tool metadata and direct independent
`random.Random(...)` construction are supported; global random-state APIs remain
excluded (apart from existing test-only `random.sample`).
The [complete 87-candidate review](../benchmarks/python-betting-math-full.md)
accounts for 19 kills, 59 survivors and nine errors; a separate 222-test copy
kills 24 survivors and two endpoint-error candidates. Original reports and
classification rules remain intact; combined accounting is not a full score.
The [Scrubify acceptance](../benchmarks/python-scrubify-findings.md) records a real
blank-query survivor killed by stronger tests, timings and preserved inputs/reports.
The [feedback profile](../benchmarks/python-feedback-profile.md) measures repeated
full and focused runs and optimizes runtime traversal while retaining content checks.
The [scope and baseline integration](../benchmarks/python-baseline-integration.md)
adds `python-list` and `python-check` for one dependency-free package with
audited `test_*.py` files at the root or under `tests/`, and a pinned virtual environment supplied through `--python`.
Both flat `package/` and native `src/package/` layouts are supported. The
[native src acceptance](../benchmarks/python-src-layout.md) runs Scrubify with its
original layout, source, tests and manifest, then kills a survivor with stronger
tests in a separate copy. Narrow static setuptools metadata, a pytest-only test
extra, the matching import root and quiet output are admitted without installing
the project or executing its build backend.
The [unchanged Cycler validation](../benchmarks/python-cycler-unchanged.md) covers a
second, larger 28-test suite, root tests, literal version metadata and inert
documentation/CI files. Its original mixed-failure error stays unscored; one
additional test assertion in a separate copy makes that mutation a body-failure kill.
Explicit `--lang python --python /path/to/venv/bin/python` also supports
[scored runs, saved reports, focused reruns and review](../benchmarks/python-shared-runs.md).
[Scoped unittest acceptance](../benchmarks/python-unittest-pluralizer.md) adds direct
synchronous `unittest.TestCase` classes through pytest, with separate setup,
body, teardown and cleanup evidence. Custom dispatch, subtests, skips, expected
failures, mock/runner APIs and asynchronous cases remain excluded.
Scoped root and `tests/conftest.py` fixture declarations are Validated within
the audited subset, including unchanged Binary Tree Library acceptance.
Function/class/module/session scopes, autouse fixtures, literal parametrization,
yield cleanup and registered finalizers have native lifecycle controls. Fixture
configuration changes require a fresh run. Root `pytest.ini` admits only full-scope
testpaths; a test-package initializer must contain only comments/docstrings. See the
[fixture audit](../benchmarks/python-conftest-fixtures.md).
[Unchanged Route Mini validation](../benchmarks/python-route-mini.md) adds 68-test
fixture-based acceptance, audited conftest helpers/imports and inert example/tool
metadata. All 38 mutations are accounted for across four runs; three selected
survivors are killed by stronger tests. Mixed errors/timeouts remain unscored.

The runtime is CPython 3.14.7 / pytest 9.0.2 with the audited dependency set.
Boolean flips, binary arithmetic, single-comparison inversion/boundaries, binary
`and`/`or` swaps and `not` removal are supported, with full tests and one worker. Chained comparisons and annotations
are excluded. See [comparison semantics and validation](../benchmarks/python-comparison-boundaries.md). Other application
dependencies, general conftest hooks/user plugins, async/thread-starting suites and
multi-package layouts remain excluded. Completed assertion bodies can be kills;
runtime exceptions and ambiguous/lifecycle failures remain errors. See
[support limits](../SUPPORT.md#python--pytest) before using an existing project.

Create a dedicated environment with CPython **3.14.7**, install only the pinned
[pytest requirements](../benchmarks/fixtures/pytest-audit/requirements.txt), and
pass that environment's launcher without resolving its symlink. For a project
that declares the supported `webencodings` requirement, use a separate environment
with the additional [hash-pinned wheel](../benchmarks/fixtures/python-dependency/requirements.txt).
Mutie never resolves or installs dependencies; undeclared additions are rejected:

```sh
mutie python-list /path/to/project --python /path/to/venv/bin/python
mutie python-check /path/to/project --python /path/to/venv/bin/python
mutie /path/to/project --lang python --python /path/to/venv/bin/python
mutie /path/to/project --lang python --python /path/to/venv/bin/python \
  --rerun-missed /path/to/saved/run
```

## Rust support

Rust supports ordinary packages and workspaces whose members have a binary,
a library, or a library plus one binary, with a usable root `Cargo.lock`, default
features, and standard unit/integration tests on Unix. The ordinary-library scope is validated through
fixtures and [Marathon findings and focused reruns](../benchmarks/rust-library-marathon.md);
Mutie remains the binary-only real-project control.
Try the [small fixture](../tests/fixtures/rust/README.md):

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
The [focused rerun validation](../benchmarks/rust-focused-reruns.md) records the
fixture and Mutie self-test evidence, including strict matching and runtime costs.
The [dismissal validation](../benchmarks/rust-dismissals.md) covers review commands,
saved decisions, policies and source/artifact preservation.
The [comparison-boundary fixture](../tests/fixtures/rust-boundaries/README.md) adds
five ordering mutations: four killed and one missing an equality test. A stronger
integration test kills that survivor through a focused rerun.
See the [boundary validation record](../benchmarks/rust-comparison-boundaries.md)
for regression checks and the Mutie self-run.
The [logical-swap fixture](../tests/fixtures/rust-logical/README.md) covers grouping,
short-circuit effects and let-chains, including a saved survivor killed by a mixed-input
test. The [acceptance review](../benchmarks/rust-milestone-acceptance.md) collects the
verified workflow, scope limits and runtime evidence. The
[first feedback-time improvement](../benchmarks/rust-report-write-reruns.md) reduces
redundant report writes and preserves the saved-selection outcomes in a measured
Mutie self-run; tests still run in full. The [library fixture](../tests/fixtures/rust-library/README.md)
extends discovery to all production roots and exercises fresh/cached execution
and stronger-test reruns. [Workspace acceptance](../benchmarks/rust-workspaces.md)
adds virtual/root-package workspaces, inherited metadata and internal path dependencies.
Run from the workspace root; repeat `--rust-package NAME` to select mutation members,
or set `rust_packages = ["core", "cli"]` in `mutie.toml`. Omit selection (or use
`--clear-rust-packages`) to mutate all members. Both baseline and mutations run
all workspace members' unit/integration tests with `--workspace`, regardless of
`default-members`. Member selection produces a filtered score, and source globs
are workspace-relative. Saved reports and browser commands retain the selection.

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
compilation (all local packages in a workspace). This forces local binaries, unit tests and integration tests
to rebuild even when source timestamps move backwards. Caches are never shared
between workers or runs and are removed when the run ends. Use
`--rust-build-cache fresh` to select the default behavior explicitly. Saved
settings and text summaries identify the mode; focused reruns use the current
mode. On the six-mutation Mutie sample, warm compilation took 22–46 seconds
versus 66–73 seconds for the same mutations with fresh artifacts. Whole-run
timing also varied between fresh baselines; see the
[cache validation record](../benchmarks/rust-build-cache.md) for results and limits.

The [process reliability follow-up](../benchmarks/process-reliability.md) records
parallel capture and report-lock regressions discovered while testing Mutie
itself, including the fixes and unmutated reproduction checks. A subsequent
[parallel score rerun](../benchmarks/rust-parallel-score-reruns.md) kills all six
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

Discovery follows ordinary Rust modules and mutates boolean literals, logical operators, arithmetic, and ordering
comparisons in function/method bodies, including closures and match guards.
`comparison_boundary` swaps `<` ↔ `<=` and `>` ↔ `>=` by editing only the operator
token. It leaves operand expressions intact and supports generic/overloaded
comparisons. It does not change `==`, `!=`, shifts, ranges, or generic brackets.
`comparison_invert` edits `== ↔ !=`, `< ↔ >=`, and `<= ↔ >` tokens, retaining
implicit comparison borrowing and native `PartialEq`/`PartialOrd` method dispatch.
Ordering inversions are not Boolean complements for unordered floating-point values.
`negation_remove` covers Boolean, integer bitwise and custom `Not` uses of `!`.
It replaces `!operand` with `(operand,).0`, preserving a single evaluation in value
context even beneath a borrow such as `&!x`. It removes the `Not` call and its
effects; output-type mismatches remain build failures. Nested `!!x` has two
independent candidates. Never types, macro bangs and unary minus are excluded.
See [semantic controls and unchanged-project findings](../benchmarks/rust-inversions.md).
`logical_swap` exchanges `&&` and `||`, preserving operand grouping and comments
with parentheses where required. Operands retain their order and appear once;
the changed operator intentionally changes short-circuit evaluation. Let-chain
connectors are excluded, while ordinary boolean expressions inside the chain
remain eligible. Bitwise operators, reference syntax and closure delimiters are
not logical-swap candidates.
`arithmetic_replace` edits binary `+ ↔ -`, `* ↔ /`, and `% → *` tokens. These
pairs share precedence/associativity, retaining grouping, evaluation order and
ownership without added borrows or clones. Native overloaded traits, integer
truncation, floating-point values and panics apply. Missing replacement traits or
compile-time arithmetic errors produce `build_failed`; runtime panics retain the
existing test-failure classification. Unary signs, dereferences and compound
assignments are excluded. See [semantic controls and real-project evidence](../benchmarks/rust-arithmetic.md).
Test-only code, `cfg`/`cfg_attr` subtrees (including active platform code),
macro tokens/expansions, generated files, constants/type-level code and array repeat lengths are
excluded. The run prints and saves discovery exclusions. Libraries use ordinary
`lib` or `rlib` crate types; doctests and ignored tests are not run. Proc-macro
packages, other library crate types, multiple binaries per package, nested workspaces,
external path/git dependencies, project build scripts, custom dev/test profiles
or harnesses, feature selection, and custom
compiler/runner settings are outside this scope. Release/bench-only profiles and
non-test examples/benchmarks are retained; they do not expand the test scope.
Only artifact-location,
network and terminal Cargo config sections are accepted; in-project artifact
directories must be under `target/`. Toolchain override files are rejected;
an inherited `RUSTUP_TOOLCHAIN` selection is preserved and actual tool versions
are recorded. See [the original binary contract](../DESIGN.md#initial-rust-support-contract)
and [library scope](../benchmarks/rust-library-scope.md) for detail.

The first self-testing sample uses two literals in Mutie's score code:

```sh
cargo run -- . --include src/score.rs --operator boolean_flip \
  --jobs 1 --timeout 600 --baseline-timeout 300
```

See [the validation record](../benchmarks/rust-first-slice.md) for measured costs and
results. Fresh builds and the full test suite make self-testing a minutes-long run.

Real Cargo workflow regression tests are opt-in to prevent recursive self-tests:

```sh
cargo test --test rust_workflow -- --ignored --test-threads=1
```
