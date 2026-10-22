# Rust workspace regression fixture

This independent virtual workspace uses inherited metadata and an internal path
dependency. Only the core is a default member; the consumer's tests must nevertheless
run and detect a core mutation. Three mutations yield one kill and two survivors.
A separate stronger copy exercises `fallback(false)` and kills both survivors.

Examples and benchmarks deliberately do not compile: they are not selected by
`--tests`. Release/bench profile declarations do not change the test profile.

Run the opt-in regression with:

```sh
cargo test --test rust_workflow full_workspace_tests_catch_selected_member_changes_and_preserve_identity -- --ignored --test-threads=1
```

The root test suite does not recursively execute this workspace.
