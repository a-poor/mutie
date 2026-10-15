# First Vitest scope

Recorded 2026-10-15, after Mutie `f51193b`. **Planned execution; read-only discovery preview implemented.**
This selects the next milestone and records direct runner probes. A subsequent
read-only discovery preview is implemented below; test execution is not. Neither
the probes nor the fixture establish Experimental support. Data: [scope results](vitest-initial-scope.json).

## Decision

Start with one ESM package, Node execution, plain `.js` and `.ts` source, and
`boolean_flip`. Use a self-contained, preinstalled pnpm dependency tree and locked
registry dependencies. Target the measured Node 24.22.0, pnpm 11.10.0 and Vitest
4.1.9 combination first; do not infer compatibility with other versions (including
Vitest 5). Vite is 8.1.3 in Pathe and 8.3.4 in the fixture lockfile;
record and validate each resolved combination. Broaden versions only with acceptance evidence. Initial platform is
macOS/Unix, following the existing process-group cleanup boundary.

Use [the mixed JS/TS fixture](../tests/fixtures/vitest/README.md) for separate
syntax evidence and [Pathe](https://github.com/unjs/pathe/tree/bc7477a01f0bd60ada017add8142c9f9d69ccdc5)
for the first real TypeScript project. JavaScript remains fixture-only until a
real JS project passes the workflow; a TypeScript result cannot promote both.

## Candidate audit

Local candidates were inspected without changing them. `imgload` has one small
`squared` test and does not offer useful boolean-flip evidence. `cms` includes
React/Cloudflare/Vite integration and one test file; Lexical is a monorepo.
Avoid expanding the first contract merely to accommodate those applications.

Pathe revision `bc7477a01f0bd60ada017add8142c9f9d69ccdc5` is a single ESM package,
with five TypeScript source files and four test files importing source directly.
It has no runtime dependencies. Correction discovered during preview implementation:
`vite.config.ts` supplies a coverage-reporter list; the original audit checked
only Vitest-named configs and incorrectly described the project as config-free. Its pnpm workspace
settings file only disables esbuild install scripts; the lockfile has one root
importer. A settings file alone must not be mistaken for a multi-package project.
The package's `test` script also runs lint and coverage; the selected scope is
**the full Vitest suite**, not that composite npm script, lint or type checking.

A temporary clone installed its unchanged lockfile with pnpm 11.10.0,
`--frozen-lockfile --ignore-scripts`, and a temporary store. Direct
`node node_modules/vitest/vitest.mjs run --reporter=json --outputFile=<owned-file>`
passed **467 tests**, with **8 skipped and 8 todo**, across four files. Vite resolved
to 8.1.3. Tracked files remained unchanged (`git status --short` empty).
This baseline selects a candidate; useful mutation findings and stronger-test
reruns on Pathe remain required. No real-project JavaScript validation is claimed.

The host's unpinned pnpm 12 rejected Pathe's settings-only workspace file. An
initial escalated npm invocation also selected an older Node from PATH. Both
setup issues were resolved using explicit Node 24 and pnpm 11.10.0 paths; the
adapter must resolve and record actual tools, not assume its caller's shell tools.

## Intended contract

- One package rooted at `package.json`, `type: module`, one root lockfile importer,
  no enclosing/multi-package workspace, local/git/link dependencies or escaping
  source/dependency symlinks. pnpm's internal links are permitted only when every
  target stays within the copied dependency tree. Other package managers are deferred.
- User prepares dependencies with the pinned manager and frozen lockfile. Mutie
  does not implicitly install, download, run lifecycle scripts or invoke `npx`.
  Check manifest/lockfile/installed runner consistency before baseline; reject
  unsupported versions and incomplete installations with actionable diagnostics.
- Initial config scope: Vitest defaults, Node environment and no project Vite or
  Vitest config. Reject custom configs rather than silently bypass them. A
  Mutie-owned config may enforce private cache paths, full one-shot execution,
  no coverage, no snapshot updates, no retries and bounded runner concurrency.
  Browser/jsdom, setup files, plugins, aliases, projects, watch mode, sharding,
  related/changed tests and custom loaders are deferred.
- Default production discovery: `src/**/*.js` and `src/**/*.ts`, excluding test
  files, declaration files, generated output and dependencies. Report exclusions;
  reject unsupported production syntax in the selected scope rather than claim
  complete discovery. No JSX/TSX, CommonJS, `.mts`/`.cts`, decorators or in-source
  tests initially. Tests import isolated source, not prebuilt `dist` exports.
- Parse JS and TS with their respective grammars. Flip only boolean value
  expressions; exclude TypeScript literal types, declaration/type contexts,
  comments, strings, regex and template text. Cover runtime literals within typed
  functions and nested expressions; make exact byte edits and retain stable IDs.
- Run the full suite with a fresh Node/Vitest process per baseline and mutation.
  Start with one Vitest worker per Mutie worker to avoid multiplying concurrency.
  TypeScript transpilation is not a `tsc` type-check guarantee.
- Copy dependencies with independent file storage (CoW is acceptable), never
  writable hardlinks or a symlink to the caller's `node_modules`. Verify pnpm
  links resolve into each copy. Fresh owned transform caches and temp roots for
  every command; source resets cannot retain transformed mutants. Record copy
  cost before considering dependency/cache reuse. Do not weaken existing symlink
  checks to accommodate pnpm.
- Saved identity includes runner, syntax inventory, source roots, manifest/lockfile
  hashes, actual Node/Vitest/Vite versions, config and full-test execution settings.
  Reruns rediscover source identities and allow stronger test changes while
  rejecting changed dependency/tool/config scope. Preserve old Go/Rust reports.
  Keep the built-in adapter boundary small; no plugin protocol is needed.

## Outcome evidence and probes

[Vitest's one-shot CLI](https://v4.vitest.dev/guide/cli) and
[file reporters](https://v4.vitest.dev/guide/reporters) provide the execution
mechanism. Do not parse the terminal presentation. The pinned installed
`JsonReporter.onTestRunEnd` computes `success` from suites/tests without consuming
unhandled errors. The fixture confirms the consequence:

| Direct probe | Exit | Passed / failed tests | Interpretation required from Mutie |
| --- | --- | --- | --- |
| Baseline | 0 | 4 / 0 | Passing baseline |
| JS and TS tested boolean flips, separately | 1 | 3 / 1 | Killed, after complete evidence validation |
| JS and TS untested flips, separately | 0 | 4 / 0 | Missed |
| Each survivor with stronger test | 1 | 4 / 1 | Killed |
| TS syntax/transform error | 1 | 2 / 0 | Build failure, never a kill |
| No matching tests | 1 | 0 / 0 | Tool error / baseline rejection |
| Unhandled rejection | 1 | 5 / 0; JSON `success: true` | Tool error, never missed |
| Config throws | 1 | No report | Tool error |
| All tests skipped | 0 | 0 / 0; JSON `success: true` | Reject empty execution, never missed |

Plan a small Mutie-owned reporter using the runner's
[completion/error hooks](https://vitest.dev/api/advanced/reporters.html), version
checked against 4.1.9. Emit explicit completion, executed test counts, failed test
identities, module/collection and hook errors, unhandled errors and interruption
reason into a private, bounded evidence channel. Keep diagnostics separate.
Prove this protocol against the pinned implementation before relying on it; the
custom reporter itself is not implemented here. Missing, malformed, inconsistent
or truncated evidence is an error even when the process exits zero. The shared
classifier currently treats zero exit as missed, so validate evidence before
calling it or extend its contract with Go/Rust regression coverage.

Conservative precedence: cancellation/capture failure and timeout keep existing
semantics; runner/config/protocol/unhandled failures cannot become kills.
Known transform failures become build failures. Require a completed, nonempty
run with attributable test failures for killed, or successful exit plus complete
passing evidence for missed. Mixed infrastructure and test failures remain errors.
Hook errors and import-time runtime exceptions need explicit probes and policy
before implementation acceptance; don't infer their origin from a stderr phrase.

## Acceptance order

1. Project validation, dependency ownership, runner identity and separate JS/TS
   boolean discovery; deterministic listing and unsupported-scope diagnostics.
2. Reporter/protocol probe and isolated full-suite baseline/mutation adapter.
   Test all cases above plus hook/import errors, mixed failures, noisy output,
   missing/corrupt results and zero-exit early termination. Meet Experimental only
   when automated fixture workflows pass through Mutie.
3. Focused reruns, review/raw scores, source/lock/config invalidation, preservation,
   timeout/cancellation and descendant cleanup, bounded logs/captures, fresh cache
   and parallel-worker contamination controls. Keep real Vitest tests opt-in.
4. Fixed Pathe selection with useful finding review, stronger-test rerun, timings,
   source/dependency/cache/report preservation and serial/parallel outcome agreement.
   Add a real JavaScript project before claiming JS reaches Validated. Update the
   ledger by syntax and scope rather than raising both labels automatically.

The read-only preview completes step 1 within the limitations below. The next
implementation task is the completion/error protocol and isolated adapter; decide
how to accommodate Pathe's coverage-only config explicitly before claiming
unmodified real-project acceptance.

## Reproduction

The fixture README describes dependency preparation. Run
`python3 benchmarks/vitest-scope-probe.py <prepared-fixture> <output.json> <node>`.
The script copies the prepared fixture (including its self-contained dependencies)
to a temporary directory, applies one change at a time, asserts expected counts
and exits, and removes its copy. The twelve direct probes also passed through
this disposable-copy reproduction. They do not test Mutie cancellation, report
persistence, dependency integrity checking or production mutation discovery.

## Implemented: read-only discovery preview

`mutie vitest-list PATH [--include GLOB] [--exclude GLOB]` validates the initial
package/install metadata, queries Node identity with bounded capture and a deadline,
and lists stable file/byte-ordered boolean flips. It never invokes Vitest or
package scripts, mutates source, saves a run, evaluates scores or runs review/reruns.
It does not read `mutie.toml`; filters are explicit command arguments. This
separate preview keeps execution unavailable until its outcome contract exists.

Validation covers the ESM manifest, pnpm 11.10.0 layout, one lockfile importer,
manifest/direct-installed dependency agreement, unchanged installed lockfile,
registry integrity metadata, pinned Vitest/Vite versions, dependency-link
containment, source/configuration restrictions and actual Node 24.22.0 identity.
This is metadata/containment validation, not a cryptographic audit of all installed
package bytes or proof of a usable transitive installation. Execution baseline
and owned dependency copies still require implementation. Source symlinks are
rejected; internal dependency links are accepted. Source discovery parses JS and
TS separately, excludes declarations/tests/generated headers and type contexts,
and rejects unsupported syntax instead of returning partial results.

The initial plain-JSON `tsconfig.json` allowlist covers Pathe's compiler checks
and basic ESM compiler options; `extends`, paths/baseUrl, custom transforms and
nested configs are rejected. No `tsc` guarantee is made. Arbitrary Vite/Vitest
configs are rejected without executing them. TypeScript enums/namespaces,
ambient declarations and decorators are outside this preview's runtime scope.

Verification on Node 24.22.0/macOS:

- Prepared mixed-syntax fixture: six mutations, three JS and three TS. Include
  filtering selects three. Source edits are tested for exact bytes and stale-source
  refusal, including Unicode/CRLF.
- Unchanged Pathe clone: rejected for `vite.config.ts`, as required by the contract.
- Explicitly adapted temporary Pathe copy: removed only its coverage-reporter
  config. Listing finds fourteen boolean flips in `_glob.ts`, `_path.ts` and
  `utils.ts`. Its direct Vitest baseline still passes 467 tests, with 8 skipped and
  8 todo. The original clone remains unchanged. This adaptation is not a claim
  that the CLI accepts upstream Pathe unchanged.
- Unit and CLI regressions check discovery, config/module boundaries, lock/installed
  metadata mismatch, external/broken dependency links, deterministic listings,
  filters, runtime mismatch and read-only behavior. Default tests use synthetic
  metadata and a fake Node identity; the two real prepared trees supply the
  separate installation/Node checks. Vitest execution remains outside these tests.

Final checks for the preview: `cargo fmt --check`, `cargo clippy --all-targets --
-D warnings`, focused Vitest unit/CLI regressions and the complete default
`cargo test` pass. The first full run failed the existing
`resizing_preserves_scrollback_and_prompt_near_the_bottom` terminal test; its
isolated rerun and a subsequent complete run both passed. No terminal code changed.
The final suite totals are 223 passed, 32 ignored; real runner mutation workflows
remain opt-in. Adapted Pathe's baseline matches all 483 test names and statuses
after normalizing the differing temporary project paths.
