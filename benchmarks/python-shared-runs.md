# Scoped Python shared runs

The first Python slice now connects explicit `--lang python` to shared scored
runs, saved reports, focused reruns, dismissal review and score/findings policies.
Fixture success alone did not establish the milestone. Subsequent scored cleanup
and [Scrubify real-project acceptance](python-scrubify-findings.md) complete
the scoped **Validated** gate.

```sh
mutie tests/fixtures/python --lang python \
  --python /path/to/pinned-venv/bin/python --ui plain
mutie tests/fixtures/python --lang python \
  --python /path/to/pinned-venv/bin/python --rerun-missed /path/to/saved/run
mutie dismiss --module tests/fixtures/python --lang python \
  --python /path/to/pinned-venv/bin/python check
```

The launcher can also be set with `python = "/path/to/venv/bin/python"` alongside
`lang = "python"` in `mutie.toml`. Relative configured launchers resolve against
the project root; relative command-line launchers resolve against the invocation
directory. The venv launcher symlink is preserved. Serial, full-suite execution
and bounded captures are required. Boolean flips are the only supported operator.
`--list` validates scope/runtime and discovers without importing project code.

Before the baseline, an isolated pristine collection records the complete ordered
test IDs. The execution identity stores those IDs and test files. Every candidate
must collect exactly the pristine IDs and then complete those same tests.
Changes in parametrization or test selection are errors, even when the candidate
would otherwise pass its reduced suite. Snapshot validation compares all source,
test and manifest identities, preventing report exclusions from hiding code.

The shared command classifier retains complete failure evidence independently of
saved log limits. Pristine collection failures save a discovery command and
bounded logs before failing the run. Saved completed Python reports require a
valid runtime identity and complete pristine inventory. Browser rerun exports
include the recorded `--python` launcher.

Focused reruns validate runtime/package/manifest/recipe identity and shared
source identity. Stronger tests, additional test files and new command budgets
are permitted; each new run collects its own pristine inventory. Report/schema
and operator-catalog versions are unchanged.

## Fixture evidence

On 2026-10-20, all eight ignored Python workflow tests passed in 102.20 seconds
with CPython 3.14.7 / pytest 9.0.2 on macOS arm64. Evidence includes:

- Two candidates produce one kill and one survivor, with no errors, including
  when saved log data is capped at 32 bytes.
- Adding a separate stronger test and rerunning only the survivor produces one
  kill and no survivors. Original source bytes and original run/outcome files
  remain unchanged.
- Dismissal add/update/list/check/remove works for Python. Dismissing the survivor
  satisfies findings policy while the unchanged raw score still fails a 100%
  minimum-score policy.
- A changed manifest is rejected on focused rerun. Removing the saved pristine
  test IDs is rejected by the report reader.
- A source flip that reduces pytest parametrization is recorded as an error,
  with zero kills and an invalidated full-run score.
- An assertion during pristine collection produces a failed saved run and
  retained discovery logs. Log data stays within 64 bytes per stream; the
  shared truncation notice is additional.
- The earlier baseline, scope-rejection and stale-bytecode controls still pass.

Initial integration exposed debug-build stack overflows on ordinary Tokio
workers, including an existing Vitest baseline after the shared engine grew.
Nested command and dispatch futures are boxed without increasing worker stack
size. Actual scored CLI workflows and the existing Vitest preview regression
exercise these paths.

After the dispatch fix, the full default `cargo test` suite passed, including
the existing Vitest preview regression. The scored/review/stronger-test workflow
passed again in 66.86s. Clippy with warnings denied, formatting and diff checks
passed. Python-only optional fields are omitted from other runners' saved
configuration, preserving their existing report shape.

## Scored cleanup acceptance

On 2026-10-20, `scored_resource_limits_preserve_score_boundaries` passed its
three real CLI cases in 43.06 seconds. Each starts with a passing pristine
baseline and triggers the resource failure only after the production boolean
is flipped. An infinite loop becomes a timeout with no score denominator;
raw output exceeding an 8 KiB budget and a 5 MiB write exceeding the 4 MiB
per-file limit become errors with invalidated scores. None counts as a kill,
and original source bytes remain unchanged.

`scored_cancellation_cleans_owned_workspaces` passed in 15.33 seconds. The test
waits for the mutant body to write a checkpoint through pytest's `tmp_path`
fixture, sends SIGINT to the CLI, and checks the interrupted run and mutation,
zero kills, absent percentage and unchanged source. The entire dedicated
temporary parent is empty after shutdown, including the checkpoint's owned
workspace. It does not add subprocess or thread imports to the admitted scope;
the direct production-observer descendant controls remain documented in
[process integration](python-process-integration.md).

The [Scrubify acceptance](python-scrubify-findings.md) now supplies the real
finding and stronger-test rerun, completing the scoped support milestone.
Broader projects/layouts/dependencies remain separate follow-up work.
Final verification ran all ten ignored CLI workflow tests successfully in
172.93s; the expanded static scope suite contains thirteen cases. The full
default Cargo suite and all four direct native adapter/control tests also pass.
