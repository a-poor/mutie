# Rust comparison-boundary fixture

This independent, dependency-free binary package exercises Rust's
`comparison_boundary` operator alongside the original boolean-flip fixture.
It is copied to a temporary directory by an ignored integration test:

```sh
cargo test --locked --test rust_workflow boundaries:: -- --ignored
cargo run -- tests/fixtures/rust-boundaries --operator comparison_boundary --list
cargo run -- tests/fixtures/rust-boundaries --jobs 2 --timeout 30
```

Its four unit tests and two CLI tests pass before mutation. Discovery produces
five edits in `src/comparisons.rs`; each replaces only the ordering token.

| ID | Function | Change | Expected outcome |
| --- | --- | --- | --- |
| 1 | `less` | `<` → `<=` | Killed at equality; signed extremes also covered |
| 2 | `at_most` | `<=` → `<` | Killed at equality through a generic, borrowed `String` |
| 3 | `greater` | `>` → `>=` | Killed at equality; signed zero and NaN also covered |
| 4 | `at_least` | `>=` → `>` | Missed: tests only compare unequal unsigned values |
| 5 | `evaluated_once` | `<` → `<=` | Killed at equality; operand order/count covered |

To close the deliberate gap, add this integration test **in a temporary copy**:

```rust
#[test]
fn equality_is_included() {
    check("at-least", "10", "10", "true\n");
}
```

Then `--rerun-missed RUN_DIR --operator comparison_boundary` selects source ID 4,
assigns new ID 1, and kills it. The regression checks saved diffs/operator counts,
the 80% → 100% score change, the failure of this exact test, preservation of source
and old reports, strict report reading, and cleanup of owned build artifacts.

Syntax-only regressions in `src/rust/discovery.rs` separately cover nested
comparisons, closures and match guards, UTF-8 byte coordinates, CRLF, comments,
generic angle brackets, shifts, ranges, equality operators, and existing source
exclusions. Macro contents, conditional/test-only code, generated sources,
constants, type-level expressions and array repeat lengths remain outside discovery.
