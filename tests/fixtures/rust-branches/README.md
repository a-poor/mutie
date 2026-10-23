# Rust branch-forcing fixture

This independent package has three original tests and six branch candidates.
The selected branch workflow yields three kills, one survivor and two build
failures. `weak(true)` does not exercise the false branch; a stronger
`weak(false) == 2` test kills the force-true survivor.

The `initialized` function assigns its result inside the condition. Both forced
conditions can skip that assignment, so Rust rejects the possibly uninitialized
return. These stay build failures rather than kills. Native controls separately
cover skipped side effects, non-Copy values, borrowing and drop timing.

```sh
cargo test --test rust_workflow branches:: -- --ignored --test-threads=1
```

The root default suite does not recursively execute this Cargo fixture.
