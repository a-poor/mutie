# Python execution overhead

This follow-up profiles the validated pinned Python scope on macOS arm64 using
Scrubify's complete six-test suite and the retained three stronger blank-query
tests. The reference is `24aff2b`; both executables are development builds with
the existing `sha2` optimization. No package/schema/catalog version changes.

## Measurement

The [reproduction instructions](fixtures/python-profile/README.md) describe the
serial CLI comparison and ignored adapter phase probe. Compilation is outside
the measurements. Three full-run/stronger-test-rerun pairs alternate build order.
Each build creates its own fresh full report before the stronger-test rerun.
Filesystem caches are warm; this is one machine and a small real project.

The test-only phase probe times the production adapter: initial runtime identity,
original scope, execution identity, workspace copy, copied scope, private
command setup, collection and test subprocesses, repeated runtime/scope checks,
and diagnostics. Adapter elapsed time includes cleanup on return. It does not
include the CLI's initial admission or saved-report coordination.

Initial profiling identified repeated runtime tree traversal as an avoidable
cost. `Path.rglob` enumerated excluded cache/site directories and repeated path
construction, relative-path parsing and filesystem metadata queries for each
included entry. Pytest process startup is another substantial cost because each
process uses a fresh bytecode prefix; native pytest timings with existing caches
are not a direct measure of that isolated startup cost.

## Results

All **12 CLI runs** completed with matching inventories, outcomes, runtime hashes
and test IDs. Sources and original reports remained unchanged. The
[retained measurements](python-feedback-profile-results.json) include the exact
commands, identities and per-run preservation hashes.

| Workload | Reference samples (seconds) | Candidate samples (seconds) | Reference median | Candidate median |
| --- | --- | --- | --- | --- |
| Full three-mutation run | 40.114, 45.425, 49.607 | 45.456, 36.978, 64.287 | 45.425 | 45.456 |
| Two-survivor stronger-test rerun | 51.852, 33.064, 31.321 | 27.932, 28.446, 33.725 | 33.064 | 28.446 |

The observed focused median is **14.0% lower**. The full-run median is effectively
unchanged, and the candidate full-run range is wide. These samples do **not**
establish a consistent end-to-end full-run speedup. Pytest subprocess durations
also varied substantially; runtime traversal is only one part of total feedback.
Historical milestone timings (28.38s full, 23.49s focused) are not the reference
for this comparison: they were measured in a different session.

A separate five-pair alternating calibration within one pinned interpreter
isolates the actual full-content stdlib/package scans from startup. Their combined
median falls from **0.503s to 0.330s (34.3%)**, with identical hashes on every
sample. All included file contents are reread; these are warm-filesystem scans,
not cached identity lookups. This supports retaining the localized traversal
optimization without claiming that every complete run becomes faster.

The three-baseline adapter probe records the following medians. Before/after
probe batches ran separately, so changing pytest durations also reflect timing
variation; their total difference is not attributable entirely to the walker.
Component medians need not sum to the total median.

| Adapter component | Reference median | Candidate median |
| --- | --- | --- |
| All three runtime checks | 5.017s | 2.878s |
| Collection + test subprocesses | 4.459s | 3.491s |
| Scope checks + evidence validation | 1.581s | 1.439s |
| Workspace copy | 0.0037s | 0.0030s |
| Complete adapter, including cleanup | 11.104s | 7.923s |

This identifies isolated pytest startup and remaining runtime/scope checks as
future profiling targets. It does not justify weakening fresh-bytecode or drift
checks, or adding workspace caching for a cost measured in milliseconds.

## Optimization and correctness boundary

`runtime.py` now walks with `os.scandir`, reuses directory-entry metadata and
prunes the already-excluded `__pycache__` and standard-library `site-packages`
subtrees. It sorts relative path components exactly as the original `Path`
ordering and retains the existing filename/content length framing and SHA-256
format. Every admitted file is still opened and hashed on every identity check.
No persistent fingerprint cache or size/mtime shortcut is introduced.

Runtime checks still occur before execution and after collection and testing.
The executable, shared library, venv configuration, pinned package versions,
stdlib and package content hashes remain part of identity. Project/snapshot
checks, pristine collection IDs, fresh bytecode prefixes and process-lifetime
attribution are unchanged. Directory scanning errors now propagate rather than
being silently suppressed by glob traversal.

The implementation changes the recipe fingerprint, as intended. Existing
reports remain readable, but a new full run is required before focused reruns
with this recipe. This is separate from runtime content hashes, which must match
between both builds. There is no broader Python support claim.

## Regression evidence

`tests/support/python_runtime.py`, invoked by the ignored
`runtime_fingerprint_regressions` CLI test, covers:

- Byte-for-byte fingerprint compatibility with the prior format, including
  component sorting, Unicode/hidden files, empty files and ignored caches.
- No traversal of excluded subtrees; retained traversal of a directory whose
  name ends in `.pyc`, matching the prior included-child behavior.
- Same-size edits with restored mtime, additions, removals and renames changing
  the fingerprint; restoring original content restores the fingerprint.
- Allowed stdlib file symlinks, rejected package/broken/directory symlinks,
  nonregular files, startup hooks, editable-install hooks and scan failures.

The comparison harness checks matching full and selected inventories and
outcomes, runtime hashes and test IDs. Every full run must produce one kill and
two survivors; adding the stronger tests must produce one kill and one survivor
in the focused rerun, with no errors/timeouts. It verifies all source bytes and
all original report files after rerunning.

## Final checks

- `cargo test --offline --bin mutie python::` — four unit tests passed; five
  opt-in native/profile tests ignored in that invocation.
- `MUTIE_PYTHON=... cargo test --offline --test python_workflow -- --ignored --test-threads=1`
  — all 11 workflows passed, including the five runtime-tree regression cases.
- `MUTIE_PYTHON=... cargo test --offline --bin mutie python::tests:: -- --ignored --test-threads=1`
  — all four direct native attribution/resource/descendant/shared-execution tests passed.
- Both phase probes passed all three isolated baselines; all 12 comparison runs
  passed their complete inventory/outcome/hash assertions.
- `cargo clippy --offline --all-targets -- -D warnings`, `cargo fmt --check`
  and `git diff --check` passed.
- Both prior Scrubify milestone report identities and all 28 original report
  files still match the retained milestone hashes.

This follow-up changes runtime traversal and adds measurement/regression tools.
It does not broaden Python scope or alter scoring, command deadlines or lifecycle
classification. Native `src/` layout support with unchanged-project acceptance
would be a separate bounded milestone.
