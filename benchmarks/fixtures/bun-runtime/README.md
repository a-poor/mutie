# Bun runtime acceptance

The retained JSON files contain independent JS/TS native and observed runs for
exact Bun 1.3.11+af24e281e and 1.4.4+38afee6c8. The original production observer is
unchanged. Expected classifications are fixed before execution, and
`tests/bun_protocol.rs` replays the records through the production Rust reader.
Raw-record hashes describe pre-sanitization evidence; published paths/hostnames
are placeholders. The fixtures contain Mutie test code, not upstream codebases.

Recreate native captures with a fresh output filename:

```sh
python3 benchmarks/fixtures/bun-runtime/audit.py --bun /path/to/bun-1.3.11 --out /tmp/bun-1.3.11.json
python3 benchmarks/fixtures/bun-runtime/audit.py --bun /path/to/bun-1.4.4 --out /tmp/bun-1.4.4.json
cargo test --locked --test bun_protocol
```

The capture script records the comparison; it does not independently classify
the new output. The Rust replay tests check the retained records. With each exact
runtime first on `PATH`, also run the live adapter and scored workflow controls:

```sh
cargo test --locked --bin mutie bun::tests::real_ -- --ignored --test-threads=1 --skip real_composer_baseline_and_stronger_finding
cargo test --locked --test bun_workflow -- --ignored --test-threads=1
cargo test --locked --test javascript_assignments --test javascript_numbers --test javascript_strings bun -- --ignored --test-threads=1
```

## Real projects

Fetch pp-is at `1d5b3bac804e1e67da289f3d2a14bd0705a383fb` from
https://github.com/carlos-sweb/pp-is and Composer at
`3de15b846e3f5ef46cf8aebd5d136908baa43b54` from https://github.com/gramiojs/composer.
Make complete original and stronger copies of every tracked file, preserving
symlinks/modes and omitting only Git metadata. Neither target needs an installation
for these tests. Do not use the historical pp-is layout adaptation: its original
checkout is now accepted.

Use Bun 1.4.4 first on `PATH` and a release Mutie build. Keep configuration outside
the copies:

```toml
jobs = 1
timeout = 5
baseline_timeout = 15

[js]
runner = "bun"
path = "/path/to/pp-is-original"
operators = ["boolean_flip"]
```

For Composer use its original directory and add `include = ["src/utils.ts"]`.
Run both native `bun test` baselines and both `mutie run --plain --config CONFIG`
selections. Expected original outcomes are 12 kills/10 survivors for pp-is and
one kill/one survivor for the two-site Composer selection.

Hash the original reports before review. In the separate pp-is copy, add
`../bun-pp-is/stronger.test.js` as `test/mutie-stronger.test.js`; its native baseline
has 65 passes. In the separate Composer copy, add
`../bun-audit/composer-stronger.test.ts` to its test directory; the baseline has
273 passes. Change only the external configs' paths and run:

```sh
mutie rerun /path/to/pp-is-original/.mutie/RUN --missed --plain --config /path/to/pp-is-stronger.toml
mutie rerun /path/to/composer-original/.mutie/RUN --id 1 --plain --config /path/to/composer-stronger.toml
```

Confirm Composer ID 1 is `src/utils.ts:7:17`, `true` to `false`, using the saved
inventory. IDs are local to a report. Expected focused outcomes are seven kills/
three survivors and one kill respectively. Re-run these commands with 1.3.11 first
on `PATH`: identity validation must fail before any baseline or mutation. Preserve
the failed diagnostic reports as well as all original input/report hashes.

## Elysia native prerequisite

The Elysia revision and full native command are documented in
[`bun-elysia-baseline.md`](../../bun-elysia-baseline.md). Fetch it separately; keep
all 296 tracked inputs, including the large test video. Use Node 24.22.0 and npm
11.19.1. In a disposable complete copy, run:

```sh
/path/to/bun-1.3.11 install --frozen-lockfile --ignore-scripts
/path/to/bun-1.3.11 run build
/path/to/bun-1.3.11 run test
```

The older complete command fails four native tests. Run `test:imports`,
`test:types` and `test:node` independently to diagnose the stages that `&&` skipped;
their success does not repair that failed full command. Use Bun 1.4.4 first on
`PATH`, clear only generated compiler build-info caches, and run the original
build/test commands again. Expect 1,709 passes, two unchanged upstream skips and
passing import, type and Node phases. The separate Cloudflare CI command remains
outside this acceptance. This native baseline does not make Elysia admissible to
Mutie or produce a mutation score.
