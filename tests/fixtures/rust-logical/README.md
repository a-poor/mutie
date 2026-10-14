# Rust logical-swap fixture

Independent Cargo binary package; root `cargo test` does not run this fixture.

```sh
cargo test --locked --manifest-path tests/fixtures/rust-logical/Cargo.toml
cargo test --locked --test rust_workflow logical:: -- --ignored
```

Selecting `logical_swap` discovers eight mutations in `src/logic.rs`: seven
killed and one missed. Side-effect traces check left-to-right evaluation and
short-circuit behavior. Three-operand truth tables cover grouping; a let-chain
keeps its binding connector while mutating its ordinary boolean operand.
`weak_any` deliberately tests only equal inputs. The opt-in workflow adds a
mixed-input assertion in `tests/cli.rs`, then kills that survivor through a saved
rerun without changing its production-source identity. It checks configuration,
parallel worker caches, score policies, saved reports and source/artifact cleanup.
