# Target support

This is the support ledger for Mutie's language and test-runner targets. It tracks
verified behavior, scope limits, evidence and the work needed to advance each
target. Last reviewed: 2026-10-15. [README.md](README.md) documents usage;
[TODO.md](TODO.md) sets implementation priorities. A planned target is not available
through the CLI, and this table does not set implementation order.

## Support levels

Levels are cumulative and apply only to the explicitly documented scope. They
measure implementation and validation maturity, not a percentage of language
coverage, a stability guarantee, or equivalence with another target.

| Level | Required evidence |
| --- | --- |
| Planned | Target selected for the roadmap; no working mutation execution claimed. Initial scope and validation project may still be undecided. |
| Experimental | A defined initial scope has working discovery, isolated baseline/mutation execution and saved outcomes, exercised by an automated fixture. Remaining workflow gaps are explicit. |
| Validated | The scoped run → inspect → improve tests → rerun workflow works on a real project. Reports, review/policies, failure classification, timeout/cancellation and source/artifact preservation have regression evidence; runtime and finding usefulness are recorded. |
| Established | Validated behavior has evidence across multiple representative real projects, with documented scope, useful findings, runtime comparisons and ongoing regression coverage. |

Mutie remains an alpha. Established does not mean every project layout, platform,
runner version or language construct is supported. Operator breadth and project
compatibility are tracked separately below. A shared UI or report implementation
does not by itself validate a new runner.

## Matrix

| Language | Test runner | Level | Verified scope | Main gap or next gate |
| --- | --- | --- | --- | --- |
| Go | `go test` | Established | One Go module, multiple packages; full or affected-package tests; 10 operators | Broader project/runtime evidence and useful operator expansion |
| Rust | Cargo / standard Rust test harness | Validated | One package on Unix: binary, library, or library + one binary; unit/integration tests; 3 operators | Broader project/platform evidence; workspaces remain separate |
| JavaScript / TypeScript | Vitest | Experimental | One pinned ESM/pnpm package; JS/TS boolean flips; serial full-suite scored runs, saved reports, focused reruns and review/policies | Lifecycle attribution, parallel execution, real-project findings |
| JavaScript / TypeScript | Node.js test runner | Planned | None | Define runtime, module/TypeScript execution scope and fixture |
| JavaScript / TypeScript | Bun test runner | Planned | None | Define runtime, package scope and fixture |
| Python | pytest | Planned | None | Define package/import/environment scope and fixture |

JavaScript and TypeScript are desired for each listed runner. Acceptance must
record evidence for each syntax separately; JavaScript validation must not
silently promote TypeScript, JSX or TSX support. Version ranges for planned
targets have not been chosen or verified.

## Go — go test

**Level: Established within the single-module scope.**

- Project/test scope: a root `go.mod`, multiple packages within that module,
  Go 1.24+ on PATH. Full-module tests are the default; affected-package mode keeps
  a full baseline and selects transitive reverse dependencies for mutations.
