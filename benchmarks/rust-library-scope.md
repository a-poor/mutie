# First Rust library-package scope

## Decision

The next adapter slice will support one ordinary library package, either alone
or with one binary. Keep the existing binary-only workflow. This is a scoped
implementation objective, not a change to current CLI support.

Continue to run default-feature unit and integration tests with
`cargo test --locked --tests --no-fail-fast`, with compilation separated using
`--no-run`. Discover mutations from every production target root, including the
binary in a library-plus-binary package. Integration tests are execution targets,
not mutation roots. A library linked by a binary must be rebuilt after mutation.

Doctests remain explicitly excluded: Cargo's `--tests` selects test-enabled
library/binary targets and integration tests, while documentation tests use a
separate selection. Do not describe this as all possible Cargo tests. See
[Cargo test target selection](https://doc.rust-lang.org/cargo/commands/cargo-test.html#target-selection)
and [Cargo targets](https://doc.rust-lang.org/cargo/reference/cargo-targets.html).

Accept ordinary `lib`/`rlib` outputs with the standard harness, including explicit
target names and source paths inside the package. Continue rejecting proc macros,
other library crate types, multiple binaries, custom harnesses, disabled target
tests, required target features, examples/benches, build scripts, semantic Cargo
configuration overrides and cross-compilation. Existing feature, dependency,
source-path and artifact exclusions still apply. Multi-package/enclosing
workspaces and local/git dependencies remain separate scope decisions; an
explicit single-package workspace stays allowed under the current rules.

## Evidence collected on 2026-10-14

Host: macOS arm64, Rust/Cargo 1.99.0. Raw logs and source hashes are in
`/tmp/mutie-library-scope-3uttyu4n` (local, temporary evidence).

The independent `tests/fixtures/rust-library` package has no dependencies and
contains a library, one binary linking that library, and API/CLI integration
tests. In isolated copies, direct Cargo commands produced:

| Layout | Baseline | Flip `enabled` | Flip `preview_enabled` | Stronger baseline | Stronger preview mutation |
| --- | --- | --- | --- | --- | --- |
| Library + binary | 3 pass | 3 fail | 3 pass | 4 pass | 1 fail, 3 pass |
| Library only | 2 pass | 2 fail | 2 pass | 3 pass | 1 fail, 2 pass |

The stronger test asserts that `preview_enabled()` is false. Each case restores
the original library before applying its change, uses `--locked --offline --tests
--no-fail-fast`, and retains the copy's Cargo target directory between cases.
Failures return 101; passing cases return 0. This establishes a small test-improvement
scenario and ordinary Cargo invalidation, not Mutie's isolation or cache contract.
The library-only copy removes `src/main.rs` and `tests/cli.rs`.

Marathon is the additional real-project candidate, revision
`6d16015fc9747a8b84e9cfd286b230ce091f1a7f` (clean checkout). Mutie remains the
binary-only control and the first real Rust validation project. Marathon has one
2024 package with registry dependencies, no package build script, and default
`src/lib.rs` plus `src/main.rs`. Locked offline Cargo metadata in a copy confirms
one ordinary library, one binary, and four integration-test targets, all with
standard tests enabled. The binary imports the library's public modules.

A fresh artifact directory and a copy of tracked files were used for
`cargo test --locked --offline --tests --no-fail-fast`: 146 library unit tests,
zero binary unit tests, and 36 integration tests passed (30 + 4 + 1 + 1).
Compilation reported 15.07 seconds; this is one baseline, not a mutation runtime
estimate. Original tracked-file SHA-256 hashes remained unchanged. Cargo commands
ran only in the copy with a target directory under the evidence root; the original
checkout's artifacts were not used. No Mutie mutation run on Marathon has yet
been attempted, and its discovery syntax/exclusions still need adapter validation.

## Implementation and acceptance gates

1. Generalize `rust::project::ExecutionScope` from a single `binary`/`source` to
   a deterministic production-target inventory (kind, name, source and edition).
   Validate manifest settings against Cargo metadata, including automatic versus
   explicit target discovery, crate types, harness flags and paths. Reject target
   additions/removals or changed identities on rerun. Normalize legacy binary
   report identities when reading; retain existing strict manifest/lock/toolchain
   checks without a schema version bump. Update saved summary target descriptions.
2. Traverse all production roots with shared visited-file tracking and stable
   deduplication. Cover library-only, combined targets, custom names/paths, shared
   modules, test exclusions and unsupported layouts with regression tests. Keep
   source identities stable for unchanged binary-only runs.
3. Exercise both fixture layouts through Mutie's complete fresh and worker-cache
   commands, reports, score policy and focused reruns. Verify the stronger-test
   scenario, restoration, source/caller-artifact preservation, local library and
   linked binary rebuilds, and no stale dependency reuse. Current outcome
   classification uses build-finished/compiler diagnostics and harness summaries;
   it does not assume every compiled artifact has an executable. Confirm that
   behavior with library build failures and test failures rather than changing
   classification speculatively.
4. Run discovery and a bounded, useful mutation selection on isolated Marathon,
   inspect findings, improve a test, and rerun the saved finding. Record runtime,
   exclusions and preservation evidence; keep Mutie's existing binary regression
   and real-Go workflows covered. Only then update implemented scope in README,
   SUPPORT and CLI claims. Real Cargo integration coverage stays opt-in to avoid
   recursive mutation self-tests.

The next coding task is target inventory and validation, followed by multi-root
discovery. These gates deliberately do not expand to workspaces or claim that
ordinary Cargo baselines alone validate the mutation adapter.

## Implementation progress

Saved target inventory is implemented with legacy binary-report normalization,
strict target-change rerun checks, and per-target saved-summary details. The
current binary metadata check also verifies edition and crate type; explicit
binary edition overrides are recorded. Unit regressions cover legacy migration,
missing evidence, target changes, invalid inventories and metadata mismatches.
The opt-in rerun workflow includes executing a focused selection from an
older-format report and publishing it with the new inventory.

Validation: `cargo test` passes 206 tests (27 opt-in tests ignored),
`cargo test --test rust_workflow -- --ignored --test-threads=1` passes all 17
real Cargo workflows, and formatting plus Clippy with warnings denied pass.
All five opt-in real Go workflows also pass with access to the Go build cache
(the initial sandboxed attempt could not write that cache).

This is the report/identity foundation only. Manifest and metadata acceptance of
library packages and multi-root discovery remain the next implementation step;
the CLI still rejects libraries before execution.
