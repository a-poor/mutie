# Composer through isolated Bun execution

Recorded 2026-10-19, Bun 1.3.11+af24e281e, macOS arm64. This extends the
[baseline diagnostic](bun-isolated-execution.md) to unchanged
[GramIO Composer](https://github.com/gramiojs/composer), revision
`3de15b846e3f5ef46cf8aebd5d136908baa43b54`.

## Scope audit

Composer's root tsconfig is plain JSON with NodeNext module/resolution, ES2022
target, forced module detection, ESNext/DOM libraries and type-check/emission
settings. `bun/config.rs` accepts only this narrow set of option names and values.
Typed JSON objects reject duplicate and unknown keys. Inheritance, references,
path mappings, base URLs, JSX, decorators and alternate module settings remain
unsupported. Config bytes are part of the full input fingerprint checked after
snapshotting; configuration is never executed by Mutie. This is native execution,
not type checking or a promise of general tsconfig support.

The original `examples/` and `scripts/` are copied and fingerprinted but cannot
be imported by audited source/tests. They cannot contain test filenames. Nested
packages/configs, jsconfig, symlinks and unsupported test extensions remain rejected.
The publish-only `tsconfig.build.json` is retained as data; it is not the runtime
configuration and cannot be imported through the audited module set. Dev tool
imports in release scripts do not cause installation or execution.

Bun's [module-resolution documentation](https://bun.sh/docs/runtime/module-resolution)
identifies tsconfig path remapping as runtime behavior, so aliases are rejected.
Its [test-discovery documentation](https://bun.sh/docs/test/discovery) describes
recursive test suffix matching; the validator inspects the complete original tree
and rejects out-of-scope candidates instead of excluding them during copying.
Those current documents are background; acceptance below uses the pinned runtime.

## Executor results

[Machine-readable results](bun-composer-execution-results.json) retain all original
input hashes, the observer hash, command durations and diagnostic hashes.

| Isolated control | Passed | Failed | Evidence |
| --- | ---: | ---: | --- |
| Unchanged baseline | 272 | 0 | Passed |
| `configurable: true` → `false` | 272 | 0 | Survives original tests |
| Stronger-test baseline | 273 | 0 | Passed |
| Same candidate with stronger test | 272 | 1 | Confirmed body failure |

The stronger test is the retained
[`composer-stronger.test.ts`](fixtures/bun-audit/composer-stronger.test.ts): reusing
and renaming a middleware function requires a configurable name property. The
executor finds the same useful boundary as the earlier direct-native probes.
Command times were about 0.49–0.50 seconds. These exclude validation, copies and
executable hashing and are not an end-to-end performance claim.

The original checkout remains unchanged. Each control starts from an owned copy,
and its input hashes are checked again after execution. Original config, package,
source and tests were not adapted for the baseline. The stronger test and manual
candidate exist only in temporary copies. The original 11-file inventory and the
12-file stronger inventory both pass strict reader accounting.

The internal shared outcome names `Missed`/`Killed` in the JSON describe these
controls only. There is no saved mutation report or computed score. Bun remains
**Planned** until discovery, persisted execution identity, scored runs and the
full rerun/review workflow are accepted independently for JS and TS.

## Reproduce

```sh
cargo run --offline -- bun-check /path/to/unchanged/composer --timeout 15
MUTIE_BUN_COMPOSER=/path/to/unchanged/composer \
  cargo test --offline --bin mutie bun::tests::real_composer_baseline_and_stronger_finding \
  -- --ignored --nocapture
cargo test --offline --bin mutie bun:: -- --skip real_
cargo fmt --check
cargo clippy --offline --all-targets -- -D warnings
```

The real-project test rejects a checkout whose file hashes differ from the earlier
audited revision. Default scope regressions also reject forbidden config fields,
duplicate keys, ancillary test files and imports into release scripts, and verify
that config-only edits change input identity.
