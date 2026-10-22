# Scoped JS/TS logical swaps and negation removal

Acceptance work on 2026-10-22 extends the shared discovery used by Vitest, Node
and Bun. Existing runtime, installation, layout and lifecycle limits remain.
No package, report-schema or operator-catalog versions are bumped.

## Semantics and source identity

`logical_swap` replaces parsed `&& ↔ ||` expressions. Each operand and the entire
result are parenthesized, with inter-operand whitespace/comments retained. This
preserves the existing parse grouping despite the changed precedence. For example,
the inner mutation in `a && b && c` becomes `((a) || (b)) && c`, rather than
`a || b && c`. Short-circuit decisions intentionally follow the new operator;
operands are never duplicated and the result remains a native operand value.

`negation_remove` replaces a parsed unary `!` expression with its parenthesized
operand, retaining trivia after the operator. It removes Boolean conversion as
well as negation: `!value` becomes `(value)`, not `Boolean(value)`. This can change
the returned type. Each negation in `!!value` is independently eligible. Tests
verify value identity for objects and Symbols, falsy numbers, strings, BigInt,
`NaN`, null and undefined. Native truthiness does not call object coercion hooks.

The contract follows ECMAScript's
[logical operators](https://tc39.es/ecma262/2026/multipage/ecmascript-language-expressions.html#sec-binary-logical-operators)
and [logical NOT](https://tc39.es/ecma262/2026/multipage/ecmascript-language-expressions.html#sec-logical-not-operator).
Native exception and lifecycle classification remains unchanged. A newly evaluated
right operand can throw; this is an intentional consequence of the swapped
operator, subject to the adapter's existing provenance rules.

Type-only subtrees, TypeScript postfix non-null and definite-assignment assertions,
`??`, logical assignment and bitwise operators are excluded. Existing logical
expressions nested inside parenthesized nullish expressions remain eligible.
No new syntax/layout is admitted by runner validation.

Mutations cover full original expressions. Nested candidates are independent,
ordered by path, start byte, operator and end byte; boolean/comparison candidates
remain independent. Saved-report readers parse each saved logical original in its
JS/TS grammar and reconstruct the exact replacement recipe. An ungrouped token
swap or arbitrary expression is rejected. Focused reruns still require complete
source identity and fresh rediscovery; report-local IDs are not reused as fresh
inventory IDs. Bun's existing source-bound recipe requires fresh scans when the
discovery implementation changes. Node/Vitest execution recipes are unchanged.

## Controls

`src/javascript/logical_tests.rs` verifies Unicode/CRLF byte ranges, retained
comments, nested same-start edits, malformed replacement rejection, TS assertion
exclusion, and contexts containing await/yield, object literals and templates.
An initial expectation that `!value!` includes the TS postfix assertion in its
unary node was corrected: this grammar parses a postfix assertion around the
unary `!value`. The mutation edits only `!value`, leaving the postfix assertion
in place. The initial failing assertion is retained in the diagnostic log.

The ignored native probe exercises 18 semantic cases on both Node and Bun, in
both JavaScript and TypeScript: **72 bounded process cases**, passing in 4.53
seconds. Each has a 10-second deadline and 64 KiB capture limit. Checks include
explicit value/trace tables, grouping and nullish coexistence, raw object/Symbol
identity, no coercion hooks, exact left exceptions, skipped right side effects
and exceptions on newly evaluated branches. Expected results are independent
assertions, not generated with the replacement recipe.

`tests/javascript_logical.rs` passes all **six runner/syntax workflows** in 94.96
seconds using the pinned executables. Each fixture discovers five mutations:
three kills, one logical survivor and one negation survivor. Filtering selects
three logical swaps. Stronger body assertions kill both survivors; original
source and reports remain unchanged. A corrupted ungrouped replacement is
rejected. A separate setup hook triggered by the logical mutation produces an
error and zero kills, preserving the score boundary.

Existing Node/Bun boolean workflow controls explicitly select `boolean_flip` so
their original candidate counts, review IDs and assertions remain meaningful as
the default catalog expands. The native TS discovery unit test now explicitly
expects its three booleans plus the logical swap. This fixes an initial default
suite failure caused by the old three-candidate expectation; it does not remove
the newly discovered candidate.

## Unchanged public-project finding

Project: [pp-is](https://github.com/carlos-sweb/pp-is), unchanged commit
`1d5b3bac804e1e67da289f3d2a14bd0705a383fb`, using pinned Bun 1.3.11. The existing
checkout `/tmp/mutie-bun-pp-is` remains clean, including the original
manifest, packaging files, sources and tests. The prior implicit-ESM scope applies;
no source, test, metadata or layout adaptation is made.

The selected files are `src/main/isBoolean.js`, `isElement.js`, `isTruthy.js` and
`isFalsy.js`. All 58 original tests run for baseline and each candidate. Eight
selected mutations finish in 6.52 seconds: six kills, two survivors, no errors or
timeouts. The filtered original score is 6/8 (75%), not a full-project score.
All five selected negation removals are killed. The two survivors are logical
swaps in `isBoolean`:

1. The primitive fast path `value === true || value === false` becomes a conjunction.
   The original fallback still recognizes the primitive booleans exercised by tests.
2. The outer disjunction joining that primitive check to `getTypeCompare` becomes
   a conjunction. Primitive boolean tests still pass, but boxed booleans are rejected.

A separate copy adds [a boxed-Boolean test](fixtures/javascript-logical/pp-is-boolean.test.js)
checking both `new Boolean(true)` and `new Boolean(false)`. The unchanged production
implementation accepts both via its existing type helper. The 59-test pristine
baseline passes; the focused two-survivor rerun kills the outer swap while the
inner fast-path candidate remains missed. The latter is left unresolved, without
a blanket equivalence claim or dismissal. This is a useful expanded-input finding,
not a new full stronger-test score.

The original run is
`/tmp/mutie-js-logical-audit/exploratory-reports/run-1792696400-nzPoyQ`;
the stronger run is
`/tmp/mutie-js-logical-audit/stronger-reports/run-1792696501-yyOCbT`.
Despite the original directory's exploratory name, that report is finalized and
completed. It is preserved byte-for-byte. The stronger rerun took 2.52 seconds.
[Machine-readable evidence](javascript-logical-results.json) records identities,
outcomes, timings, all **63** original tracked input hashes and **45** original
report-file hashes. Both copies preserve all original files; only the new test is
added to the stronger copy. Historical comparison reports are untouched.

## Reproduction and limits

Put Node 24.22.0 and Bun 1.3.11 executable directories directly on PATH. This
acceptance used the prepared npm Vitest 4.1.9 / Vite 8.1.3 installation at
`/tmp/mutie-package-managers-audit/npm` for `MUTIE_VITEST_FIXTURE`.
Mutie never installs dependencies.

```sh
cargo test --bin mutie javascript::logical_tests -- --include-ignored
cargo test --test javascript_logical -- --ignored --test-threads=1
cargo test --test node_workflow reports_reruns_review_policies_and_preservation -- --ignored --test-threads=1
cargo test --test bun_workflow scored_js_ts_reports_reruns_and_preservation -- --ignored --test-threads=1
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

```sh
mutie /path/to/unchanged/pp-is --lang bun --ui plain \
  --include src/main/isBoolean.js --include src/main/isElement.js \
  --include src/main/isTruthy.js --include src/main/isFalsy.js \
  --operator logical_swap,negation_remove --timeout 20 --baseline-timeout 30 \
  --output-dir /outside/original-reports
mutie /path/to/separate/stronger --lang bun --ui plain \
  --rerun-missed /outside/original-reports/RUN \
  --timeout 20 --baseline-timeout 30 --output-dir /outside/stronger-reports
```

Final verification: **311 default tests passed, 0 failed, 120 ignored**.
The two Node boolean workflows passed in 87.98 seconds; the paired Bun JS/TS
boolean workflow passed in 85.93 seconds. Clippy (`--all-targets -- -D warnings`),
formatting and diff whitespace checks pass. Native adapter regressions remain
opt-in to avoid recursive self-mutation.

This is macOS evidence within the pinned serial scopes. The deferred intermittent
startup/readiness issue is not fixed by operator support or passing reruns. No
security settings, existing deadlines, assertions or classifiers are relaxed.
Arithmetic/modulo, remaining Rust/Python expression gaps, control-flow/removal,
literals and release qualification remain on the roadmap.
