An independent Cargo fixture for project build scripts and inactive profiles.
It generates Rust in OUT_DIR, emits cfg/env directives, and rejects stale output
left by a previous compilation. The two source booleans produce one kill and
one survivor. Build scripts and generated code are outside mutation discovery.

Run `cargo test --manifest-path tests/fixtures/rust-build/Cargo.toml` directly,
or run the ignored `build_scripts::` tests in the root `rust_workflow` suite.
