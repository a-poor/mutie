# Target support

This is the support ledger for Mutie's language and test-runner targets. It tracks
verified behavior, scope limits, evidence and the work needed to advance each
target. Last reviewed: 2026-10-20. [README.md](README.md) documents usage;
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
| TypeScript | Vitest | Validated | One pinned ESM/pnpm package; boolean flips; serial full-suite runs, reports, reruns and review/policies; Pathe findings and stronger tests | Broader project/config/runtime evidence; parallel workers |
| JavaScript | Vitest | Validated | One pinned ESM/pnpm package; serial boolean flips; Hyparquet full-inventory attempts, stronger-test reruns and review/policies | Broader project/config/runtime evidence; unresolved policy/redundant cases and timeout |
| JavaScript | Node.js test runner | Validated | One dependency-free ESM package on Node 24.22.0; serial boolean flips; reports, reruns and review/policies | Broader package/runtime scope; remaining scheduling/delivery findings |
| TypeScript | Node.js test runner | Validated | Native erasable `.ts` on Node 24.22.0; serial full-suite runs, reports, reruns and review/policies; unchanged Bragkit findings and stronger tests | Broader layouts/dependencies/runtime evidence |
| JavaScript | Bun test runner | Validated | Pinned 1.3.11, dependency-free ESM; serial boolean flips, reports/reruns/review; unchanged pp-is real-project findings | Broader project/runtime evidence; remaining pp-is findings |
| TypeScript | Bun test runner | Validated | Pinned serial dependency-free scope with audited TS/NodeNext config; Composer findings, reports, reruns, review/policies and cleanup | Broader project/runtime evidence |
| Python | pytest | Validated | CPython 3.14.7 / pytest 9.0.2; one dependency-free flat or src package, root/tests-directory suites, serial boolean flips, single-comparison boundaries and scoped binary arithmetic, reports/reruns/review; unchanged Scrubify, Cycler, Pluralizer, Route Mini and betting-math-kit evidence | Broader layouts, dependencies and runtime evidence; remaining doseq survivor |

JavaScript and TypeScript are desired for each listed runner. Acceptance must
record evidence for each syntax separately; JavaScript validation must not
silently promote TypeScript, JSX or TSX support. Bun pins 1.3.11+af24e281e;
Python pins CPython 3.14.7 / pytest 9.0.2 within its documented scope.

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

**Levels: TypeScript and JavaScript Validated within the pinned ESM/pnpm scope.**

The [first scope audit](benchmarks/vitest-initial-scope.md) selects one ESM package,
Node execution, `.js`/`.ts` source, pnpm's self-contained locked dependencies and
`boolean_flip`. Initial tool evidence is Node 24.22.0, pnpm 11.10.0 and Vitest 4.1.9
on macOS. Config support is limited to static coverage-reporter-only declarations;
other custom configs/plugins, browser environments, JSX/TSX, CommonJS,
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
[CLI acceptance](benchmarks/vitest-shared-runs.md). The first complete Pathe run
with its original config produced 12 kills and two survivors in 94.00 seconds,
with no execution errors. Both survivors expose observable boundary behaviors:
an empty-cwd path fallback and repeated alias expansion. Three stronger tests
kill both in an 18.79-second focused rerun; see
[TypeScript milestone acceptance](benchmarks/vitest-pathe-survivors.md). JavaScript
now has a real-project target: [Hyparquet](benchmarks/vitest-javascript-hyparquet.md)
provides 133 discovered flips and a passing 724-test isolated baseline. Its
NodeNext config is accepted only for JavaScript-only type checking. Nineteen
sampled mutations produced 17 kills and two survivors. Stronger tests kill both
in a 55.60-second saved-selection rerun; the byte-length gap affects binary filter
equality, while default strictness is a lower-priority helper contract. See
[JavaScript acceptance and runtime limits](benchmarks/vitest-hyparquet-findings.md).
The [full inventory review](benchmarks/vitest-hyparquet-full.md) subsequently
attempts all 133 candidates across preserved runs: 89 kills, 41 survivors, two
unhandled callback errors and one timeout. Original failed runs retain unavailable
scores; the aggregate is not a full-project score. Test-only improvements kill
36 survivors and turn both callback errors into ordinary kills in a 43-candidate
rerun; five remain (four redundant scan settings and an unknown-operator policy
case). No classification rule was relaxed. Direct probes also
show why JSON `success` and exit status alone are insufficient: unhandled errors
and all-skipped runs need explicit rejection.

