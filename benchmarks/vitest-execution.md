# Vitest completion evidence and isolated execution

2026-10-15, following discovery commit `da255b1`. This is an adapter milestone,
not acceptance of the full mutation/review/rerun workflow. Vitest remains Planned:
`vitest-list` previews mutations and `vitest-check` checks an isolated baseline;
normal scored runs, saved reports, review and reruns are not wired to Vitest yet.

## Implemented boundary

The adapter validates the pinned project, resolves actual Node identity, then
copies source and dependencies into a fresh owned workspace for every command.
It revalidates manifest/lockfile identity and dependency-link containment in the
copy. Mutations supplied by the opt-in harness apply only to that copy. Dependency
files use the existing CoW/copy implementation, never writable hardlinks to the
caller. Existing Vite caches are excluded; the command gets a private config,
reporter, transform cache and temporary directory outside the project tree.

The command uses the resolved Node executable and copied Vitest CLI directly:
`node <copy>/node_modules/vitest/vitest.mjs run --config <owned-config> --configLoader native`.
It does not install packages or execute package scripts. Configuration fixes Node
execution, one fork worker, full discovery, isolation, no watch, no coverage,
no snapshot updates, no retries and no bail. Environment-file loading is disabled;
`NODE_OPTIONS` and `NODE_PATH` are cleared. Mutie's deadline controls command
shutdown; default test/hook timeouts are disabled. Per-test retries, repeats and
nonzero timeout overrides are rejected in evidence rather than silently accepted.
This scope does not cover custom configuration, transforms or browser environments.

`vitest-check PATH --timeout 60 --max-capture-bytes 4194304` uses this path and
accepts only a complete, nonempty passing baseline. It does not discover or run
mutants, write a saved run, evaluate scores, or consume `mutie.toml`. Errors include
bounded diagnostics. Printed duration is command time, not workspace-copy time.

## Evidence protocol and outcome policy

The bundled reporter targets installed Vitest 4.1.9. Its completion file records
scheduled and completed modules, terminal module/test states, test identities,
collection/suite errors, unhandled errors and hook activity. It is written once
with exclusive creation in an owned directory, limited to 1 MiB, and read before
cleanup. The reader requires a regular bounded file and rejects malformed,
unknown, duplicate, inconsistent, incomplete or empty evidence. User logs cannot
serve as completion evidence. Captured stdout/stderr has a configurable combined
raw budget; displayed diagnostics retain at most 64 KiB with head/tail retention.
These bounds do not claim to bound test/runner memory or all test-created files.

Classification preserves process facts first: cancellation is interrupted;
capture overflow is an error; a command deadline is timeout, even if no completion
file was produced. Otherwise require matching exit status and complete evidence:

| Evidence | Adapter result |
| --- | --- |
| Executed tests passed, no infrastructure errors, exit 0 | Missed (passing baseline in `vitest-check`) |
| Attributed failed tests, exit 1, no module/suite/unhandled errors or reported hooks | Killed |
| Known local JS/TS Vite transformer errors, exit 1, no test failures or hook activity | Build failed |
| Import/runtime collection errors, mixed failure categories, unhandled errors, missing/corrupt completion, empty/all-skipped runs, early exit | Error |
| Failed/unfinished hooks, or test failures in a run that used reported lifecycle hooks | Error, conservatively |

Transform classification uses the pinned runner's structured
`__vitest_rollup_error__` plugin and source identity (`vite:oxc`), not error-message
matching. The first matrix exposed that metadata nesting; expecting top-level
plugin metadata initially classified the transform probe as an error. After
correcting the extractor, the complete matrix passed. Unknown transformer metadata
stays an error. This internal field is a compatibility constraint, not a promise
of support for other Vitest/Vite versions.

The public hook events cannot reliably attribute every returned cleanup error.
Consequently, even a legitimate assertion failure after a successful reported hook
is conservatively an error. Advanced fixture/around-hook/test-context cleanup APIs
are not validated by this milestone; resolve their attribution before broadening
mutation execution claims. A passing baseline alone never establishes that those
failure modes are supported.

