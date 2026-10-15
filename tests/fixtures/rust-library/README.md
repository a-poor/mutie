# Rust library scope fixture

This independent package exercises experimental Rust library support. Use
`cargo run -- tests/fixtures/rust-library --jobs 1` from the repository root to
produce one killed and one missed mutation. The opt-in tests in
`tests/support/rust_library.rs` cover both layouts, fresh/cached runs and a
stronger-test focused rerun. The `library_acceptance_*` checks additionally cover
dismissal review/raw scores, compiler failures, and interruption with descendant
cleanup; run them using `cargo test --test rust_workflow library_acceptance --
--ignored --test-threads=1` from the repository root.
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
the remaining real-project acceptance gates.
