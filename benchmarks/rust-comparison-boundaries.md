# Rust comparison boundaries — 2026-10-12

This extends the Rust workflow after `096ec5d` (dismissal review) with the shared
`comparison_boundary` operator. The initial binary-package Cargo scope and all
package/report/operator catalog versions remain unchanged.

## Discovery and fixture evidence

Rust discovery now changes `<` ↔ `<=` and `>` ↔ `>=` in eligible function/method
bodies. Only the parsed operator token changes. It also visits executable match
guards, which Tree-sitter nests inside a pattern field; patterns remain excluded.
The existing exclusions for tests, conditional code, macros, generated sources,
constants and type-level expressions still apply. A regression also exposed
array repeat lengths being visited as ordinary expressions; discovery now skips
that constant-expression field while retaining the array's value expression.

Syntax regressions cover all four directions, nested comparisons, closures and
guards, UTF-8 byte coordinates, CRLF and exact source preservation. They reject
false candidates in generic angle brackets, shifts, ranges, equality operators,
comments and strings. Go discovery is unchanged.

The independent [boundary fixture](../tests/fixtures/rust-boundaries/README.md)
has five mutations. A parallel run records **four killed and one missed**, with
no build failures, timeouts or execution errors, and a raw score of 80%. It covers
signed/unsigned integers, generic comparisons borrowing non-Copy strings,
floating-point NaN/signed-zero behavior, and left-to-right, once-only operand
effects. The missed `>=` → `>` mutation has tests on both sides of the boundary
but none at equality.

A stronger integration test for equality passes against the original program.
`--rerun-missed RUN_DIR --operator comparison_boundary` maps source ID 4 to new
ID 1 and kills it through that exact assertion. The selected raw score becomes
100%. The regression also verifies operator filters, saved diffs/summaries,
source and source-report preservation, strict report reading, caller artifact
preservation and owned-directory cleanup.

## Validation commands

```sh
cargo test --locked
cargo test --locked --bin mutie rust::discovery::tests
cargo test --locked --test rust_workflow boundaries:: -- --ignored
cargo test --locked --test rust_workflow rust_fixture_runs_lists_filters_and_saves_results_without_touching_inputs -- --ignored --exact
cargo test --locked --test go_workflow go_workflow -- --ignored --exact
cargo fmt --check
cargo clippy --all-targets --locked -- -D warnings
```

The default suite passed **195 tests** (131 unit and 64 integration), with 20
opt-in tests ignored. Real Cargo workflows remain ignored by default to prevent
recursive self-mutation. The real-Go workflow passed after allowing its normal
build-cache write, which the sandbox initially blocked. The final boundary
workflow passed in 60.72 seconds, the existing boolean workflow in 116.65 seconds,
and the real-Go workflow in 28.22 seconds, under concurrent validation load.
The remaining 12 opt-in Rust workflows were not repeated for this change.

## Mutie self-run

The current source was copied to
`/tmp/mutie-rust-boundaries-self-6c9bcv9l/project`, excluding build outputs.
Unlike earlier reproductions, no tests were weakened. A fresh run selects all
six comparison boundaries in `src/score.rs`, using two workers, a 300-second
baseline deadline and 600-second mutation deadlines:

```sh
target/debug/mutie "$PROJECT" --include src/score.rs \
  --operator comparison_boundary --jobs 2 --timeout 600 \
  --baseline-timeout 300 --ui plain --output-dir "$BEFORE"
```

`CARGO_TARGET_DIR` and `CARGO_BUILD_BUILD_DIR` point to caller sentinel directories;
`TMPDIR` points to an owned temporary root. Input hashes cover every copied file.
The baseline passed in 161.24 seconds while other regression checks were running.
Fresh artifact directories are used for every baseline and mutation; these are
observed workflow costs, not a controlled performance comparison.

The completed run took **780.31 seconds (13 minutes)**: **five killed and one
missed**, with no timeouts, build failures or execution errors. Its raw mutation
score is 83.33%. Baseline and survivor each passed all 195 default tests, with
20 opt-in tests ignored. The temporary copy retained the discovery tests from
the start of the run; the later array-length exclusion refinement was verified
separately. The six selected score expressions and their identities are unchanged.

| ID | Location in `src/score.rs` | Change | Outcome | Command time |
| --- | --- | --- | --- | --- |
| 1 | Line 25, minimum-score parsing tolerance | `>` → `>=` | Killed | 219.13 s |
| 2 | Line 47, decimal precision limit | `>` → `>=` | Killed | 219.31 s |
| 3 | Line 159, unfinished-result count | `>` → `>=` | Killed | 195.51 s |
| 4 | Line 161, execution-error count | `>` → `>=` | Killed | 195.06 s |
| 5 | Line 185, saved-percentage tolerance | `<=` → `<` | Missed | 199.22 s |
| 6 | Line 238, exact minimum-score comparison | `>=` → `>` | Killed | 194.96 s |

The baseline compilation took 47.28 seconds; mutation compilation took about
68–89 seconds each. Cumulative workspace work was 1.011 seconds. Run and test
costs dominate copying, and cumulative command durations overlap across workers.

The survivor tightens saved-percentage validation at a discrepancy of exactly
`1e-12` percentage points. For example, an expected zero and saved `1e-12` are
accepted by the original comparison and rejected by the mutant. This is a narrow,
low-priority review finding about a numerical-tolerance endpoint. It does not
change the minimum-score gate, which uses exact outcome counts. The fixture's
missing equality assertion supplies the demonstrated stronger-test improvement;
the real survivor remains recorded for a decision about the intended tolerance
contract. It has not been used to claim a user-facing scoring defect.

All **94 input-file hashes** remained unchanged, caller target/build sentinels
were preserved, and the owned temporary root was empty after completion. The
strict retention reader accepted the finalized report in preview mode.

Local evidence (temporary, not committed):

- Root: `/tmp/mutie-rust-boundaries-self-6c9bcv9l`
- Report: `before/run-1791858659-K4V1j9` beneath that root
- CLI command/log: `command.txt`, `before.log`
- Preservation evidence: `inputs-before.json`, `preservation-result.json`
