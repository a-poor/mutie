# Bun discovery preview and diagnostic identity

Recorded 2026-10-19 on the unchanged Composer checkout at
`3de15b846e3f5ef46cf8aebd5d136908baa43b54`. This follows the
[isolated Composer controls](bun-composer-execution.md).

## Read-only mutation preview

`mutie bun-list PATH` validates the whole scoped package, then discovers runtime
boolean flips under `src/`. It neither launches Bun nor executes project code.
`--include` and `--exclude` filter production mutations without weakening package,
test or import validation. Inputs are checked again after discovery to reject
changes between validation and collection.

`--json` emits ordered IDs, relative source paths, source fingerprints, byte ranges,
line/column locations, original/replacement text, exclusions, expected test files,
and an aggregate input fingerprint. `runtime_checked: false` explicitly distinguishes
this read-only preview from an executed baseline. There are no saved reports.

Composer contains **12 runtime flips**: three in `composer.ts`, three in
`factory.ts`, two in `macros.ts`, two in `queue.ts`, and two in `utils.ts`.
The known name-configurability finding is candidate 11 in this full preview.
Type-only literals are omitted. Composer's `declare const ContextCallbackBrand:
unique symbol` exposed a difference from existing discovery scopes: Bun discovery
now skips the erased ambient declaration subtree. The default Node/Vitest discovery
path still rejects ambient declarations; enums and other unsupported constructs
remain rejected by Bun discovery too. No broader TypeScript transform support is
inferred from this change.

## Baseline identity JSON

`mutie bun-check PATH --json` emits a successful baseline result and its execution
identity. Identity contains the validated Bun version/revision, executable path and
byte fingerprint, package manifest and configuration/lock fingerprints, expected
test inventory, timeout and capture/file limits, and a recipe fingerprint covering
observer, protocol, validation, execution, capture/cleanup and dependency-lock code.
The executable identity is checked around completed execution. Original input bytes
are recorded separately as an aggregate fingerprint.

This JSON can be retained as diagnostic evidence, but it is **not a saved mutation
report**, an executable request, or a defined rerun compatibility policy. It does
not grant permission to reuse outcomes. Test inventory changes alter this diagnostic
identity; accepting stronger-test changes during saved reruns remains a separate
integration task. Errors still go to stderr with a nonzero exit status.

[Recorded output](bun-discovery-identity-results.json) contains the complete preview
and a passing 11-file Composer baseline. Their aggregate input fingerprints agree,
and all original audited file hashes are preserved. No source/tests/config were
adapted. Command duration excludes preparation and runtime hashing.

## Verification

```sh
cargo run --offline -- bun-list /path/to/composer --json
cargo run --offline -- bun-check /path/to/composer --timeout 15 --json
cargo test --offline
cargo fmt --check
cargo clippy --offline --all-targets -- -D warnings
```

The CLI regression runs both JS and TS previews with an empty PATH, verifies offsets
against original bytes and unchanged inputs, exercises empty selections, and rejects
an invalid test import even when all production files are excluded. Identity tests
cover serialization, unknown fields, runtime replacement, configuration presence,
inventory changes and limits. A shared-discovery regression preserves the existing
Node/Vitest rejection while checking Bun's erased declarations.

Next: connect Bun to shared scored runs and saved reports, then validate focused
reruns and review/policy behavior. Bun remains Planned; `--lang bun` is not available.

Follow-up: [shared scored runs](bun-shared-runs.md) now integrate this identity
with saved reports and focused reruns; Bun is Experimental within that scope.
