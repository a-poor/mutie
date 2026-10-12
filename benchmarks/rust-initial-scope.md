# Initial Rust scope audit — 2026-10-11

This is implementation-planning evidence, not a Rust mutation-testing benchmark.
It records the state before the Rust execution slice was implemented. The
maintainer selected **Mutie itself** as the first real Rust project; the companion fixture is
[`tests/fixtures/rust`](../tests/fixtures/rust/README.md).
The agreed first slice is documented in the
[design contract](../DESIGN.md#initial-rust-support-contract).
See [first-slice validation](rust-first-slice.md) for subsequent CLI evidence.

## Inspected project

Mutie revision `8d6e515` (`Documentation tweaks`) had a clean working tree before
this task. The audit used Rust/Cargo 1.99.0 on macOS arm64. No dependency or package
versions were changed.

| Requirement | Mutie | Fixture |
| --- | --- | --- |
| Package layout | Single package, Rust 2024 | Independent single-package workspace, Rust 2024 |
| Production targets | Binary `mutie`, `src/main.rs` | Binary `mutie-rust-fixture`, `src/main.rs` |
| Integration targets | 11, listed below | `cli` |
| Package features | None | Non-default `alternate`; baseline uses defaults |
| Lockfile | Checked-in `Cargo.lock` | Checked-in, no dependencies |
| Local dependencies/overrides | None | None |
| Root build script | None | None |
| Library/doctests/examples/benches | None | None |
| Project Cargo config/toolchain file | None | None |
| Test harness | Standard, with async tests and subprocess/PTY integrations | Standard unit and subprocess integration tests |

The integration targets are `affected_packages`, `cancellation`, `capture_limits`,
`configuration`, `dismissals`, `go_workflow`, `log_limits`, `report_browser`,
`report_retention`, `rerun`, and `terminal`. Most are Unix-gated. Six opt-in tests
require real Go builds and remain ignored in the chosen Cargo test scope. The
default suite uses fake Go commands for most execution tests; passing it does not
replace the separate real-Go regression runs.

Mutie requires registry dependencies, including native build steps for
Tree-sitter and derive/attribute macros. Its own source uses `cfg(test)`, Unix/
platform conditions, `#[tokio::main]`, and `#[tokio::test]`. Tests locate the built
CLI through `CARGO_BIN_EXE_mutie` and read fixtures/snapshots through
`CARGO_MANIFEST_DIR`. Therefore the whole source/assets snapshot and Cargo-managed
test environment matter; a hand-picked unit-test executable is not sufficient.
There is no root `build.rs`, `include!` source generation, or local dependency
layout to support for the initial project.

`cargo metadata --no-deps --format-version 1 --locked --offline` verified package,
workspace and target information for both roots. For the input checkout Cargo
resolved final and intermediate artifacts to `<project>/target`. Those paths must
be excluded and replaced with owned paths during future Rust mutation runs.

## Baseline in an isolated copy

Copied the current source tree into a fresh temporary directory, excluding Git
metadata, target directories and `.mutie` reports. The copy included the new Rust
fixture, which is not a target of the root package. With an empty owned artifact
directory and already downloaded dependencies, ran from the copy:

```sh
cargo test --tests --locked --offline \
  --target-dir "$validation_root/mutie-target" \
  --config "build.build-dir=\"$validation_root/mutie-target\""
```

Here `validation_root` was `/tmp/mutie-rust-scope-9foaxq8n`, and the working
directory was its `mutie` child. The log is `mutie-baseline.log` in that temporary
root; that path is local evidence, not a permanent dependency.

- **178 passed, 0 failed, 6 ignored**: 114 binary unit tests and 64 integration tests.
- Cargo reported **18.64 seconds** to build; total command wall time was
  **110.49 seconds**. This is one observation, with fixture probes running during
  part of the build; it is not a stable performance comparison or a cold download.
- All copied input file hashes were unchanged after tests. The original 91 tracked
  implementation/test/asset files matched their pre-run hashes (excluding docs
  intentionally edited for this task). The original `target/` file inventory,
  sizes and modification times were unchanged across 47,424 files.
- No source-project Cargo outputs were reused. Registry/download caches were
  available normally; their contents are not claimed to be immutable.

This supports choosing Mutie as the real validation target and establishes a
test-runtime cost to budget for. It does not verify parallel mutation builds,
build-cache reuse, Rust outcome classification, or Mutie's future Rust runner.

## Fixture mutation probes

Each case used its own source copy, empty target/build directory, and temporary
directory. First compiled with `cargo test --tests --locked --offline --no-run
--message-format=json`, then ran the same scope with `--no-fail-fast`. Both phases
overrode the target/build paths. Compilation succeeded for every case, emitted
successful build completion, and placed executable artifacts inside the owned
directory. The unit and integration targets both ran in every case.

| Case | Tests | Cargo test exit |
| --- | --- | --- |
| Original source | 3 passed | 0 |
| `unit_flag`: `true` → `false` | Unit assertion failed | 101 |
| `cli_flag`: `false` → `true` | CLI output assertion failed | 101 |
| `untested_flag`: `true` → `false` | 3 passed (deliberate survivor) | 0 |
| Original source + stronger smoke-test assertion | 3 passed | 0 |
| `untested_flag` flipped + stronger assertion | CLI output assertion failed | 101 |

The stronger assertion is `assert_eq!(output.stdout, b"true\n")` in
`unchecked_flag_smoke_test`; it was added only to the relevant temporary copies.
All checked-in fixture inputs were preserved. Probe logs and command timings are
under `fixture-probes/` and `fixture-probes.json` in the temporary audit root.

These are direct Cargo probes using exact manual replacements. The expected
future Mutie inventory is **three `boolean_flip` mutations, two killed and one
missed**, with the survivor becoming killed after improving its test. Discovery
exclusions are fixture inputs and a planned contract; no Rust parser has yet
verified them. There is no claim of Rust reports, reruns, timeout handling, or
source-discovery support from these probes.

## Next implementation and validation gates

Implement the fixture's path through project validation, module discovery,
`boolean_flip`, baseline, isolated execution, and saved results. Preserve Go
behavior and keep Rust identity/test scope explicit in reports and rerun checks.
Then sample Mutie's two eligible literals in `src/score.rs`, with the complete
selected Cargo test suite for each mutation. No survivor or stronger-test finding
on Mutie itself has been established yet.

Before broadening scope, cover build versus test versus tool failure, empty and
failed runs, signals/timeouts, artifact cleanup, and repeated/parallel mutations.
Fresh per-mutation builds are the initial correctness choice; any later reuse
must prove source and test changes cannot execute stale binaries. General Cargo
workspaces, libraries/doctests, feature combinations, custom build scripts,
conditional-code discovery and nonstandard test harnesses remain outside the
first contract.
