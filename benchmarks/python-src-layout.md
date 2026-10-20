# Native Python src layout acceptance

On 2026-10-20, Mutie completed the pinned Scrubify run → inspect → stronger tests
→ focused rerun workflow using the project's **original layout and manifest**.
No source/test files moved, no metadata was rewritten, no tests were removed,
and no project installation or build backend was executed.

## Audited boundary

The supported layout is exactly one ordinary importable package, either
`package/` or `src/package/`, with tests under `tests/`. Both the original tree
and every isolated snapshot are audited. Ambiguous flat/src roots, multiple
packages, namespace packages, executable files outside the selected package/tests,
symlinks and ambiguous module/package names are rejected. Report exclusions do
not bypass scope validation or the original/snapshot test inventory check.

The unchanged Scrubify manifest declares static setuptools metadata:

- `[build-system]` with `setuptools.build_meta`, `setuptools>=69` and `wheel`.
  These are metadata only: no installation, backend invocation or hook occurs.
- A test extra containing `pytest>=8.0`. The only accepted optional group is
  `test`, containing bare `pytest` or `pytest>=X[.Y[.Z]]` requirements without extras, URLs or markers.
  A minimum version may not exceed pinned pytest 9.0.2. This does not admit
  application dependencies or install anything from that extra.
- `pythonpath = ["src"]` and `addopts = "-q"`. The audited import root is applied
  explicitly in the isolated driver; quiet output is already part of its fixed
  recipe. Filters, arbitrary options and alternate/multiple import roots fail.
- `package-dir = {"" = "src"}` and `packages.find.where = ["src"]`.
  Other setuptools tool settings, dynamic/package remapping and exclusion rules
  remain unsupported. Optional `testpaths` must still be exactly `["tests"]`.

A src layout can also use minimal static metadata without these optional tables.
The validator determines the single actual package from files and validates any
configuration against it. The `src` directory itself is not required to be a
Python package. A flat package literally named `src` retains its existing
root semantics when `src/__init__.py` is present. Relative import resolution and namespace checks operate from the
selected import root; mutation byte offsets and source paths retain `src/`.

## Identity, isolation and evidence

`source_root` is captured in project scope and saved Python execution identity.
The empty root denotes the existing flat layout; `src` is the only other value.
The field defaults to empty for reading earlier flat records. Saved mutations
must remain inside the selected source root/package; changing the root rejects
both snapshot comparison and focused rerun identity checks. Manifest bytes are
still fingerprinted, and stronger-test reruns still require pristine test IDs.
No package, report-schema or operator-catalog version was changed.

The driver inserts only the owned import root into `sys.path`. Application
modules observed after pytest returns must resolve to their exact owned module
or package file. An external/missing application origin invalidates completion,
including when an assertion would otherwise look like a kill. Protocol evidence
also rejects parent traversal in returned import paths. These controls complement
static import restrictions; this is not a security sandbox.

Runtime/content/recipe fingerprints, fresh bytecode prefixes, bounded captures,
full-test selection, conservative assertion-only kills and lifecycle cleanup
remain in force. Implementation bytes change the execution recipe; earlier
reports remain readable, but rerunning with this recipe requires a fresh full run.
The scope does not admit dependencies, build execution, editable installs,
plugins/conftest, unittest, async/thread-starting tests or multiple packages.

## Unchanged real-project loop

