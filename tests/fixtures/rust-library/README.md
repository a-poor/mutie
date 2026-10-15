# Rust library scope fixture

This independent package scopes the next Rust adapter expansion. Mutie currently
rejects library packages; these Cargo checks do not establish adapter support.
The root `cargo test` does not exercise this fixture.

Run `cargo test --locked --tests --no-fail-fast` in this directory. The three
tests cover a library unit test, a public API integration test, and a CLI that
links the library. The library documentation example is outside `--tests` scope.

Use temporary copies for mutation experiments:

- Flip `enabled()` from `true` to `false`: all three tests fail.
- Flip `preview_enabled()` from `false` to `true`: all three tests pass.
- Add an API test asserting `!preview_enabled()`: the pristine baseline passes
  and the second mutation fails.
- Remove `src/main.rs` and `tests/cli.rs` in another copy to check the library-only
  layout. Its two baseline tests have the same killed/surviving behavior.

See [the scope audit](../../../benchmarks/rust-library-scope.md) for evidence and
the adapter acceptance gates. Fresh/cached Mutie runs and saved-report reruns
remain implementation work.
