# Bun Rust evidence-reader replay

The reader in [`src/bun/protocol.rs`](../src/bun/protocol.rs) ports the
[bounded attribution prototype](bun-body-attribution.md). It is compiled through
[`tests/bun_protocol.rs`](../tests/bun_protocol.rs), with the XML parser currently
a dev dependency. No CLI command invokes it yet. Bun remains **Planned**.

## Checks and results

The default integration test replays every retained native record: 56 JavaScript
and 56 TypeScript cases. Results match the prototype exactly: 16 passes, 20 body
failures, two native timeouts and 74 errors/unsupported cases. No Bun installation
or project execution is needed for this replay. Corrupted identities, sequences,
checkpoints, exit statuses, counts and XML/JSONL data are rejected.

An opt-in replay also checks all four full Composer captures against their retained
SHA-256. Its expected file inventory comes from the original source manifest:
11 original test files, plus the retained stronger test for the two stronger runs.
The original baseline, surviving candidate and stronger baseline pass; the stronger
candidate has one confirmed body failure. Adding an expected but unreported file
rejects each run. These are replays of earlier native execution, not new adapter
runs or mutation scores.

## Additional reader boundaries

The caller must supply a nonempty, unique expected-file inventory independently of
JUnit. Every top-level file suite must occur exactly once, and nested suites and
cases must retain that file identity. A missing or extra file rejects the report.
The observer still matches bodies by registration ID, order and name; source-file
attribution in the observer and package discovery require execution acceptance.

Both evidence inputs are limited to 8 MiB. XML rejects DTDs/entities, namespaces,
unsupported result nodes, hidden result nodes in properties, more than 200,000 XML
nodes and excessive nesting. Observer input requires complete newline-terminated
JSONL, at most 100,000 events, positive integer IDs, prior declarations and exactly
one matching terminal per invocation. A provisional failure cannot become a return.
The full raw stderr diagnostic is negative evidence only. Outer command timeouts
remain errors at this reader boundary; the future executor must handle its own
shared timeout/cancellation outcomes before asking for completed-run attribution.

These are **reader limits**, not disk/capture limits during execution. Before any
score is emitted, the adapter still needs package/import/config validation,
independent discovery, observer/runtime identity checks, bounded output production,
owned workspaces, process-group cleanup and report/rerun integration.

## Verification

```sh
cargo test --offline --test bun_protocol
MUTIE_BUN_COMPOSER_EVIDENCE=/tmp/mutie-bun-observed-project-results.json \
  cargo test --offline --test bun_protocol -- --include-ignored
cargo fmt --check
cargo clippy --offline --all-targets -- -D warnings
```

The full local Composer evidence path is also recorded in
[`bun-observer-composer-results.json`](bun-observer-composer-results.json).
It is optional and not required for the default suite.

The [isolated baseline follow-up](bun-isolated-execution.md) now connects this
reader to `bun-check` and moves XML parsing into runtime dependencies. It also
rejects native `EFBIG` diagnostics so the file-size limit cannot supply positive
body-failure evidence. Scored runs remain pending.
