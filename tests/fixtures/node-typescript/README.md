# Native TypeScript Node fixture

One dependency-free ESM package on audited Node 24.22.0. Run
`node --test test/*.test.ts` from this directory, without installing tooling.
The three runtime boolean flips produce two kills and one survivor. Type-only
booleans in aliases, interfaces, generic arguments and return annotations are
excluded. The `.js` identity helper also exercises mixed static imports.

Native execution strips types without checking them or applying tsconfig settings.
The test's deliberately invalid `const runtime: true = false` still executes;
mutated literal return types do not turn runtime assertion failures into compiler
failures. Adding `assert.equal(fallback(false), true)` kills the survivor.

From the Mutie repository root, with the pinned Node on PATH:

```sh
cargo test --test node_workflow -- --include-ignored --test-threads=1
cargo test --bin mutie node::tests -- --include-ignored --test-threads=1
```

The workflow checks saved reports, browser validation, corrupt/legacy scope
rejection, review/policies, test-only reruns, manifest/config identity and source
preservation. Native TS repeats all 42 mutation-triggered body/hook/import/late
failure scenarios. Cleanup checks timeout, cancellation, output overflow and
spawned descendants. These real process tests remain ignored by default.

The workflow also changes only the test script to `node --test` to exercise native
default selection: nested helpers and newly added test files participate, empty
helpers fail, and out-of-scope tests or output exclusions cannot shrink the suite.
A separate default-discovery cleanup probe covers the same process limits.
