# Vitest shared CLI runs

The first scored Vitest path is explicit `mutie PATH --lang vitest`, using the
pinned ESM/pnpm scope in [the project audit](vitest-initial-scope.md). Automatic
language selection remains Go/Rust only. Node 24.22.0, pnpm 11.10.0 and Vitest
4.1.9 are required; the fixture uses Vite 8.3.4. This is Experimental fixture
acceptance on macOS arm64, not real-project mutation acceptance.

## Execution and persisted identity

The shared coordinator runs a baseline, discovers JS/TS boolean flips, publishes
an inventory, executes mutations and saves outcomes/diffs/logs and a raw score.
Both the baseline message and saved baseline outcome require valid completion
evidence; an exit-zero process with missing evidence is never a passing baseline.
The full CLI probe exposed Vitest 4.1.9 intercepting `process.exit` inside a test
as a plain test Error. The classifier now rejects its pinned structured error
message prefix (`process.exit unexpectedly called with `) conservatively, instead
of counting it as a kill. This is a version-specific rejection rule, not positive
failure attribution from terminal text.

Structured outcome errors remain infrastructure errors rather than scored kills
or survivors. Existing Go/Rust adapters retain their outcome rules.

`run.json` includes `vitest_scope`: package name, manifest/lockfile fingerprints,
Node version/executable, pnpm/Vitest/Vite versions, supported syntax, normalized
generated runner settings, allowed project config fingerprints (`tsconfig.json`
and settings-only `pnpm-workspace.yaml`, when present), and the reporter fingerprint. Each command revalidates
package/tool/settings identity. Saved-report loading checks identity shape and
supported mutation scope without invoking Node or reopening the source project.
Raw Vitest stdout is a `.log`, not Go's JSONL format. Inventory package arguments
are empty because every command runs the full suite. No report/schema version bump.

The existing snapshot/baseline/worker workspaces are retained. The Vitest adapter
also creates a fresh inner command copy, including dependencies, with private
config/reporter/cache/temp files. This deliberately preserves the adapter's tested
isolation boundary; it costs additional copies and metadata scans. Shared workspace
metrics currently measure outer workspaces only. No performance claim is made.
Diagnostic retention obeys `--max-log-bytes`; raw capture obeys
`--max-capture-bytes`, including the normal unlimited/clear behavior. Completion
evidence remains independently bounded to 1 MiB.

## Automated acceptance

`tests/vitest_workflow.rs` uses an independent copy of the prepared fixture and
real CLI processes. The opt-in test checks:

- Six mutations: four killed and two missed, one survivor in each syntax.
- Original JS/TS source, manifest and lockfile preserved; saved logs and identity.
- A stronger JS test and fresh `--include src/flags.js` scan: three killed, zero
  missed; earlier reports unchanged. This is not a saved-report focused rerun.
- Shared report loading, rejection of corrupted identity, and explicit rejection
  of unsupported reruns.
- A mutation-triggered early exit is an infrastructure error, not a survivor.
- A runner that exits zero without completion fails the baseline, saves its
  diagnostic, publishes no mutations, and prints no baseline-success message.

Default tests reject unsupported parallel/affected/cache/policy settings before
execution. Existing ignored adapter probes cover timeout/cancellation, capture
limits, descendant cleanup, hooks, transforms and mixed failures; see
[the execution record](vitest-execution.md).

```sh
MUTIE_VITEST_FIXTURE=/path/to/prepared-fixture cargo test --test vitest_workflow -- --include-ignored
cargo test
cargo fmt --check
cargo clippy --all-targets -- -D warnings
```

## Remaining boundaries

Only `--jobs 1`, full-suite selection and `boolean_flip` are supported. Saved
reruns/command export, dismissal review, `--fail-on-missed` and `--min-score` are
explicitly rejected. Custom configs, JSX/TSX/CommonJS and workspaces remain outside
scope. Reported-hook failure attribution remains conservative; broader lifecycle
and fixture failures need separate validation. Parallel isolation, review/policies,
focused reruns, unmodified Pathe config support, useful real-project findings and
feedback time remain acceptance gates. JavaScript needs its own real project;
TypeScript evidence does not promote JavaScript independently.

Final validation (2026-10-15): the complete default suite passed, 229 tests with
35 opt-in tests ignored. The real CLI acceptance test and unsupported-option test
both passed against one unchanged CLI binary. Formatting and all-target Clippy
passed. The real test includes the intercepted-exit regression above; earlier
partial probes do not substitute for that final run.
