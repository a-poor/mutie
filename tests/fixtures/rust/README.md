# Rust scope fixture

This dependency-free Rust 2024 package is the fixture for Mutie's first Rust
execution slice. It mirrors Mutie's binary target, ordinary `mod` files, inline
unit tests, and integration tests that launch `CARGO_BIN_EXE_<name>`.
Mutie now runs this package end to end:

```sh
cargo run -- tests/fixtures/rust --list
cargo run -- tests/fixtures/rust --jobs 2 --timeout 30
```

The inventory has three mutations: two killed and one missed. The opt-in
`cargo test --test rust_workflow -- --ignored --test-threads=1` regression checks
the complete workflow, stronger tests through a fresh run, and preservation.
Rust focused reruns remain a later step.

Run the baseline independently of Mutie's own tests:

```sh
cargo test --manifest-path tests/fixtures/rust/Cargo.toml --locked --offline --tests
```

There are three tests: one binary unit test and two CLI integration tests.
No feature is enabled by default. The empty `[workspace]` keeps the fixture
independent of any future parent workspace. Its lockfile is checked in.

## Verified first inventory

With `boolean_flip` and the initial discovery contract, exactly these three
production literals in `src/flags.rs` are eligible:

| Function | Change | Expected test result |
| --- | --- | --- |
| `unit_flag` | `true` → `false` | Unit test fails |
| `cli_flag` | `false` → `true` | CLI output assertion fails |
| `untested_flag` | `true` → `false` | Tests pass; smoke test checks only exit status |

To check the stronger-test scenario, add this assertion to
`unchecked_flag_smoke_test` **in a temporary copy**:

```rust
assert_eq!(output.stdout, b"true\n");
```

The unmodified baseline must still pass; changing `untested_flag` to return
`false` must now fail. Restore each mutation before checking the next one.

## Discovery boundaries

The first implementation excludes the inline `#[cfg(test)]` module,
`src/test_support.rs` reached only through a test-only module, and `tests/cli.rs`.
It also excludes:

- `alternate.rs`: gated by the non-default `alternate` feature.
- `inactive.rs`: gated by `#[cfg(any())]`.
- `generated.rs`: a checked-in `@generated` example, still compiled normally.
- `orphan.rs`: not reachable from the binary's module tree.
- `define_flag!`'s definition and expansion in `main.rs`.

The fixture has no build script, proc-macro dependency, local dependency, library,
doctest, example, benchmark, or custom test harness. Execution regressions cover
failed build/test baselines, mutation timeouts, baseline cancellation,
repeated/parallel runs, saved outcomes and artifact cleanup. In temporary copies,
the cancellation regression adds a fourth mutation and blocking test helpers to
verify serial SIGTERM and parallel SIGINT after a completed result, retained
partial reports/diagnostics, pending work, and process/artifact cleanup.
The compilation regressions use test-only Cargo/rustc proxies to hold a live
compiler at a checkpoint, verify baseline/parallel build cancellation and
timeouts, and check the shared compilation/test deadline. They delegate normal
work to the installed tools and do not broaden the supported Cargo scope.
Additional fault-injection tests cover Cargo startup/exits, missing or malformed
completion evidence, metadata errors, and stdout/stderr capture overflow. They
verify partial results and peer cleanup with truncated saved logs, including
listing failures that must not publish a partial inventory.

See [the initial support contract](../../../DESIGN.md#initial-rust-support-contract)
and [the Cargo probe results](../../../benchmarks/rust-initial-scope.md).
