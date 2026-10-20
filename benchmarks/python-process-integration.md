# Python process integration controls

The production observer and Rust reader now have direct native regressions in
`src/python/tests.rs`. These probes intentionally bypass static project admission
to test defense against unsupported behavior; they do not admit those APIs.

On 2026-10-20, CPython 3.14.7 / pytest 9.0.2 on macOS arm64 passed all 18 cases
across three ignored tests in 55.53 seconds:

- Twelve attribution cases: passing body, ordinary assertion, `pytest.fail`,
  runtime error, same-named fake AssertionError, setup assertion, combined
  body/teardown assertion, late atexit error, late uncaptured stderr, unittest
  setup failure, a clean joined thread, and a warning.
- Three resource cases: raw capture overflow, a project output file exceeding
  4 MiB, and observer evidence exceeding 4 MiB. Retained evidence/output files
  stay within the per-file limit; none receives a positive outcome.
- Three descendant cases: successful interpreter exit with a pending child,
  timeout with a pending child, and cancellation after the child starts. Every
  case rejects positive completion, and the child's delayed marker never appears.

The shared process runner now exposes `group_remaining`: whether the child's
process group still existed after reaping the direct child and before cleanup.
Python rejects this condition, including after an otherwise successful exit.
Other runners retain their existing outcome policy. Shared process tests verify
the distinction between normal foreground exit and a background child.
This detects ordinary process-group descendants, not escaped sessions or a
hostile sandbox escape. Application subprocess/thread APIs remain excluded.

The Rust reader also recognizes a killed candidate only when the complete
inventory has setup/call/teardown evidence, all failures are positively identified
assertions in call phases, exit/session/return statuses agree, and both lifetime
checkpoints remain intact. Unsupported phases, warnings and cleanup failures
remain errors. Replay tests cover exit disagreement, missing teardown, false
assertion evidence, xfail, skips and corrupt checkpoints. This classifier is
used by [shared scored execution](python-shared-runs.md).

Baseline JSON now records a typed execution identity: exact runtime environment,
manifest, package, observer/validator/classifier/process recipe, test-file
inventory and execution limits. Compatibility permits stronger/additional tests
and changed command budgets while rejecting runtime/configuration/recipe drift.
Saved-report integration and actual focused reruns are now covered by the
[shared fixture workflow](python-shared-runs.md).

The adapter now returns shared `command::CommandOutput` values rather than
discarding failure captures in the preview command. `project.rs` converts the
static reader's spans into shared `Mutation` values and independently checks
source hashes, UTF-8 offsets, coordinates, ordering and overlap, including
excluded source files. The direct shared execution contract passed in 19.18s:
one applied boolean flip is killed, one survives, and runtime/collection errors
remain errors with retained diagnostics. Caller input bytes remain as supplied
to each isolated execution. A default unit test covers Unicode spans, include/
exclude filtering, application and rejection of changed source.

These adapter tests are supplemented by scored CLI and saved-report acceptance
in the shared fixture workflow. Candidates must match pristine baseline node IDs;
comparing collection and execution within one mutated copy is insufficient.

The default Cargo suite passed after the shared process change. The focused
process tests passed (12 tests), as did both protocol tests, the identity test,
all five Python CLI workflow tests (39.95s), Clippy with warnings denied and
formatting/diff checks.

```sh
MUTIE_PYTHON=/path/to/pinned-venv/bin/python \
  cargo test --bin mutie python::tests -- --ignored --test-threads=1
cargo test --bin mutie python::protocol
cargo test --bin mutie python::identity
cargo test --bin mutie process::tests
```

## Real-project selection notes

These inspected candidates are **not** acceptance evidence:

- [Braceexpand](https://github.com/trendels/braceexpand/tree/334cdab18e52edc1aa3b94ee6badba3efb26abd8)
  uses unittest; its original suite is excluded.
- [Humps](https://github.com/nficano/humps/tree/a0f712b8ffb4d2473ce2f50669f9aae5b99b7f51)
  loads installed distribution metadata at package import and uses Poetry
  metadata. It is outside the current owned-source import contract.
- [RFC3986](https://github.com/python-hyper/rfc3986/tree/3bd3832b72eb981d080514a8192046e3177263be)
  uses conftest fixtures and test helpers outside the current test-file scope.

Do not silently omit these features or claim unchanged acceptance. Subsequent
[Scrubify acceptance](python-scrubify-findings.md) completes the full
mutation → stronger-test → rerun loop with an explicit layout/metadata adaptation.
