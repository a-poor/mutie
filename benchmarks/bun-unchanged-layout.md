# Bun unchanged pp-is package layout

Recorded 2026-10-20. This follows the explicit adaptation in
[JavaScript acceptance](bun-javascript-pp-is.md), using the same pp-is commit
`1d5b3bac804e1e67da289f3d2a14bd0705a383fb` and pinned Bun 1.3.11+af24e281e.

## Narrow layout extension

The adapter now accepts packages whose `type` field is absent only when every
runtime file is JavaScript with a top-level static import/export statement.
CommonJS identifiers (`require`, `module`, `exports`, `__dirname`, `__filename`)
are rejected in this mode. Explicit `type: commonjs`, unknown/null types, plain
scripts and runtime TypeScript without explicit `type: module` remain unsupported.
This is not general CommonJS support or a general-purpose module-mode detector.

Root and `dist/` `.d.ts` files are opaque packaging inputs. JavaScript packaging
artifacts under `dist/`, and ancillary `.mjs`/`.cjs` files under the existing
examples/scripts directories, can be present without being added to runtime
import resolution or mutation discovery. Every byte is still fingerprinted and
copied; the adapter does not execute build scripts or package entry points.
Tests continue to import audited source/test `.js`/`.ts` files only. Dependencies,
self-package imports, directory imports and extensionless resolution are unchanged.

Test-like names in packaging directories are rejected, including `.test.mjs`,
`.spec.cjs`, `_test.ts` and `.test.d.ts`; packaging cannot hide native test inputs.
Declaration files in runtime directories, `.mjs`/`.cjs` runtime files, nested
package manifests, custom Bun configuration, aliases and symlinks remain rejected.
The existing original/snapshot input comparison still includes packaging bytes.

Three new unit regressions cover accepted/fingerprinted packaging, import
rejection, hidden tests, runtime extensions, implicit module syntax, CommonJS and
implicit TypeScript rejection. Existing Node/Vitest discovery is unchanged.
The project-validator bytes already join execution identity, so this change
invalidates old runner recipes without changing schema/catalog/package versions.

## Unchanged real-project controls

The original checkout is used directly: no added `type`, no removed packaging,
no rewritten sources/tests and no installed dependencies. Its preview matches all
22 adapted mutation records exactly, including paths, locations, source hashes,
original/replacement bytes and IDs, with the same three test files. Generated
bundle booleans are not mutation targets because audited production scope is `src/`.

A separate full copy retains every original file and adds only the existing
[seven stronger tests](fixtures/bun-pp-is/stronger.test.js). It reruns the source
report's ten survivors under the same new execution recipe. Historic adapted
reports are preserved, not reinterpreted or combined into a new full score.

Scope remains serial, dependency-free, full-suite boolean flips on the pinned
runtime. Native declarations, general generated-source discovery, custom builds,
package exports resolution and CommonJS execution are not established by this
packaging acceptance. The three remaining pp-is findings retain their earlier
redundancy/contract limits.

## Results and preservation

The unchanged 58-test baseline passes. All 22 candidates match adapted outcomes:
12 kills, ten survivors, zero errors/timeouts, 54.55% full score in 248.17 seconds.
The 65-test stronger baseline passes, and the ten-survivor rerun reproduces seven
kills and three survivors, zero errors/timeouts, 70% selected score in 107.94
seconds. Timings are debug-build observations, with the full run overlapping the
default regression suite; they are not a controlled speed comparison.

Both complete mutation inventories and all per-ID outcomes match the adapted
full/rerun records. All 63 original files match their pre-audit hashes, including
manifest and packaging; the stronger copy differs only by its added test file.
The unchanged source report and historical adapted source report retain every
file hash. [Machine-readable evidence](bun-unchanged-layout-results.json) contains
identities, reports, baseline logs and preservation hashes.

`cargo test --offline`, the three new scope regressions,
`cargo clippy --offline --all-targets -- -D warnings`, `cargo fmt --check` and
`git diff --check` pass. Full and focused real Bun runs above pass. JavaScript
remains Validated, now with unchanged-checkout acceptance within this narrow
layout. Next work should reassess feedback cost or unresolved useful findings;
there is no need to broaden operators or package scope merely to clear a backlog.
