# Rust dismissal review — 2026-10-12

This validates Rust dismissal support on top of `baa8455` (focused reruns), using
the Rust fixture and **Mutie itself**. The initial binary-package Cargo scope is
unchanged. A broader useful Rust operator set remains the milestone's next item.

## Behavior and compatibility

`dismiss add|list|update|remove` supports roots containing `go.mod` or `Cargo.toml`
without language tools or a valid `mutie.toml`. `dismiss --lang go|rust` resolves
ambiguous roots; `add` rejects a saved report from another language. `check`
honors repository language/configuration plus an explicit language override. It
validates the initial Cargo project scope, runs metadata in an isolated copy,
and discovers source identities from the pristine snapshot without compiling
tests. Source/test files and caller build artifacts are preserved.

New dismissal records carry a language; records without it retain their legacy
Go meaning. Language mismatches become review issues, never applied decisions.
Saved reviews validate their language against the run. Package, report, operator
catalog and dismissal-format versions are unchanged.

Dismissals remain source judgments: they use full-file identity and do not expire
solely because a manifest, lockfile or compiler changes. Focused Rust reruns still
perform their separate execution-scope checks. A dismissed mutation continues to
execute, remains in raw counts/scores, and can later be killed by stronger tests.
Saved reports retain their original review even after repository metadata changes.

## Regression validation

```sh
cargo test --locked
cargo test --locked --test rust_workflow dismissals::rust_dismissals_preserve_outcomes_scores_reports_and_focused_reruns -- --ignored --exact
cargo test --locked --test rust_workflow dismissals::rust_dismissal_checks_isolate_metadata_side_effects_and_clean_up_failures -- --ignored --exact
cargo test --locked --test go_workflow go_workflow -- --ignored --exact
cargo fmt --check
cargo clippy --all-targets --locked -- -D warnings
```

The default suite passed **193 tests** (129 unit and 64 integration), with 19
opt-in tests ignored. The new Rust workflows passed in 27.57 and 7.76 seconds;
the real-Go workflow passed in 26.31 seconds. The other 11 opt-in Rust
workflows were not repeated for this change.

The first new test covers tool-free CRUD even with broken run configuration,
duplicate-add preservation and lock cleanup, explicit language selection,
source-language rejection, metadata-only checks with an uncompilable test,
dismissed/undismissed policies, raw score preservation, source-ID remapping,
valid decisions omitted from a focused selection, language mismatch issues,
frozen historical reports, moved checkouts, stale/excluded source and a
stronger-test rerun that kills a dismissed survivor. All five reports pass the
strict retention preview reader.

The second delegates metadata to real Cargo through a test proxy, then changes
source and dismissal files in the discovery workspace. These side effects do
not change the checked identities or original inputs. It also checks malformed
metadata, overflow with one-byte retained logs, timeout and SIGTERM. Processes
and owned source/build/test directories are cleaned up, no check creates a report,
and caller artifacts remain untouched. The Cargo tests stay ignored by default
to prevent recursive mutations when Mutie tests itself.

## Real-project validation setup

A new temporary copy of the current Mutie source uses the known score-validation
survivor from the [focused rerun record](rust-focused-reruns.md), source ID 2 in
`before/run-1791815464-6mVzNd`. The `src/score.rs` fingerprint, Cargo manifest and
lockfile still match. As in that earlier reproduction, only the temporary copy
marks the two percentage-presence regressions ignored. The working repository's
tests are not weakened, and this is not a claim of a newly discovered gap.

The temporary decision explicitly describes a workflow validation, not an actual
acceptance of that actionable gap. Adding succeeds without Cargo on PATH, and
`dismiss check` reports one match at current mutation ID 2. The project config
selects `src/score.rs`, a 300-second baseline limit and 600-second mutation limit.
The focused run selects the saved survivor with `--rerun-missed`, enables both
`--fail-on-missed` and `--min-score 100`, and uses one worker with fresh artifacts.

```sh
target/debug/mutie dismiss --module "$PROJECT" add "$SOURCE_RUN" --id 2 \
  --reason 'Temporary dismissal workflow validation' \
  --note 'Known actionable gap; accepted only in this disposable validation copy.'
target/debug/mutie dismiss --module "$PROJECT" check
target/debug/mutie "$PROJECT" --rerun-missed "$SOURCE_RUN" --jobs 1 --ui plain \
  --fail-on-missed --min-score 100 --output-dir "$REPORTS"
```

`CARGO_TARGET_DIR` and `CARGO_BUILD_BUILD_DIR` point to caller-artifact sentinel
directories; `TMPDIR` points to an empty owned root. Input and source-report hashes
allow checking preservation after execution. The host is macOS arm64 with
rustc/Cargo 1.99.0.

## Real-project results

The run completed in **236.31 seconds** with one missed mutation and no kills,
timeouts, build failures or execution errors. Both baseline and mutation ran
191 passing tests; the 21 ignored tests comprise the 19 ordinary opt-in workflows
and the two deliberately skipped regression cases in this temporary copy.

| Measurement | Result |
| --- | --- |
| Baseline command | 123.59 s, including 39.96 s compilation |
| Mutation command | 110.86 s, including 25.22 s compilation |
| Cumulative workspace work | 0.051 s |
| Focused identity | Source mutation 2 → current mutation 1 |
| Saved review | Dismissal 1 applied; zero entries needing review |
| Raw mutation score | 0%; the dismissed mutation remains included |
| `--fail-on-missed` | Passed: zero undismissed misses, one dismissed miss |
| `--min-score 100` | Failed; CLI exit 3, as expected |

All 90 project-input hashes and all 24 source-report artifact hashes were
unchanged after the run. Caller target/build sentinels were preserved and the
owned temporary root was empty. A retention preview accepted the new report as
completed; it did not delete anything.

After an intentional repository dismissal-reason update in the temporary copy,
the real saved report opened in a pseudo-terminal with an empty PATH. It showed
the original reason and `[D]` badge, supported the saved-review view and dismissed
filter, and exported a marked `--lang rust --rerun … --id 1` command. It exited
successfully, restored terminal settings and left every saved-report artifact
unchanged. Only the intentional metadata edit changed a project input.

This completes the shared review/rerun workflow for the initial Rust scope.
It does not broaden Cargo support or establish the usefulness of additional
operators. Fresh compilation/testing still dominates this small focused run.

Local evidence (temporary, not committed):

- Validation root: `/tmp/mutie-rust-dismissals-self-zrmy5tla`
- Report: `reports/run-1791854939-xtyu1t` beneath that root
- Command logs: `check.log` and `run.log`
- Browser capture/results: `browser.ansi` and `browser-validation.json`
- Preservation hashes: `inputs-before.json`, `source-report-before.json` and
  `saved-review-before.json`
