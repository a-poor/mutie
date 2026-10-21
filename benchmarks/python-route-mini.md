# Larger fixture-based Python validation: Route Mini

Validated on 2026-10-20. The selected project is the public
[Route Mini reference library](https://github.com/JustinJLeopard/route-mini), pinned
at `35a3db157731ba0073d66f432b69b997ce2e9f01`. Its unchanged four-file suite has
68 cases, compared with seven in the preceding Binary Tree Library acceptance.
Four shared fixtures supply requests and model capabilities to 30 of the 68
collected cases across policy/router tests. Two ordinary conftest helpers construct those capabilities and scripted
providers; a test module imports one helper. All provider responses are scripted;
no service credentials or network calls are involved. This is a larger reference
library, not evidence for arbitrary production applications or dependencies.

## Scoped compatibility changes

- Synchronous undecorated conftest helper functions are permitted, with literal
  defaults and the same body/import/request audit as fixtures. Classes, async
  helpers, hook names, executable module statements and nonliteral defaults stay
  excluded. Direct `from .conftest import helper` is allowed only from direct
  `tests/test_*.py` files in a regular tests package. Only ordinary helper names
  may be imported, without aliases; fixtures and wildcard/escaping imports fail.
- `tests/__init__.py` may contain only comments/docstrings. Its bytes remain
  bound to snapshot and saved execution identity. Helper bytes are already bound
  by the conftest fingerprint; ordinary stronger test files can still be added.
- Direct example scripts under `examples/` use the existing inert ancillary-file
  audit: no tests/hooks, no imports from application/tests, no execution by Mutie.
- Static Hatch wheel package paths support the single audited flat/src package.
  Named optional development tools, Ruff py311 target and narrowly checked Mypy
  settings are inert manifest data. Builds, installers and Mypy plugins never run.
- Specific imports admit ABC/abstractmethod, MappingProxyType, time/time.sleep,
  and test-only in-memory JSON loads/dumps. General types/time modules, JSON file
  APIs, submodules and module aliasing stay excluded. An annotated class field
  named `input` is data; interactive input calls remain rejected.

These are compatibility restrictions, not a hostile-code sandbox. The pinned
runtime, full test inventory, complete fixture lifetime evidence, setup/cleanup
error precedence, process limits and source-preservation checks are unchanged.
The scope implementation joins execution recipe identity, so older recipes need
a fresh run; there is no schema, operator or package version bump.

## Validation and reproduction

The native and isolated original baselines both pass all 68 cases. Discovery
finds 38 boolean sites. Initial runs expose mixed body assertion/runtime errors;
those remain errors and failed reports have no score. Complete accounting uses
focused runs of remaining candidates, matching their file, source hash, operator,
byte span and replacement because focused reports assign new local IDs.

Retained [run.py](fixtures/python-route-mini/run.py) executes the original stage,
then a separate review stage. `--stage account` resumes accounting from saved
records if needed. Every command rehashes original inputs and all earlier reports;
the checkout must remain clean. Review changes live only in a separate copy.

```sh
cargo build --offline
python3 -B benchmarks/fixtures/python-route-mini/run.py \
  --stage original --project /path/to/route-mini \
  --mutie target/debug/mutie --python /path/to/audited-venv/bin/python \
  --output /tmp/mutie-route-mini-new-run
python3 -B benchmarks/fixtures/python-route-mini/run.py \
  --stage review --project /path/to/route-mini \
  --mutie target/debug/mutie --python /path/to/audited-venv/bin/python \
  --output /tmp/mutie-route-mini-new-run
```

Do not rebuild Mutie during this sequence. Use the pinned CPython 3.14.7 /
pytest 9.0.2 environment and preserve its launcher symlink. Native pytest adds
only the project's existing src directory to its import path, disables plugin
autoload and bytecode/cache writes, and reads the original pytest configuration.
The initial recorded native command redundantly cleared the configured `-q` and
supplied `-q`; a separate original-config control in the review stage verifies
that no output-option override is necessary.

## Results and limits

| Run | Tests | Kills | Survivors | Errors | Timeouts | Seconds |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Native original baseline | 68 | — | — | 0 | 0 | 0.30 |
| Isolated original baseline | 68 | — | — | 0 | 0 | 3.73 |
| Original full attempt | 68 | 1 | 7 | 1 | 0 | 45.85 |
| Original remaining IDs 10–38 | 68 | 0 | 0 | 1 | 1 | 36.89 |
| Original remaining IDs 12–38 | 68 | 3 | 5 | 1 | 0 | 43.35 |
| Original remaining IDs 21–38 | 68 | 2 | 16 | 0 | 0 | 77.48 |
| Original configured native control | 68 | — | — | 0 | 0 | 0.27 |
| Stronger native baseline | 72 | — | — | 0 | 0 | 0.27 |
| Original selected survivor control | 68 | 0 | 3 | 0 | 0 | 18.38 |
| Stronger selected rerun | 72 | 3 | 0 | 0 | 0 | 19.03 |

The four original runs account for every candidate exactly once: six kills,
28 survivors, three errors and one timeout. **This combined accounting is not a
full score.** The first three reports failed and retain unavailable scores; the
last completed report covers only its selected 18 candidates. Existing assertion
failures mixed with unexpected FallbackExhaustedError remain errors (original
IDs 9, 11 and 20). Forcing provider-down retries (ID 10) times out and is excluded.
No classifier rule was relaxed to obtain kills.

The separate review copy adds only
[test_stronger_boundaries.py](fixtures/python-route-mini/test_stronger_boundaries.py).
It checks that pending decisions default to unsuccessful, blocked 400/401 routes
report failure, and no-eligible-model routes report failure in both their exception
and telemetry records. Original IDs 3, 21 and 24 survive the unchanged control and
all die in the focused stronger-test run, with no errors/timeouts. The original
score/status/outcomes remain untouched.

Twenty-five original survivors remain unreviewed: 24 frozen/slots dataclass
settings and the unknown-error retry flag (ID 14). No equivalence is claimed.
The errors and timeout also remain open; the stronger selected run does not
convert them into scores or establish a passing full stronger mutation run.

[results.json](fixtures/python-route-mini/results.json) retains source identities,
all inventories/outcomes, report hashes, baseline inventories and timings. The
script matches original IDs using source identity, rather than assuming report
IDs are stable across selection. All original checkout bytes, including tests,
fixtures, metadata and inert examples, are preserved; Git remains clean. The
stronger copy is also hashed before/after. Times describe this machine only.

## Regression checks

The 34 static scope tests pass, including positive helper imports and rejection
of alias/wildcard/fixture imports, unsafe helper defaults/bodies, executable test
initializers, hidden example tests, unreviewed standard-library APIs and config
plugins. Four native conftest workflows pass in 82.42 seconds. The new shared-run
control imports the same helper from a test and calls it during fixture cleanup:
a body-only assertion is a kill, while the mixed cleanup failure is a saved error.
Clippy passes with all targets and warnings denied, as do formatting and diff
checks. The complete ignored Python workflow suite passes all 20 tests in 326.82
seconds, including helper lifecycles, flat/src/root layouts, unittest, resource
limits and Python cancellation.

The broad default suite is not claimed clean: two serial attempts failed varying
Go cancellation timing checks (first 5/7 passed, then 4/7). An isolated rerun
outside the filesystem sandbox passed 6/7 in 210.33 seconds, failing the
worker-temporary-file startup wait. Other cases had passed in prior attempts;
there is no established root cause. No Go/process implementation was changed in
this task, and no score/lifecycle checks were weakened. A separate default run
excluding those seven cancellation tests passed through the configuration and
dismissal suites, then passed 2/3 log-limit tests: the interrupted noisy-command
case failed its `command never became ready` wait. The remaining integration
binaries were run separately with no-fail-fast so this did not hide later checks.
These are unresolved broad-suite startup/timing failures, not passing checks.

The remaining default integration binaries all passed with their ordinary ignored
tests left opt-in: node_workflow, python_workflow, report_browser, report_retention,
rerun, rust_workflow, terminal, vitest_preview and vitest_workflow. The unit binary
also passed 198 tests (24 opt-in ignored). Combined with the earlier passing
integration results, the outstanding default failures are confined to the
recorded cancellation/readiness checks. This is still not a clean full-suite run.
