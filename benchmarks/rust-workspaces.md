# Ordinary Rust workspaces

Recorded 2026-10-22 on macOS. Implements the workspace goal after
`41b24db` (JS/TS package-manager compatibility). No schema/operator/package version
was changed.

## Selection and execution contract

Run Mutie from a workspace root. Virtual manifests and roots containing a package
are accepted. `--rust-package NAME` is repeatable; `rust_packages = ["name"]` is
the TOML equivalent, and `--clear-rust-packages` clears configured selection.
Omitting selection mutates all supported members. Explicit selection is a filtered
score. Source include/exclude globs remain relative to the workspace root.

Member selection controls mutation discovery only. Baseline and mutation commands
both use `cargo test --locked --workspace --tests --no-fail-fast
--message-format=json` (first with `--no-run`). They test every workspace member,
including dependents and members omitted by Cargo's `default-members`. Default
features are enabled; doctests and ignored tests remain outside the existing scope.
See Cargo's [workspace](https://doc.rust-lang.org/cargo/reference/workspaces.html)
and [test-selection](https://doc.rust-lang.org/cargo/commands/cargo-test.html#target-selection)
contracts. Running from a member directory gives a root/selection diagnostic.

Static inventory supports member globs/exclusions, automatically included internal
path dependencies, inherited package metadata/dependencies, and workspace lints.
It rejects external/absolute path dependencies, nested workspaces, git dependencies,
and dependency overrides. Cargo metadata in an owned copy must agree with every
member ID, manifest, ordinary target and edition. Discovery may enter only selected
member package directories; unrelated nested packages remain excluded.

Each member retains the existing ordinary library/one-binary target contract.
Workspace build scripts and proc-macro packages remain unsupported (registry
build-time dependencies retain the previous Cargo behavior). Custom dev/test
profiles, feature-gated test targets, custom test harnesses and cross-compilation
remain unsupported. Release/bench-only profiles are retained and fingerprinted:
Cargo's [test profile inherits dev](https://doc.rust-lang.org/cargo/reference/profiles.html#test),
not those profiles. Examples/benchmarks with `test = false` remain copied but are
not tested or mutated; enabling them as tests is rejected. A fixture with
uncompilable auxiliary targets demonstrates the exclusion under the actual command.

## Identity, caches and regressions

Saved identity includes every member's manifest fingerprint, directory, production
targets, the selected names, root manifest, shared lockfile and toolchain. Capture
must match the original identity. Changed member metadata or selection invalidates
focused reruns; stronger tests are permitted. Browser-exported commands retain
member selectors. Existing standalone packages with an empty `[workspace]` retain
their prior report identity and commands.

Every baseline uses fresh owned artifacts. Worker mode cleans **all local workspace
packages** before each mutation, preserving only external dependency artifacts.
This prevents unselected dependent packages or libraries from reusing stale code.

`tests/fixtures/rust-workspace` has inherited edition/version/license, a globbed
virtual root, and an internal dependency. `default-members` includes only the core
library, while consumer integration tests detect its mutation. The native workflow
finds three candidates (one kill, two survivors) with matching fresh/worker results,
checks both local libraries rebuilt, and kills both survivors in a separate stronger
copy. Source/lock/manifests, caller artifact directories and original reports remain
unchanged; owned temporary artifacts are removed. Member-manifest drift is rejected.
Unit controls cover root packages, implicit members, unknown/duplicate selections,
path escapes, malformed saved identities and configuration precedence. Existing
library-only and library-plus-binary fresh/cached/stronger-rerun acceptance also passes.

## Unchanged public workspace

[UCD Generate](https://github.com/BurntSushi/ucd-generate), commit
`e8fa937f0cac643669dcaf5edac2785b15cab917`, provides a root binary and three library
members with internal path dependencies and a checked-in lockfile. Original source,
tests, manifests, profiles, benchmarks and lockfile are unchanged. Dependencies were
fetched with `cargo fetch --locked`; no project files were adapted for acceptance.
Mutation selection is `ucd-util`; all four members are tested.

An exploratory 30-candidate run stopped at candidate 14: the ASCII-range logical
swap violates the library's UTF-8 safety invariant and triggers an abort. It retains
8 kills, 5 survivors, 1 error and 16 unattempted candidates, with **no score**.
The error was not reclassified, and the original failed report is preserved.
That early report predates the explicit-member score-label correction; its
`all_discovered` label does not mean all workspace code was selected. The current
strict reader rejects that pre-fix scope label; it is retained as raw diagnostic
evidence, not used as the stronger-test rerun source. The completed filtered
boolean reports remain readable and are the rerun source.

A separate boolean-flip selection completes with **four survivors**, no errors or
timeouts, in **40.66s** with worker reuse and **61.90s** fresh. These were separate
runs under different concurrent host load; they establish outcome agreement, not
an isolated performance benchmark. The 0% score applies only to that selection.

A separate unchanged copy adds
[`normalization_boundaries.rs`](fixtures/rust-workspace-ucd/normalization_boundaries.rs).
Two tests exercise non-medial leading/trailing hyphens and one-character symbolic
names versus the explicit `IsC` prefix. The focused rerun of boolean-report IDs
**1, 2, 4** kills all three in **54.90s**, with no errors/timeouts. The fourth
`next_e` default-value survivor remains unresolved; no equivalence or dismissal is
claimed. The 100% focused score does not replace either original report's score.

[Machine-readable evidence](rust-workspaces-results.json) retains source identities,
input/report hashes, outcomes, toolchains, selection and paths. The stronger copy
changes only its additional test file. This is macOS evidence, not a Linux run or
support for every Cargo workspace, build script, proc macro, profile or feature set.

## Acceptance results

All **108 tracked original inputs** match in both checkouts. All **120 files** in
the original exploratory and completed boolean reports match their captured hashes.
Original full-workspace baselines pass **140 tests** (3/84/40/13 by member); the
stronger copy passes **142**, including its two new integration tests.

The existing library-only/library-plus-binary regression passes (83.75s). After
adding explicit member score filtering, the native workspace rerun exposed a
reader/config mismatch; the reader now includes `rust_packages` when reconstructing
scope. The final native workflow passes (35.91s), including saved-score validation,
stronger-test reruns, drift rejection and temporary-artifact cleanup. The default
suite passes 306 tests (106 ignored), and all-target Clippy passes with warnings
denied. The final full-suite rerun after the reader fix also passes: **306 passed,
0 failed, 106 ignored**. Formatting and diff checks pass.

To reproduce on a fresh unchanged checkout with the locked dependencies available:

```sh
mutie /path/to/ucd-generate --lang rust --rust-package ucd-util \
  --operator boolean_flip --rust-build-cache worker --timeout 60 \
  --baseline-timeout 120 --ui plain --output-dir /path/to/original-reports
# Repeat with --rust-build-cache fresh to compare original outcomes.
# In a separate copy, add the retained test to ucd-util/tests/.
mutie /path/to/stronger --lang rust --rust-package ucd-util \
  --rerun /path/to/original-reports/RUN --id 1,2,4 \
  --rust-build-cache worker --timeout 60 --baseline-timeout 120 \
  --ui plain --output-dir /path/to/stronger-reports
```