## Verified cases

Host: macOS arm64, Node 24.22.0, pnpm 11.10.0, Vitest 4.1.9. Fixture Vite 8.3.4;
adapted Pathe Vite 8.1.3. No package/schema/operator versions changed.

The ignored Rust adapter matrix uses the prepared
[mixed-syntax fixture](../tests/fixtures/vitest/README.md), makes an independent
input copy, and invokes the same isolated adapter used by the baseline command:

- Baseline: four passed tests. Six independent boolean mutants: four killed and
  two missed, with one missed per syntax. Adding a stronger test kills each saved
  source mutation independently. These are direct adapter calls, not saved-report
  or CLI-focused-rerun acceptance.
- A CLI that exits zero before initializing the reporter and a CLI startup
  exception both fail without completion evidence.
- Broken TS syntax becomes build failed. Adding an independent assertion failure
  to that run becomes error rather than a kill/build-failure shortcut.
- Import exceptions, before/after hooks, returned hook cleanup exceptions,
  unhandled rejection, early process exit, retry/timeout overrides, all-skipped
  and no-test suites are rejected.
- A passing noisy test emits 200 KB and non-JSON text. Diagnostic retention
  truncates while complete classification stays passing.
- Separate timeout, cancellation and 8 KiB raw-capture-overflow probes deliberately
  write source/dependency/temp markers and spawn a lingering child. The copied
  project and command temp directory disappear, child PIDs terminate, and no
  marker or source change leaks into the input copy.

The baseline CLI passes the prepared fixture and the explicitly adapted Pathe
copy: **467 passed, 16 skipped/todo** on Pathe. The adaptation removes only its
coverage-reporter Vite config, as documented in the
[scope correction](vitest-initial-scope.md). Unmodified upstream Pathe remains
rejected. This does not promote real-project mutation support.

Default protocol tests exercise contradictory status/evidence, pending states,
duplicated IDs/modules, skipped/empty execution, missing/corrupt/oversized files,
symlinks, mixed failures and hook policy. Real execution tests are ignored by
default so Mutie's self-tests cannot recursively launch Vitest mutation work.

## Reproduction and next gate

Prepare a temporary fixture with its pinned frozen lockfile as described in its
README, then run:

```sh
cargo run -- vitest-check /path/to/prepared-fixture
MUTIE_VITEST_FIXTURE=/path/to/prepared-fixture cargo test --bin mutie vitest::execution -- --ignored --test-threads=1
cargo test
cargo fmt --check
cargo clippy --all-targets -- -D warnings
```

Next, connect the adapter to the shared run/coordinator/report pipeline with explicit
runner/tool/config identity. Preserve strict evidence validation before accepting
a baseline or a mutation result. The shared command boundary now carries validated
Vitest outcomes or evidence errors, with interruption/capture/timeout precedence;
exit zero cannot bypass completion validation. `vitest-check` and the real adapter
probes use that boundary, and shared baselines call the same classifier. Verify saved diagnostics/checkpoints and fixture CLI
runs before Experimental promotion. Review/reruns, parallel-worker contamination
controls, broader lifecycle attribution and useful real-project mutation evidence
remain subsequent acceptance gates.

Validation checks: the complete default suite passed (226 passed, 34 ignored).
Final targeted protocol/preview tests, both opt-in real adapter tests, formatting
and Clippy passed; the expanded startup/missing-evidence matrix was rerun separately.
That initial adapter validation did not change Go/Rust execution, report schema
or shared classification. The subsequent shared-command integration preserves
Go/Rust outcome rules while requiring structured Vitest evidence before accepting
success. Scored Vitest CLI runs and saved-report integration remain unimplemented.

Shared-command validation: 228 default tests passed (34 ignored), both opt-in
real Vitest adapter tests passed through the shared classifier, and the baseline
CLI reported four passing fixture tests. Formatting and all-target Clippy passed.
The classifier retains typed cancellation errors, and regression tests cover
missing completion after exit zero, retained diagnostics, validated outcomes,
and interruption/capture/timeout precedence over completion evidence.