The initial preview used a Pathe copy with its coverage-only config removed.
The [coverage-only config gate](benchmarks/vitest-coverage-config.md) now accepts
the original config without executing it: discovery finds fourteen mutations and
the isolated baseline passes 467 tests with 16 skipped/todo. Package dependencies
remain the prepared pinned installation; this is not arbitrary upstream config support.

Current CLI scope is explicit `--lang vitest`, one worker and full-suite tests.
Focused saved-ID and missed-only reruns now support stronger-test validation for
both JS and TS, with fresh source matching and strict package/runner/config
identity checks. The browser exports explicit `--lang vitest` commands; see
[rerun acceptance](benchmarks/vitest-focused-reruns.md). Dismissal review and CI
policies now have separate JS/TS fixture evidence: accepted gaps remain raw
survivors, minimum scores still include them, and stale decisions need review.
See [review/policy acceptance](benchmarks/vitest-dismissals.md). Next gates are
broader lifecycle/config scope and parallel execution. The [feedback profile](benchmarks/vitest-feedback-profile.md)
records stable roughly 26-second survivor reruns, a slower launcher PATH pair,
and test-only copy/validation/cleanup timings. The subsequent
[Node reuse change](benchmarks/vitest-node-reuse.md) resolves the launcher once per
run while retaining per-command executable and project identity checks.
The [lifecycle evidence](benchmarks/vitest-lifecycle-attribution.md) now distinguishes
body failures from lifecycle failures in the pinned worker. Successful test-scoped
fixtures, traditional hooks, aroundEach and completion callbacks can accompany
body kills. Lifecycle and mixed failures remain errors. Pending async assertions
from setup and post-body assertion-count failures remain conservatively rejected;
`test.fails` is unsupported. Instrumentation only modifies the disposable dependency
copy and requires the exact audited runner bytes. File/worker fixture setup, body
and teardown, nested hooks, and concurrent body/cleanup mixtures now have fixture
evidence. Cross-file fixture reuse, cancellation during scoped teardown and
parallel workers remain unvalidated. JavaScript acceptance is independent of the
TypeScript milestone; neither promotes JSX or TSX support.

## JavaScript / TypeScript — Node.js test runner

**JavaScript and native TypeScript: independently Validated within the pinned scope.**

Explicit `--lang node` supports one dependency-free ESM package with `.js` files
under `src/` and `test/`, exact `node --test` or `node --test test/*.test.js` script, no installed
dependencies, and audited Node 24.22.0. Static literal relative and `node:` imports only;
no runtime package dependencies, CommonJS, workspaces, loaders or
parallel workers. Test files run serially in isolated processes and fresh copies.

Boolean discovery shares the concrete JS/TS parser with Vitest. The CLI supports
saved reports, focused source-identity reruns, browser exports, dismissal CRUD/check
and shared raw-score/findings policies. Completion requires per-file/global
agreement and hook evidence from the pinned internal implementation. Only
body failures matched to exact worker-observed exceptions are kills. Hook failures,
unhandled process errors, incomplete/empty/early-exit cases and ambiguous attribution
remain errors. Dynamic imports are now rejected; rejected non-native thenables
remain unclassified. See [the attribution audit](benchmarks/node-body-attribution.md).

