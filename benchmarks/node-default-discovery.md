# Native Node default discovery and unchanged TypeScript acceptance

Recorded 2026-10-18 on macOS with audited Node **24.22.0**. This follow-up removes
Bragkit's test-script adaptation from the [first native TS slice](node-typescript-scope.md)
and completes **Validated** acceptance for the existing native, dependency-free ESM
TypeScript scope. JavaScript and TypeScript evidence remain independent.

## Selection contract and audit

`package.json` may now declare exactly `"test": "node --test"`, in addition to the
existing explicit `.js`/`.ts` globs. Custom flags/loaders and package dependencies
remain unsupported. Source/import/config restrictions are unchanged.

The [Node test-runner documentation](https://nodejs.org/docs/latest-v24.x/api/test.html)
and the installed runtime's `internal/test_runner/runner` and `utils` were audited.
The runner uses an internal glob, excludes `node_modules`, and sorts the results.
With native type stripping enabled, its default pattern is:

```text
**/{test,test/**/*,test-*,*[._-]test}.{js,mjs,cjs,ts,mts,cts}
```

This selects more than `*.test.ts`: every matching-extension file under a `test`
directory is a test entry, including nested helpers. The initial adapter still
supports only audited `.js`/`.ts` files under the project's top-level `test/` tree.
Any native-selected file outside that set is rejected, including source-adjacent,
root-level, generated-output and unsupported-extension tests. No selected file is
silently dropped to fit the adapter. Each entry must complete real tests; empty,
all-skipped and incomplete entries retain the existing failure rules.

`node/discovery.rs` scans the full visible tree with entry/depth bounds. Dot
components and `node_modules` follow the native exclusions; visible symlinks and
non-regular entries are rejected. Original and snapshot test selections must
match, preventing an output/report-directory exclusion from removing tests.

Before commands, the pinned executable independently lists the native glob without
executing project code. The complete sorted result must equal the audited set.
Three exact hashes also pin the discovery implementation:

| Native module | SHA-256 |
| --- | --- |
| `internal/test_runner/runner` | `b1bb09fb385d58ef4bc751153c3e36a8b95165f4768642bd273cc2d466fd0e1f` |
| `internal/test_runner/utils` | `56bc4b056d4f77ff87a9e691a385e91dff0c15ac47e4444179c5b7fdd0a26013` |
| `internal/fs/glob` | `ecee340045b8d9d2f4ebca8f20ffa69306613c838e28d916d89eecb78ef12a64` |

The pattern, hashes and default-selection mode join the runner recipe. The final
command copy is rechecked, then Node runs without file arguments and performs its
own default discovery. Per-file completion still must match the audited selection.
This also preserves literal filenames containing glob punctuation: they are not
reinterpreted as separate user-supplied glob arguments.

Old explicit JavaScript and TypeScript reports retain their previous identities.
Default mode adds `default_discovery: true`; native TS also records
`test_pattern: native-default`. Conflicting saved modes fail validation. Switching
a package's test script requires a fresh scan; adding stronger test files under
the unchanged mode remains valid for focused reruns. Saved command descriptions
correctly show `node --test --test-concurrency=1` without an explicit glob.

## Unchanged Bragkit campaign

Project: [Bragkit](https://github.com/artmsilva/bragkit) at
`458d31127f1d3a6f3fd28afd2d7a4741ebc648a7`.
[Machine-readable evidence](node-default-discovery-results.json) retains source
hashes, exact ranges/outcomes, execution identity, timings and local report paths.

The original package, config, production sources and tests are unchanged. No
install/build was performed. Native default discovery selects its ten original
test files; the isolated baseline passes all **61 tests**. All **23** boolean
mutations complete: **2 killed, 21 missed, no errors or timeouts**. Every mutation's
source fingerprint, byte range and outcome matches the earlier adapted scan.
The original full-scan score remains **8.70%**.

The saved full run takes **68.71 seconds**. Concurrent Cargo and native workflow
verification overlapped this run, so this is an observed duration, not evidence
of a performance regression or a controlled comparison with the earlier scan.

A separate review copy adds only the [two retained HTML-list tests](fixtures/node-bragkit/mutie-html.test.ts).
Its baseline passes **63 tests**. A focused rerun of original source IDs **20, 21,
22** kills all three in **10.95 seconds**, with no errors/timeouts. The tests observe
an unmatched list close, a missing reopened list container, and repeated/unclosed
containers for adjacent items. Its 100% score is for that selection only. The other
**18 survivors were initially unreviewed**, covering CLI behavior, collector settings/error
responses and browser fallback. The [subsequent review](node-bragkit-survivors.md)
kills all 18 with stronger tests.

Hash checks verify all **57** original files in both the original and review
copies; the original checkout has clean Git status. All **105** files of the new
full report and all **105** files of the previous explicit-glob report remain
unchanged. No combined improved whole-project score is synthesized.

## Regression and milestone checks

- Default-discovery workflow runs both JS and TS fixtures. Nested helper entries
  kill the formerly missed fallback; a newly added test with brackets in its
  filename participates in a focused rerun. Unicode entries, including astral
  characters, use Node's UTF-16 sorting order. Hidden files are not executed.
- An empty discovered helper fails the baseline. A test outside the audited
  directory rejects the original project. A report-output exclusion that contains
  a test fails snapshot validation. Native inventory/hash disagreements are errors.
- Default-discovery TS independently passes timeout, cancellation and capture
  overflow cleanup, including descendant termination and owned-file removal.
- All nine Node workflow tests pass, preserving the 42 attribution scenarios per
  syntax, review/policies, configuration-sensitive reruns and report validation.
- Node unit/cleanup checks pass 13 tests. The default suite passes **250 tests**,
  with **53 ignored**; formatting and strict Clippy pass. Real adapter campaigns
  remain opt-in to prevent recursive self-mutation runs.

Reproduce with the audited runtime on PATH and an unchanged checkout of the
recorded Bragkit commit:

```sh
mutie /path/to/bragkit --lang node --ui plain --timeout 10 --baseline-timeout 15
# Copy the retained HTML test into a separate review checkout's test/ directory.
mutie /path/to/review --lang node --ui plain --timeout 10 --baseline-timeout 15 --rerun ORIGINAL_RUN --id 20,21,22
cargo test --test node_workflow -- --include-ignored --test-threads=1
cargo test --bin mutie node:: -- --include-ignored --test-threads=1
cargo test
cargo fmt --check
cargo clippy --all-targets -- -D warnings
```

Together with the first slice's typed fixture, independent lifecycle/cleanup
checks and source/report preservation, the unchanged real-project
run/inspect/improve-tests/rerun loop now meets **Validated**. This does not validate
arbitrary root/source-adjacent test layouts, declaration files, runtime dependencies,
loaders/transforms, other Node versions or parallel workers. The [subsequent findings review](node-bragkit-survivors.md) closes the 18 remaining
Bragkit survivors; scope expansions remain explicit decisions.
