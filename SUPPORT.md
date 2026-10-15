# Target support

This is the support ledger for Mutie's language and test-runner targets. It tracks
verified behavior, scope limits, evidence and the work needed to advance each
target. Last reviewed: 2026-10-14. [README.md](README.md) documents usage;
[TODO.md](TODO.md) sets implementation priorities. A planned target is not available
through the CLI, and this table does not set implementation order.

## Support levels

Levels are cumulative and apply only to the explicitly documented scope. They
measure implementation and validation maturity, not a percentage of language
coverage, a stability guarantee, or equivalence with another target.

| Level | Required evidence |
| --- | --- |
| Planned | Target selected for the roadmap; no working integration claimed. Initial scope and validation project may still be undecided. |
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
| Rust | Cargo / standard Rust test harness | Validated | One ordinary binary package on Unix; full supported suite; 3 operators | Separately scoped library/workspace expansion and broader project evidence |
| JavaScript / TypeScript | Vitest | Planned | None | Define first package, syntax/module scope and fixture |
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

**Level: Validated within the initial binary-package scope.**

- Project/test scope: one ordinary binary package, usable lockfile, registry
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
- Limits: no libraries/proc macros, multiple binaries, multi-package/enclosing
  workspaces, local/git dependencies, project build scripts, custom harnesses,
  feature selection or cross-compilation. Doctests and ignored tests are outside
  the execution scope. Validation is on macOS; Unix scope is not a claim of equal
  platform evidence. Full-suite feedback time remains a practical constraint.
- Next support gains: define the next library-package slice against a real project,
  with workspace expansion scoped separately. Further runtime work should follow
  measured costs while preserving full tests. Advancement to
  Established needs additional representative real projects, not just more operators.

## JavaScript / TypeScript — Vitest

**Level: Planned. No adapter, mutation discovery or end-to-end validation yet.**

Desired outcome: the shared mutation/review/rerun workflow for projects using
Vitest. Before implementation, select a real project and fixture; define supported
package layout, package manager/lockfile, JavaScript and TypeScript syntax, module
formats, transformations and runner configuration. Decide how one-shot execution,
test discovery and structured results distinguish assertion failures from
transform/configuration errors, missing tests and interrupted runs.

Initial validation must exercise isolated dependencies and caches, prevent watch
mode from keeping runs alive, and demonstrate a missed mutation killed by stronger
tests. Monorepos, browser execution, JSX/TSX and custom transformations are scope
decisions still to make, not promised initial features.

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