[Acceptance evidence](benchmarks/node-initial-scope.md) records the fixture,
mutation-triggered classification cases, real timeout/cancellation/capture
cleanup, and `sri-dynamic-loader` findings with stronger-test reruns. Its original
production source and original tests are preserved. The body-attribution audit
completes the unchanged 21-candidate inventory with 12 kills, eight survivors,
one timeout and zero errors; all four formerly unclassified candidates are kills.
The initial generic-error boundary is superseded by that audit.
[SRI follow-up](benchmarks/node-sri-final-review.md) explains the pending-promise
timeout; five further cleanup tests leave the survivor unresolved, not proven
equivalent. [Second-project acceptance](benchmarks/node-openclaw.md) completes
`openclaw-cc` unchanged: 39 candidates, 22 kills, 15 survivors, two timeouts and no
errors. Three added tests kill three selected survivors in a full-suite rerun. A separate
test-only teardown patch makes both original timeouts complete as kills; original
reports and scores are preserved.
[Storage/profile follow-up](benchmarks/node-storage-profiles.md) adds a passing
67-test baseline and five further survivor kills, including independent-process
lock contention and profile inclusion/exclusion checks. Its milestone audit meets
Validated within the existing pinned JavaScript scope. Seven `openclaw-cc`
survivors remain open; the SRI cleanup survivor remains unresolved. Neither
unresolved findings nor separate rerun scores are silently removed from the
original accounting. Broader dependencies/layouts, runtime versions and parallelism
need separate acceptance; this is not Established.

The [initial native TypeScript slice](benchmarks/node-typescript-scope.md) extends the same
runner to erasable `.ts`, explicit local imports and an exact `test/*.test.ts`
script (or `.js` test entries importing TS). It performs no type checking or
configuration transforms. Optional audited plain-JSON NodeNext tsconfig bytes and
absence join the saved execution identity. Declaration files, enums, namespaces,
parameter properties, angle assertions, decorators, TSX/MTS/CTS and loaders are
outside this scope. Existing JavaScript recipe identity is unchanged.

Independent evidence includes a typed automated fixture, 42 TS attribution cases,
timeout/cancellation/capture cleanup, review/policies and config-sensitive reruns.
Unchanged dependency-atlas passes its nine-test isolated baseline but contains no
runtime boolean candidates. Bragkit has 23 candidates: two killed and 21 survived;
two retained tests kill three HTML-list survivors in a separate focused rerun.
All original production/test files and reports are preserved. Bragkit required
changing only `scripts.test` from `node --test` to the explicit TS glob; direct
runs confirmed the same 61 tests. [Native default discovery](benchmarks/node-default-discovery.md)
now closes that gate: unchanged Bragkit completes all 23 candidates with matching
outcomes, and a separate 63-test review copy kills the three saved HTML-list findings.
Whole-tree discovery rejects any selected test outside audited `test/` sources,
validates the native file set and implementation, and rejects snapshot exclusions
that would hide tests. Default-discovery JS/TS fixtures cover nested helpers,
new-test reruns, empty files and preservation; native-default cleanup also passes.
This independently meets the scoped TypeScript Validated gate. The
[remaining survivor review](benchmarks/node-bragkit-survivors.md) kills all 18 other
findings in 53.39 seconds with a 76-test baseline and preserved sources/reports.
Broader layouts and dependencies are not implied.

## JavaScript / TypeScript — Bun test runner

**Levels: JavaScript and TypeScript independently Validated within the pinned scope.**

