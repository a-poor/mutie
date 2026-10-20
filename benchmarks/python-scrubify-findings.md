# Python / pytest scoped milestone acceptance

On 2026-10-20, the scored Scrubify workflow completed the real-project
run → inspect → improve tests → focused-rerun loop. This validates only the
documented dependency-free, serial, pinned Python scope. It does not establish
general pytest compatibility or support for Scrubify's original `src/` layout.

## Reproducible inputs

- Project: [MartiAble/scrubify-py](https://github.com/MartiAble/scrubify-py), commit
  `558265805d942c03d2e6d8522baa0ca2992ec067`.
- Runtime: CPython 3.14.7, pytest 9.0.2, pluggy 1.7.0, packaging 26.3,
  iniconfig 2.3.1 and pygments 2.21.0 in a dedicated virtual environment;
  macOS arm64. Mutie used the development build with optimized `sha2`.
- The [preparation script](fixtures/python-scrubify/prepare.py) requires a
  pristine pinned checkout. It moves `src/scrubify/` to the project root,
  retains the entire test directory, copies the license/README and supplies
  minimal static project/test metadata. All production and test bytes are
  unchanged. No tests or assertions are removed, and no packaging hooks execute.
- Original native pytest: **6 passed in 0.04s**, plugin autoload disabled,
  fresh bytecode prefix and cache provider disabled. The upstream checkout
  remains clean. The complete test suite is small; this is not performance or
  compatibility evidence for large Python applications.

The scope additionally admits only named string-processing helpers from
`fnmatch` and `urllib.parse`; urllib networking modules remain rejected.
Earlier [case-converter](python-case-converter.md) evidence covers a larger
107-test suite and conservative runtime-error handling. These adaptations do
not claim support for source layouts, build metadata or imports outside the
audited contract.
The [Cycler audit](python-cycler-audit.md) additionally verifies a 28-test
baseline and conservative handling of mixed assertion/runtime failures.

## Finding and stronger tests

The first complete run found three boolean flips and completed in **28.38s**:
one kill, two survivors, no errors/timeouts, **33.33% raw full-run score**.
The default `preserve_length=False` mutation was killed by the original tests.

The useful survivor changes `parse_qsl(..., keep_blank_values=True)` to `False`
at `scrubify/core.py:73`. Existing tests covered nonempty secret values only.
The mutant silently drops empty query parameters, including nonsensitive
parameters whose presence can matter to the consuming application. Redaction
should mask sensitive values while retaining the rest of the query.

The retained [three stronger tests](fixtures/python-scrubify/test_blank_queries.py)
check public URL outputs:

- `?next=&mode=login#finish` retains the empty `next` value and fragment.
- `?token=&next=` masks the empty sensitive value and retains both keys.
- `?tag=one&tag=&flag` preserves repeated values and normalizes the bare flag
  to `flag=` instead of removing it.

Adding that file produces a passing **9-test isolated baseline**. An actual
`--rerun-missed` against the first saved report selects both survivors and
finishes in **23.49s**: one kill, one survivor, no errors/timeouts. The blank-query
mutation is now an assertion kill. Its selected-run score is **50%**; the original
33.33% full-run score remains unchanged.

The remaining `urlencode(..., doseq=True)` survivor at line 78 appears redundant
for the documented string-mask inputs: `parse_qsl` yields strings and masking
also yields strings. This is a bounded explanation, not a universal equivalence
claim for unsupported argument types or future implementation changes. It remains
a recorded survivor; no dismissal is needed to establish the stronger-test loop.

## Preservation and records

All original prepared input hashes match after the rerun. All **28 original
report files**, including logs, hashes and saved outcomes, match their recorded
SHA-256 values. The [results record](fixtures/python-scrubify/results.json)
retains input hashes, exact original/rerun inventories, report hashes and scores.
Only the added stronger-test file changes the test inventory.

Local reports:

- `/tmp/mutie-python-scrubify-reports/run-1792524950-SeRYbK` — original.
- `/tmp/mutie-python-scrubify-reports/run-1792525001-74NIpJ` — focused rerun.

The commands use explicit `--lang python --python VENV/bin/python`, serial full
tests, a 10-second candidate command deadline and 20-second baseline deadline.
Wall-clock feedback includes runtime/stdlib hashing and fresh workspace checks;
native pytest timing alone does not describe Mutie's overhead.

## Completion evidence and limits

| Requirement | Evidence |
| --- | --- |
| Strict original/snapshot scope and source spans | `tests/support/python_scope.py`, `python/project.rs` unit tests and [baseline integration](python-baseline-integration.md) |
| Pristine/candidate test inventory equality | `mutation_cannot_change_parametrized_inventory` and strict `python/protocol.rs` replay tests |
| Isolated baseline, scored discovery/execution and saved reports | Complete Scrubify run above; `tests/python_workflow.rs` |
| Identity-checked stronger-test reruns | Actual Scrubify rerun; runtime/recipe/manifest drift unit controls; corrupt inventory and changed-manifest CLI controls |
| Review and CI policies | Fixture dismissal CRUD/check, findings-policy acceptance and unchanged raw minimum-score rejection in `scored_reports_and_stronger_test_reruns` |
| Lifetime, timeout, cancellation and resource limits | [Production controls](python-process-integration.md) and [scored cleanup](python-shared-runs.md#scored-cleanup-acceptance) |
| Original source and old artifact preservation | Retained hashes above plus fixture workflow regressions |

Only assertion/`pytest.fail` failures with complete call/setup/teardown and
process-lifetime evidence count as kills. Runtime exceptions, collection/hook
errors, warnings, incomplete evidence, late failures and ambiguous outcomes
remain errors. A failed run has no full score. Ordinary descendants are shut
down; starting threads is rejected. Per-file Unix limits are not a total disk
quota, and the static scope validator is not a security sandbox.

The scope excludes application dependencies, user plugins/conftest, unittest,
async suites, thread-starting suites, multi-package layouts, editable installs,
namespace packages, parallel execution and affected-test selection. Broader
project/runtime acceptance and additional useful findings are follow-up work,
not implied by this scoped milestone.

Final verification on the completed worktree:

- `cargo test` — passed, including 195 unit tests and all non-ignored integration
  tests; opt-in language workflows remain ignored by default.
- `MUTIE_PYTHON=... cargo test --test python_workflow -- --ignored --test-threads=1`
  — all **10** native CLI workflow tests passed in **172.93s**.
- `MUTIE_PYTHON=... cargo test --bin mutie python::tests:: -- --ignored --test-threads=1`
  — all **4** direct native adapter/control tests passed in **47.74s**.
- The CLI scope regression includes **13** static admission/rejection tests.
- `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check` and
  `git diff --check` — passed.

No package, report-schema or operator-catalog version was bumped.
