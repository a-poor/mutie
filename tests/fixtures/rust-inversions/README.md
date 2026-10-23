# Rust comparison inversion and Not removal fixture

This independent library covers all six comparison inversions, Boolean/integer
Not removal, two intentionally weak assertions and a custom Not output-type change.
Operator selection is explicit so boundary mutations remain a separate control.

```sh
cargo test --locked --manifest-path tests/fixtures/rust-inversions/Cargo.toml
cargo test --test rust_workflow inversions:: -- --ignored --test-threads=1
cargo test --bin mutie rust::discovery::inversion_tests -- --include-ignored
```

Eleven selected mutations produce eight kills, two survivors and one build failure.
Ordered-input and observed-result assertions kill both survivors in a focused rerun.
The workflow rejects corrupted comparison and negation recipes and checks saved
source IDs, input/report preservation and owned-artifact cleanup. Native controls
separately cover borrowed comparisons, overloaded dispatch, NaN, integer and custom
Not, operand evaluation, move/drop timing and nested negation removal.
