# Rust assignment fixture

This independent package is exercised by the ignored `rust_workflow` assignment
tests. With `assignment_remove` alone, it has four candidates: one field-write
kill, one indexed-write survivor, and two build failures (lost type inference and
an unrestored partial move). Build failures are excluded from its 50% score.

A separate stronger test checks the actual returned item and kills the survivor.
Controls compare fresh serial/cached parallel runs, review and score policies,
exact saved recipes, forged test-only spans, stale sources, and preservation of
inputs, caller artifacts and original reports. Other temporary fixture copies
exercise genuine timeout and capture-error outcomes.

```sh
cargo test --locked --test rust_workflow assignments:: -- --ignored --test-threads=1
```