The [selected initial audit](benchmarks/bun-initial-scope.md) pins Bun 1.3.11,
retains 24 JUnit/inspector cases per syntax, and establishes an unchanged 272-test
Composer baseline plus a useful manual stronger-test finding. Native failure
signals conflate test bodies, hooks and unhandled errors. Positive provenance is
required before scored execution. The [bounded observer follow-up](benchmarks/bun-body-attribution.md)
passes 112 paired JS/TS cases, 30 evidence-rejection checks and matching Composer
baseline/finding controls. The [isolated diagnostic](benchmarks/bun-isolated-execution.md)
now connects the Rust reader to fresh copies and shared process cleanup. JS/TS
fixtures pass independently. [Unchanged Composer acceptance](benchmarks/bun-composer-execution.md)
adds a narrow plain-JSON tsconfig and non-executable ancillary-file scope: 272-test
baseline, surviving manual candidate, and one stronger-test body failure. The later scored workflow and syntax-specific milestone status are recorded below.
[Discovery and diagnostic identity](benchmarks/bun-discovery-identity.md) now cover
12 Composer runtime flips and a passing baseline with matching input fingerprints.

Explicit `--lang bun` connects serial full-suite boolean flips to saved reports,
scores and focused reruns. Runtime bytes, package/config and observer recipe must
match saved identity; stronger tests and changed budgets are allowed. Captures have
a mandatory 4 MiB default budget. Source and independent test inventories are
validated before exclusions and after copying. Hook/process/ambiguous failures
remain errors and invalidate scores. See [shared workflow evidence](benchmarks/bun-shared-runs.md)
for independent JS/TS fixtures, Composer's 12 candidates and stronger-test finding.

Independent JS/TS [scored cleanup regressions](benchmarks/bun-scored-cleanup.md)
now cover output/file limits, timeout, SIGINT, descendant shutdown and score
boundaries. Independent JS/TS [review/policy acceptance](benchmarks/bun-dismissals.md) now
covers CRUD, staleness, raw scores, focused reruns and error precedence. Together
with Composer, this completes scoped TypeScript validation. The
[Composer findings review](benchmarks/bun-composer-survivors.md) subsequently kills
the four remaining survivors and converts the queue timeout to an ordinary
body-failure kill with stronger tests. Historical scores remain unchanged.
Independent
[JavaScript acceptance](benchmarks/bun-javascript-pp-is.md) uses pp-is with all
runtime source/tests unchanged in an explicitly adapted ESM root: 22 candidates,
12 kills/10 survivors, then seven stronger-test kills. That first run used a temporary adaptation. [Unchanged-layout acceptance](benchmarks/bun-unchanged-layout.md)
now accepts the original checkout with matching inventory/outcomes. An absent
`type` requires static JS import/export syntax; ancillary packaging and root/dist
declarations stay fingerprinted and non-importable. Explicit CommonJS, implicit
runtime TypeScript and hidden packaging tests remain rejected. Parallel workers,
dependencies, preloads, custom Bun configuration and multi-package layouts remain
unsupported. Do not infer compatibility from Node or Vitest support.

## Python — pytest

**Level: Validated within the pinned serial scope; verified on macOS arm64.**

The [initial audit](benchmarks/python-initial-scope.md) selects CPython 3.14.7,
pytest 9.0.2 and an adapted Inflection runtime suite. It records 39 native probes,
stale-bytecode controls and a manual Unicode-boundary stronger-test finding.
Two negative controls exposed correctness blockers: unittest setup assertions
appear in the call phase, and an atexit exception can follow pytest success with
exit zero. A fresh cache prefix avoids the demonstrated stale-bytecode false
result; `-B` alone does not. The [lifetime follow-up](benchmarks/python-process-lifetime.md)
rejects those controls, validates process completion and explicitly excludes
thread-starting suites because pytest can suppress late thread failures.
Uncaptured stderr is conservatively an error.
[`python-list` and `python-check`](benchmarks/python-baseline-integration.md) now
provide static discovery and an isolated baseline for one dependency-free flat
or `src/` package, root or `tests/` `test_*.py` files, and an explicit pinned virtual environment. They do
not produce scored reports themselves; explicit `--lang python` connects
[scored runs, saved reports, reruns and review](benchmarks/python-shared-runs.md).
[Production process controls](benchmarks/python-process-integration.md)
cover attribution, cancellation, ordinary process-group descendants and resource
limits. Scored timeout, cancellation and capture/file-limit probes also pass;
the [Scrubify milestone](benchmarks/python-scrubify-findings.md) completes the
real-project loop: six original tests, three candidates (one kill/two survivors),
then nine tests and a focused rerun killing the blank-query survivor. Original
inputs and all 28 original report files remain unchanged. Its 28.38-second full
run and 23.49-second rerun include runtime fingerprinting overhead. The remaining
`doseq` survivor is documented rather than hidden or counted as a kill.
The [feedback follow-up](benchmarks/python-feedback-profile.md) records repeated
comparisons and a faster runtime tree traversal with unchanged content hashes
and validation frequency. It does not expand the supported scope.

