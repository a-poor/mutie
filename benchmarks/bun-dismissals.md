# Bun review, policies and milestone acceptance

Recorded 2026-10-20. The opt-in regression in
`tests/support/bun_dismissals.rs` exercises JavaScript and TypeScript independently
through `tests/bun_workflow.rs`, with pinned Bun 1.3.11+af24e281e.

## Review contract

Each syntax starts with one kill and one survivor. The test adds, lists, updates,
removes and re-adds a dismissal for the survivor; duplicate additions preserve
metadata. CRUD and fresh source rediscovery run with an empty PATH, so review
does not depend on launching Bun. Saved review records retain the Bun language.

Configured `fail_on_missed` passes after dismissal, while `min_score = 100` still
fails: dismissal does not remove the survivor from the raw denominator. A focused
missed-only rerun remaps the saved source ID to its new selection ID and preserves
the dismissal. Clearing the minimum-score policy permits that reviewed run to
succeed with a zero-percent selected score.

A source edit invalidates the dismissal and restores the undismissed-finding
failure. Restoring source and adding a stronger test kills the selected survivor;
both configured policies then pass. A subsequent cleanup failure caused by that
same dismissed mutation produces an execution error, zero kills, an unavailable
score and neither policy evaluated. Review cannot waive an infrastructure error.

The original source and original run/inventory/outcome/summary files are checked
byte-for-byte after the workflow. The saved reviewed report passes reader
validation before the browser rejects noninteractive launch; this is not a new
interactive rendering test.

## Scoped milestone audit

- [Scope and native failure attribution](bun-body-attribution.md): independent
  JS/TS controls, complete accounting, body evidence and conservative error cases.
- [Isolated execution](bun-isolated-execution.md) and
  [configuration acceptance](bun-composer-execution.md): pinned runtime, owned
  copies, independent file inventory, bounded evidence and unchanged Composer.
- [Shared runs](bun-shared-runs.md): both syntax fixtures, saved identities and
  stronger-test reruns. Composer's unchanged 272-test baseline and 12 candidates
  yield six kills, five survivors and one timeout in 106.14 seconds. A retained
  stronger test kills the name-configurability survivor in a 15.86-second focused
  rerun, preserving original inputs and report hashes.
- [Scored cleanup](bun-scored-cleanup.md): JS/TS capture/file-limit errors,
  timeout/SIGINT, descendant shutdown, workspace removal and source preservation.
- This review regression: CRUD, staleness, policy precedence and historical reports.

The TypeScript evidence covers the scoped real-project feedback loop; independent
real JavaScript findings remain the next gate. Four Composer survivors and the
queue timeout remain unresolved. No equivalence claim is made; the original full
score remains 54.55%, with the timeout excluded.

Scope stays one dependency-free ESM package, serial full-suite boolean flips,
pinned Unix runtime, audited source/test layout and plain-JSON NodeNext/ES2022
settings. This does not establish other runtimes, dependencies, arbitrary config,
parallel workers, preloads or total disk quotas. TypeScript evidence must not
promote JavaScript without its own real-project loop.

## Validation result

`cargo test --offline --test bun_workflow review:: -- --ignored --test-threads=1`
passed in 187.56 seconds with the pinned runtime on PATH. The two default Bun
workflow tests, `cargo fmt --check`, `cargo clippy --offline --all-targets -- -D warnings`
and `git diff --check` passed. No production code changes were needed.

TypeScript is now Validated within this scope. JavaScript remains Experimental
until an independent real-project run/inspect/improve-tests/rerun loop is recorded.
