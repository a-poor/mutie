# Vitest focused reruns

Following the scored-run commit `e3950db`, the pinned Vitest slice supports
`--rerun RUN_DIR`, `--rerun-missed RUN_DIR` and saved-ID/operator filtering.
Selection uses freshly discovered mutations and exact source identity; saved
replacement text is never replayed against an arbitrary current file. Every
selected mutation still runs the full test suite in a fresh command copy.

```sh
mutie /path/to/prepared-project --lang vitest --ui plain
# Improve tests, then use the directory printed by the first run:
mutie /path/to/prepared-project --lang vitest --rerun-missed /path/to/saved-run
mutie /path/to/prepared-project --lang vitest --rerun /path/to/saved-run --id 6
```

## Identity and preservation

Saved Vitest execution scope is mandatory for reruns and browser exports. Before
baseline execution, the current captured project must match package name, raw
manifest/lockfile fingerprints, pinned Node/pnpm/Vitest/Vite versions, Node
executable path, generated runner settings, reporter fingerprint, syntax scope,
and the presence/content of allowed project configs (`tsconfig.json` and the
settings-only `pnpm-workspace.yaml`). Even whitespace-only manifest/lockfile
changes require a fresh scan. Moving a checkout is allowed; changing the Node
executable path is conservatively rejected.

Installed dependencies still use the project's existing containment and locked
metadata validation. This is not a cryptographic inventory of every installed
dependency or Node executable byte. Such stronger integrity guarantees remain
outside the current scope.

Test-file edits are intentionally allowed. Selected production source must match
its saved whole-file fingerprint, byte span, operator and replacement identity.
Source changes require a fresh scan even if an apparent boolean mutation remains
at the same location. Missing/excluded targets fail rather than silently replaying
old changes. Source matching occurs during discovery/selection; normal reruns may
execute their baseline before detecting stale source. `--list` checks selection
without running the baseline. Metadata identity mismatches fail before baseline.

New reports preserve source-run/checkpoint/status provenance and map new IDs to
saved IDs. Scores are scoped to the rerun. Original saved reports are read-only.
The report browser's existing mark/display/export flow emits `--lang vitest`,
quotes checkout/report paths, and preserves marked source IDs; it does not execute
the exported command or require the current checkout to render it.

## Acceptance

The ignored `tests/vitest_workflow.rs` CLI test extends the six-mutation fixture:

- The original report has four kills and two misses (saved IDs 3 and 6).
- A stronger JavaScript test kills saved ID 3. Missed-only rerunning the original
  report selects exactly two mutations: one killed, one still missed; provenance
  maps new IDs 1/2 to saved IDs 3/6.
- A stronger TypeScript test and `--rerun-missed ... --id 6` select exactly one
  mutation and kill it. Original run/outcome files remain byte-for-byte unchanged.
- Changed manifest bytes, lockfile bytes, or a newly added allowed tsconfig are
  rejected with no baseline record. Source edits are rejected by list-mode
  selection; intersecting missed-only selection with an originally killed ID
  produces an empty selection.

Default tests cover changes to every saved execution-identity field. Browser
state/render tests cover missing identity, marked-ID export, explicit runner
selection, and paths containing spaces/apostrophes. They use the pinned Ratatui
0.30.1/Crossterm 0.29 setup; terminal lifecycle is unchanged and existing browser
PTY tests remain in the default suite.

```sh
MUTIE_VITEST_FIXTURE=/path/to/prepared-fixture cargo test --test vitest_workflow -- --include-ignored
cargo test
cargo fmt --check
cargo clippy --all-targets -- -D warnings
```

Vitest remains Experimental: this is fixture evidence for both JS and TS, not
real-project mutation acceptance. Dismissal review, CI policies, parallel workers,
broader lifecycle attribution, unmodified Pathe configuration support and useful
real-project findings remain separate gates.

Final validation (2026-10-15): all 231 default tests passed, with 35 opt-in tests
ignored. Both tests in the real CLI acceptance invocation passed, including the
JS/TS stronger-test reruns, metadata rejection, stale-source and empty-selection
checks. Formatting and all-target Clippy passed. The initial extended run exposed
a test-helper conflict between `--list` and `--output-dir`; after correcting that
invocation, the complete matrix passed against the unchanged CLI implementation.
