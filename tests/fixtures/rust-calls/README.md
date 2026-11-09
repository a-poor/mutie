# Rust standalone-call fixture

This independent package is exercised by the opt-in `rust_workflow` call tests,
not by the root default suite. With only `call_remove` enabled, its three
candidates should produce one kill, one survivor, and one build failure.
Removing the only push into an untyped vector removes its element-type constraint.
A stronger test of `weak(true)` kills the survivor on a focused rerun.

The workflow compares fresh serial and cached parallel execution, validates saved
recipes and policy scores, and checks original source/report/artifact preservation.

```sh
cargo test --locked --test rust_workflow calls:: -- --ignored --test-threads=1
```
