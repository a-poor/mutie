# Second unchanged Python project: Cycler

Recorded 2026-10-20 with CPython 3.14.7 / pytest 9.0.2 on macOS arm64.
Upstream [matplotlib/cycler](https://github.com/matplotlib/cycler) is pinned at
`d1737eb0a42bd451e8770cef245ebe60ebb36bfc`.

This acceptance uses the **original checkout**, including its root-level
`test_cycler.py`, manifest, documentation configuration and CI script. Nothing
is installed, built, moved or rewritten for the initial runs. It is a second
unchanged project beyond Scrubify: **28 tests**, compared with Scrubify's six,
and three concrete boolean mutations. This is a modest increase in suite size,
not evidence for large applications or dependency-heavy projects.

The earlier [Cycler audit](python-cycler-audit.md) moved its test file and
replaced metadata. That historical adaptation remains separate evidence.

## Selection and scope

The existing project audits and additional checkouts were inspected before
selecting Cycler. Dacite's original pytest options require a benchmark plugin;
Sqids uses Hypothesis; Parsy's suite uses unittest; Humps reads installed package
metadata during import. Darija Tools additionally needs package-resource and
CLI access, while Numerizer has optional spaCy integration. Those requirements
were not admitted or bypassed. The original case-converter suite remains an
adapted-layout control rather than unchanged acceptance.

Cycler's runtime API requirements were already audited, so the narrow additions
are about layout and inert metadata:

- Inventory all root `test_*.py` files and all accepted `tests/test_*.py` files.
  Pass the full inventory to pytest and compare collected files and IDs with the
  pristine collection. Test-only stronger reruns establish a fresh inventory.
  Explicit `testpaths=["tests"]` with a root test is rejected, rather than
  silently hiding that test; `testpaths=["."]` is admitted.
- Recognize a flat setuptools package list with `py.typed` package data, and
  `dynamic=["version"]` only when its attribute points to a literal top-level
  string assignment in that package's initializer. No metadata hook runs.
- Recognize the audited optional `tests` tool names (`pytest`, `pytest-cov`,
  `pytest-xdist`) and `docs` names (`ipython`, `matplotlib`, `numpydoc`, `sphinx`)
  as declarative strings only. They are not installed; plugin autoload remains
  disabled, and pytest still runs serially without coverage or xdist.
- Retain and fingerprint `doc/source/conf.py` and direct `.circleci/*.py`
  scripts without importing or executing them. Test filenames, test definitions,
  pytest hooks and pytest/unittest imports in these files are rejected. Runtime
  imports remain restricted to the audited application package and allowed APIs.
  Other ancillary Python locations remain unsupported.

Existing flat/src source roots, bytecode isolation, strict lifetime evidence,
runtime fingerprints, source identities and score/error rules are unchanged.
The changed recipe is fingerprinted, so earlier recipes require a fresh run.

## Outcomes and test improvement

Native pytest, using the original manifest with plugin autoload and cache writes
disabled, passes all **28 tests**. Mutie's isolated baseline collects and passes
the same suite. The original full mutation run stops at line 390: changing
non-Cycler equality from `False` to `True` produces 14 assertion/pytest.fail
failures and one `KeyError`. The classifier correctly records an **error**,
leaves two candidates pending and publishes **no full score**.

A focused rerun of original IDs 2 and 3 kills both remaining candidates (length
and key-set equality checks at lines 392 and 394). That is a **2/2 selected
score**, not a repaired or combined full-project score. No original-suite
survivor is established.

Inspection of the error shows that `test_keychange` attempts to rename a key
after a composed cycle has already lost it. A separate complete project copy
adds exactly one assertion before that operation:

```python
c = (c1 + c2) * c3
assert c.keys == {'c', 'lw', 'edgecolor'}
c.change_key('c', 'color')
```

This checks the public composition contract before exercising rename. It does
not inspect the mutation or merely assert a boolean literal. Every original
assertion remains; production source and metadata remain byte-identical.
The stronger pristine suite still contains 28 test IDs and passes. A focused
rerun of original ID 1 now yields a completed assertion-body **kill**. This is
an **error-to-kill test diagnostic improvement**, not a newly discovered survivor
or a classifier relaxation. The original failed report remains failed.

The reproduction also runs all three mutations against the stronger copy to
verify a separate complete score. Exact outcomes, identities, timings, input
hashes and report hashes are retained in the results record below.

The final stronger full run completes **three kills, zero survivors, errors or
timeouts**, with a separate 100% full score. This does not replace the original
failed run or its unavailable score.

| Command | Wall time | Result |
| --- | ---: | --- |
| Discovery | 2.860s | Three candidates, original root test file |
| Native baseline | 2.588s | 28 passing tests |
| Isolated baseline | 12.432s | Same 28 tests, complete evidence |
| Original full run | 38.666s | One error, two pending; score unavailable |
| Original pending-ID rerun | 58.149s | Two kills; selected 100% score |
| Stronger error-ID rerun | 39.666s | One kill; selected 100% score |
| Stronger full run | 45.737s | Three kills; separate full 100% score |

## Reproduction and preservation

The [script](fixtures/python-cycler-unchanged/run.py) requires the pinned clean
checkout and a fresh output directory:

```sh
python3 benchmarks/fixtures/python-cycler-unchanged/run.py \
  --project /path/to/clean-pinned-cycler \
  --mutie target/debug/mutie \
  --python /path/to/pinned-venv/bin/python \
  --output /path/to/new-evidence-directory
```

It checks every original input hash and Git cleanliness after each command,
retains every saved report's hashes, and verifies that the stronger copy differs
only in `test_cycler.py`. The source project never receives stronger tests or
generated reports. [Results](fixtures/python-cycler-unchanged/results.json)
include full test inventories, mutation records, scores and command timings.
These are single workflow measurements, with other validation running on the
same machine; they are not a controlled performance comparison.
All 22 original input hashes and every prior report hash match after completion.
The retained final records were refreshed after the `doc` package guard; their
recipe fingerprints were recomputed from current production files and verified.

## Regression coverage

The native root-layout fixture covers full scoring, saved-report validation,
dismissal CRUD/check, raw-score versus findings policy, stronger reruns, manifest
drift and original source/report preservation. Its ancillary scripts raise if
executed; the successful workflow proves that they remain inert. Its stronger
test under `tests/` also checks mixed root/directory inventories.

Static regressions cover hidden root tests, forbidden ancillary tests/hooks,
nonliteral versions, external version attributes, extra packages and unreviewed
optional dependency names. Replay tests admit complete root-test evidence and
reject escapes, absolute paths, source-package tests and invalid file names.
An application package named `doc` cannot use the documentation exception to
bypass source validation: its `doc/source/conf.py` remains executable source,
is mutation-discovered, and must pass the same API checks as every other module.
Existing flat/src execution, cancellation, capture, failed-baseline and inventory
controls remain required. No package, report-schema or operator versions change.

## Final verification

- `cargo test --offline --bin mutie python::`: five unit/replay tests passed.
- Pinned Python `-I -S -B tests/support/python_scope.py`: 22 static tests passed.
- All 15 Python workflow checks passed across the regression invocation and
  targeted root/src runs. The root workflow passed in 100.35s. The initial
  regression invocation passed 13 checks but the src review fixture timed out
  both candidates at its five-second budget (its passing baseline took 5.13s).
  The shared report/layout fixture now uses 15 seconds, matching the existing
  layout control; the src rerun passed in 107.12s. Dedicated deadline/resource
  probes retain their short limits and passed. This changes test headroom, not
  production timeout or classification behavior.
- `cargo clippy --offline --all-targets -- -D warnings`, `cargo fmt --check` and
  `git diff --check` passed. Shared non-Python production code did not change;
  this is targeted Python verification rather than a claim of a full Cargo run.

The goal is complete: a second larger unchanged suite, complete original
candidate accounting with conservative errors, a useful test-only diagnostic
improvement, selected and full stronger-test results, preserved original inputs
and reports, measured feedback time, reproducible evidence and regression checks.
