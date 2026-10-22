# Progress

Track implementation here; keep design decisions and rationale in
[DESIGN.md](DESIGN.md) and current usage in [README.md](README.md). Update this
checklist as work lands, checking items off after appropriate verification.

## Public release scope and goal sequence (2026-10-22)

This is the current release direction and supersedes historical “next runner”
priorities below. Target a substantial public release usable by other developers;
it may remain `0.x`. No version bump is authorized by this roadmap. Go, Rust,
Python/pytest and JavaScript/TypeScript through Vitest, Node and Bun are all
first-class targets. macOS and Linux are release platforms; Windows is deferred.
Use public unchanged projects for acceptance until user-selected projects arrive.

Release scope includes ordinary Rust workspaces, broader JS/TS package-manager
compatibility (npm, pnpm, Yarn and Bun), and an explicit Vite+ compatibility audit.
Keep language, test runner, package manager and toolchain/version distinct.
Support a documented tested version set, not arbitrary runtime versions. Exact
runner instrumentation currently makes upgrades more than a command-line check.

Aim to fill the README operator table, including bounded numeric/string literal
replacement. Existing operator-family parity is a commitment; evaluate scoped
statement/block removal too. Document concrete semantic or candidate-quality
blockers and discuss significant deferrals with the user rather than silently
excluding an entire family. A “Yes” requires implementation and validation.

Configuration/onboarding includes `mutie init`, inferred project tooling with
explicit choices for ambiguity, noninteractive setup, and TOML/CLI overrides.
Include mutation-source filters, operator inclusion/exclusion and validated
runner options. Distinguish source selection from test selection; options must
not silently omit tests, disable attribution or change saved execution scope.
Existing precedence remains defaults < TOML < explicit CLI options. Arbitrary
shell test commands are not an implied replacement for supported adapters.

Documentation will be developed collaboratively with the user. HTML dashboards,
hosted services, editor integrations and a plugin framework are outside this
release. No hosting is required. Exhaustive review of historical survivors is
not a release gate. The macOS startup investigation remains explicitly deferred;
record its impact honestly during release qualification without changing deadlines.

Each numbered item is a proposed bounded goal, not an automatically active goal.
Split implementation goals by language/runner when acceptance becomes too large.

1. [x] **Project configuration and onboarding contract — first goal.** Audit
   current configuration and representative public project manifests; define
   language/runner/package-manager/version selection and ambiguity handling.
   Implement `mutie init` for currently supported scopes, interactive and
   noninteractive, with safe existing-file handling, actionable unsupported-scope
   diagnostics, config round-trip/precedence checks and fixture smoke runs.
   Specify validated runner-option and operator-exclusion semantics; do not emit
   configuration for capabilities that have not shipped.
   Completed: [init contract and acceptance](benchmarks/project-init.md).
2. [ ] **JS/TS package-manager compatibility.** Audit npm/pnpm/Yarn/Bun installation
   layouts, lockfiles, invocation and dependency isolation; assess Vite+ explicitly
   before promising an adapter. Implement in bounded manager-specific goals,
   covering dependency resolution, source preservation, rerun drift and unchanged
   public-project workflows. Record supported versions and any PnP/workspace limits.
3. [ ] **Ordinary Rust workspaces.** Support workspace roots/member selection,
   inherited metadata and in-workspace path dependencies; copy complete needed
   context and bind selection to reports/reruns. Start with full supported workspace
   tests, not affected selection. Validate virtual/root-package fixtures and an
   unchanged public project, fresh/cached builds and cleanup. Audit build-script,
   proc-macro and out-of-tree dependency limits explicitly.
4. [ ] **Expression-operator parity.** Add missing arithmetic/modulo, comparison
   inversion/boundaries, logical swaps and negation removal by language. Preserve
   evaluation/grouping, short-circuit and overloaded/coercion semantics; retain
   compilation/runtime error distinctions. Require discovery/identity regressions,
   independent JS/TS runner checks and useful stronger-test acceptance per slice.
5. [ ] **Control-flow and removal parity.** Add scoped branch forcing, conditional
   loop skipping and standalone-call removal across languages. Audit binding
   conditions, empty blocks and side effects; evaluate broader statement/block
   removal and record concrete exclusions. Measure invalid-candidate noise.
6. [ ] **Numeric/string literal mutations.** Define bounded useful replacement
   sets per language, avoid duplicate/no-op and type-only candidates, and validate
   escaping, numeric edge cases, source identity and real-project usefulness.
   Discuss significant blockers rather than requiring unbounded transformations.
7. [ ] **Routine project compatibility and feedback.** Maintain representative
   unchanged public projects for every language/runner, including realistic
   dependencies/configuration. Address demonstrated onboarding/runtime blockers
   in bounded goals; verify run → inspect → improve tests → rerun and record
   timings/noise. Do not select only projects that fit existing restrictions.
8. [ ] **Public release qualification and documentation.** Automate a documented
   macOS/Linux test matrix including real adapters and supported versions; verify
   clean installation, cancellation, reports and example CI policies. Collaborate
   on quickstarts, compatibility/errors and release notes; choose distribution
   channels and compatibility policy. Review unresolved failures explicitly;
   focused green tests are not a substitute for release acceptance.

Start compatibility/project sampling alongside the first goal so it informs the
configuration contract. Build the release matrix incrementally rather than
postponing all automation until the final goal. Tinycss2's remaining 78 candidates
are useful optional evidence, not the next automatic priority.

## Current direction

The Go MVP and the initial Rust binary-package milestone are complete for their
supported scopes. The first measured Rust feedback-time improvement is also
validated: unchanged derived-report writes are skipped, and the saved six-mutation
self-run fell from 564 to 393 seconds in one controlled pair with matching outcomes.
See [the acceptance evidence](benchmarks/rust-report-write-reruns.md) for limits.
Ordinary Rust library packages now meet the scoped Validated level: fixture
acceptance is complemented by Marathon's real-project findings, stronger-test
reruns, matching fresh/cached outcomes and preservation evidence. See
[the results](benchmarks/rust-library-marathon.md). The scoped JavaScript/TypeScript + Vitest workflow now has independent real-project
acceptance. Repeated Node launcher resolution is now avoided while preserving per-command
identity checks; see [validation](benchmarks/vitest-node-reuse.md). The complete
133-candidate Hyparquet inventory has now been attempted, with stronger-test
review and explicit callback-error/timeout accounting; see [full evidence](benchmarks/vitest-hyparquet-full.md).
The first Node test-runner slice is now Validated and JavaScript-only; see
[scope and evidence](benchmarks/node-initial-scope.md). Body-exception attribution now uses exact worker evidence and rejects mixed
lifecycle/process failures; see [the audit](benchmarks/node-body-attribution.md).
The remaining SRI findings are reviewed in [the follow-up](benchmarks/node-sri-final-review.md):
the timeout is a reproducible pending promise; the cleanup survivor remains unproven
equivalent. The second unchanged package, `openclaw-cc`, completes 39 candidates
with 22 kills, 15 survivors, two timeouts and no errors; three stronger tests kill
three selected survivors. A test-only teardown fix turns both timeouts into
completed kills in a separate rerun. See [acceptance](benchmarks/node-openclaw.md).
The [storage/profile review](benchmarks/node-storage-profiles.md) kills five more
survivors and records the scoped Node milestone as Validated. Seven scheduling,
delivery and entry-point survivors remain open. The milestone is complete within
its pinned scope; choose the next major package/runner scope explicitly rather
than defaulting to another small operator or UI change.
Native TypeScript on Node followed that milestone; its first Experimental
slice and independent evidence are recorded in [the audit](benchmarks/node-typescript-scope.md).
[Unchanged Bragkit acceptance](benchmarks/node-default-discovery.md) now closes the
native-discovery gate and establishes scoped TypeScript validation.
Bun is now the selected next runner. The [initial audit](benchmarks/bun-initial-scope.md)
pins 1.3.11, adds independent JS/TS fixtures and selects Composer (272 passing tests).
Native reports conflate body, hook and process failures. A bounded file-backed
[observer prototype](benchmarks/bun-body-attribution.md) now passes 112 paired cases
and Composer controls. Shared scored execution, focused reruns,
review/policies and scored cleanup pass. TypeScript and JavaScript are independently Validated;
JavaScript now has unchanged pp-is layout acceptance following the documented adaptation. Rust workspaces remain a separate decision.
Explicit user priorities and demonstrated reliability blockers take precedence.

