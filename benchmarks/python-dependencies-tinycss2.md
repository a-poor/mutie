# Scoped Python dependency acceptance: Tinycss2 1.2.1

The first application-dependency slice supports one audited pure-Python package,
`webencodings` **0.5.1**. It is validated with unchanged
[Tinycss2](https://github.com/Kozea/tinycss2/tree/v1.2.1) **1.2.1**, commit
`853bc5a5661a59c7a9795a4e2758d0081808ce06`, and its pinned CSS test-data submodule
`c5749e51dda3868b3e8062e65a36584c2fec8059`.

This is explicitly an older tagged-release validation. The current audited
Tinycss2 checkout, `2f5cedd8eb51bb4fa2ff3c1f841e1a236e5ba5fd`, passes all **17,120**
native tests in 9.13 seconds, but is not production acceptance: its full phase
ledger would exceed the current 4 MiB evidence bound, and its additional scope
has not been admitted. No suite was trimmed to fit. The tagged release's complete
**480-test** native suite passes in 0.37 seconds; production execution uses that
same test inventory and unchanged data. Evidence bounds and classification stay
unchanged.

## Audit selection and dependency boundary

Initial alternatives were inspected without executing their project code:
Dep-logic adds platform/marker APIs and metadata; python-slugify adds CLI and
backend tests; dependency-groups adds CLI/subprocess fixtures; url-normalize adds
CLI/mocking and pytest-socket/coverage configuration; markdown-it-py adds test
plugins and build tests. They were not silently trimmed or promoted to support.
Tinycss2 1.2.1 provides a focused decoding dependency and complete small suite.

The wheel is `webencodings-0.5.1-py2.py3-none-any.whl`, SHA-256
`a0af1213f3c2226497a97e2b3aa01a7e4bee4f403f95be16fc9acd2947514a78`.
The inspected runtime implementation uses fixed encoding labels, BOM detection,
stdlib codecs, and an internal x-user-defined codec. It has no runtime requirements
or entry points. Auxiliary tests/label-generation modules remain inaccessible
through the supported imports; all five package files are included in the trusted
package-tree fingerprint:
`sha256:c1e7f05855fc3004d21de68f4a04d7e9d88ba13544f02f705f9af26a1cb48026`.

Static admission accepts only a sole requirement `webencodings >=0.4`, its compact
spelling `webencodings>=0.4`, or `webencodings==0.5.1`. This is an allowlist, not a
PEP 508 resolver. Only named root imports of `UTF8`, `Encoding`, `decode`, `lookup`
and `ascii_lower` are allowed. General module/submodule imports, extra APIs,
additional requirements, markers, URLs and extras remain rejected.

The caller supplies an environment containing the pinned pytest requirements and
this audited wheel. Mutie never installs or resolves it. Runtime identification
with `-I -S` checks version, pure-wheel metadata, absence of transitives/entry
points and exact package bytes before execution. The original complete runtime
fingerprint, post-exit checks and fresh bytecode prefixes remain in force.
The manifest-derived dependency map joins snapshot, saved execution and rerun
identity; missing and undeclared installed dependencies are rejected. Stronger
tests cannot waive dependency/runtime drift. No dependency source is mutated.
See the retained [hash-pinned requirement](fixtures/python-dependency/requirements.txt).

## Other scoped compatibility

The unchanged checkout also justifies these additions:

- Literal Flit package versions, including the conventional `VERSION = __version__`
  assignment; inert sdist, optional documentation/lint dependencies and tool metadata.
  No build backend or optional tooling executes.
- Read-only `sys.maxunicode`, named `colorsys.hls_to_rgb`, unaliased test-only
  `pprint.pprint`, and decorator-function parameter name reads.
- Direct initialization of a same-module direct base class, inside the subclass
  constructor with `self`. Ordinary class-method names do not rebind same-named
  module imports. Global rebinding and unittest dispatch overrides remain rejected.
- Retained, non-importable documentation and two data-generation helpers. The exact
  data-directory exclusion cannot conceal tests or other executable helpers.
- Regular nested Git pointer files are excluded consistently with workspace copying;
  nested Git directories and symlinks remain rejected. Data bytes and directory
  inventory remain identity inputs.

These do not establish general reflection, filesystem, submodule execution,
plugin or dependency compatibility. The process observer and error classifier
are unchanged.

## Original selection and useful findings

Discovery finds **92 candidates** across all three Python operators. Acceptance
executes the **14 candidates** in `tinycss2/bytes.py` and `tinycss2/serializer.py`:
two boolean flips, two comparison boundaries and ten arithmetic replacements.
Each candidate runs the complete 480-test suite. The other 78 sites are discovered,
not claimed as executed.

Original accounting is **four survivors, ten errors, zero kills/timeouts**.
Three string-arithmetic changes fail in bodies; seven module-level arithmetic
changes fail collection and remain fatal. The harness resumes only the unattempted
original inventory after each fatal stop. All seven original reports remain
failed, exit 1 and unscored; combined inventory accounting is not a full score.

| Selection ID | Source | Survivor and stronger contract |
| ---: | --- | --- |
| 1 | bytes.py:50, skip_comments default | Decode Windows-1252 comment text and retain the comment by default. |
| 2 | bytes.py:50, skip_whitespace default | Preserve decoded leading whitespace by default. |
| 4 | serializer.py:46, `>` to `>=` | Escape ASCII DEL at the start of an identifier. |
| 6 | serializer.py:60, `>` to `>=` | Escape ASCII DEL in subsequent name characters. |

A separate copy adds only
[tests/test_mutie_dependency.py](fixtures/python-dependency/test_dependency_contracts.py).
All **484 baseline tests pass** and the four selected survivors are **killed** in a
full-suite focused rerun. The comment/whitespace checks exercise the dependency's
decoding and returned encoding identity; serialization checks cover observable
output boundaries. No original assertion, input, manifest, source or fixture data
is changed. The focused score does not replace the original failed scores.

## Evidence and timings

All paths below are under `/tmp/`:

- Original checkout: `mutie-python-tinycss2-1.2.1-original` (43 non-Git input files).
- Audit/native controls: `mutie-python-dependency-audit`, including both tagged
  and current-release native baselines, original hashes, scope and runtime JSON.
- Disposable dependency environment: `mutie-python-dependency-audit/env`.
- Initial acceptance: `mutie-python-dependency-acceptance/results.json`.
- Final acceptance after pprint alias rejection:
  `mutie-python-dependency-final/results.json`.
- Runtime drift controls: `mutie-python-dependency-runtime-controls/results.json`.

Final original selection takes 36.658 seconds before its first collection stop;
six pending-selection reruns take 8.205–8.504 seconds each, **86.613 seconds total**.
The final stronger run takes **22.538 seconds**, baseline included, and is saved
at `mutie-python-dependency-final/reports/run-1792651101-Qa9sOl`.
The earlier recipe independently matched outcomes (88.074 seconds original,
22.437 seconds stronger); its reports remain preserved, not overwritten.

The retained [run harness](fixtures/python-dependency/run.py) freezes the CLI,
records commands and hashes, and maps outcomes through exact source identities.
The [independent verifier](fixtures/python-dependency/verify.py) passes for both
acceptance sets, original/stronger inputs, original and focused report files,
dependency identities and all four stronger-test kills. Current recipe bytes
match the final saved execution identity. Both upstream checkouts and the
original dependency-free pytest environment remain separate from the copied
acceptance environments.

## Regression controls

- 52 static scope tests and six runtime-tree tests pass.
- All eleven native Python regression groups pass in 132.78 seconds, including
  the new dependency body/exception/cleanup controls and existing fixture,
  unittest, expected-failure, capture/file-limit, timeout/cancellation and
  descendant-cleanup cases.
- Twelve [disposable-environment controls](fixtures/python-dependency/runtime_controls.py)
  pass: original/restored bytes accepted; backdated same-size code changes, wrong
  versions, transitives, plugins, native metadata/additions, extra modules,
  startup hooks, missing packages and package symlinks rejected.
- Saved identity controls reject missing/undeclared dependencies, wrong versions
  and dependency-byte changes during rerun. Runtime exceptions and cleanup mixtures
  remain errors; no production classifier change is involved.
- Final repository checks pass: 203 unit tests (28 ignored), all-target lint,
  formatting and diff whitespace checks. Logs are retained in the audit directory.
  The complete integration suite is not claimed passing; its previously deferred
  startup/readiness limitation remains unresolved. No host settings or deadlines
  were changed.
