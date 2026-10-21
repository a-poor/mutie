# New unchanged Python project: betting-math-kit

Validated 2026-10-21 against [bene-art/betting-math-kit](https://github.com/bene-art/betting-math-kit)
at commit `0bd3c25e93df8974f432310835768d77a5e45d3e`, using pinned CPython
3.14.7 / pytest 9.0.2. The complete original suite passes **198 tests** in five
files, compared with Route Mini's 68. Source, tests, layout and manifest are
unchanged. Nothing is installed or built from the project; no tests are removed,
filtered or deselected. The README's 195-test count is older than this checkout.

This is numerical-library validation, not validation of betting advice or model
accuracy. The exercised contracts are public probability intervals, calibration
binning and input validation.

## Compatibility extension

The existing native `src/` package and plain pytest classes already fit. The
small additions are:

- Optional development/test/documentation tools already on the audited list may
  declare simple numeric minimum versions. Unsupported names, URLs and version
  syntax remain rejected; pytest minima must fit the pinned version. None of
  these extras is installed or loaded.
- Inert Ruff `py310` and Mypy Python `3.10` settings, plus boolean
  `warn_return_any`, `warn_unused_configs` and `disallow_untyped_defs`, are
  recognized. Plugins and other settings remain rejected.
- `import random` may directly construct an independent `random.Random(...)`
  instance in application code or tests. Module alias escape, constructor
  references/rebinding, class mutation, `SystemRandom`, global `seed` and global
  draw methods remain rejected. Existing `random.sample` remains test-only.
  Unseeded generators are not promised deterministic. This project's simulation
  tests use explicit seeds.

All manifest bytes and validator recipe bytes remain fingerprinted. The change
requires fresh execution identity; old reports are not rewritten or silently
accepted under the new recipe. No observer, classifier, runtime pin, deadline,
report schema, operator version or package version changes.

The existing candidate audit and additional public projects were considered
before selection. Alternatives requiring plugins, package metadata execution,
reflection or broader dependencies were not adapted into acceptance.

## Complete suite and selected mutation accounting

Discovery lists **87 sites: 20 boolean flips and 67 comparison boundaries**.
The scored selection is all ten comparison boundaries in `metrics.py`, always
running all 198 original tests. This is a deliberate file/operator selection,
not a full 87-candidate run or a project-wide score.

The first three selected candidates each stop their run with runtime errors.
Focused pending-ID reruns account for every selected candidate by source identity.
The final original run reaches a fourth error after six survivors. All failed
reports remain failed and unscored.

| Original ID | Metrics line | Change | Outcome |
| --- | ---: | --- | --- |
| 1 | 24 | Probability below zero: `<` → `<=` | Error |
| 2 | 24 | Probability above one: `>` → `>=` | Error |
| 3 | 181 | Minimum calibration bins: `<` → `<=` | Error |
| 4 | 194 | Final calibration-bin clamp: `>=` → `>` | Survived |
| 5 | 282 | Opening probability zero: `<` → `<=` | Survived |
| 6 | 282 | Opening probability one: `>` → `>=` | Survived |
| 7 | 286 | Closing probability zero: `<` → `<=` | Survived |
| 8 | 286 | Closing probability one: `>` → `>=` | Survived |
| 9 | 371 | Minimum edge-analysis bins: `<` → `<=` | Survived |
| 10 | 403 | Final edge-bin clamp: `>=` → `>` | Error |

Combined accounting is **six survivors and four errors**, not a combined score.
Errors are unchanged-suite runtime exceptions, not assertion-only kills; no
classification rule was weakened to improve these counts.

## Useful findings and stronger tests

The original tests exercise interior probabilities but miss four valid closed-
interval endpoints in `clv`. The mutations reject a documented valid input.
Another survivor allows probability 1.0 to index past the final calibration bin.
The sixth rejects a valid one-bin edge analysis.

A complete separate copy adds [seven test cases](fixtures/python-betting-math/test_metric_boundaries.py):
four probability endpoints, a probability-one calibration case, and single-bin
calibration and edge-analysis contracts. They assert returned values, bin counts,
means and bounds. Expected-success calls explicitly convert unexpected
`InvalidProbabilityError`, `ValueError` or `IndexError` into `pytest.fail`; this
expresses the public success contract within the tests, without changing Mutie's
classifier. Other error types remain errors. All original tests remain intact.

Both the native and isolated **205-test stronger baselines pass**. The focused
rerun of the six original survivors yields **six kills, no survivors, errors or
timeouts**, in 62.87 seconds. This is a selected 100% rerun score; the original
failed reports retain unavailable scores. The four original error candidates and
77 unselected sites are not claimed resolved or scored by this rerun.

## Reproduction and preservation

The [driver](fixtures/python-betting-math/run.py) accepts a clean pinned project,
CLI, pinned venv launcher and fresh output directory, with successive stages:

```sh
python3 -B benchmarks/fixtures/python-betting-math/run.py \
  --project /tmp/mutie-python-betting-math-original \
  --python /tmp/mutie-pytest-audit-env/bin/python \
  --mutie target/debug/mutie --output /tmp/new-betting-acceptance \
  --stage baseline
# Repeat with --stage score, then --stage stronger, using the same output.
```

It checks original hashes and Git cleanliness before/after commands, rejects CLI
hash drift, and preserves saved report hashes. Native runs use fresh bytecode
prefixes, plugin autoload disabled and no cache provider; the production adapter
retains its usual fresh bytecode isolation and complete test inventory checks.
The [verifier](fixtures/python-betting-math/verify.py) accepts output directory,
original project and optional prior ledgers. It verifies all original/stronger
bytes, only one added test file, report hashes and original-ID mappings using
file, source fingerprint, operator, span and tokens.

[Final results](fixtures/python-betting-math/results.json) retain the full
inventory, commands, identities, timings, outcomes and hashes. Original inputs,
all new reports, preliminary reports and earlier Route Mini reports remained
unchanged. Preliminary acceptance before the final direct-constructor guard is
retained separately in [its ledger](fixtures/python-betting-math/preliminary-results.json);
the final workflow was repeated with the final binary and new recipe.

| Command | Seconds |
| --- | ---: |
| Discovery | 2.92 |
| Native 198-test baseline | 1.49 |
| Isolated 198-test baseline | 6.06 |
| Original selected run | 14.50 |
| Three pending selections | 13.49 / 16.34 / 73.26 |
| Native 205-test baseline | 1.43 |
| Six-survivor stronger rerun | 62.87 |

Timings are single observations with concurrent repository checks, not controlled
performance benchmarks.

## Regression validation

- All **41 static scope tests** pass, including permitted metadata and rejection
  of unknown tools, incompatible pytest minima, URLs, bad types, plugins, random
  module escape, global mutation and constructor/class escape.
- The new real random-generator workflow passes: independent seeded generators
  give equal results, while a mutant producing an invalid seed remains a saved
  runtime error with zero kills and preserved source.
- The existing native-src scored/report/review/stronger-rerun workflow passes
  in 68.92 seconds.
- The unfiltered default Cargo suite passes **278 tests**, with **94 opt-in
  ignores**, including the newly added real Python check. All-target Clippy with
  warnings denied, formatting and whitespace checks pass. Complete logs are
  retained alongside the results.

Support remains one audited dependency-free package on the pinned serial runtime;
this evidence does not admit arbitrary randomness APIs, dependencies, plugins,
concurrency, reflection or build execution.