Python/pytest's first scoped milestone is Validated on CPython 3.14.7 / pytest
9.0.2. [Scrubify acceptance](benchmarks/python-scrubify-findings.md) records a
useful blank-query survivor, stronger tests and a focused kill, with unchanged
source and old reports. Scored review/policies and cleanup have fixture coverage.
The [lifetime observer](benchmarks/python-process-lifetime.md) preserves strict
late-cleanup evidence and excludes thread-starting suites. Scoped unittest now
adds separate dispatch evidence; broader layouts/dependencies and further startup optimization need separate follow-up decisions.
The [Python feedback profile](benchmarks/python-feedback-profile.md) now measures
full and focused feedback and optimizes runtime traversal without skipping hashes.
[Native `src/` acceptance](benchmarks/python-src-layout.md) now preserves Scrubify's
original layout and manifest through the complete scored/stronger-test rerun loop.
[Unchanged Cycler validation](benchmarks/python-cycler-unchanged.md) extends the
evidence to a 28-test suite: root-test inventory, conservative original error,
pending-candidate rerun and a separate test-only error-to-kill control.

## Target support tracking

[SUPPORT.md](SUPPORT.md) tracks maturity, scope and evidence independently for each
language/test runner. The planned targets below are explicit roadmap targets;
Vitest and Node have independently validated JavaScript and TypeScript scopes;
Bun also has independently validated JavaScript and TypeScript scopes; Python's
validated initial scope and follow-up limits are recorded below.

- [x] Define support levels and record current Go and Rust evidence and limits.
- [x] Define and implement the first JavaScript/TypeScript + Vitest slice.
- [x] Define and implement the first JavaScript + Node test runner slice.
- [x] Define and implement the first native TypeScript + Node slice with independent
  fixture and adapted real-project evidence.
- [x] Complete unchanged-project TypeScript + Node acceptance before promoting maturity.
- [x] Define and implement the first JavaScript/TypeScript + Bun test runner slice.
- [x] Define and implement the first Python + pytest slice.

Each new slice starts with a fixture and a real validation project, then advances
through the support gates recorded in the ledger. The selected Vitest objective is scoped below.

## Selected Python / pytest scope audit

- [x] Validate one audited pure-Python runtime dependency through unchanged
  Tinycss2 1.2.1: exact webencodings 0.5.1 bytes, explicit declaration, lifecycle
  and identity controls, 480 original tests and four stronger-test kills in a
  separate 484-case copy. See [dependency acceptance](benchmarks/python-dependencies-tinycss2.md).
  Other dependencies and current Tinycss2 remain separate scope decisions.

- [x] Review all 19 Tomli-W errors and the remaining survivor. Separate operand
  limitations from behavioral failures; a test-only validity assertion yields
  four kills while three error controls and the survivor retain their outcomes.
  No discovery defect established; see [quality review](benchmarks/python-tomli-w-review.md).

- [x] Validate unchanged Tomli-W's complete 271-case suite and all 30 candidates
  across all Python operators; retain failed/unscored error reports and kill a
  useful formatting survivor in a separate 272-case stronger-test rerun. Bind
  fixture data/directory identity and explicit expected failures; see
  [Tomli-W acceptance](benchmarks/python-tomli-w.md).

- [x] Validate bounded opt-in candidate-error continuation against all 194
  unchanged betting-math-kit arithmetic outcomes, preserving fatal boundaries,
  cancellation, failed/unscored error reports and historical evidence; see
  [continuation acceptance](benchmarks/python-candidate-continuation.md).

- [x] Pin an isolated Python/pytest environment and retain reproducible native probes.
- [x] Audit parametrization, setup/call/teardown, collection errors, incomplete runs,
  warnings, exits and conservative attribution; retain unittest/atexit negative controls.
- [x] Demonstrate owned application imports and a stale-bytecode control fixed by
  a fresh cache prefix.
- [x] Run adapted Inflection's 467-test baseline and a manual Unicode-digit
  survivor → 468-test stronger baseline → assertion-failure loop. This is
  direct audit evidence, not automatic discovery or a score.
- [x] Add native process-lifetime evidence for late cleanup and strict
  completion/replay validation; initially reject unittest and thread-starting suites.
- [x] Validate production observer cancellation, bounded captures, ordinary
  process-group descendants and per-file resource limits; see
  [integration controls](benchmarks/python-process-integration.md).
- [x] Implement initial project/config/import scope, discovery and isolated baseline
  previews; see [integration boundary](benchmarks/python-baseline-integration.md).
- [x] Select real-project acceptance within the exact supported test recipe.
  Adapted [case-converter](benchmarks/python-case-converter.md) passes all 107
  original tests; six candidates account for five kills and one conservative
  error across two runs. Adapted [Scrubify](benchmarks/python-scrubify-findings.md)
  completes the useful finding and stronger-test loop without changing source bytes.
- [x] Convert Python discovery and execution to shared mutation/command types,
  retaining failure diagnostics and conservative outcome classification.
- [x] Bind each candidate's collected test IDs to the pristine baseline inventory.
- [x] Connect scored runs, saved reports, focused reruns and dismissal review/policies;
  [fixture acceptance](benchmarks/python-shared-runs.md) preserves inputs and old reports.
- [x] Validate scored timeout/cancellation/resource-limit boundaries; the running
  mutant checkpoint, interrupted report and empty owned temporary parent are
  covered in [fixture acceptance](benchmarks/python-shared-runs.md).
- [x] Complete the real-project findings/stronger-test/rerun loop and support audit.
- [x] Profile Python feedback overhead, optimize repeated runtime traversal and
  compare complete full/stronger-test reruns with preservation checks.
- [x] Support one native `src/` package with narrowly audited static metadata,
  source-root/import identity, rejection and snapshot/rerun regressions, and
  unchanged Scrubify full-run plus separate-copy stronger-test acceptance.
- [x] Validate a second, larger unchanged Python project: Cycler's 28-test suite,
  complete candidate accounting, preserved failed reports and a separate
  test-only error-to-kill rerun plus full stronger-test run.

- [x] Add scoped synchronous unittest.TestCase through pytest with exact body
  exception matching, complete lifecycle evidence, static rejection controls,
  scored review/reruns and unchanged Pluralizer validation. See
  [acceptance](benchmarks/python-unittest-pluralizer.md).

- [x] Complete scoped conftest fixture acceptance: static admission, lifecycle
  attribution, shared/parametrized/autouse controls, scored reruns and unchanged
  Binary Tree Library validation. All six survivors are killed by stronger tests
  in a separate copy; original inputs/reports are preserved. See the
  [fixture audit](benchmarks/python-conftest-fixtures.md).

- [x] Validate a larger unchanged fixture-based Python project: Route Mini,
  68 tests and 38 candidates, scoped helper/metadata compatibility, three
  stronger-test kills and preserved original failed reports. See
  [acceptance and remaining findings](benchmarks/python-route-mini.md).

- [x] Review Route Mini retry/fallback and dataclass findings in a separate copy;
  preserve original inputs/reports and distinguish scoped scored reruns from
  native-only slots diagnostics. See the [finding review](benchmarks/python-route-mini-review.md).

