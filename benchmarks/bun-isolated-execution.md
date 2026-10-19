# Isolated Bun baseline diagnostic

Recorded 2026-10-19 on macOS arm64 with Bun 1.3.11, revision
`af24e281ebacd6ac77c0f14b4206599cf4ae1c9f`.

`mutie bun-check PATH` now executes a baseline in an owned copy. It does not expose
`--lang bun`, run mutations, save reports, or implement reruns/review policies.
Bun remains **Planned** pending the complete real-project acceptance loop.

## First execution scope

The package must be named ESM with exactly `scripts.test: "bun test"`, no runtime
dependencies, installed dependencies or workspaces. Source and helpers must be
`.js`/`.ts` under `src/`, `test/` or `tests/`; native test files must use `.test.js`
or `.test.ts` under the test directories. Static relative imports must resolve to
those audited files. `node:` builtins and `bun:test` are allowed; `node:test`,
in-source Bun tests, dynamic imports and CommonJS require are rejected.

The whole original tree is inspected (except `.git`), so a test outside the scope
cannot be hidden by snapshot exclusions. Symlinks, alternate test/source extensions,
custom Bun configuration, environment files, nested packages, and tsconfig are
rejected. Input files and expected tests must match again after copying. The
TypeScript fixture uses Bun defaults; this does not establish configurable TS
project support. Composer's unchanged package is intentionally not accepted yet:
its tsconfig and additional source layout require the next scope audit.

## Execution and outcome boundaries

Bun is identified from a private directory with no project code loaded. The
command uses a private Bun config, home/config directory and temporary directory,
disables automatic dependency installation and environment-file loading, and clears
runner-injection environment variables. The exact Bun version/revision is required;
executable bytes and the bundled observer are checked around completed execution.
Saved execution identity is still pending because there are no saved Bun runs.

The shared process runner owns timeout, cancellation, combined raw capture limits
and process-group shutdown. Unix `RLIMIT_FSIZE` limits each regular file written by
Bun or descendants to 8 MiB, covering observer and native JUnit output. This also
limits project-generated files. It is **not a total disk quota**. The platform must
support this limit; the diagnostic does not silently fall back on other platforms.

Evidence is read only after command cleanup, using the Rust reader and independently
computed file inventory. Process/capture failures take precedence. Diagnostics are
retained to 64 KiB only after complete evidence classification. Owned workspace,
home and temporary paths are removed before returning the output.

A regression probe found that an oversized `Bun.write` rejects its body promise
with `EFBIG`, which otherwise looked like a confirmed body failure. The reader now
rejects the raw `EFBIG` diagnostic as negative evidence, alongside the native
unhandled-error diagnostic. Printing either marker may cause a conservative error;
neither can establish a kill. This rule must remain in future scored integration.

## Verification

Both CLI fixture baselines passed independently:

```sh
cargo run --offline -- bun-check tests/fixtures/bun-js --timeout 5
cargo run --offline -- bun-check tests/fixtures/bun-typescript --timeout 5
```

The real adapter matrix covers six cases per syntax: pass, body exception, combined
body/hook failure, body/pending-rejection mixture, assertion-plan failure and explicit
exit. Only the body exception returns body-failure evidence. All original input
hashes remain unchanged. Passing cases also verify that the actual workspace,
private home and temp paths no longer exist after execution.

Additional real probes cover stdout overflow, oversized observer evidence, an
oversized project-file write, timeout and cancellation. Cleanup probes require a
ready marker proving the test spawned its child before stopping it; delayed child
writes must never appear. A default shared-runner regression checks that the
per-file limit reaches children without affecting subsequent unrestricted commands.
The 112-case recorded native JS/TS replay remains part of the default suite.

```sh
cargo test --offline --bin mutie bun::tests -- --include-ignored
cargo test --offline --test bun_protocol
cargo test --offline
cargo fmt --check
cargo clippy --offline --all-targets -- -D warnings
```

The real Bun tests remain ignored by default. Baseline command timing excludes
validation, snapshotting and executable hashing; it is not end-to-end latency.

Next: audit Composer's configuration/import/layout boundary, establish unchanged
real-project baseline acceptance through this executor, then connect discovery,
scored runs and persisted execution identity to the shared workflow.
