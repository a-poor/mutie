# Rust arithmetic fixture

This independent library checks all five binary replacements, two weak tests,
zero-division and overflow panics, and a missing replacement trait (`String - &str`).
The root default suite does not run Cargo mutation workflows recursively.

```sh
cargo test --locked --manifest-path tests/fixtures/rust-arithmetic/Cargo.toml
cargo test --test rust_workflow arithmetic:: -- --ignored --test-threads=1
cargo test --bin mutie rust::discovery::arithmetic_tests -- --include-ignored
```

The scored workflow expects seven kills, two survivors and one build failure.
The two survivors are killed by nonzero-operand assertions in a focused rerun.
Native controls separately test moved/borrowed overloads and drop/evaluation order,
integer truncation/overflow, signed zero, NaN and Infinity. No deadlines or outcome
classifications are relaxed for arithmetic.