- [x] Isolate Go fixture startup/readiness failures: reproduce fresh-executable
  loader stalls outside Mutie and use the existing system shell for fixture
  execution. Separate the noisy-log cancellation checkpoint from its one-second
  timeout scenario; preserve readiness, cleanup and outcome assertions. Stabilize
  the fake-Node response test through explicit shell execution and unchanged
  production output validation, retaining real spawn-error checks; preview CLI
  tests use a prepared native identity mock and exact mismatch errors. See
  [reproduction and validation](benchmarks/process-readiness.md).

- [x] Add scoped Python comparison boundaries with native token replacement,
  operand-order/NaN/custom-dispatch controls, saved identity validation and an
  unchanged Route Mini capacity finding killed by a stronger-test focused rerun.
  See [acceptance](benchmarks/python-comparison-boundaries.md).
- [x] Validate comparison boundaries on unchanged Cycler: 28-test baseline,
  one justified equivalent survivor, preserved inputs/reports, and no invented
  stronger-test kill. See [review](benchmarks/python-cycler-boundaries.md).

- [x] Diagnose and fix the native Node fixture startup failures reproduced during
  final Python-boundary validation. A macOS pre-main loader stall is avoided with
  system-interpreter fixture data; ten-second readiness and identity assertions
  remain intact. Twenty preview repetitions pass, and the full default suite
  passes 277 tests. See [evidence and limits](benchmarks/vitest-fixture-startup.md).
- [ ] **Deferred by user (2026-10-21):** resolve the intermittent CLI startup delay: actual CLI children now have
  captured pre-main `_dyld_start` stalls, with failures inside and outside the
  sandbox. Readiness checks retain their deadlines and now capture bounded
  post-failure diagnostics; a negative control checks failure and reaping.
  Three full suites pass 278 tests, but focused repetitions still reproduce
  failures. A temporary Developer Tools comparison passed in both enabled and
  user-confirmed disabled states, so it remains inconclusive. A host-level
  remedy remains unverified; see
  [diagnosis and controls](benchmarks/cli-startup-readiness.md).

- [x] Complete the seven comparison boundaries on unchanged Route Mini's
  68-test suite: preserve the original error, account for six pending candidates,
  and kill the newly identified extra-network-retry survivor with a separate
  70-test stronger baseline and focused rerun. See
  [complete boundary review](benchmarks/python-route-mini-all-boundaries.md).

- [x] Validate a new unchanged Python project larger than Route Mini:
  betting-math-kit passes 198 tests, with minimal inert metadata and independent
  Random construction support. A separate 205-test copy kills six selected
  metric-boundary survivors; four original runtime errors remain preserved.
  See [acceptance and scope](benchmarks/python-betting-math.md).

- [x] Account for all 87 unchanged betting-math-kit candidates: 19 kills,
  59 survivors and nine errors across preserved reports. Review every survivor
  and error; a separate complete 222-test copy kills 24 survivors and turns two
  endpoint errors into assertion kills without classifier changes. Retain open
  numerical cases and distinguish combined accounting from a full score. See
  [complete review](benchmarks/python-betting-math-full.md).

## Selected first Bun test-runner slice

- [x] Pin Bun 1.3.11+af24e281e and add independent dependency-free JS/TS fixtures.
- [x] Audit JUnit and inspector evidence with 24 cases per syntax. Retain the
  body/hook/process ambiguity and incomplete-run controls; see [audit](benchmarks/bun-initial-scope.md).
- [x] Select unchanged GramIO Composer: 272-test baseline and three manual mutation
  probes; a stronger test exposes a passing name-configurability flip with 273 tests.
  These are direct probes, not adapter acceptance or scored outcomes.
- [x] Validate a bounded file-backed phase observer: 112 paired JS/TS cases,
  30 malformed-evidence controls and matching Composer baseline/finding outcomes;
  see [attribution evidence](benchmarks/bun-body-attribution.md). Inspector shutdown
  loss and successful-callback checkpointing were rejected approaches.
- [x] Port the evidence reader to Rust and replay all 112 recorded JS/TS cases
  plus four Composer controls. Require an independent expected-file inventory
  and bounded, strict XML/JSONL parsing; see [Rust replay](benchmarks/bun-rust-protocol.md).
- [x] Connect the Rust reader to baseline-only `bun-check`: owned copies, independent
  test inventory, pinned runtime/observer checks, bounded captures and per-file OS
  limits. Fixture outcomes and cleanup probes are recorded in
  [isolated execution](benchmarks/bun-isolated-execution.md).
- [x] Accept unchanged Composer's audited configuration/layout: 272-test baseline,
  surviving candidate and stronger-test body failure through isolated execution;
  see [Composer acceptance](benchmarks/bun-composer-execution.md).
- [x] Add read-only `bun-list` and JSON baseline identity: 12 Composer flips,
  unchanged input hashes and separate Bun ambient-declaration handling; see
  [discovery/identity](benchmarks/bun-discovery-identity.md).
- [x] Connect Bun execution identity and discovery to shared scored runs, saved
  reports and focused reruns; independently exercise JS/TS fixtures. See
  [shared workflow](benchmarks/bun-shared-runs.md).
- [x] Run all 12 Composer candidates and kill the name-configurability survivor
  through a saved-ID stronger-test rerun; preserve original sources/reports.
- [x] Validate package/import/config/discovery boundaries and connect isolated
  execution to reports, reruns, review and policies; independent JS/TS CRUD,
  staleness and error precedence pass. See [review acceptance](benchmarks/bun-dismissals.md).
- [x] Verify baseline diagnostic timeout/cancellation/capture limits and descendant cleanup.
- [x] Verify scored JS/TS output/file limits, timeout, cancellation, descendant
  shutdown, saved score boundaries and source preservation; see
  [scored cleanup](benchmarks/bun-scored-cleanup.md).
- [x] Complete the scoped TypeScript real-project feedback loop and milestone
  audit using Composer; promote TypeScript independently to Validated.
- [x] Complete an independent real JavaScript loop on adapted pp-is: 22 candidates,
  seven stronger-test kills and preserved sources/reports; promote scoped JS to
  Validated. See [adaptation and evidence](benchmarks/bun-javascript-pp-is.md).
- [x] Accept unchanged pp-is packaging and static JS implicit ESM with strict
  import/test boundaries; match all 22 adapted outcomes. See
  [unchanged layout](benchmarks/bun-unchanged-layout.md).
- [x] Profile Bun feedback cost across debug/release and actual adapter phases:
  SHA-256 executable checks dominate; preserve verification and prefer release
  builds. A temporary sha2-only optimization recovers most debug overhead; see
  [profile](benchmarks/bun-feedback-profile.md).
- [x] Optimize `sha2` in development/test builds while preserving complete
  fingerprints; validate matching full and focused Bun outcomes.
- [x] Review Composer's four remaining survivors and queue timeout: four new
  behavioral tests and queue wait preconditions kill all five in a full-suite
  focused rerun. See [findings](benchmarks/bun-composer-survivors.md).
- [x] Reassess after scoped Bun validation and Composer review: select the
  Python/pytest feasibility audit before further Bun scope expansion.

## Validated first Node test-runner slice

- [x] Select Node 24.22.0, a dependency-free ESM fixture and `sri-dynamic-loader`.
- [x] Audit completion, nested results, imports, early exits and hidden cleanup
  failures; pin the hook observer to the audited internal implementation.
- [x] Share boolean discovery with Vitest, including byte-preserving parsing of
  literal NUL text found in the real project.
- [x] Connect isolated execution to reports, reruns, review and policies.
- [x] Verify mutation-triggered failures and timeout/cancellation/capture cleanup.
- [x] Attempt the real inventory, inspect survivors and rerun stronger tests while
  preserving sources and original reports. See [evidence](benchmarks/node-initial-scope.md).
- [x] Broaden non-assertion body-error attribution with exact body evidence and
  mixed hook/import/late-process regressions. Reject ambiguous dynamic imports.
