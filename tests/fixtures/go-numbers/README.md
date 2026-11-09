# Go numeric fixture

Independent Go module for `number_replace`. Four selected literals produce one
kill, one float survivor and two compiler failures (constant division by zero
and an out-of-bounds array index). Named constants and array lengths are excluded.

The consumer package kills the checked return value; an unrelated package stays
in the full baseline and outside affected mutation tests. The opt-in workflow
checks full/affected agreement, score/review policies, exact saved recipes,
source preservation and a separate stronger test for the float survivor.
Separate controls keep timeouts and capture errors outside kills.

```sh
cargo test --test go_workflow numbers:: -- --ignored --test-threads=1
```
