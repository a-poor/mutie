# Scoped pytest conftest fixtures — validated acceptance

The audited fixture extension is Validated with native lifecycle controls and an
unchanged public project. This does not admit arbitrary conftests, hooks, plugins
or fixture frameworks. Evidence below was collected on 2026-10-20.

## Audited scope

CPython 3.14.7 / pytest 9.0.2, the existing dependency-free flat/src package and
full serial test scope. Admit only root `conftest.py` and direct
`tests/conftest.py`, with audited imports, literal module data and synchronous
`pytest.fixture` declarations. Function, class, module and session scopes are
supported, with literal autouse/name/options and literal params/ids sequences
(or references to literal module data). General helpers, classes, hooks,
pytest_plugins, collection overrides, dynamic scopes/IDs, nested conftests and
arbitrary dependencies remain rejected. The common import/runtime audit still
applies inside fixture bodies.

Request access is limited to param, addfinalizer and literal ordinary fixture
lookup. Request aliasing, configuration/internal access and lookups of framework
configuration fixtures are rejected. This is a compatibility boundary, not a
security sandbox for hostile Python programs.

The driver replaces `--noconftest` with an explicit root `--confcutdir`. Its
observed loaded conftest paths must equal the static inventory. Snapshot checks
cover conftest bytes and paths. Saved execution identity binds fixture files;
changing/adding/removing a conftest requires a fresh run. Separate ordinary
stronger test files remain compatible with focused reruns.

## Attribution

Audited pinned pytest FixtureDef setup and finish behavior. Finalization drains
registered callbacks in LIFO order and retains failures, including multiple
failures. Fixture setup can also happen inside a test body through
getfixturevalue; a body can catch that exception. Call-phase status therefore
cannot establish body attribution alone.

The observer independently records setup/finalization errors and requires an
ordered started/ready/finished ledger for each fixture incarnation. Cached use
and redundant finalization callbacks do not create additional incarnations.
Session finish requires an empty active ledger and intact finalization observer.
Unknown/error events, incomplete/reordered evidence and loaded-config mismatch
fail closed. Existing exact body-exception, pytest phase, unittest dispatch and
process-lifetime requirements remain in force. Recipe changes invalidate older
execution identities without a report/operator/package version bump.

## Reproducible checks

Use the environment launcher without resolving its virtualenv symlink:

```sh
export MUTIE_PYTHON=/tmp/mutie-pytest-audit-env/bin/python
cargo test --offline --bin mutie native_conftest -- --ignored --nocapture
cargo test --offline --test python_workflow conftest_ -- --ignored --test-threads=1
"$MUTIE_PYTHON" -I -B tests/support/python_scope.py
cargo test --offline --bin mutie python::
```

On 2026-10-20, the native probe passed 55 cases in 96.29 seconds: four scopes
against 12 pass/body/setup/yield/finalizer/dynamic-lookup cases, four shared
parametrized dependency/autouse controls and three unittest/autouse controls.
Setup errors remain errors even when caught by a subsequently failing body.
Mixed body/finalizer, multiple finalizer and empty/double-yield failures remain
errors. The 31 static scope tests pass.

The three scored conftest workflow tests passed in 97.37 seconds: saved cleanup
errors dominate body assertions; parent conftests cannot execute; an output-dir
snapshot exclusion cannot hide the fixture module; full reports, policies,
dismissals and stronger ordinary-test reruns work; fixture edits reject reruns.
Replay controls require every fixture stage/token and the exact conftest inventory.
Execution identity rejects a missing fixture inventory; snapshot unit controls
explicitly reject added, removed and byte-changed conftests.

The focused Rust Python unit suite passes seven tests. Clippy with all targets
and warnings denied passes, as do formatting and diff checks. The complete
default suite passes with `cargo test --offline -- --test-threads=1` after the
final scope changes. Earlier concurrent runs hit Go cancellation timing failures;
those failures are retained in local logs, not represented as Python regressions
or as passing checks. The final complete ignored Python workflow suite passes
all 19 tests in 472.48 seconds with the final executable held fixed, including
conftest, unittest, flat/src/root layouts, resource limits and cancellation:

