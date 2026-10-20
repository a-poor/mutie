# Bun scored resource limits and shutdown

Recorded 2026-10-19, with pinned Bun 1.3.11+af24e281e from the separate
temporary runtime used for shared-run acceptance. No runtime or production
implementation changes were required.

`tests/bun_workflow.rs` now exercises the following through the scored CLI,
independently for JavaScript and TypeScript. Every case passes its baseline and
reaches a mutation of `enabled()` from true to false.

| Mutation behavior | Verified saved result |
| --- | --- |
| Writes 200,000 output bytes with a 32 KiB capture budget and 64-byte retained logs | Failed report, one error, zero kills, unavailable score, minimum-score policy not evaluated |
| Writes a 9 MiB regular file under the inherited 8 MiB per-file limit | Same error/score behavior; no file in original project |
| Spawns a delayed-writing descendant and hangs, with a one-second mutation timeout | Completed report, one timeout, zero kills, unavailable score for the empty denominator |
| Spawns that descendant and hangs, then receives CLI SIGINT after the mutation checkpoint | Interrupted report and mutation, zero kills, unavailable score |

Timeout/cancellation tests verify that the owned execution workspace disappears
and the descendant never writes its external marker after its scheduled deadline.
All cases verify original production bytes remain unchanged. The checkpoint is
mutation-only, so a baseline failure cannot accidentally satisfy these assertions.
These are bounded process-group probes, not an OS sandbox or total disk quota.

Validation:

- `cargo test --offline --test bun_workflow scored_ -- --ignored --test-threads=1`
  with pinned Bun on PATH: three tests passed in 338.33 seconds, including the
  existing JS/TS reports, identity-drift and stronger-test rerun workflow.
- `cargo test --offline --test bun_workflow`: two default tests passed; four native
  tests ignored as intended.
- `cargo fmt --check`, `cargo clippy --offline --all-targets -- -D warnings`, and
  `git diff --check` passed.

Bun remains Experimental. Next gates are Bun-specific dismissal/review/policy
acceptance and independent real JavaScript findings. Composer's remaining four
survivors and queue timeout remain open; these fixture checks do not resolve them.

Follow-up: [review/policy acceptance](bun-dismissals.md) closes that gate and
promotes scoped TypeScript independently; real JavaScript acceptance remains open.
