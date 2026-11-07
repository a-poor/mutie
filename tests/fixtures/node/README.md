# Node fixture

Dependency-free ESM JavaScript package for the `[js]` / `runner = "node"` scope.
Use audited Node 24.22.0 on PATH. `node --test test/*.test.js` passes three tests;
Mutie's three boolean flips produce two kills and one survivor. Adding an
assertion for `fallback(false) === true` kills that survivor on a focused rerun.

Run the opt-in workflow and cleanup acceptance from the repository root:

```sh
cargo test --test node_workflow -- --include-ignored --test-threads=1
cargo test --bin mutie node::tests::timeout_cancellation_capture_and_owned_files -- --ignored
```

The normal Cargo suite runs scope/protocol/discovery regressions but skips real
Node execution, so self-mutation tests cannot recurse into runner campaigns.
