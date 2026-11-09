# Rust numeric fixture

Independent Cargo package for `number_replace`, outside the root default suite.
Four selected candidates produce one kill, one float survivor and two compiler
failures (constant division by zero and an out-of-bounds index). Constants, array
lengths and the cfg(test) function are excluded.

The opt-in `numbers::` workflow checks fresh/cached agreement, score/review
policies, exact saved recipes, source preservation and a separate stronger test
for the float survivor. A separate control keeps timeouts and capture errors
outside kills. Run via `cargo test --test rust_workflow numbers:: -- --ignored`.
