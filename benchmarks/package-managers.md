# JS/TS package-manager compatibility audit

Goal started 2026-10-22. `mutie init` was committed as `8597084` before this work.
The previous committed support contract was the original pinned pnpm installation.
Chronological failed probes below remain part of the record. Later acceptance
results distinguish installation formats from runner and public-project evidence.

## Boundaries found

Vitest project validation assumes pnpm lockfile v9, `.modules.yaml`, an isolated
virtual store and matching installed lock. Saved execution identity also contains
an exact `pnpm` version. Lifecycle instrumentation locates an exact-hash worker
through a hardcoded pnpm store path. Vite lookup likewise assumes a sibling in
that store. Installation identity and dependency resolution must be separated
from the unchanged test-runner attribution recipe.

Node/Bun initially had dependency-free executable import scopes and rejected node_modules;
package-manager compatibility must not silently broaden imported dependencies.
Manager metadata alone does not choose the runner. Workspaces and external-store
loaders need explicit acceptance rather than following links outside snapshots.

## Initial local audit

Disposable artifacts: `/tmp/mutie-package-managers-audit`.
Direct installed tools report Node 24.22.0, npm 11.19.1, Yarn 4.18.1 and pinned Bun
1.3.11. The host's Vite+ shims can choose different manager versions; probes use
explicit existing executables and separate temporary caches, with no global updates.

A temporary npm fixture installed Vitest 4.1.9 and Vite 8.1.3 with scripts disabled.
It generated lockfile version 3 (76 package records) and installed 45 packages.
The hoisted `@vitest/runner/dist/chunk-artifact.js` hashes to
`c10a7a80fca65fb42e27eabf7655c5ce5ecf36e8aaeb1811499efd063df62556`, matching the
existing audited worker. This is installation evidence, not scored acceptance.

## Official documentation audit

