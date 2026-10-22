# Tomli-W candidate-quality review

This follow-up reviews all 19 original errors and the remaining `TYPE_CHECKING`
survivor from [unchanged acceptance](python-tomli-w.md). The already resolved
`dumps(multiline_strings=False)` survivor (canonical ID 6) stays separate.
All IDs below refer to the original 30-candidate inventory, matched through file,
source fingerprint, operator, byte span and replacement; rerun-local IDs differ.

## Findings

No discovery defect was demonstrated. Arithmetic replacement is deliberately
syntactic, including overloaded operators, and these candidates match the
[documented contract](python-arithmetic.md). Filtering by observed runtime types
would change that contract and could suppress valid custom-dispatch cases.
No production code, operator eligibility or outcome classification was changed.

| Canonical IDs | Count | Original error and review |
| --- | ---: | --- |
| 2 | 1 | Frozenset difference becomes unsupported addition during import; fatal collection error. |
| 4, 17, 19 | 3 | String repetition becomes unsupported division; body TypeError. |
| 14, 15, 16, 20, 21, 23, 26, 27, 28 | 9 | String concatenation becomes unsupported subtraction; body TypeError. |
| 3 | 1 | `< 0` becomes `<= 0`; existing zero-indent style test raises ValueError. |
| 7, 10, 24, 29 | 4 | Incorrect headers, multiline keys or escape slicing produce invalid TOML; original parser calls raise decoding errors, sometimes mixed with assertion failures. |
| 25 | 1 | Skipped string-formatting loop implicitly returns None; writer TypeErrors and parser failures mix with assertions. |

Thus 13 errors expose operand incompatibilities, four expose invalid serialized
output, one exposes a boundary rejection, and one exposes a loop-skip contract
failure. Chained IndexErrors in IDs 26/27 are caught by the original implementation;
the escaping failure is the subsequent string-subtraction TypeError.
The detailed [review ledger](fixtures/python-tomli-w-review/review.json) retains
each source identity, explanation, original report/local ID and stdout hash.
Full extracted diagnostic counts are retained externally at
`/tmp/mutie-python-tomli-w-review/error-diagnostics.json`.

ID 1 enables the imports guarded by `TYPE_CHECKING`. The extra names are used in
annotations or otherwise have no demonstrated public serialization consequence.
The full stronger suite still misses it. This is a low-value import/namespace
change for the tested API, **not a proof of equivalence**. No assertion against
private flags or import details was added merely to manufacture a kill.

## Test-only contract refinement

The original tests already detect these behavioral changes. This review does not
claim four newly discovered coverage gaps. The separate copy makes the writer's
valid-TOML contract explicit before checking round-trip equality:

- Only the parser call on writer output is inside `try`; its ValueError family
  becomes an explicit `pytest.fail` with the parser diagnostic.
- Writer calls, fixture loading and lifecycle operations are outside that handler.
  Unrelated TypeErrors, zero-indent rejection and cleanup failures are not wrapped.
- The complete original suite remains selected. No original assertion is removed.
- Four added exact-output tests cover root headers, array-of-table headers,
  newline escaping in keys, and resuming after escaped characters.

The pinned stdlib TOML parser uses a ValueError subclass for malformed TOML.
This scoped test edit is not a production classifier change or a general
exception-to-kill conversion. The retained
[patch](fixtures/python-tomli-w-review/validity.patch) and
[tests](fixtures/python-tomli-w-review/test_serialization_contracts.py) reproduce it.

## Rerun and controls

Frozen CLI from the original acceptance, CPython 3.14.7 / pytest 9.0.2:

- Pristine full baseline: **275 cases, 273 passed and the same two expected failures**.
- Eight selected candidates complete in **50.121 seconds**, including baseline.
- IDs **7, 10, 24, 29 are killed** by assertion evidence.
- IDs **3, 4, 25 remain errors**, checking that boundary rejection, arithmetic
  operand errors and the mixed None-return case are not converted wholesale.
- ID **1 remains missed**.
- Exit 1, failed report, unavailable score: the three errors retain precedence.

The result is **four kills, one survivor, three errors in a focused selection**;
it is not a full stronger-suite mutation score. Original accounting remains nine
kills, two survivors and 19 errors, with its failed scores unchanged.
No host permissions or test deadlines were changed. No repository-wide suite was
rerun for these fixture/documentation changes; the deferred full-suite startup
limitation from acceptance remains unresolved.

## Reproduction and preservation

Original upstream commit: `1210bb6f5708a4dc084cc3deb4a3a3e3939f4d6d`.
Original evidence: `/tmp/mutie-python-tomli-w-final/results.json`.
Separate copy: `/tmp/mutie-python-tomli-w-review/project`.
Only `tests/test_valid.py` differs and `tests/test_mutie_quality.py` is added.
All original implementation, metadata and fixture data bytes remain unchanged.
The review report is
`/tmp/mutie-python-tomli-w-review/run/reports/run-1792649176-eMbCAt`;
its receipt, command, hashes, baseline and canonical outcome mapping are in
`/tmp/mutie-python-tomli-w-review/run/results.json`.

To reproduce, copy the unchanged checkout, apply the retained patch and copy the
retained serialization tests to `tests/test_mutie_quality.py`. Use a fresh output
path with the harness:

```sh
python3 benchmarks/fixtures/python-tomli-w-review/run.py \
  --evidence /tmp/mutie-python-tomli-w-final/results.json \
  --project /tmp/mutie-python-tomli-w-review/project \
  --output /tmp/mutie-python-tomli-w-review/repeat \
  --python /tmp/mutie-pytest-audit-env/bin/python
python3 benchmarks/fixtures/python-tomli-w-review/verify.py \
  /tmp/mutie-python-tomli-w-review/run/results.json
```

Independent verification passes for original input hashes, both original report
sets (preliminary and final), the historical stronger-test reports, frozen CLI,
review source/report hashes, exact retained patch/tests, all 20 review entries
and source-mapped outcomes. Original checkout remains Git-clean. These results
support test diagnostic refinement; they do not justify changing discovery or
reclassifying historical errors.
