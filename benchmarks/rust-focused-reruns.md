# Focused Rust reruns — 2026-10-12

This validates the focused rerun implementation on top of `69bcc47`, using the
small Rust fixture and **Mutie itself**. It extends the
[first-slice evidence](rust-first-slice.md); the broader operator/dismissal
milestone remains incomplete.

## Contract and regression checks

Rust uses the shared saved-ID/operator/missed-outcome selection and strict source
identity. Every selected mutation must be freshly rediscovered. Before the
baseline, reruns also compare package, binary/source entry, edition, manifest and
lockfile fingerprints, default features, test scope, profile, and actual
rustc/Cargo versions. Changes require a fresh scan. Whole manifest fingerprints
also reject comment-only edits; this is intentionally conservative.

New reports record the manifest fingerprint without a schema/package version
bump. Earlier Rust reports remain inspectable but cannot establish the required
identity for a rerun. Legacy Go reports remain usable. Browser command export
specifies `--lang rust` and keeps normal shell quoting; it never executes Cargo.

The fixture's original run produces two kills and one survivor. Adding the output
assertion in `tests/cli.rs`, then selecting `--rerun-missed RUN_DIR --id 3,1,3
--operator boolean_flip`, runs just source ID 3 as new ID 1 and kills it. The fresh
baseline passes, the 100% minimum-score policy passes for the rerun selection,
and the mutation log identifies the strengthened test. The source report and
production file remain unchanged. A chained preview uses the new run's IDs, and
missed-only selection of the all-killed report produces an empty completed run.

Additional real-Cargo checks cover stale/excluded source, changed manifest and
lockfile, changed saved toolchain versions, legacy scope, both language-mismatch
directions, unknown IDs before tools run, pending/interrupted selections from a
finalized partial report, a new failing baseline, and a moved checkout. Caller
artifact sentinels and owned-directory cleanup are checked throughout.

Validation passed:

```sh
cargo test --locked
cargo test --locked --test rust_workflow rust_fixture_runs_lists_filters_and_saves_results_without_touching_inputs -- --ignored --exact
cargo test --locked --test rust_workflow reruns::rust_reruns_validate_scope_source_partial_runs_and_moved_checkouts -- --ignored --exact
cargo test --locked --test go_workflow go_workflow -- --ignored --exact
cargo fmt --check
cargo clippy --all-targets --locked -- -D warnings
```

The default suite passed 192 tests (128 unit and 64 integration), with 17 opt-in
tests ignored. The two Rust workflows passed in 25.78 and 31.58 seconds. The Go
workflow passed in 25.21 seconds after allowing access to its build cache; its
initial sandbox attempt failed on a cache write. The remaining nine opt-in Rust
execution/reliability tests were not repeated for this change.

## Real-project setup

The previous real survivor was the `false` branch in
`MutationScore::matches_saved`, which accepted inconsistent saved percentage
presence. Its regression tests now live in `src/report_reader.rs`, separate from
the selected `src/score.rs`. In a temporary copy of the current Mutie inputs,
the two percentage-presence regression tests are initially marked ignored to
reproduce that already-understood gap. The working repository's tests are never
weakened. Restoring the original test file is the only intended input change
between discovery of the survivor and its focused rerun.

Both runs use one worker, fresh owned compilation artifacts, a 300-second
baseline limit and 600-second mutation limit. Go/Cargo caches are already warm;
the host is macOS arm64 with rustc/Cargo 1.99.0.
Compiled Cargo targets are never reused between baseline/mutations. Caller
target/build paths contain preservation sentinels, and an owned temporary root
allows checking cleanup. Input-file and source-report hashes are compared after
execution.

The initial scan completed in **356.62 seconds** with one kill and one survivor,
no timeouts/build failures/errors, and a 50% score for this two-mutation selection.
Its baseline passed 190 tests, with 19 ignored (including the two temporarily
disabled regressions). It preserved all 87 copied input files and all caller
artifact sentinels, and left the owned temporary root empty. The survivor is
source ID 2, `src/score.rs:187:18`, `false` → `true`.

After restoring `src/report_reader.rs`, the ordinary `--rerun-missed --list`
preview selected exactly that survivor as new ID 1. In a real PTY browser, marking
source ID 2 and exporting produced the correctly quoted `--lang rust --rerun ...
--id 2` command, restored the original termios, and preserved all 24 source-report
files. This supplements the browser's deterministic renderer/dispatch test.

The CLI commands use the freshly built `target/debug/mutie`:

```sh
target/debug/mutie "$PROJECT" --include src/score.rs --operator boolean_flip \
  --jobs 1 --timeout 600 --baseline-timeout 300 --ui plain --output-dir "$BEFORE"

# Restore the two score-validation regression tests in the temporary copy.
target/debug/mutie "$PROJECT" --rerun-missed "$SOURCE_RUN" \
  --include src/score.rs --operator boolean_flip --list

target/debug/mutie "$PROJECT" --rerun-missed "$SOURCE_RUN" \
  --include src/score.rs --operator boolean_flip --jobs 1 \
  --timeout 600 --baseline-timeout 300 --min-score 100 \
  --ui plain --output-dir "$AFTER"
```

`CARGO_TARGET_DIR`, `CARGO_BUILD_BUILD_DIR`, and `TMPDIR` point to the validation
copy's sentinel/owned directories during execution. These commands run the
complete ordinary Cargo test suite for each baseline and selected mutation.

## Focused rerun result

The rerun completed in **285.91 seconds** (4 minutes 46 seconds). Its baseline
passed all 192 ordinary tests, with the usual 17 opt-in tests ignored. It selected
exactly one mutation, recorded the mapping from source ID 2 to new ID 1, and
reported **one kill**, zero missed/timeouts/build failures/errors, and a passing
100% minimum-score policy for the rerun selection only.

Only the two restored report-reader tests failed under the mutation:

- `saved_score_requires_percentage_for_scored_outcomes`
- `saved_score_rejects_percentage_for_an_interrupted_run`

All integration tests passed, including terminal tests. This verifies detection
of the intended score-validation gap rather than an unrelated failure. It
reproduces a previously discovered useful finding, not a newly discovered gap.

| Run/phase | Fresh compilation | Whole command |
| --- | --- | --- |
| Initial scan baseline | 30.31 s | 122.19 s |
| Focused rerun baseline | 45.08 s | 140.16 s |
| Focused rerun mutation | 53.69 s | 142.79 s |

The initial scan took 356.62 seconds for baseline plus two mutations; the focused
rerun took 285.91 seconds for baseline plus one mutation. Compilation timing
varied substantially, so these single runs are observations, not a controlled
speedup comparison. Rerun workspace copying/resetting consumed only 0.097
cumulative seconds. Fresh compilation and the full test suite still dominate
the cost of even a one-finding self-rerun.

All 87 input files matched the restored-input hashes, all 24 source-report files
matched their original hashes, caller artifact directories contained only their
sentinels, and the owned temporary directory was empty. The new report passed
the strict retention preview reader without deletion.

Local evidence paths (temporary artifacts, not repository dependencies):

- Root: `/tmp/mutie-rust-focused-self-1n9y_mzl`
- Source report: `before/run-1791815464-6mVzNd`
- Focused report: `after/run-1791815885-AmP1Gt`
- Preview/browser: `preview.log`, `browser.ansi`
- Hash records: `inputs-before.json`, `inputs-strong.json`, `report-before.json`
- Default suite log: `/tmp/mutie-rust-reruns-default-tests.log`
- Go workflow log: `/tmp/mutie-rust-reruns-go-tests.log`
