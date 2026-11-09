# Go string fixture

Four string candidates: a consumer-package assertion kills the checked value,
an empty raw string survives, and constant string indexing/division mutations
fail compilation. Constants, imports and test strings stay excluded. The unrelated
package remains in the full baseline but outside affected mutation tests.

```sh
go test ./... # from this directory
cargo test --test go_workflow strings:: -- --ignored --test-threads=1
```

The opt-in workflow checks full/affected agreement, review and score policies,
stronger-test reruns, exact recipes, source drift and preserved reports/inputs.
Separate controls keep timeouts and capture errors outside kills.