Repository: [MartiAble/scrubify-py](https://github.com/MartiAble/scrubify-py), commit
`558265805d942c03d2e6d8522baa0ca2992ec067`. Environment: CPython 3.14.7,
pytest 9.0.2 and the existing pinned dedicated venv on macOS arm64. The development
build retains the adopted `sha2` optimization. Timings are observations from this
acceptance run, not a controlled performance comparison.

| Step | Evidence | Wall time |
| --- | --- | --- |
| Read-only discovery | Three flips in `src/scrubify/core.py`; original manifest admitted | 2.72s |
| Isolated baseline | All six original tests pass | 13.87s |
| Full scored run | One kill, two survivors, no errors/timeouts; 33.33% full score | 40.89s |
| Separate-copy stronger-test rerun | Nine-test baseline; one kill, one survivor; 50% selected score | 32.63s |

The useful survivor is the previously investigated `keep_blank_values=True`
flip at line 73. Changing it to false drops empty query parameters. The
[retained three tests](fixtures/python-scrubify/test_blank_queries.py) check blank
nonsensitive values and fragments, masking an empty secret while retaining keys,
and repeated/blank query fields. They are added only to a separate complete copy
of the unchanged project. `--rerun-missed` then kills that survivor with a real
assertion failure and complete process evidence. This revalidates the earlier
finding in the native layout; it is not a claim of a newly discovered defect.

The `doseq=True` survivor at line 78 remains a recorded survivor: it appears
redundant for the documented string-mask inputs, not universally equivalent.
The original full report and its score are unchanged by the selected rerun.

The [results record](fixtures/python-src-layout/results.json) retains all original
input hashes, the stronger-copy hashes, discovery scope, baseline IDs, execution
identities, commands, scores, inventories and complete report hashes. The
[reproduction script](fixtures/python-src-layout/run.py) requires the clean pinned
checkout, runs discovery/baseline/full directly against it, and makes only a
separate stronger-test copy. It asserts the original checkout remains clean and
unchanged and every original report file is preserved.

```sh
python3 benchmarks/fixtures/python-src-layout/run.py \
  --project /path/to/clean-pinned-scrubify \
  --mutie target/debug/mutie \
  --python /path/to/pinned-venv/bin/python \
  --output /path/to/new-evidence-directory
```

## Regression coverage

- Eighteen static scope tests cover ordinary/nested src imports, source offsets, unchanged
  metadata, forbidden filters/import roots/extras/remapping, ambiguous roots,
  namespace packages, symlinks, extra executable files and relative import escapes.
  A separate native scored fixture preserves a flat package named `src` with subpackages.
- Rust unit tests reject source-root snapshot drift, changed test inventory,
  saved root/manifest/recipe/runtime drift, escaped source paths and returned
  import paths; source-coordinate and stale-file checks remain covered.
- The src fixture exercises full scoring, saved-report browsing validation,
  dismissal CRUD/check, findings versus raw-score policies, stronger-test reruns,
  source/report preservation, wrong saved source roots and manifest drift.
- Existing flat-layout workflows and native lifetime/resource/cancellation
  controls run separately; a direct forged application-origin probe must fail
  closed even after an assertion failure.

## Verification and completion audit

The acceptance results were refreshed after the flat-package-named-`src`
compatibility fix. Their recipe fingerprint was recomputed from the current
production source files and matched both final reports.

| Goal requirement | Verified evidence |
| --- | --- |
| Original layout and static metadata, no installation/build | Clean pinned checkout and all original hashes; discovery and six-test isolated baseline |
| Scored native src execution | Complete three-mutation run, one kill/two survivors, all files under `src/scrubify/` |
| Useful survivor → stronger tests → focused kill | Separate complete copy with only `tests/test_blank_queries.py` added; nine test IDs, one kill/one survivor |
| Original inputs and prior reports preserved | Original Git status clean; all original/stronger-copy input hashes and full report hashes match |
| Flat workflow compatibility | Existing flat workflow checks plus a scored flat `src` package with a nested package |
| Strict scope and import ownership | 18 static cases; source-root snapshot/identity/path controls; forged application-origin native rejection |
| Reports, reruns, review and policies | Src fixture: browser validation, wrong-root rejection, dismissal CRUD/check, raw score versus findings policy, stronger rerun and manifest drift rejection |
| Cleanup and conservative failures | Existing resource/cancellation workflows and all four direct native process tests, including the new origin rejection |
| Documentation and scope limits | README, DESIGN, TODO, SUPPORT and AGENTS synchronized; no version changes or dependency/plugin/build-execution expansion |

Validation passed across the relevant invocations:

- `cargo test --offline --bin mutie python::` — four unit tests passed.
- Existing Python workflow regression invocation — 12 checks passed, including
  the non-ignored option rejection and 11 native workflows.
- `src_scored_reports_and_stronger_test_reruns` — passed separately.
- `flat_package_named_src_preserves_native_execution` — passed separately.
  This layout control uses a 15-second candidate budget, matching baseline
  startup headroom; separate resource tests retain short-deadline checks. An
  initial five-second-budget run during concurrent validation missed its expected
  survivor count; a standalone rerun passed before adding this headroom.
- Four direct `python::tests::` native tests passed, including body/lifetime
  attribution, capture/file bounds, descendant shutdown and shared mutation execution.
- The final unchanged real-project script passed discovery, baseline, full score
  and focused rerun checks. All retained report and input hashes were rechecked.
- Clippy with all targets and warnings denied, formatting and diff checks passed.

The first real-project pass also succeeded; the retained results are from the
refreshed final recipe. The timings above exclude compilation and should not be
compared with earlier profiling as a controlled speed measurement.