- [x] Review the remaining cleanup-guard survivor and timeout separately; retain
  the unresolved survivor and timeout classifications. See [review](benchmarks/node-sri-final-review.md).
- [x] Validate another real package before broadening dependency/module scope:
  `openclaw-cc` full inventory and three stronger-test kills are recorded in
  [acceptance](benchmarks/node-openclaw.md).
- [x] Review storage-locking and workspace-profile findings: all five are killed
  by stronger tests; see [evidence and milestone audit](benchmarks/node-storage-profiles.md).
- [x] Reassess the Node milestone: Validated within the unchanged pinned JavaScript
  scope. Preserve seven open `openclaw-cc` survivors and the SRI limits.
- [x] Select native TypeScript on Node as the next major scope, using Bragkit for
  mutation findings and dependency-atlas as an unchanged baseline control.

## Validated native TypeScript on Node

- [x] Audit native stripping, explicit `.ts` imports and NodeNext type-checking config;
  reject syntax requiring transforms and keep type-only booleans out of discovery.
- [x] Connect `.ts` source/test files to isolated execution, reports, browser validation,
  focused reruns, review and policies; preserve the legacy JavaScript recipe.
- [x] Repeat the 42 attribution cases in TS and verify timeout/cancellation/capture
  cleanup. Reject config changes and missing `import type` failures correctly.
- [x] Run unchanged dependency-atlas: nine baseline tests, zero eligible mutations.
- [x] Run Bragkit with only its test script adapted to the explicit TS glob:
  23 candidates, two killed, 21 survived, no errors/timeouts, 31.17 seconds.
- [x] Retain two HTML-list tests; kill source IDs 20–22 in an 11-second focused rerun
  with a passing 63-test baseline and preserved source/report hashes.
- [x] Audit safe native `node --test` discovery and validate Bragkit without the
  script adaptation. Match all 23 outcomes and kill the three saved HTML-list findings;
  reject out-of-scope tests and snapshot exclusions that would drop tests.
- [x] Review the 18 remaining Bragkit CLI/collector/browser-fallback survivors:
  all killed by 13 stronger tests in a 53.39-second focused rerun; see
  [review evidence](benchmarks/node-bragkit-survivors.md).
- [x] Reassess TypeScript maturity independently: Validated within the native,
  dependency-free ESM scope; see [unchanged-project evidence](benchmarks/node-default-discovery.md).

## Validated first Vitest slice and follow-up work

- [x] Scope the first package/runner/syntax/dependency boundary; add separate JS/TS
  fixtures and select Pathe for real TypeScript validation. Record direct baseline,
  stronger-test and failure probes in [the audit](benchmarks/vitest-initial-scope.md).
  This is planning evidence, not implemented support.
- [x] Add read-only `vitest-list`: validate one ESM/pnpm package, installed
  dependency metadata and runner identity; discover JS/TS runtime boolean flips
  with deterministic listings and exclusions. Six fixture mutations and fourteen
  in an adapted Pathe copy are listed. Correct the audit: upstream Pathe has a
  coverage-only Vite config and is rejected until explicitly supported.
- [x] Implement bounded completion/error evidence and a fresh-copy execution
  adapter, exposed through `vitest-check` for baseline diagnostics. Direct adapter
  probes cover fixture kills/survivors/stronger tests, transform/import/hook and
  mixed failures, noisy/empty/early-exit cases and timeout/cancellation/capture
  cleanup. See [the evidence and limits](benchmarks/vitest-execution.md).
- [x] Route Vitest completion evidence and diagnostics through the shared command
  type and classifier. Require validated completion even after exit zero, preserve
  process-failure precedence, and make shared baselines use outcome classification.
  The real adapter probes now exercise this boundary.
- [x] Connect Vitest to shared runs/coordinator/saved reports with runner/tool/config
  identity. Require completion evidence in baseline messages and saved outcomes.
  Verify six fixture CLI mutations, a stronger-test rescan, report reading,
  source preservation and infrastructure failure persistence; see
  [the CLI evidence](benchmarks/vitest-shared-runs.md). Experimental scope is
  explicit `--lang vitest`, serial full-suite runs.
- [x] Add Vitest saved-ID/missed-only reruns and browser command export. Match
  fresh source identities, reject changed package/lock/tool/config identity before
  baseline, and verify JS/TS survivors killed by stronger tests with saved
  provenance and original-report preservation. See
  [rerun evidence](benchmarks/vitest-focused-reruns.md).
- [x] Audit broader lifecycle/fixture attribution against pinned Vitest. The
  [16-case audit](benchmarks/vitest-lifecycle-attribution.md) identified false kills
  outside traditional reporter hooks, addressed by the worker evidence below.
- [x] Add conservative worker-side lifecycle participation evidence for fixtures,
  aroundEach and completion callbacks. Reject ambiguous failed tests, require
  audited runner bytes in the isolated copy, and convert audit cases to acceptance
  regressions, including mutation-triggered CLI failures after passing baselines.
- [x] Distinguish body failures from lifecycle failures so successful hooks,
  test-scoped fixtures and callbacks can accompany genuine kills. Preserve errors
  for mixed failures and ambiguous async/assertion-count attribution.
- [x] Validate file/worker fixture setup, body and teardown attribution, nested
  hooks and concurrent body/cleanup mixtures. Add mutation-triggered scoped
  fixture CLI score checks; see [lifecycle evidence](benchmarks/vitest-lifecycle-attribution.md).
- [ ] Resolve remaining async/assertion-count attribution conservatively; cross-file
  fixture reuse and cancellation during scoped teardown remain unvalidated.
- [x] Connect Vitest dismissal CRUD/check, saved review snapshots and CI policies.
  Verify separate JS/TS decisions, stale-source invalidation, unchanged raw
  outcomes/scores, stronger-test reruns, config/CLI overrides and infrastructure
  error precedence. See [review evidence](benchmarks/vitest-dismissals.md).
- [ ] Validate parallel workers and broader cache/timeout/cancellation preservation
  through the complete CLI workflow.
- [x] Accept Pathe’s original static coverage-reporter-only config without
  executing it; keep coverage disabled, reject extra settings, and fingerprint
  the file for snapshots and reruns. See [config evidence](benchmarks/vitest-coverage-config.md).
- [x] Run Pathe with its original config: 14 mutations, 12 kills, two survivors,
  zero errors in 94.00 seconds; preserve input files and validate saved reports.
- [x] Investigate Pathe survivors 12 and 13: both are distinguishable boundary
  behaviors. Three stronger tests kill both in an 18.79-second focused rerun;
  record preservation and [TypeScript milestone acceptance](benchmarks/vitest-pathe-survivors.md).
- [x] Select Hyparquet for independent JavaScript validation; accept its original
  JavaScript-only NodeNext type-checking config. Discover 133 flips and pass the
  724-test isolated baseline; see [audit](benchmarks/vitest-javascript-hyparquet.md).
- [x] Run 19 sampled Hyparquet mutations: 17 kills and two survivors. Three stronger
  tests kill both survivors in a 55.60-second rerun; record independent scoped
  [JavaScript acceptance](benchmarks/vitest-hyparquet-findings.md).
- [x] Investigate Hyparquet focused feedback time: three unchanged reruns take
  roughly 26 seconds; test-only phase profiles separate copy/validation/cleanup.
  A PATH comparison takes 58.81 seconds through the Node launcher versus 27.56
  through the same resolved binary; see [profile](benchmarks/vitest-feedback-profile.md).
- [x] Resolve Node once per run and revalidate the pinned executable without
  repeating the launcher. Preserve executable/version and project/config identity
  checks; regression tests cover identity failures and a controlled saved-selection
  comparison is recorded in [validation](benchmarks/vitest-node-reuse.md).
- [x] Attempt all 133 Hyparquet candidates across explicit saved continuations:
  89 killed, 41 survivors, two unhandled callback errors and one timeout. Preserve
  failed reports without manufacturing a full-project score.
