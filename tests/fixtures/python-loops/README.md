# Python while-loop fixture

Run with the pinned Python/pytest environment through the ignored
`loops_preserve_native_semantics_and_rerun` workflow in `tests/python_workflow.rs`.
The count loop is killed, including its unchanged `else` behavior. The weak loop
survives the zero-input test; a separate-copy positive-input assertion kills it.
Native truthiness, assignment-expression exclusions and generator/else controls
live in `tests/support/python_scope.py`. Runtime errors stay errors.