```sh
MUTIE_PYTHON=/tmp/mutie-pytest-audit-env/bin/python \
  cargo test --offline --test python_workflow -- --ignored --test-threads=1
```

## Selected unchanged fixture project

The Binary Tree Library example project at
<https://github.com/burpeesDaily/sphinx-sample-code>, commit
`68d688fb4fe9c867c77a9d54f4fdb46a58dea34e`, uses `tests/conftest.py` to share an
11-pair tree dataset across its seven tests. It is a small public example library,
not evidence for large dependency-heavy applications. Its original random.sample
traversal test remains unchanged and unseeded. Native and isolated baselines pass;
all 11 boolean sites are included in the complete run.

The unchanged layout requires a few bounded compatibility additions: a root
`pytest.ini` containing only full-scope testpaths, an empty `tests/__init__.py`,
test-only `random.sample`, direct `Exception.__init__(self, ...)` inside a direct
Exception subclass constructor, and inert `docs/source/conf.py` metadata. No
requirements are installed, no Sphinx config is executed and no suite is trimmed.
The ini and empty initializer are included in snapshot checks and saved identity;
adding, removing or changing them requires a fresh run. Scope controls reject
other ini options, competing pytest config, nonempty test initializers, random
seeding/aliasing and unrelated Exception dunder calls.

The retained reproducer is
[run.py](fixtures/python-conftest-trees/run.py), with separate-copy
[boundary tests](fixtures/python-conftest-trees/test_empty_traversals.py).

## Complete real-project results

| Run | Tests | Killed | Survived | Errors/timeouts | Seconds |
| --- | ---: | ---: | ---: | ---: | ---: |
| Native original baseline | 7 | — | — | 0 | 1.06 |
| Isolated original baseline | 7 | — | — | 0 | 5.66 |
| Original full run | 7 | 5 | 6 | 0 | 77.08 |
| Original focused survivor rerun | 7 | 0 | 6 | 0 | 47.00 |
| Native stronger baseline | 16 | — | — | 0 | 1.33 |
| Stronger focused rerun | 16 | 6 | 0 | 0 | 44.98 |
| Stronger full run | 16 | 11 | 0 | 0 | 76.44 |

Times are end-to-end commands on this machine, not generalized performance claims.
The original full score remains 5/11 (45.45%). Its six survivors are two AVL
`empty` property returns and four default `recursive=True` traversal arguments.
Nine added cases cover emptiness before/after insertion using the existing
`basic_tree` fixture and empty default traversals for both tree types. The latter
expose a real behavioral difference: nonrecursive empty generators raise a
RuntimeError from StopIteration, while the original recursive defaults yield no
pairs. The stronger tests convert that unexpected RuntimeError into an explicit
pytest.fail assertion; runtime exceptions are not newly classified as kills.
All six are observably different, with no equivalence claims needed.

The separate copy adds only `tests/test_empty_traversals.py`. Original source,
tests, conftest, metadata and all other input bytes match before/after, the Git
checkout remains clean, and each earlier saved report is rehashed after every
subsequent command. Full hashes, execution identities, inventories, outcomes and
timings are retained in [results.json](fixtures/python-conftest-trees/results.json).
These results validate this small example library, not arbitrary dependencies,
large suites or additional platforms.

Reproduce with an unchanged clone at the pinned commit and the audited Python
environment; keep the output directory outside the checkout and do not rebuild
the Mutie executable during the sequence:

```sh
cargo build --offline
python3 -B benchmarks/fixtures/python-conftest-trees/run.py \
  --project /path/to/sphinx-sample-code \
  --mutie target/debug/mutie \
  --python /path/to/audited-venv/bin/python \
  --output /tmp/mutie-conftest-trees-new-run
```