- [x] Review all survivors and retain stronger tests plus an awaited-callback test
  patch. A 43-candidate rerun kills 36 former survivors and both error candidates,
  leaving four redundant scan settings and one unknown-operator policy case;
  see [full evidence](benchmarks/vitest-hyparquet-full.md). No production changes
  or dismissals were needed. The earlier 978-second sample remains partly unexplained.

## Completed: first Rust feedback-time improvement

- [x] Profile compilation, adapter phase overhead and individual test-target
  execution in repeated serial/parallel cold/warm Mutie runs. See
  [the feedback profile](benchmarks/rust-feedback-profile.md). The residual outside
  harness timings bounds further investigation; it does not isolate Cargo startup.
- [x] Measure prebuilt CLI startup and full dismissal-harness execution with
  default/one/two Tokio threads in interleaved repetitions. All nine harnesses
  pass; the small 2–3% median difference does not justify changing runtime defaults.
  See [the runtime experiment](benchmarks/rust-runtime-startup.md).
- [x] Attribute dismissal workflow time to command execution, report persistence
  and cleanup using isolated diagnostics. Focused and complete harnesses identify
  report persistence as substantial; observer overhead is quantified. See
  [workflow costs](benchmarks/rust-workflow-costs.md). Extend instrumentation to
  terminal/log-limit targets if needed after evaluating this candidate.
- [x] Avoid unchanged derived-report writes while retaining matching
  authoritative checkpoints, atomic durable publication for changed files, missing
  file/error handling and final report semantics. Full prebuilt suites pass serially
  and in pairs, with roughly one-third less test time in this sample; dismissal
  report writes drop from 4,347 to 2,024 without dropping checkpoints or commands.
  See [validation and limits](benchmarks/derived-report-writes.md).
- [x] Compare the unchanged saved six-score selection before/after: six kills and
  exact failure sets match, all local targets rebuild, and sources/reports/caller
  artifacts are preserved. One sequential end-to-end pair plus the prebuilt-suite
  matrix demonstrates an improvement, not a repeated-run distribution or general
  performance guarantee. See [the results](benchmarks/rust-report-write-reruns.md).

## Completed: ordinary Rust library packages

- [x] Scope ordinary library packages using a small fixture and a representative
  real project. Support library-only and library-plus-one-binary layouts with the
  existing unit/integration test scope; doctests remain excluded. Direct Cargo
  probes verify the fixture's test-improvement scenario, and Marathon's isolated
  baseline passes 182 tests. These are scope evidence, not implemented adapter
  support. See [the audit and acceptance gates](benchmarks/rust-library-scope.md).
- [x] Generalize saved production-target identity, preserving strict rerun checks
  and compatibility with saved binary-only reports. New reports record target
  kind/name/source/edition; Cargo metadata must match the selected binary's full
  identity and crate type.
- [x] Extend manifest and metadata validation to ordinary libraries, including
  explicit names/paths, crate types and automatic target discovery, together with
  multi-root discovery so no accepted production root is silently omitted.
- [x] Discover from all production roots with stable shared-file deduplication;
  cover explicit names/paths and unsupported target configurations.
- [x] Validate fresh/cached fixture workflows, stronger-test focused reruns,
  local rebuilds and source/artifact preservation through Mutie.
- [x] Add library-specific review, cancellation and build-failure acceptance
  probes for both library layouts: raw-score/dismissal separation, stale review,
  baseline and mutation failure classification, serial test interruption, saved
  diagnostics and descendant/source/artifact cleanup.
- [x] Validate useful findings and focused reruns on Marathon, retaining Mutie as
  the binary-only control, then update implemented support claims. Workspaces and
  local dependencies remain separately scoped. The 15-mutation selection yields
  11 kills and four survivors; two added tests kill three survivors in matching
  fresh/cached reruns. One equivalent survivor is reviewed, not forced into a kill.

See [the acceptance review](benchmarks/rust-milestone-acceptance.md) for evidence,
limits and the reasoning behind this priority.

## Completed milestone: initial Rust support

