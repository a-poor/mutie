# JavaScript target: Hyparquet

Date: 2026-10-15. This initial audit established discovery and an isolated
baseline while JavaScript was Experimental. Subsequent [findings and acceptance](vitest-hyparquet-findings.md)
complete the scoped JavaScript workflow; the baseline alone did not establish it.

## Candidate and preparation

Selected [Hyparquet](https://github.com/hyparam/hyparquet/tree/49c88a44fa04c10905821e9a6a7b7a08bf091557),
commit `49c88a44fa04c10905821e9a6a7b7a08bf091557` (1.31.4): an ESM JavaScript
Parquet library with 31 test files and no runtime dependencies. Its declaration
file is excluded from mutation discovery. GeoParquet at
`f6794975547dcda4843e206101bb59936c15785b` was also inspected; its single production
boolean literal offered less opportunity to assess finding usefulness.

Pristine checkout: `/tmp/mutie-hyparquet-audit`.
Prepared copy: `/tmp/mutie-hyparquet-scope`.
Only package preparation changed: added `packageManager: pnpm@11.10.0`, pinned
Vitest and its coverage-v8 package from 5.0.4 to 4.1.9, added Vite 8.3.4,
and generated a pnpm lockfile/install. Other dependencies were retained.
Installation used `--ignore-scripts`; no declaration build was needed. Source,
tests and the original root tsconfig were retained. This validates the prepared
pinned copy, not upstream's Vitest 5 runner.

Tools: Node 24.22.0, pnpm 11.10.0, Vitest 4.1.9, Vite 8.3.4, macOS arm64.

## Config blocker and scoped change

Hyparquet's plain-JSON tsconfig uses `allowJs: true`, `checkJs: true`,
`noEmit: true` and `module: "nodenext"`. Mutie previously rejected this before
discovery. The adapter now accepts NodeNext only with those three flags enabled
and a bounded, cancellable whole-project scan proving there are no runtime
`.ts`, `.tsx`, `.mts` or `.cts` files. Declaration files are allowed.
`node_modules`, `.git` and `.mutie` directories are excluded; project symlinks
are rejected conservatively. Even a static `.ts` runner config is excluded from
this JavaScript-only allowance. Existing TypeScript support still requires
ESNext. `allowJs` and `checkJs` must be booleans. Other existing config restrictions
remain in force, and the original tsconfig bytes remain part of execution and
rerun identity.

Unit regressions cover acceptance/fingerprinting, source/test/helper TypeScript
rejection, declaration allowance, required flag values, symlinks and cancellation.

## Observed results

- Direct pinned Vitest baseline: 724 passed, zero failed/skipped.
- `mutie vitest-list /tmp/mutie-hyparquet-scope`: 133 boolean flips.
- `mutie vitest-check /tmp/mutie-hyparquet-scope --timeout 120`:
  724 passed, zero skipped/todo, 14.86 seconds command time. This excludes project
  validation/copy overhead and is not an end-to-end feedback-time measurement.
- No source or test adaptation was needed for either baseline. After validation,
  SHA-256 comparison confirmed all 292 original source/test/root-config files
  still matched the pristine checkout.

Local diagnostics: `/tmp/mutie-hyparquet-baseline.json`,
`/tmp/mutie-hyparquet-preview.log`, and
`/tmp/mutie-hyparquet-check.log`. Temporary paths are evidence from this
session, not committed fixtures.

## Subsequent evidence

The [findings review](vitest-hyparquet-findings.md) records scored samples, survivor
usefulness, stronger-test reruns, end-to-end time and preservation. Remaining
work at that point included feedback-time investigation and broader mutation
coverage. Subsequent [Node reuse](vitest-node-reuse.md) and the
[full-inventory review](vitest-hyparquet-full.md) record those follow-ups.