- Mutation coverage: ten operators, including arithmetic/comparison changes,
  boolean flips, logical swaps, call removal, branch forcing and loop skipping.
  See [operator behavior](README.md#mutation-operators) for eligibility and limits.
- Workflow: file/operator filters, live/plain status, saved reports and browser,
  focused reruns, dismissals, scores and CI policies, retention and log budgets.
- Evidence: [godotenv](benchmarks/godotenv-v1.5.1-call-remove.md),
  [go-cmp](benchmarks/go-cmp-v0.7.0.md), and
  [fzf](benchmarks/fzf-v0.65.2-loops.md) record real-project findings and comparisons.
  Default regressions and opt-in `tests/go_workflow.rs` cover execution behavior.
- Limits: no general multi-module/workspace claim. Affected selection follows Go
  import dependencies; external test systems and unrepresented runtime dependencies
  need separate validation. Recorded macOS/Linux testing is not universal platform
  coverage. Large or asset-heavy projects still need runtime/noise evidence.
- Next support gains: expand evidence where concrete projects expose gaps; add
  operators based on useful findings rather than catalog size alone.

## Rust — Cargo

**Level: Validated within the ordinary single-package scope.**

- Project/test scope: one ordinary package with a binary, library, or library plus
  one binary; usable lockfile, registry
  dependencies, default features and standard unit/integration tests on Unix.
  Executes `cargo test --locked --tests --no-fail-fast`; no affected-test selection.
- Mutation coverage: `boolean_flip`, `comparison_boundary`, `logical_swap`.
  Discovery follows supported source modules and excludes test-only, generated,
  macro-token and other unsupported contexts described in the
  [scope contract](DESIGN.md#initial-rust-support-contract).
- Workflow: shared status/report/browser, focused reruns, dismissals, scores and
  policies are validated. Fresh artifacts are the default; dependency reuse is
  opt-in and worker-local, with local package targets rebuilt every time.
- Evidence: Mutie itself is the selected real project. The
  [milestone acceptance](benchmarks/rust-milestone-acceptance.md) links fixture,
  stronger-test rerun, review, cancellation and preservation evidence. The
  [feedback profile](benchmarks/rust-feedback-profile.md) records current runtime.
  [Derived-report write validation](benchmarks/derived-report-writes.md) records
  a measured reduction in prebuilt-suite time. The
  [saved-selection comparison](benchmarks/rust-report-write-reruns.md) preserves
  all six kills and exact failure sets while reducing one self-run from 564 to
  393 seconds; this is not a universal Rust performance estimate.
  Real-Cargo workflows are opt-in in `tests/rust_workflow.rs`.
- Limits: no proc macros or library crate types other than `lib`/`rlib`,
  multiple binaries, multi-package/enclosing
  workspaces, local/git dependencies, project build scripts, custom harnesses,
  feature selection or cross-compilation. Doctests and ignored tests are outside
  the execution scope. Validation is on macOS; Unix scope is not a claim of equal
  platform evidence. Full-suite feedback time remains a practical constraint.
- Library evidence: the [scope and implementation record](benchmarks/rust-library-scope.md)
  covers library-only and library-plus-one-binary fixture workflows, fresh/cached
  execution, stronger-test reruns, local rebuilds and source/artifact preservation.
  Custom library names/paths and `rlib` metadata have real Cargo listing checks.
  Both layouts also have dismissal/raw-score and stale-review checks, baseline
  compile/test failures, an injected mutation compiler failure, and serial
  baseline/mutation interruption with descendant cleanup. These fixture checks
  complement the [Marathon validation](benchmarks/rust-library-marathon.md): a
  15-mutation selection finds two useful test gaps; two stronger tests kill three
  saved survivors in matching fresh/cached reruns, with one equivalent survivor
  retained and reviewed. Full-suite baselines pass 182/184 tests. Marathon supplies
  library-plus-binary real-project evidence; library-only evidence is fixture-based.
- Next support gains: broader project/platform evidence before Established, and
  separately scoped workspaces/local dependencies. The ordinary-library milestone
  is complete; reassess the next major objective against the planned JS/TS and
  Python targets rather than automatically adding another Rust operator.

## JavaScript / TypeScript — Vitest

**Level: Experimental within the pinned ESM/pnpm fixture scope.**

The [first scope audit](benchmarks/vitest-initial-scope.md) selects one ESM package,
Node execution, `.js`/`.ts` source, pnpm's self-contained locked dependencies and
`boolean_flip`. Initial tool evidence is Node 24.22.0, pnpm 11.10.0 and Vitest 4.1.9
on macOS. Custom configs/plugins, browser environments, JSX/TSX, CommonJS,
monorepos and other package managers are deferred.

The mixed-syntax fixture passes four direct Vitest tests; separate JS/TS probes
show survivors killed by stronger tests. Pathe is selected for real TypeScript
validation: its direct full Vitest baseline passes 467 tests (8 skipped, 8 todo).
`mutie vitest-list` validates metadata and discovers boolean flips.
[`mutie vitest-check`](benchmarks/vitest-execution.md) now runs an isolated baseline;
its adapter has fixture mutation/failure/cleanup probes through the shared command
classifier, including rejection of missing completion after exit zero.
`mutie PATH --lang vitest` now produces scored fixture runs and saved reports:
four kills and two survivors across six JS/TS mutations, with a stronger JS test
killing the remaining survivor in a filtered fresh scan. See
[CLI acceptance](benchmarks/vitest-shared-runs.md). No real-project mutation run
has occurred, and JavaScript still needs its own real-project validation target. Direct probes also
show why JSON `success` and exit status alone are insufficient: unhandled errors
and all-skipped runs need explicit rejection.

Preview evidence: six fixture mutations and fourteen in an explicitly adapted
Pathe copy with its coverage-only Vite config removed. The initial audit missed
that config; unchanged Pathe is correctly rejected by the preview.

Current CLI scope is explicit `--lang vitest`, one worker and full-suite tests.
Focused saved-ID and missed-only reruns now support stronger-test validation for
both JS and TS, with fresh source matching and strict package/runner/config
identity checks. The browser exports explicit `--lang vitest` commands; see
[rerun acceptance](benchmarks/vitest-focused-reruns.md). Dismissal review and CI
policies now have separate JS/TS fixture evidence: accepted gaps remain raw
survivors, minimum scores still include them, and stale decisions need review.
See [review/policy acceptance](benchmarks/vitest-dismissals.md). Next gates are
broader lifecycle attribution, parallel execution and useful real-project findings.
Hook-related failure attribution remains deliberately conservative. The scope and execution
records define classification, preservation and real-project acceptance.

## JavaScript / TypeScript — Node.js test runner

**Level: Planned. No adapter, mutation discovery or end-to-end validation yet.**

Desired outcome: the shared workflow for projects using Node's test runner.
Choose a runtime version, real project and fixture; specify package/module formats,
test discovery, and whether TypeScript uses a supported native execution path or
an explicitly supported transformation setup. These choices require verification
against the selected runtime rather than assumptions based on Vitest support.

Initial validation must distinguish test failures from import/parse/loader errors,
handle nested test results without double counting, and verify worker/child-process
cleanup, isolated source resolution and stronger-test reruns. Custom loaders and
multi-package layouts remain undecided scope.

## JavaScript / TypeScript — Bun test runner

**Level: Planned. No adapter, mutation discovery or end-to-end validation yet.**

Desired outcome: the shared workflow for projects using Bun's test runner.
Choose a Bun version, real project and fixture; define package/lockfile handling,
JavaScript/TypeScript syntax and module scope, test discovery and result capture.
Do not infer runner compatibility from the Node integration or vice versa.

Initial validation must separate assertion failures from runtime/import/transform
errors and verify dependency/cache isolation, timeout/cancellation, saved review
and stronger-test reruns. Preloads, custom configuration and multi-package layouts
need explicit scope decisions.

## Python — pytest

**Level: Planned. No adapter, mutation discovery or end-to-end validation yet.**

Desired outcome: the shared workflow for pytest projects. Choose Python/pytest
versions, a real project and fixture; define package layouts, source/test discovery,
environment ownership and dependency installation. Resolve import paths and editable
installs so tests exercise the isolated mutated copy rather than the checkout.

Initial validation must distinguish assertion failures from collection/import and
configuration errors, handle parametrized tests and no-tests outcomes, and verify
bytecode-cache invalidation, cleanup and stronger-test reruns. Plugins, distributed
testing and multi-environment execution need explicit scope decisions.

## Updating this ledger

For each scope change or level promotion, update its row and section together:

1. Record the exact language syntax, runner/toolchain versions, platforms, project
   layouts, dependency handling and test scope that were exercised.
2. Link automated fixtures and real-project evidence, including outcome correctness,
   useful findings, runtime and source/artifact preservation. Keep known failures
   and limitations visible.
3. State remaining gaps and the next acceptance gate; synchronize usage in README,
   architectural decisions in DESIGN and scheduled work in TODO.

Keep runner evidence independent even where parsers/operators can be shared.
If a regression invalidates a support claim, narrow the documented scope or lower
the level until the evidence is restored. Planned integrations do not require a
plugin abstraction or imply support merely because their command can be launched.
