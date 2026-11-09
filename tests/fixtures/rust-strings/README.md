# Rust string fixture

An independent Cargo package with two `string_replace` candidates: an ordinary
string killed by an assertion and an empty raw string whose value is unchecked.
The cfg-test string stays excluded. Root `cargo test` does not run this package.

```sh
cargo test --manifest-path tests/fixtures/rust-strings/Cargo.toml --locked
cargo test --test rust_workflow strings:: -- --ignored --test-threads=1
```

The opt-in workflows compare fresh and worker-cached runs, review/policy results,
stronger-test reruns, exact saved recipes and source/report preservation. Separate
controls keep timeouts and capture errors outside kills and scores.