Scope: one flat or `src/` package with audited `pyproject.toml`, root or `tests/` `test_*.py`,
boolean flips and single-comparison boundaries, full tests and one worker. The dedicated venv must match the pinned
dependency set. The validator admits a small standard-library/API set, rejects
configuration that could change collection, and verifies original/snapshot
inputs. Initial Scrubify acceptance adapted its layout/metadata; the
[native src follow-up](benchmarks/python-src-layout.md) now verifies the original
checkout and manifest unchanged: six tests, one kill/two survivors, followed by
a nine-test baseline and focused kill in a separate stronger-test copy. Static
setuptools src discovery, a compatible pytest-only test extra, matching pythonpath
and quiet output are admitted declaratively; project builds are never executed. The 107-test case-converter adaptation supplies additional
execution/error evidence, not unchanged-layout support.

[Unchanged Cycler validation](benchmarks/python-cycler-unchanged.md) preserves its
layout, manifest, source and 28-test suite. The original full run stops at a mixed
assertion/KeyError error; the two pending candidates are killed in a selected
rerun. One additional public key-set assertion in a separate copy turns the
error into an assertion kill. Original failed-report scores remain unavailable.
Root tests and directory tests share the complete audited inventory; a configured
`testpaths=["tests"]` cannot silently hide a root test. Narrow flat setuptools
metadata permits a literal `package.__version__` reference and `py.typed` data.
Audited optional documentation/test-tool names remain metadata only: no optional
dependencies are installed and no plugins load. `doc/source/conf.py` and direct
`.circleci/*.py` scripts may be retained but cannot contain tests/hooks or be
imported by application/tests. Other ancillary Python files remain rejected.

[Scoped unittest validation](benchmarks/python-unittest-pluralizer.md) admits direct
synchronous `unittest.TestCase` subclasses via pytest. Setup, body, teardown and
registered instance cleanups need complete dispatch evidence; a call failure must
match the exact observed body exception. Class/module fixture failures and mixed
body/cleanup failures remain errors. Custom dispatch/inheritance/metaclasses,
subtests, skips, expected failures, mock/runner APIs, module cleanup registration
and async cases remain unsupported. A conventional guarded `unittest.main()` is
retained but never executed. Narrow static Hatch package metadata, Ruff settings,
and audited development/resolver dependencies are inert and fingerprinted; no
build, tool, dependency installer or resolver runs.

Scoped root and `tests/conftest.py` fixture declarations are Validated within
the audited subset, including unchanged Binary Tree Library acceptance.
Function/class/module/session scopes, autouse fixtures, literal parametrization,
yield cleanup and registered finalizers have native lifecycle controls. Fixture
configuration changes require a fresh run. Root `pytest.ini` admits only full-scope
testpaths; a test-package initializer must contain only comments/docstrings. See the
[fixture audit](benchmarks/python-conftest-fixtures.md).
[Unchanged Route Mini validation](benchmarks/python-route-mini.md) adds 68-test
fixture-based acceptance, audited conftest helpers/imports and inert example/tool
metadata. All 38 mutations are accounted for across four runs; three selected
survivors are killed by stronger tests. Original mixed errors/timeouts remain unscored.
The subsequent [finding review](benchmarks/python-route-mini-review.md) adds explicit
retry/fallback and immutable-record expectations in a separate copy: 17 kills
and 12 remaining slots survivors across two focused runs, with 85 passing baseline
tests and no new errors/timeouts. Native slots
inspection remains outside the scored scope; it does not broaden reflective APIs.