- [npm lockfiles](https://docs.npmjs.com/cli/v11/configuring-npm/package-lock-json/):
  audit root and hidden installed lock metadata separately.
- [Yarn install modes](https://yarnpkg.com/features/linkers): node-modules, pnpm
  and PnP are distinct supported Yarn layouts. PnP uses a loader/global-store
  model and requires separate isolation evidence; it is not equivalent to hoisting.
- [Bun installation](https://bun.sh/docs/pm/cli/install) and
  [isolated installs](https://bun.sh/docs/pm/isolated-installs): distinguish hoisted
  and isolated installations; audit actual pinned-version output.
- [Vite+ tests](https://viteplus.dev/guide/test): current docs describe bundled
  Vitest 5.0.5, resolved ahead of a project's own Vitest, and `vite-plus/test`
  imports. `vp test` is distinct from `vp run test`. This exceeds Mutie's current
  exact Vitest 4.1.9 instrumentation; do not claim runner support merely because
  Vite+ can delegate package installation. A new runner audit would be required.

## Remaining acceptance gates

Manager-specific lock/manifest consistency, installed version and path validation;
installation identity in snapshots/reports/reruns; malformed/drift rejection;
independent JS/TS native execution and lifecycle controls; unchanged public-project
findings and stronger-test reruns; init diagnostics and synchronized support docs.
The Vite+ audit must distinguish package-manager delegation from bundled testing.

## First implementation checkpoint

Dependency resolution now follows owned node_modules ancestry instead of a fixed
pnpm store path; exact worker-byte instrumentation is unchanged. Hoisted/nearest
and isolated-link/escape unit controls pass. The original 15 non-ignored Vitest
unit tests passed after the resolution change. The pnpm installation validator is
extracted, and npm v3 lock/installed-lock validation plus optional manager identity
are under implementation. These changes are not yet release acceptance.

Yarn 4.18.1 node-modules and Bun 1.3.11 fixtures also install with scripts disabled;
all three worker files have the same audited hash. Native tests with a 40-second
bound produced no output and timed out for all three installations outside the
sandbox; npm also timed out inside it. These remain failed probes, not passing
baseline evidence. The retained `native.py` uses process-group cleanup and saves
per-manager logs/results. The cause is not established; do not infer that the
previous host startup issue explains these failures without diagnostics.

## Installation-validation checkpoint

Implementation now separates installer formats into `src/vitest/installation.rs`.
The audited selectors are pnpm 11.10.0 (existing isolated layout), npm 11.19.1
(lock v3), Yarn 4.18.1 (lock v10, node-modules/classic) and Bun 1.3.11 (text lock
v1/config v1, hoisted). PnP, Yarn's pnpm linker and Bun's isolated linker remain
explicitly rejected pending their own acceptance. Mixed locks and configuration
are rejected. No installation tool is run by Mutie.

Yarn's generated lock includes conditionally installed platform packages without
checksums and its built-in fsevents 2.3.3 patch. That exact built-in locator is
recognized; arbitrary patches remain rejected. Installed versions and locations
are checked against lock/state records. npm root/hidden lock consistency and Bun
root manifest/direct package consistency have dedicated checks.

Installed dependency files, links and directories now receive a bounded content
fingerprint (512 MiB, 200,000 entries), excluding the same three owned cache
locations omitted during snapshotting. This joins execution/rerun identity and
snapshot comparison. The validator source also joins the recipe hash. Legacy
pnpm report fields remain readable; current runs carry the additional fingerprint.
No report/schema/operator version was bumped.

Validation observed in this checkpoint:

- Prepared npm/Yarn/Bun metadata loads all pass through production project loading.
- 21 non-ignored Vitest unit tests pass; seven native/measurement tests remain opt-in.
- All-target Clippy and formatting pass.
- The npm native retry passes both JS/TS files (four tests); Yarn/Bun retry probes
  still time out at the original 40 seconds without output.
- A production npm adapter outcome regression reaches its baseline assertion but
  returns Timeout instead of Missed at its unchanged 20-second deadline. It fails.
- One metadata-test startup was sampled at `_dyld_start`, then eventually ran;
  this establishes that probe's pre-main delay, not the cause of every Node timeout.
- A production CLI probe exceeded its 45-second outer bound without output; it
  was terminated and logged. It is not passing scored evidence.

At that checkpoint the goal remained open. Required next work included native scored/lifecycle and
stronger-test acceptance for each manager, pnpm regression acceptance, useful
unchanged public-project evidence, init/documentation integration and the
Node/Bun package-manager boundary (including development-only installations).
Do not infer completion from successful static metadata checks.

## Development-installation acceptance

Node and Bun retain their runtime-import scope while allowing declared development
installations. Complete owned dependency trees are copied, bounded and fingerprinted;
PnP entry points and external symlinks are rejected. Lock/config bytes join identity,
including settings-only pnpm metadata. No installer is launched by Mutie. Tests that
read installed files see those files in the snapshot; omitting node_modules would
have changed observable behavior and is not the implementation.

The retained `fixtures/package-managers/run_development.py` ran **16 independent
cases**: npm/pnpm/Yarn/Bun × Node/Bun × JavaScript/TypeScript. Every case passed:
Node fixtures produced two kills and one survivor; Bun fixtures produced one kill
and one survivor. Separate stronger copies killed each survivor, with zero errors
or timeouts. Every original project and original report file hash was preserved.
All 16 subsequent content-only installation drift controls rejected focused reruns
at the snapshot phase, before baseline or mutation execution, with unavailable
scores. The CLI saves a failed diagnostic report for this rejection; an initial
harness assertion incorrectly expected no report and was corrected after inspecting
the zero-operation report. The failed attempt and original artifacts remain intact.
[Machine-readable results](package-managers-results.json) retain these outcomes.

Fixtures use installer-generated metadata and deliberately adapted manifests;
they are not unchanged public-project evidence. They cover pnpm's local isolated
store and npm/Yarn/Bun hoisting. No PnP, workspaces, Yarn pnpm-linker, Bun isolated
linker, runtime dependency imports or arbitrary reflective loading is claimed.

The production Vitest adapter's `real_fixture_outcomes_and_failure_evidence` passes
for all four managers (npm 31.44s, pnpm 37.60s, Yarn 42.60s, Bun 33.70s). These
exercise both syntaxes, baseline, all six fixture mutations, stronger tests,
startup failures and original input preservation without changing deadlines.
The scored CLI/report/stronger-test/zero-exit-failure regression passes for npm
(47.77s), Yarn (84.83s) and Bun (98.57s); further controls are recorded below.
The default `cargo test` suite passes **302 tests**, with **105 ignored**, no failures;
all six init integration tests also pass independently. Formatting and diff checks
pass. Earlier native timeouts remain failed probes and do not establish a host fix.

## Final scored, lifecycle and unchanged-project acceptance

The pnpm scored CLI regression also passes (54.77s). The five-case
`lifecycle_mutation_failures_do_not_improve_scores` regression passes for every
manager: npm 27.88s, Yarn 36.65s, Bun 24.99s, pnpm 27.78s. Fixture setup,
`onTestFinished`, and around-hook teardown failures remain errors; successful
fixture/hook lifecycles accompanying body assertions remain kills. Original worker
bytes and source are preserved. No classifier, assertion or readiness bound was
relaxed. All-target Clippy passes with warnings denied.

Unchanged [Bragkit](https://github.com/artmsilva/bragkit), commit
`458d31127f1d3a6f3fd28afd2d7a4741ebc648a7`, was copied from its clean original
checkout and installed with npm 11.19.1 `ci --ignore-scripts --no-audit --no-fund`
using its original lockfile. All **57 original input files** remain byte-identical,
including source, tests, scripts, manifest and lock. Its development installation
includes TypeScript, Node types and Vite+; its original `node --test` script selects
the Node adapter. Nothing executes the Vite+ runner, build or prepare scripts.

The complete 23-candidate run takes **206.76s**, producing **2 kills, 21 survivors,
no errors/timeouts**, matching the earlier dependency-free installation state.
A separate copy adds the retained `node-bragkit/mutie-html.test.ts`. A focused rerun
of original IDs **20, 21, 22** takes **27.60s** and kills all three HTML-list boundary
survivors. Its 100% score is only for that selection; the original 8.70% score is
unchanged. All **105 original report files** and all 57 original files in both
copies retain their hashes. Exact identities, scores, paths and hashes are in
[the results record](package-managers-results.json).

This closes the scoped installation goal, with macOS native evidence. It does not
claim Linux execution was performed here, new runner versions, arbitrary package
manager versions, runtime imports for Node/Bun, PnP/workspace support, or an
unchanged public Vitest project independently installed through every manager.
Vitest's manager evidence is the controlled mixed-JS/TS fixture plus its existing
real-project runner evidence; unchanged installation evidence is Bragkit/Node.
`vp test` remains unsupported because its bundled Vitest version/imports differ
from the exact audited observer. Vite+ choosing a supported installer/runtime can
coexist with an independently supported runner. This distinction is enforced in
`mutie init` diagnostics and documented in README/SUPPORT.
