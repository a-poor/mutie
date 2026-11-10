# Reproducing the Node composed-recipe evidence

Use fresh directories and explicit local paths. The external project and its
dependencies are not vendored. `identical-this.js` and
`is-promise-type-contract.js` are new stronger tests, installed only in a separate
copy after the original runs. See [the acceptance record](../../node-eslint.md)
and [results](results.json) for expected outcomes and limits.

## Installation

Use Node 24.22.0, npm 11.19.1 and a release Mutie build. Put the pinned Node
directory first on `PATH`, set `CI=true`, and unset `NODE_OPTIONS` and `NODE_PATH`.

```sh
git clone https://github.com/sindresorhus/eslint-node-test.git /path/to/native
git -C /path/to/native checkout dcb850e2bd01534792fd67bcec4747828fa96df9
cd /path/to/native
npm install --legacy-peer-deps --ignore-scripts --no-audit --no-fund
npm test
```

The recorded original installation passes lint but fails two of 5,989 tests.
Its `package.json` omits `@types/node`, although the test fixture requests it.
Upstream disables package-lock creation. `installation.json` retains the observed
npm resolution, including package integrities; it is not an upstream lockfile.
Do not assume a later floating install matches. Mutie rejects drift in the pinned
recipe tools; other dependency changes also need a fresh baseline and new evidence.

Install the missing declarations separately, without changing the project:

```sh
mkdir /path/to/type-tools
cd /path/to/type-tools
npm install --ignore-scripts --no-audit --no-fund --save-exact @types/node@24.19.2 undici-types@7.24.6
```

Make two complete copies of the installed project, preserving symlinks and modes:
`/path/to/original` and `/path/to/stronger`. Keep `.git` and previous `.mutie`
outputs out of these copies. Copy only `node_modules/@types/node` and
`node_modules/undici-types` from the type-tools installation into each copy.
Do not replace either project's hidden npm installation metadata. The original
source/tests/configuration remain byte-for-byte unchanged; the installation has
the documented 122-file addition. The complete native `npm test` must now pass
5,989 tests before mutations. The 491 original input hashes are already retained
under `projects.eslint-node-test.inputs` in
[`compatibility-audit-results.json`](../../compatibility-audit-results.json).

## Original runs

Keep configuration outside the project. Create three configs with this template,
using `contains-suspension-point.js`, `is-same-reference.js`, and
`is-promise-type.js` respectively:

```toml
jobs = 1
timeout = 120
baseline_timeout = 180

[js]
runner = "node"
path = "/path/to/original"
operators = ["boolean_flip"]
include = ["rules/utils/is-promise-type.js"]
```

Run each config with the same release executable:

```sh
/path/to/mutie run --plain --config /path/to/promises.toml
```

Expected results: suspension has one kill then one lint error; reference stops
at its first lint error; promise has three survivors. Keep the two failed reports
and their pending mutations. Use the saved reference report to run its second
candidate against the original suite:

```sh
/path/to/mutie rerun /path/to/original/.mutie/REFERENCE_RUN --id 2 --plain --config /path/to/references.toml
```

Confirm the selected source identity is `is-same-reference.js:122:11`,
`true` to `false`. It survives. Report-local IDs change in focused runs; use the
new report's inventory instead of assuming the original ID persists.

## Stronger tests and preservation

Hash every original report file before proceeding. Copy both retained `.js`
files into `/path/to/stronger/test/unit/`. Change only the external configs'
`js.path` to that stronger directory. The full native `npm test` must pass
5,993 tests. Rerun the original promise report and the completed focused reference
report, keeping the complete lint/test scope:

```sh
/path/to/mutie rerun /path/to/original/.mutie/PROMISE_RUN --missed --plain --config /path/to/stronger-promises.toml
/path/to/mutie rerun /path/to/original/.mutie/FOCUSED_REFERENCE_RUN --missed --plain --config /path/to/stronger-references.toml
```

Compare source identities, outcomes and saved scores with `results.json`. Recheck
all original input/report hashes, the native installation and both supplemented
installations. Hash symlink targets as link bytes, not dereferenced file content;
retain modes too. Dependency inventory digests use sorted relative paths and
compact sorted-key JSON entries `{link, mode, sha256}`, with octal permission modes.
New stronger tests and new rerun reports are separate artifacts. Do not rewrite
the original reports or their 0%/unavailable scores.

Run the regression controls with the pinned Node on `PATH`:

```sh
cargo test --bin mutie node::recipe_tests:: -- --ignored --test-threads=1
cargo test --test node_workflow -- --ignored --test-threads=1
cargo test --locked
cargo fmt --check
cargo clippy --all-targets --locked -- -D warnings
```
