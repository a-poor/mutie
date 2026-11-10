# Node packages with dependencies and composed tests

[`eslint-node-test`](https://github.com/sindresorhus/eslint-node-test/tree/dcb850e2bd01534792fd67bcec4747828fa96df9)
now completes a bounded mutation/review/rerun workflow with its original source,
tests and configuration. Its installation needs one documented addition:
`@types/node`, which upstream's typed tests request but its manifest does not
declare. This is **not unchanged-installation acceptance**.

The [results](fixtures/node-eslint/results.json) retain mutation identities,
outcomes, timings and preservation hashes. The [reproduction guide](fixtures/node-eslint/README.md)
records installation and run commands. The two retained stronger-test files are
new Mutie test code; the upstream codebase and dependencies are fetched separately.

## Native baseline and installation

The audited revision is `dcb850e2bd01534792fd67bcec4747828fa96df9`, with 491 tracked
inputs. Node is 24.22.0 and npm is 11.19.1. Upstream disables lockfile creation;
`npm install --legacy-peer-deps --ignore-scripts --no-audit --no-fund` resolved
501 packages. [Recorded installer metadata](fixtures/node-eslint/installation.json)
contains versions, registry URLs and integrities. It is not an upstream lockfile
or a promise that a later floating installation resolves identically.

The original native `npm test` passes all five lint tasks and 5,987 of 5,989 tests.
Two `no-test-return-statement` cases fail: `return t.name` and `return t`.
Their fixture tsconfig requests `types: ["node"]`; a direct compiler check reports
TS2688. A targeted native rerun reproduces both failures.

A separately installed `@types/node` 24.19.2 and its `undici-types` 7.24.6 dependency
are copied into a separate complete installation, with explicit user approval.
All 491 tracked files and all 15,251 existing installed files remain unchanged;
only 122 declaration-package files are added. The full native command then passes
**5,989 tests in 96 suites**, with no skips, todos or failures, in 22.77 seconds.
No source, manifest, config or test is edited to obtain that baseline.

## Implemented scope

Mutie recognizes the exact composed recipe documented in [runner configuration](../docs/runners.md).
It runs dogfooding ESLint, documentation checks, ordinary ESLint, Markdown lint,
package JSON lint and the original `node --test test/*.js test/unit/*.js` inventory.
The five tools run directly and serially with pinned Node and owned executables;
Mutie does not evaluate arbitrary shell scripts. Ordinary prerequisite failures
retain the complete task inventory but remain errors even if runtime tests also
fail. Commands share one deadline and output budget. Timeout, cancellation,
capture overflow or incomplete process-group shutdown stops execution.

Pinned tools are ESLint 10.14.0, eslint-doc-generator 3.7.1, markdownlint-cli 0.49.1,
npm-package-json-lint 10.5.1 and npm-run-all2 9.0.4. Configuration, all tooling
scripts, typed-test configuration, manager metadata and installed bytes bind
execution and focused-rerun identity. This is an exact recognized recipe, not
general npm-script compatibility.

The additional JavaScript layout has root `index.js`, sources under `rules/`,
the two explicit test globs, and statically reached test/tooling helpers. Declared
dependencies in the owned installation and relative `package.json` JSON imports
are supported. A narrow module-level `createRequire(import.meta.url)` binding may
load literal declared packages during initialization; it cannot escape into test
bodies or load application modules. Project dynamic imports, workspaces and other
layouts remain outside this extension. Legacy `src/` JavaScript and native
TypeScript recipes retain their existing identity and restrictions.

## Failure attribution

Imported test helpers expose two boundaries that needed real evidence:

- Native failures identify the helper's declaration file, which may differ from
  its worker entry file. The composed reporter binds failures to the pinned serial
  runner's explicit per-file completion, then requires that exact worker and body
  test ID. The declaration must remain in the owned test/helper graph. Native
  controls reuse the same helper, test name and ID across two different workers;
  hook and plan mixtures still fail closed.
- Dependencies can load modules lazily inside a body. A negative control first
  reproduced a missing CommonJS dependency incorrectly becoming a kill. A new
  per-worker loader ledger observes pinned `module.require` and `module.import`
  tracing channels. Loading/evaluation errors, caught errors, unfinished imports,
  missing/duplicate evidence and overflow now remain errors. Late exit-time
  loading failures are retained too. Ordinary dependency body exceptions and
  successful loads followed by assertion failures still count as kills.

The runner, CommonJS/ESM loaders and tracing implementation require exact hashes.
Reporter/observer/validator bytes join the composed recipe identity. Existing
hook and process-lifetime checks remain required; a test-body checkpoint alone
is insufficient. No loader function is replaced. This is failure attribution for
owned projects, not an OS sandbox for hostile test code.

## Run, inspect, improve, rerun

Discovery finds 441 boolean flips. Before inspecting outcomes, the bounded sample
selects 13 sites in suspension detection, reference equality and promise-type
helpers. Every attempted mutation retains all original tests and lint tasks;
filters change mutation sources only. Runs use one worker, a 180-second baseline
deadline and 120 seconds per mutation.

| Original selection | Attempted | Killed | Survived | Errors | Pending | Wall time |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Suspension helper | 2 of 6 | 1 | 0 | 1 | 4 | 228.04 s |
| Reference helper | 1 of 4 | 0 | 0 | 1 | 3 | 152.60 s |
| Promise-type helper | 3 of 3 | 0 | 3 | 0 | 0 | 294.68 s |
| Focused original reference ID 2 | 1 | 0 | 1 | 0 | 0 | 152.75 s |

The two errors are actual dogfooding lint prerequisite failures. Their reports
remain failed, with unavailable scores and unattempted work pending. The promise
and focused reference selections each retain their original **0%** score. Across
reports, seven distinct sample sites were attempted and six remain unattempted;
this accounting is not a full project score or a replacement for saved scores.

The public finding is `rules/utils/is-same-reference.js:122`: changing `true` to
`false` suppresses the `no-identical-assertion-arguments` diagnostic for
`assert.strictEqual(this, this)` inside a class method. A new public `Linter` test
requires the exact diagnostic. It passes on the original and fails on the manual
mutation control.

Three new tests use real TypeScript compiler types to check the internal
`isPromiseType` contract for all-promise, all-value and mixed unions. They cover
the three surviving sites at lines 25, 28 and 29. The current public rule
decomposes unions before calling this helper, so these are internal contract
findings, not three demonstrated public-rule failures or equivalence claims.

The separate stronger copy adds only those four tests in two new files. Its full
native command passes **5,993 tests** in 25.65 seconds. Focused Mutie outcomes and
timings are:

| Stronger-test selection | Killed | Survived | Errors | Wall time |
| --- | ---: | ---: | ---: | ---: |
| Three promise-type survivors | 3 | 0 | 0 | 316.99 s |
| Public identical-`this` survivor | 1 | 0 | 0 | 156.46 s |

Both focused selections score 100%; neither is a full stronger-suite mutation
score. Historical reports retain their scores. Timings overlap other local checks
and are not a controlled performance comparison. Instrumented full-suite commands
take roughly a minute; this does
not establish fast feedback or broad project compatibility.

## Validation and preservation

The ordinary Cargo suite passes **386 tests** (241 opt-in tests ignored), with
formatting and Clippy clean. Four opt-in native composed-recipe groups also pass:
outcomes/identity, shared budgets, helper ownership and deferred loader failures.
These groups use deterministic prerequisite programs
with real Node; actual ESLint and other tool behavior is established by the external
project runs. Legacy Node JS/TS workflows and cleanup controls pass independently.

The original Git checkout remains clean. All 491 tracked files in each copy,
the 15,251-file native and 15,373-file supplemented installations, and all **224
original report files** are hash-checked and unchanged. This also verifies the
earliest independently recorded 67 report hashes. Provisional reports from before helper/loader
fixes are retained separately and are not acceptance scores or rerun sources.
No package, report-schema or operator-catalog version changes are needed.

All ten jobs for the previously pushed UFO/Vitest change passed in
[Linux CI](https://github.com/a-poor/mutie/actions/runs/42740310729).
The new Node native controls are configured in the existing Node job but remain
uncommitted and have not run remotely. Broader scripts, dependency layouts,
runtime upgrades and the deferred macOS startup issue remain separate work.
