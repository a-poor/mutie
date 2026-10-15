# Static coverage-only Vitest config acceptance

The prepared Pathe project retains its original `vite.config.ts`:

```ts
import { defineConfig } from "vitest/config";

export default defineConfig({
  test: {
    coverage: {
      reporter: ["text", "json", "html", "clover"],
    },
  },
});
```

Mutie recognizes this declarative shape statically. It never loads the project
config; the private execution config continues to disable coverage. Reporter
preferences therefore have no effect on mutation execution. This removes the
need to delete Pathe's config from the prepared project.

## Supported shape and identity

At most one `vite.config.ts`, `vite.config.js`, `vitest.config.ts` or
`vitest.config.js` is allowed. It must import the unaliased `defineConfig` from
`vitest/config` and default-export its call with only `test.coverage.reporter`.
The reporter array must contain nonempty literal selections from `text`, `json`,
`html` and `clover`. Quotes, comments, whitespace and trailing commas are accepted;
escaped strings, computed values, functions, spreads, getters, other imports,
extra statements, extra settings and other config extensions are rejected.
TypeScript files must use this plain JavaScript-compatible form. This does not
provide general custom-config, plugins or coverage support.

Recognition uses the existing Tree-sitter JavaScript parser, checks parse errors,
then compares the complete token structure. No project JavaScript is evaluated.
Files must be regular files; multiple configs are rejected. Input is limited to
64 KiB and traversal depth to 32. Accepted bytes join the existing project config
fingerprint map. The same map is compared after isolated snapshot copying, and
saved reruns require exact equality, including otherwise harmless formatting edits.
No package, report or operator versions changed.

## Validation

Unit tests accept Pathe's shape and formatting variants, reject executable or
unsupported forms, cover all four filenames, check fingerprint changes and reject
multiple configs. The real CLI regression adds one boolean mutation with the
coverage-only config present, verifies a passing baseline and one saved kill,
checks source/config preservation and absence of a source coverage directory,
then rejects a rerun after a config-only edit. A config with a filesystem write
is rejected without creating its sentinel.

```sh
cargo test --bin mutie vitest::
MUTIE_VITEST_FIXTURE=/tmp/mutie-vitest-fixture-scope \
  cargo test --test vitest_workflow coverage_only_config_runs_and_invalidates_reruns -- --ignored
cargo test
cargo fmt --check
cargo clippy --all-targets -- -D warnings
```

The 18 targeted unit tests and the real CLI config regression passed (6.32 seconds
for the CLI test). Local logs use `/tmp/mutie-vitest-config-` with
`unit.log`, `cli.log`, `default.log` and `clippy.log` suffixes.

## Pathe with its original config

Project: `/tmp/mutie-vitest-pathe-scope`, prepared from revision
`bc7477a01f0bd60ada017add8142c9f9d69ccdc5` with pinned Node 24.22.0,
pnpm 11.10.0, Vitest 4.1.9 and Vite 8.1.3. Dependencies remain the prepared pinned
installation; the source, tests and original config do not require adaptation.

`vitest-list` discovers 14 boolean flips. `vitest-check` passes 467 tests with
16 skipped/todo, taking 4.11 seconds of command time. The full CLI baseline passes
with the same original config (4.44 seconds of command time). Logs are
`/tmp/mutie-pathe-config-list.log`, `mutie-pathe-config-baseline.log` and
`mutie-pathe-config-run.log`.

The complete all-discovered run finished in 94.00 seconds: **12 killed, two missed,
zero timeout/build failures/execution errors**, raw score 85.71%. Saved run:
`/tmp/mutie-pathe-config-reports/run-1792103000-UigGuM`.
The reader validated the report before reporting that interactive terminal setup
was unavailable. Its saved identity includes the original `vite.config.ts` hash
`sha256:5e1abc92f1e706657f3bd8e05a662aec111a814de626462515cbc0f47d84a40b`.
All 13 hashed source/test/config/package files remained unchanged.

The survivors are ID 12 (`src/_path.ts:106`, `resolvedUNC` initially false to true)
and ID 13 (`src/utils.ts:42`, the normalized-alias marker value true to false).
They are findings to investigate, not yet established missing-test cases; either
could be redundant/equivalent within the supported inputs. Stronger tests and
focused reruns are the next real-project gate. JavaScript still needs independent
real-project acceptance. This result does not promote the support level.

Final validation: 234 default tests passed (41 ignored), formatting and diff
whitespace checks passed, and all-target Clippy passed. No dependency installs,
network calls or changes to Pathe's original config were needed for this run.