[Comparison-boundary validation](benchmarks/python-comparison-boundaries.md) covers
native token replacement, operand evaluation, conservative custom-dispatch errors
and an unchanged Route Mini capacity survivor killed by a separate stronger test.
Chained comparisons and annotation subtrees remain excluded.

Only completed assertion/pytest.fail body failures count as kills. Other runtime
exceptions, hooks, warnings, late cleanup and ambiguous failures remain errors.
Application dependencies, general conftest hooks/user plugins, thread-starting and
async suites, editable installs, namespace/multiple packages, parallel workers
and affected selection remain unsupported. Unix file limits are not aggregate
disk quotas. Broader project and platform claims require separate evidence.

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

### Additional unchanged Python numerical-library evidence

[betting-math-kit](benchmarks/python-betting-math.md) passes all 198 original tests.
Ten selected metrics comparisons account for six survivors and four runtime
errors across failed reports; a separate 205-test stronger suite kills all six
survivors. This is selected evidence, not a full project score. Audited numeric
minimum versions for inert optional tools, bounded Ruff/Mypy settings and direct
independent `random.Random(...)` construction are admitted. Global RNG mutation,
constructor escape, plugins, dependencies and arbitrary tool settings remain
excluded. Manifest and validator bytes retain execution identity.

The [complete betting-math-kit review](benchmarks/python-betting-math-full.md)
accounts for all 87 sites (19 kills, 59 survivors, nine errors). A separate
222-test copy kills 24 survivors and two endpoint-error candidates using explicit
expected-success assertions. Original inputs, reports and classification rules
remain intact. Domain-limited redundancy observations and unresolved numerical
boundaries are retained individually; combined accounting is not a full score.

`benchmarks/python-arithmetic.md` validates Python binary `+ ↔ -` and `* ↔ /`
under `arithmetic_replace`. Preserve native grouping/evaluation and exact token
pairs; annotations, unary/augmented operators, floor division, modulo and powers
remain excluded. This is not numeric type inference: unsupported operands and
zero division stay errors. Unchanged betting-math-kit's 30-site Kelly selection
has 21 kills, eight survivors and one string-operand error; a separate 201-test
copy kills all eight survivors. Preserve prior inputs/reports and recipe drift
checks; selected evidence is not a full 194-arithmetic-site score.

The [complete arithmetic review](benchmarks/python-arithmetic-full.md) accounts
for all 194 sites on unchanged betting-math-kit: 61 kills, 97 survivors and
36 native errors. A separate 219-test copy kills 54 more survivors; with eight
prior kills, 35 remain open. Numerical questions are recorded per candidate. Preserve original report scores and the
source-identity mapping; combined accounting is not a full-run score.

`--continue-on-error MAX_ERRORS` is invocation-only and Python-only. It continues
only positively typed ordinary pytest body errors after complete lifecycle,
inventory, process cleanup and input/runtime checks. Reaching the positive limit
stops execution. Type/value/arithmetic/lookup subclasses qualify; resource/system,
unittest and lifecycle errors remain fatal. Runs containing errors stay failed,
exit 1 and unscored. Reruns must opt in again, and changed observer/protocol recipes
require fresh execution identities. See `benchmarks/python-candidate-continuation.md`;
never infer eligibility from error names, exit status or generic adapter errors.
The unchanged 194-candidate arithmetic batch matches all prior outcomes
(61 kills, 97 survivors, 36 errors) in one failed, unscored report, preserving
original inputs and historical reports. Native exception, lifecycle/resource,
limit and cancellation controls establish this narrow continuation boundary.
