# Resolve Node once per Vitest run

Date: 2026-10-18. Follows the [feedback profile](vitest-feedback-profile.md).

## Behavior and identity boundary

A scored run resolves `node` through PATH once when capturing execution scope.
This also happens on every focused rerun, before comparison with saved identity.
Baseline and mutation checks invoke the captured absolute executable directly,
require Node 24.22.0, compare its reported executable path, and reload and compare
package, dependency, configuration and runner identity. The isolated adapter's
snapshot validation and exact-byte lifecycle instrumentation remain intact.
There is no fallback to the launcher if the pinned executable fails. PATH changes
within an active run do not select a different runtime; a new run resolves again.

This retains the existing executable-path/version identity contract, rather than
adding executable content hashing. Replacement with different reported identity
is rejected; replacing a binary with one reporting the same path/version is not
newly detected. No schema, package or operator catalog versions change.

## Regression checks

The default Node identity test exercises a path containing spaces, replacements
with wrong versions, relative/missing executable identities, malformed output,
nonzero exit, signal termination, missing/non-executable files and cancellation.
A scored CLI regression uses a counting launcher and a separate pinned executable:
changed paths, changed versions and a missing binary each fail at baseline after
exactly one launcher call. Existing saved-scope tests cover all persisted fields.

Validation passed:

- `cargo test --locked` (default unit/integration suite).
- `cargo clippy --locked --all-targets -- -D warnings`.
- `cargo fmt --check` and `git diff --check`.
- With `MUTIE_VITEST_FIXTURE=/tmp/mutie-vitest-fixture-scope` and the
  pinned Node directory first on PATH:
  `cargo test --locked --test vitest_workflow scored_runs_saved_evidence_stronger_tests_and_zero_exit_failure -- --ignored --exact`.
  This real runner regression passed in 81.15 seconds, covering scored JS/TS
  runs, stronger-test reruns, saved identity rejection, source/report preservation,
  report reading and early-exit infrastructure errors. Its time is not a performance
  comparison; default tests overlapped part of this acceptance run.

Local logs: `/tmp/mutie-node-reuse-{full-tests,clippy,real-tests,preview,tests}.log`.

## Controlled focused rerun

Built the parent commit `22c9dab` and saved its debug binary as
`/tmp/mutie-before-node-reuse`, then built the changed debug binary.
After all tests and Clippy finished, ran the binaries sequentially against the
unchanged `/tmp/mutie-hyparquet-scope` project and the two saved survivors
in `/tmp/mutie-hyparquet-reports/run-1792118088-p2pcbA`. Both used serial
full-suite execution and 120-second baseline/mutation deadlines.

The same PATH began with a counting shell wrapper that execs the actual
`/path/to/home/.vite-plus/bin/node` launcher. The wrapper counted calls but
returned the real Node executable identity; it did not bypass launcher resolution.

| Binary | External wall | Report duration | Baseline command | Mutation commands summed | Launcher calls |
| --- | ---: | ---: | ---: | ---: | ---: |
| Parent | 36.86s | 36.09s | 5.86s | 11.87s | 4 |
| Changed | 33.61s | 33.54s | 5.98s | 11.71s | 1 |

Reports:

- Parent: `run-1792346868-Vn9MHS`.
- Changed: `run-1792346904-BcS6LT`.

Both reports under `/tmp/mutie-node-reuse-reports` completed with two
survivors, zero errors/timeouts, matching execution scope, and successful shared
report-reader validation before the expected noninteractive-terminal error.
SHA-256 inventories of all files in the source project and original saved report
were unchanged after both runs.

This pair shows a 3.25-second (about 9%) wall-time improvement and confirms the
intended four-to-one launcher-call reduction. It is one ordered pair, not a stable
speedup estimate; host/launcher variation remains. It does not reproduce the older
58.81-second launcher case or explain the earlier 978-second sample. The entire
133-candidate Hyparquet campaign has not been run as part of this change.

Local reproduction harness: `/tmp/mutie-node-reuse-compare.py`.
Timings/counts: `/tmp/mutie-node-reuse-comparison.json`.
Logs: `/tmp/mutie-node-reuse-{before,after}.log`.
The scored command for each binary was:

```sh
mutie /tmp/mutie-hyparquet-scope --lang vitest --ui plain \
  --rerun-missed /tmp/mutie-hyparquet-reports/run-1792118088-p2pcbA \
  --output-dir /tmp/mutie-node-reuse-reports \
  --timeout 120 --baseline-timeout 120
```
