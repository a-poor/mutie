# Scoped unittest through pytest: unchanged Pluralizer

Validated on macOS arm64 with CPython 3.14.7 / pytest 9.0.2. This extends the
existing serial, dependency-free Python scope to direct synchronous
`unittest.TestCase` subclasses. It does not enable arbitrary unittest suites,
plugins, mocks, async cases, inheritance hierarchies or dependencies.

## Attribution boundary

The pinned pytest unittest adapter queues exceptions from TestCase execution.
An assertion from `setUp`, `tearDown` or a cleanup can appear in the call phase;
additional errors can be shifted to another phase or suppressed. A call-phase
AssertionError alone cannot establish a mutation kill.

The production observer wraps the pinned `_callSetUp`, `_callTestMethod`,
`_callTearDown`, `_callCleanup` and `doCleanups` methods before importing project
code. Each unittest call must provide this complete sequence:

1. Successful setup.
2. Successful body, or an actual assertion exception matched by object identity
   to the exception pytest reports.
3. Successful teardown.
4. Cleanup entry, zero or more successful cleanup callbacks, and cleanup return.

The reader also requires intact base/class/instance dispatch, a valid unittest
subset of the full collected inventory, all pytest phases and existing process
return/lifetime checkpoints. Missing inventory, missing evidence, failed cleanup,
wrong body identity, extra phases, warnings and late failures remain errors.
Class and module fixture failures remain errors through pytest phase evidence.
The observer/reader/scope bytes are already part of the execution recipe hash;
focused reruns reject drift without a schema or package version bump.

Static validation allows unaliased `import unittest` or `from unittest import
TestCase`, direct classes, synchronous methods, classmethod class fixtures and
registered instance/class cleanups. Runtime validation independently checks the
actual base and protected methods. Reinitialization during a test is rejected independently by the observer, so it
cannot discard registered cleanups. Custom constructors/dispatch, multiple or
indirect inheritance, metaclasses, subtests, skips/expected failures, module cleanup
registration and mock/runner APIs remain excluded. The exact conventional
`if __name__ == "__main__": unittest.main()` guard (also with an assignment) is
retained but does not execute under pytest import mode.

Native controls cover 28 unittest cases: successful/assertion/runtime bodies,
setup and teardown, mixed body/teardown, failed and multiple/suppressed cleanups,
passing cleanups, class fixtures/cleanup, module fixtures in both naming styles,
skip/expected-failure/subtest rejection, custom dispatch, fake assertions, return
warnings, initialization during a running test and late atexit errors. Ordinary pytest, process/capture/file limits and
descendant-shutdown controls remain covered. Replay controls reject missing or
altered dispatch evidence. A real scored mixed body/cleanup regression verifies
that the saved outcome is an error with zero kills and preserved source.

## Unchanged project

Repository: [weixu365/pluralizer-py](https://github.com/weixu365/pluralizer-py),
commit `860a8e4514045896e7359aa519888e2f7c93f35c`.
All 18 original files, including source, tests and pyproject.toml, remain unchanged.
Native pytest and Mutie's isolated baseline both pass the original 15 test methods;
the methods themselves exercise large tables of words. The original main guard
is retained. All four boolean candidates are attempted in one completed run:

| Location in pluralizer/pluralizer.py | Mutation | Outcome |
| --- | --- | --- |
| 153 | keep-map True → False | Killed |
| 155 | replace-map False → True | Killed |
| 159 | default inclusive False → True | Killed |
| 201 | stored uncountable value True → False | Survived |

The full score is 75%: three kills, one survivor, no errors or timeouts. A focused
rerun reproduces the survivor. The package's sole read of `uncountables` tests
key membership, not the stored boolean. This supports a redundant-value finding
within the inspected implementation; it is not a claim about every possible
external consumer of that public attribute.

A separate copy adds one TestCase method covering mixed-case uncountable words,
plural/singular classification, counts and inclusive output. Its 16-test pristine
baseline passes; the focused survivor rerun still survives and the complete run
remains three kills/one survivor. No assertion of an internal dictionary value is
added merely to force a kill. The shared unittest fixture independently proves a
useful stronger-test survivor-to-kill rerun, saved review and policy behavior.
Original inputs and every original report file are hashed before and after each
command; stronger-copy source/config/original tests also remain unchanged.

Pluralizer's additional Hatch packaging, Ruff lint settings and audited
`dependency-groups.dev`/uv override metadata are narrowly recognized as inert
configuration. Project builds, resolvers, optional tools and installers never run.
Custom build hooks and additional executable Python files remain rejected.

## Reproduction and retained evidence

The [script](fixtures/python-unittest-pluralizer/run.py) retains the stronger test,
checks the exact upstream commit and clean checkout, compares original/native
and isolated baselines, runs full/focused commands and verifies preservation.
The [results](fixtures/python-unittest-pluralizer/results.json) retain runtime
and recipe identity, node IDs, candidate inventory, outcomes, timings and hashes.

```sh
cargo build --offline
python -I -B benchmarks/fixtures/python-unittest-pluralizer/run.py \
  --project /path/to/unchanged/pluralizer-py \
  --mutie "$PWD/target/debug/mutie" \
  --python /path/to/pinned/venv/bin/python \
  --output /tmp/mutie-unittest-pluralizer-fresh
```

The output directory must not exist. Preserve the virtual-environment launcher
path; resolving its symlink loses the dedicated environment.

Regression commands:

```sh
/path/to/pinned/venv/bin/python -I -S -B tests/support/python_scope.py
MUTIE_PYTHON=/path/to/pinned/venv/bin/python cargo test --offline --bin mutie python:: -- --include-ignored --skip real_feedback_phase_profile --test-threads=1
MUTIE_PYTHON=/path/to/pinned/venv/bin/python cargo test --offline --test python_workflow -- --include-ignored --test-threads=1
cargo fmt --check
cargo clippy --offline --all-targets -- -D warnings
git diff --check
```

## Recorded checks and timings

The final acceptance run preserved all 18 original input hashes and all saved
report hashes. Times include CLI validation/fingerprinting and were measured
alongside regression work; these are acceptance timings, not an isolated overhead
benchmark.

| Command | Wall seconds |
| --- | ---: |
| discovery | 13.80 |
| native-baseline | 1.63 |
| isolated-baseline | 7.52 |
| original-full | 45.07 |
| original-survivor-rerun | 18.56 |
| stronger-survivor-rerun | 23.00 |
| stronger-full | 39.73 |

Validation passed: 26 static-scope tests; six Rust identity/discovery/protocol
checks and five native observer/execution checks, with the expanded 28-case
unittest audit rerun after the final initialization guard; all 17 workflow cases;
formatting, Clippy across all targets and diff checks. The workflow sweep initially
passed 15/16: its src-layout focused rerun correctly rejected execution-identity
drift because the CLI was rebuilt during the test. That case passed separately
against the stable final binary. The added mixed body/cleanup saved-error case
also passed separately. No identity check was relaxed to accommodate the rebuild.
