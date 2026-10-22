# Tomli-W unchanged-project validation

## Audit and native control

Selected upstream: <https://github.com/hukkin/tomli-w>, commit
`1210bb6f5708a4dc084cc3deb4a3a3e3939f4d6d` (project version 1.2.0).
The original checkout is `/tmp/mutie-python-tomli-w-original`.
`/tmp/mutie-python-tomli-w-audit/original-inputs.json` records SHA-256
hashes for all 271 non-Git input files. The native control uses a separate,
unchanged copy at `.../mutie-python-tomli-w-audit/native`.

CPython 3.14.7 / pytest 9.0.2, with plugin autoload disabled and the native
`src` directory on the import path, runs the complete default suite:
**269 passed, 2 xfailed, 271 collected, exit 0**. Pytest reports 0.35 seconds.
The original input hashes still match after the control. Logs and a summary
are `native-baseline.log` and `audit-results.json` in the audit directory.
This is a native control, **not Mutie acceptance**.

The project has no application dependencies. Its development requirements
also name pytest-randomly and pytest-cov; this control does not install or load
them, consistent with the pinned serial, plugin-disabled Mutie recipe. No
tests, configuration, data, or source files were edited or omitted.

## Compatibility boundary

The complete suite requires these additions:

- Inert Flit, isort, tox, coverage and additional mypy metadata. Build tools,
  tox commands and optional profiling plugins must never execute. The original
  manifest remains fingerprinted. Strict pytest configuration must not enable
  arbitrary addopts or plugins.
- Named datetime classes and direct `type(value).__qualname__` formatting.
  General reflection remains rejected.
- The exact pinned-version import branch selecting standard-library `tomllib`
  over the inactive pre-3.11 `tomli` dependency. Static auditing must not execute
  project code or infer arbitrary unreachable branches.
- Test-only paths rooted in the copied input tree: recursive TOML fixture
  enumeration, `stem`, and binary reads. The profiler test additionally reads
  `benchmark/data.toml`. Fixture data and directory membership must participate
  in snapshot and execution identity, including report-exclusion checks.
- Binary output through `open(path, "wb")` under the runner-owned temporary
  test directory, followed by a read. No project-input writes, parent escapes,
  symlink escapes, arbitrary host reads, or unrestricted file modes.
- The profiler's `PROFILER_ITERATIONS` read must use a deterministic recorded
  environment value rather than inheriting the host's profiling workload.
- `benchmark/run.py` is retained but is not selected or imported; its optional
  benchmark dependencies must not become supported runtime imports.
- Two explicit expected failures for `qa-array-inline-nested-1000` and
  `qa-table-inline-nested-1000`. These occur before the serialization body.
  They must remain present in collection and lifecycle evidence, never become
  kills, and never allow a new skip/xfail or missing body to masquerade as a
  surviving mutation. Fixture and teardown failures remain errors regardless
  of expected-failure status.

## Implementation and acceptance

The compatibility implementation passes 48 static scope regressions. A production
`python-check` accepts all 271 nodes in 1.389 seconds of subprocess time, retaining
both expected-failure nodes in execution identity. Native controls verify that
adding/removing an expected failure produces an error, ordinary assertion failures
remain kills, and cleanup failures cannot hide behind an expected failure.

The project I/O audit hook checks calls originating in project frames, including
their callees. It admits copied inputs, pinned-runtime reads (including probes of
the fresh bytecode prefix), temporary output, and opening the null device. It
records a fatal event before rejecting an access, so catching `PermissionError`
does not make the run pass. This complements static admission and post-run input
hashes; it is not a general Python security sandbox. The native controls exercise
input reads, temporary output, attempted input writes/removal, external reads,
writes and directory scans, inherited descriptors and symlink escape attempts.

Non-Python input files and directory membership join snapshot/execution identity.
This conservatively requires a fresh run after data or directory changes;
stronger tests added within the existing tests directory remain supported.

Preliminary complete accounting is retained at
`/tmp/mutie-python-tomli-w-acceptance/results.json` using its frozen CLI.
All 30 candidates are accounted for: **9 killed, 2 missed, 19 errors**. The initial
run stops at the collection error from replacing a set difference with addition;
the remaining 28 are selected from its preserved inventory with bounded error
continuation. Both original reports remain failed and unscored. This combined
accounting is not a full mutation score.

The useful survivor is canonical ID 6, changing `dumps()`'s default
`multiline_strings=False` to `True`. Round-trip parsing still succeeds, but a
newline-containing string changes from an escaped single-line value to a
multiline value. A separate unchanged copy plus only
`tests/test_mutie_stronger.py` asserts the default serialized representation.
Its full 272-node pristine baseline passes (270 passing bodies, the same two
expected failures), and the focused rerun kills the survivor in 12.488 seconds.
The other survivor is `TYPE_CHECKING=False`; no useful behavioral finding or
equivalence claim is established for it.

The final replay uses the frozen CLI and results ledger at
`/tmp/mutie-python-tomli-w-final/results.json`. It reproduces all 30
outcomes after the directory-identity and null-device refinements:

| Run | Wall time | Result |
| --- | ---: | --- |
| Original full selection | 22.110 s | Stops after canonical ID 2 collection error |
| Remaining 28 candidates | 135.660 s | Complete bounded accounting |
| Stronger-test ID 6 rerun | 13.152 s | 272-node baseline; survivor killed |

The inventory contains 12 boolean flips, 2 comparison boundaries and 16 binary
arithmetic replacements. The 19 errors comprise one fatal collection error and
18 body errors. Classification and original failed-report scores are unchanged.
The initial `python-check` rejection of bytecode-prefix import probes is retained
in the audit directory alongside the successful follow-up.

The replay harness, stronger assertion and independent preservation verifier are
retained under `benchmarks/fixtures/python-tomli-w/`. Verification checks both
preliminary and final input/report hashes, all outcomes, frozen CLI hashes,
expected-failure inventories, data identities and the separate stronger copy.
The preliminary test has one extra trailing blank line; the final copy matches
the retained assertion byte-for-byte. All 271 original input files remain
unchanged and the upstream checkout remains clean.

## Regression validation

All ten opt-in production Python regression groups pass (123.86 seconds), covering
body/lifecycle attribution, conftest and unittest, expected failures, guarded I/O,
output/file limits, descendant cleanup, timeout/cancellation and shared execution.
The 48 static scope controls pass, as do formatting and all-target lint checks.
Two default suite attempts hit a Go baseline timeout in
`merged_settings_drive_execution_and_are_saved_in_reports`; its unchanged focused
rerun passes, but the serial suite reproduces the timeout. Logs are retained as
`full-suite.log`, `full-suite-2.log`, `configuration-rerun.log` and
`full-suite-serial.log` in the audit directory. No deadlines, assertions or host
security settings were changed. The startup investigation remains deferred; these
results do not establish its cause or claim a passing repository-wide suite.

The serial follow-up passed 203 unit tests and completed the affected-package,
Bun, cancellation and capture-limit targets before the configuration failure.
It then stalled for more than four minutes launching the dismissals executable,
without announcing any tests, and was interrupted. Later targets were not
validated by that run. This is a recorded regression-check limitation, not a
Python acceptance failure or evidence that the deferred startup issue is fixed.

The [candidate-quality follow-up](python-tomli-w-review.md) now reviews all 19
errors and the remaining survivor, with four test-only error-to-kill reruns and
unchanged error/survivor controls. It establishes no discovery defect.