See [the Rust milestone](DESIGN.md#completed-milestone-initial-rust-support)
for the agreed scope and completion criteria. Acceptance applies to one ordinary
binary package on Unix, three operators, and the verified test-improvement workflow;
it does not imply general Cargo/workspace support.

- [x] Select Rust as the second language and the next major product milestone.
- [x] Choose a small fixture and a representative maintainer-owned Rust project:
  `tests/fixtures/rust` and Mutie itself. Define the initial binary-package/default
  feature Cargo scope, source exclusions, unsupported layouts, and owned artifact
  directories in DESIGN. Verify the fixture's two detected boolean flips and one
  survivor, plus a stronger-test probe; verify Mutie's baseline in an isolated
  copy (178 passed, 6 ignored). See `benchmarks/rust-initial-scope.md`. These are
  direct Cargo checks recorded before implementing mutation execution.
- [x] Build the first end-to-end Rust slice: project detection/configuration,
  source discovery, one operator, passing baseline, isolated mutation execution,
  and saved results for one Cargo binary package. Share command captures,
  classification, test dispatch, workers, status and reports with Go. Verify the
  three-mutation fixture, stronger tests through a fresh run, failures, empty
  selections, repeated/parallel execution, timeout/cancellation cleanup, and
  caller artifact preservation. Persist Cargo scope/tool versions and keep
  unsupported Rust rerun/dismissal actions explicit. Mutie's serial two-mutation
  score sample finished with one kill and one survivor in 429.59 seconds. See
  `benchmarks/rust-first-slice.md`, including the noisy parallel-run result.
- [x] Turn the first real Rust survivor into a test improvement: add saved-report
  regressions for missing/unexpected percentages, then run the same score sample
  through Mutie again. The fresh serial run passed all 189 baseline tests and
  killed both mutations; the former survivor failed exactly the two new cases.
  See the stronger-test follow-up in `benchmarks/rust-first-slice.md`.
- [x] Harden the PTY self-tests under parallel Cargo load: retain failure
  diagnostics, prevent inherited terminal descriptors, and allow enough time
  for a whole test workflow to finish. The descriptor regression fails before
  the fix; 12 paired terminal/browser runs (216 tests) pass afterward, including
  overlap with fresh compilation. A fresh two-worker self-run passes all 189
  baseline tests and kills both score mutations through score-validation failures;
  every terminal/browser test passes. See `benchmarks/terminal-reliability.md`.
- [x] Verify cancellation during Rust mutation test execution with real Cargo:
  serial SIGTERM and parallel SIGINT after a completed mutation, using both
  file-backed and pipe-backed captures. Keep completed results/logs, distinguish
  interrupted and pending work, validate the partial report, stop Cargo/test/
  helper processes, and preserve sources and caller artifacts. See the
  cancellation follow-up in `benchmarks/rust-first-slice.md`.
- [x] Verify compilation-stage cancellation/timeouts for Rust baselines and
  parallel mutations, using real Cargo and a live rustc checkpoint. Preserve
  partial reports and build diagnostics, stop compiler/helper descendants, and
  prevent tests from starting after compilation stops. Verify one deadline spans
  compilation and testing; deliberately resetting the test deadline makes the
  regression fail. Keep source/caller artifacts intact and owned directories
  cleaned up. See `benchmarks/rust-first-slice.md`.
- [x] Verify Cargo tool/protocol failures and raw-output limits: 17 injected fault
  scenarios cover startup, unexpected exits/signals, completion evidence,
  discovery/listing and baseline failures, and parallel overflow/peer cleanup.
  Preserve failure reasons independently of tiny saved logs; fix metadata
  overflow diagnostics that previously lost the capture-limit explanation.
  Document separate per-invocation capture budgets for compilation and testing.
- [x] Make Rust execution reliable for the initial binary-package scope:
  distinguish build/test/tool failures, enforce
  timeouts and cancellation, preserve partial results, and define build-cache
  and target-directory ownership. Verify repeated/parallel mutations execute the
  intended tests without stale artifacts, source edits, or artifact collisions.
  The first slice covers ordinary Cargo evidence, fresh artifacts, repeated/
  parallel runs, compilation/test timeouts, shared deadlines, and baseline/
  mutation cancellation with partial results. Tool/protocol failures and raw
  capture limits are now covered too; broader Cargo layouts remain unsupported.
  The PTY follow-up fixes reproduced harness defects; the original exit-only
  failure lacks enough diagnostics to establish its specific cause.
- [x] Add a small useful Rust operator set and document eligibility for test-only
  code, macros, generated sources, and conditional code. Support the chosen real
  project's workspace/package selection if required by the agreed scope. The
  accepted set is boolean flips, comparison boundaries and logical swaps; Mutie
  requires no workspace-member selection. See the acceptance review.
- [x] Add Rust comparison-boundary mutations (`<` ↔ `<=`, `>` ↔ `>=`), preserving
  operands and source exclusions. Include executable match guards while keeping
  patterns excluded. Verify exact edits, generic borrowing, floats, operand
  effects and a fixture survivor killed by an equality test through focused rerun.
  Mutie's six score-boundary mutations finish with five kills and one survivor
  at the saved-percentage tolerance endpoint in 780.31 seconds; retain that narrow
  review finding and the runtime evidence in `benchmarks/rust-comparison-boundaries.md`.
- [x] Add Rust `logical_swap` for lazy boolean operators, preserving grouping,
  comments and single operand occurrences. Exclude let-chain connectors while
  retaining valid boolean operands. Verify short-circuit effects, nested truth
  tables, source exclusions, configuration, parallel caches, reports and a
  stronger-test saved rerun in `tests/fixtures/rust-logical` (seven kills and one
  survivor, then one intended kill). A focused Mutie self-run passes 200 baseline
  tests and kills its invalid-duration conjunction solely through the existing
  timing regression, preserving sources/artifacts (285.88 seconds); see
  `benchmarks/rust-logical-swaps.md`.
- [x] Validate opt-in per-worker Cargo dependency reuse against fresh builds on
  the same six Mutie score-boundary mutations. Force package recompilation with
  Cargo cleanup; verify backdated source, restoration, stronger-test reruns,
  parallel isolation, failure/timeout/cancellation cleanup and source preservation.
  Fresh remains the default. Both self-runs produce five kills and one survivor;
  warm compilation falls from 66–73 to 22–46 seconds, with 155 dependency artifact
  records reused and all 14 local targets rebuilt. Whole runs take 846.13 versus
  651.97 seconds, but baseline variation accounts for nearly half the difference.
  See `benchmarks/rust-build-cache.md` for costs, limits and preservation evidence.
- [x] Reproduce and fix capture-shutdown and retention-lock assertion failures
  exposed during the paired self-runs. Retry process-group shutdown to catch
  racing forks and explicitly release report locks before closing their owning
  descriptors. The isolated EOF regression passes 100/100 runs; all 198 default
  tests pass. See `benchmarks/process-reliability.md` for evidence and validation
  limits. Keep cache reuse opt-in.
- [x] Stabilize nested parallel Mutie self-testing before accepting new runtime
  or finding-quality evidence. The reliability follow-up passes 198 default
  tests; all 16 Rust workflows pass either in the suite or focused rechecks, but
  the full Rust suite has two intermittent timeout/checkpoint failures. The
  six-mutation self-run produced four timeouts and an unrelated kill of the
  previous survivor despite a clean baseline. Original capture/lock assertions
  did not recur; other cancellation, log-limit and terminal tests stalled or
  failed. A fixture loader stall is observed, not established as every failure's
  cause. Preserve deadlines, source isolation and diagnostic logs; see
  `benchmarks/process-reliability.md`.
  Unmutated controls now pass all 198 tests in one and two isolated Cargo workers;
  repeated cancellation suites also pass directly and through Mutie's subprocess
  runner. The fixture now records shim startup and includes checkpoint/output
  diagnostics on readiness timeouts. The complete Cargo adapter also passes all
  nine focused executions (63 cancellation-test passes) across fresh, cold-cache
  and warm-cache modes with one/two workers; local targets rebuild every time.
  No watcher trigger or artifact leak occurs. The subsequent paired survivor
  comparison passes all six focused executions, but the full pristine cold-cache
  control reproduces capture EOF. Pipe diagnostics establish cross-command
  descriptor inheritance after the original group is gone. Serialize spawning
  alongside the group retry; the combined fix passes 100/100 capture stress runs.
  A new exact-tolerance test kills the score survivor through its intended boundary
  assertion and passes before/after restoration. With both changes, all six
  two-worker full adapter controls pass 199 tests across fresh/cold/warm caches;
  source and caller artifacts are preserved and owned temporary storage is empty.
  The stronger test now lives outside score.rs to preserve strict rerun identity.
  A saved-ID rerun passes all 199 baseline tests and kills only the selected
  tolerance mutation through that assertion, with the 100% policy passing and
  source/report/caller artifacts preserved (256.60 seconds); see
  `benchmarks/rust-survivor-isolation.md`. The subsequent two-worker six-mutation
  comparison passes in both fresh and cached modes: six intended kills each,
  identical per-ID failing tests, no timeouts/build failures/errors, and preserved
  source/report/caller artifacts. Both baselines pass 199 tests; local targets
  rebuild and warm dependencies are reused. Whole runs take 686.16/608.64 seconds.
  This closes the reproduced blocker for this sample, not every possible
  scheduling issue. See `benchmarks/rust-parallel-score-reruns.md`. The acceptance
  review subsequently passes all 17 real-Cargo workflows together.
- [x] Complete the shared user workflow for Rust: filters/configuration,
  live/plain status, reports/browser, scores/policies, dismissals, and focused
  reruns. Record language and execution scope for correct rerun validation;
  preserve existing Go reports and reject unsupported language-specific options.
- [x] Add focused Rust reruns by saved IDs/operators/missed outcomes and browser
  command export. Validate Cargo scope, manifest/lockfile identity, toolchain and
  current source; preserve legacy report browsing and Go reruns. Verify previews,
  partial/empty runs, moved checkouts and a stronger-test fixture rerun.
- [x] Add Rust dismissal CRUD, fresh rediscovery, saved review and focused rerun
  matching. Preserve raw outcomes/scores and legacy Go decisions; distinguish
  language mismatches. Verify isolated metadata side effects, output limits,
  timeout/cancellation cleanup, stronger-test reruns and historical browser review.
  A focused Mutie self-run keeps its dismissed survivor in the raw score while
  passing the findings policy and failing the minimum-score policy. See
  `benchmarks/rust-dismissals.md`.
- [x] Verify fixture outcomes, failed/empty runs, interruption, source/artifact
  preservation, and stronger-test reruns, with Go regressions remaining green.
- [x] Validate a useful focused rerun on Mutie itself: reproduce the known score
  survivor in a temporary copy, restore its two regression tests, and kill only
  the saved survivor through `--rerun-missed`. Verify browser export, ID mapping,
  a passing fresh baseline, source/report/artifact preservation, and measured
  runtime. See `benchmarks/rust-focused-reruns.md`.
- [x] Exercise the complete workflow on the selected Rust project with ordinary
  CLI commands. Review survivors, verify a useful stronger-test rerun, and record
  time to an actionable finding, run/build/workspace costs, and noise/limitations.
  Saved-percentage presence/absence regressions now catch the first real survivor
  in `MutationScore::matches_saved` through fresh and focused runs. Dismissal
  review, policies, browser export and all three initial operators are verified.
- [x] Document verified Rust usage and limits in README/design and accept the
  initial milestone. The evidence review is in `benchmarks/rust-milestone-acceptance.md`.
  Choose shorter focused Rust feedback as the next objective: full-suite runtime
  remains the dominant observed cost after worker-local dependency reuse.

## Completed foundation

- [x] Build the Rust CLI with initial Go module support.
- [x] Run a baseline and stop on failed or timed-out tests.
- [x] Discover active Go files and simple Tree-sitter mutations.
- [x] Support include/exclude globs and mutation listing without running tests.
- [x] Run parallel workers in isolated copies with full workspace restoration.
- [x] Support configurable workers and baseline/mutation timeouts.
- [x] Distinguish killed, missed, timed-out, unbuildable, and tool-error results.
- [x] Save summaries, JSON results, diffs, and logs, including partial results.
- [x] Add unit tests and a Go fixture covering outcomes, isolation, and reports.
- [x] Document the MVP and longer-term direction in `DESIGN.md`.

## MVP milestones

### 1. Mutation inventory and reports — completed

- [x] Add named operators, source fingerprints, and operator catalog versioning.
- [x] Save the complete mutation inventory and diffs before workers start.
- [x] Separate run metadata, inventory, and outcomes using the design's report
  layout; add text lists grouped by outcome.
- [x] Represent pending/running mutations and interrupted runs explicitly.
- [x] Preserve active diagnostics and completed results during cancellation;
  coordinate worker shutdown and cleanup across run phases.
- [x] Test incomplete runs and the new report contract; update usage docs while
  preserving existing saved reports.

### 2. Repository configuration — completed

- [x] Load `mutie.toml` from the selected project root.
- [x] Merge defaults, repository config, and explicit CLI overrides in that order.
- [x] Define path resolution, list replacement, and CLI list-clearing behavior.
- [x] Validate unknown keys and invalid values; save the effective configuration.
- [x] Test precedence and defaults, and document working configuration examples.

### 3. Default terminal status view — completed

- [x] Introduce shared coordinator state/events for reports and presentation.
- [x] Choose Ratatui for a live, in-place status view inspired by the Vitest CLI.
- [x] Integrate Ratatui 0.30.1 with Crossterm 0.29 and an inline viewport.
- [x] Add automatic interactive/plain mode selection and an explicit override.
- [x] Show phase, elapsed time, progress, outcome counts, active workers, and
  recent results in the minimal interactive view; redraw in place and keep time
  and activity indicators updating while commands run, not just on completion.
- [x] Handle resizing, small terminals, cancellation, and terminal restoration;
  print a final plain summary and report path.
- [x] Test rendering and terminal lifecycle, including non-interactive use.

### 4. MVP validation — completed

- [x] Verify the complete workflow with config, parallel execution, both terminal
  modes, empty/failed/interrupted runs, saved artifacts, and source preservation.
- [x] Reconcile README examples and design status with the completed MVP.
- [x] Validate on `godotenv` v1.5.1: 77 mutations with identical outcomes in plain
  and interactive modes; verify four surviving mutations become test failures
  after adding tests to an isolated copy. See `DESIGN.md` for evidence and limits.
- [x] Fix narrow-terminal outcome clipping and use all four worker rows when
  space permits; verify the fixes against the real-project run and frame tests.

## Go follow-up history and deferred backlog

Checked items below are implemented. Unchecked items remain available for later
work; their order is not a priority queue and they do not block the Rust milestone.

- [x] Rerun completed missed mutations from a saved run with `--rerun-missed`;
  preserve the source report and record old-to-new ID mappings.
- [x] Rediscover and validate rerun targets against current source, reject stale
  or ambiguous matches, and require a fresh baseline.
- [x] Test rerun report validation, source matching, previews, empty selections,
  baseline failures, source preservation, and missed-to-killed results after test changes.
- [x] Refine the live Ratatui run view before adding saved-report browsing: keep
  stable rows ordered by worker number, showing each worker's state, current
  mutation, and elapsed time, including idle workers.
- [x] Add live worker progress events for preparing/building/testing/idle states;
  use "running" when the phase is unknown. Observe captured Go JSON output live
  for build/test evidence while preserving full logs and command cleanup.
- [x] Show a bounded results tail below the workers, newest completions at the
  bottom, with killed/missed/build failed/timeout/error labels. Redraw in place,
  retain progress and outcome totals, and keep the full history in saved reports.
- [x] Verify worker transitions, results ordering, resizing, and compact layouts
  for the refined live view.
- [x] Investigate and fix the preceding shell prompt disappearing after the live
  view closes. Distinguish content pushed into scrollback from content erased;
  audit inline viewport sizing, Ratatui's full-screen clear on horizontal shrink,
  and exit cleanup so pre-run terminal content is preserved.
- [x] Add terminal regression coverage with a pre-existing prompt and output;
  verify preservation through normal completion, cancellation, and resizing,
  including narrow/short terminals and worker views taller than the screen.
- [x] Extend focused reruns to individual saved IDs and operator selection.
- [x] Verify filter intersections, duplicate/invalid selectors, previews, empty
  selections, source validation, and report mappings; exercise focused reruns
  against the real-Go fixture with a fresh baseline.
- [x] Add interactive result filtering and diff/log browsing with `mutie report`;
  mark saved IDs and preview/export focused rerun commands without executing them.
- [x] Verify browser filtering, Unicode/paste input, bounded/missing artifacts,
  compact/wide frames, and shell restoration on quit/export/Ctrl-C/SIGTERM.
- [x] Fix the live-view scrollback race: queued frames used stale absolute rows
  after resizing and moved the cleanup cursor above restored history. Use relative
  frame/allocation movement and guard line wrapping; verify queued output through
  growth/shrink, completion, and cancellation with deterministic regression coverage.
- [x] Add persistent per-mutation dismissals with required reasons and optional
  notes in `mutie.dismissals.toml`; support add/list/update/remove CLI commands.
- [x] Match dismissals against rediscovered mutations using source-aware identity;
  surface stale, missing, or ambiguous matches with `dismiss check` for review
  after source changes.
- [x] Show dismissal reasons separately from actual execution outcomes in reports
  and result views; allow dismissed findings to be included in inspection and reruns.
  Capture portable review snapshots, add browser review filters and badges, and
  expose unresolved entries without applying them or changing outcome counts.
- [x] Test dismissal persistence, matching/invalidation, removal, and preservation
  of execution outcomes and counts.
- [x] Validate findings and practical runtime on a small real Go project.
- [x] Extend validation to `go-cmp` v0.7.0: all 1,450 mutations across ten packages,
  workspace/command timings, saved-report audits, and source preservation.
  See `benchmarks/go-cmp-v0.7.0.md` for evidence and limitations.
- [x] Repeat a fixed 32-mutation sample with one/four workers; investigate the
  killed/timeout variation with controlled diff-direction probes. Add isolated
  NaN comparison tests and verify a missed mutation becomes killed on a focused rerun.
- [x] Validate a fixed 32-mutation application sample on `fzf` v0.65.2 across all
  seven operators. Full/affected outcomes matched in four measured trials;
  affected scope averaged 16.5% less run time. Record workspace/temp/report sizes,
  review build failures, and prove two call-removal survivors become killed with
  isolated test additions. See `benchmarks/fzf-v0.65.2.md` for evidence and limits.
- [ ] Validate a large application or asset-heavy tree before drawing broader
  scaling conclusions. The measured go-cmp tree is 470 KB and fzf is 1.34 MB;
  the larger Hugo candidate could not pass its baseline on the validation host.
- [x] Record workspace timings alongside existing command durations. Count successful
  CoW clones and fallback copies/bytes for snapshot, baseline setup, discovery reset,
  worker setup, and worker reset; retain partial counts on failure/interruption and
  expose metrics in saved reports and the final summary.
- [x] Document workspace placement via the existing `TMPDIR` override, outside the
  source tree on a chosen filesystem, and record the resolved temporary root.
- [ ] Compare representative runs with workspaces on a reflink-capable filesystem
  versus `/tmp` on tmpfs. The go-cmp run verified explicit reflinks on APFS;
  ordinary-copy and Linux tmpfs measurements remain pending.
- [x] Own and clean up Go temporary build directories after timeout/interruption.
  The go-cmp full run left 27 `go-build*` directories (566 MB logical bytes) even
  though all Mutie workspaces were removed. Each Go command now gets a private
  `GOTMPDIR` below the caller's temporary parent; cleanup preserves unrelated
  files and the reusable Go build cache. Regression tests cover success,
  tool/startup errors, concurrent timeouts, and SIGINT/SIGTERM. A focused go-cmp
  timeout and interruption both left zero owned temporary build directories.
- [x] Own general test temporary directories per Go command, in addition to
  `GOTMPDIR`, and clean them after completion, timeout, and cancellation. Set child
  `TMPDIR`/`TMP`/`TEMP` without changing the caller's environment. Verify separate
  concurrent lifetimes, startup/tool errors, signals, relative parent placement,
  nested files, symlinks, and preservation of caller-owned files and caches.
  The real-Go fixture deliberately leaves temporary files before its timeout;
  baseline, mutation, and rerun cleanup checks pass. Repeated fzf full/affected
  samples left zero temporary files, with all 32 outcomes unchanged in each scope.
- [x] Add opt-in affected-package testing with a full-module baseline/default,
  transitive reverse dependencies and test imports, per-mutation saved selections,
  and fresh graph discovery on reruns. Verify against real multi-package tests.
  The controlled go-cmp sample matched all 32 outcomes in four paired runs;
  affected scope averaged 25.3% less run time (see benchmark notes/data).
- [ ] Evaluate coverage-based test selection and result reuse against full-module
  testing, with explicit invalidation rules. Keep affected-package comparisons
  against full-module results as projects and operators expand.
- [ ] Future exploration, not the next implementation task: investigate Tree-sitter
  structural fingerprints and Merkle dependency hashes for durable mutation
  identity and incremental result reuse. See
  [design notes](DESIGN.md#future-exploration-structural-identities-and-result-reuse)
  for dependency tracking, invalidation, and validation requirements.
- [x] Add standalone call removal with explicit statement eligibility and multiline
  diffs. Support operator selection for fresh runs, previews, and reruns through
  CLI/config, preserving dismissal matching and source identity. Verify real-Go
  outcomes and four missed-to-killed CLI gaps on `godotenv`; full/affected outcomes
  match. See `benchmarks/godotenv-v1.5.1-call-remove.md` for evidence and noise limits.
- [x] Add `branch_force_true` / `branch_force_false` for complete Go `if` and
  `else if` conditions. Use short-circuit wrappers to preserve name references
  without evaluating the original condition; preserve initializers and bodies,
  exclude bare boolean conditions, and retain independent nested operators.
  Verify multiline diffs, config/CLI selection, real-Go types and side effects,
  dismissals, source preservation, and stronger-test reruns. A fixed 24-mutation
  fzf sample matched full/affected outcomes with no build failures; added tests
  killed four survivors in both scopes. See `benchmarks/fzf-v0.65.2-branches.md`
  for useful findings and equivalent-mutation limits.
- [x] Add `loop_skip` for explicit conditions in single-condition and three-part
  Go `for` loops. Preserve initializer execution and name references; skip the
  original condition, body, and post statement. Exclude range/missing/bare-boolean
  conditions, retain nested discovery, and verify exact diffs, types, effects,
  config/CLI selection, reports, dismissals, and stronger-test reruns. A fixed
  16-mutation fzf sample matched full/affected outcomes without build failures;
  added tests killed two survivors in both scopes. See
  `benchmarks/fzf-v0.65.2-loops.md` for findings and limitations.
- [x] Add per-operator planned/completed counts, outcomes, unfinished states, and
  cumulative command time to saved summaries and the browser's `t` view. Derive
  totals from existing inventory/execution records so older reports work without
  rewriting them. Keep totals scoped to the saved selection across filters, marks,
  and dismissals; distinguish missing timings and unpublished/empty inventories.
  Verify aggregation, shuffled legacy records, saved-summary parity, wide/compact
  snapshots, navigation/resizing, and report preservation in a pseudo-terminal.
- [ ] Expand return and further loop/branch mutations and add named operator profiles based on
  useful findings; evaluate call-removal build-failure noise on more projects.
  The fzf sample added two actionable gaps and two unbuildable call removals;
  eight stratified removals do not establish a general noise rate.
- [x] Add explicit report listing and retention: status/age/logical size, count/age
  previews, and `--apply` deletion with writer locks and revalidation. Protect active
  and unfinished reports, unsupported metadata, symlinks, and unrelated contents.
- [x] Add opt-in per-artifact log limits with CLI/config overrides, head/tail
  excerpts, byte-count metadata, and browser notices. Keep complete event parsing
  and discovery independent of retention; preserve noisy outcomes, Unicode,
  partial diagnostics, reruns, and cleanup after timeout/interruption.
- [x] Add opt-in per-command raw capture limits with CLI/config/clear overrides.
  Bound combined stdout/stderr writes before they reach disk; stop the command
  group on overflow and retain diagnostics as a tool error. Preserve exact-limit
  success, discovery completeness, saved reports, policy precedence, and reruns;
  verify noisy streams, blocked observers, concurrent workers, signals, and cleanup.
- [ ] Evaluate archival/compression and run-wide disk/memory budgets. Per-command
  raw captures and per-artifact saved logs can be bounded; rendered logs, decoded
  Go records, workspace/test files, and caches have separate resource costs.
- [x] Add opt-in `--fail-on-missed` / repository configuration, with a local
  disable override. Evaluate current undismissed missed findings after successful
  execution, save the decision separately from execution status, and return exit
  `3` only after report persistence. Cover current/stale dismissals, focused/empty
  runs, listing, failures, cancellation, and terminal restoration.
- [x] Define raw mutation scores with explicit exclusions, unchanged dismissal
  counts, and selection scope. Add opt-in `--min-score` / config with exact
  comparisons, explicit empty/incomplete-run handling, and execution/persistence
  error precedence. Surface scores in summaries, saved JSON, and browser `m`;
  validate saved decisions and cover threshold boundaries, configuration, combined
  policies, filtered/focused runs, compact/wide frames, terminal restoration, and
  real-Go killed/missed/timeout/build-failure outcomes.
- [ ] Add integration/report exports for concrete consumers.

## Longer-term options

- [ ] Evaluate [proposed operator expansion](DESIGN.md#proposed-operator-expansion)
  against useful real-project gaps: comparison inversion/negation removal across
  languages, Rust/further Python arithmetic, bounded numeric/string literal changes,
  return-value replacement, and scoped statement/block removal.
- [x] Add scoped Python binary arithmetic with semantic controls, fixture
  discovery/report/rerun validation and unchanged betting-math-kit findings.
  Preserve original inputs/reports and classification rules; see
  [arithmetic acceptance](benchmarks/python-arithmetic.md).
- [x] Account for all 194 unchanged betting-math-kit arithmetic candidates,
  review survivors/errors and retain separate-copy stronger-test evidence with
  unchanged inputs, reports and classification rules; see the
  [complete arithmetic review](benchmarks/python-arithmetic-full.md).

- [ ] Prioritize the planned JavaScript/TypeScript and Python runner slices using
  concrete projects and the largest remaining obstacles to routine use; see
  [target support tracking](#target-support-tracking).
- [ ] Extend project support beyond the chosen Rust milestone scope to additional
  workspace/multi-module layouts and test frameworks as needed.
- [ ] Consider HTML and standard interchange exports for concrete consumers.
- [ ] Explore external language plugins, possibly using WASM, after multiple
  built-in languages establish the interface. Design the protocol, host controls,
  packaging, and versioning before choosing a runtime.
