# Scoped JS/TS comparison mutations

Acceptance on 2026-10-22 extends the existing scoped Vitest, Node and Bun
adapters. Package-manager, runtime, source-layout and lifecycle restrictions
remain unchanged. No package, report-schema or operator-catalog version changes.

## Operator contract

Shared Tree-sitter discovery emits these token-only replacements:

| Operator | Original → replacement pairs |
| --- | --- |
| `comparison_invert` | `== ↔ !=`, `=== ↔ !==`, `< ↔ >=`, `<= ↔ >` |
| `comparison_boundary` | `< ↔ <=`, `> ↔ >=` |

Only the `operator` field of a runtime `binary_expression` is replaced. Operands,
comments, grouping and evaluation counts stay in the original source. Strict
and loose equality never cross classes. Type annotations, interfaces, generic
parameters/arguments, conditional types, strings, comments and regular expressions
are excluded; so are `in`, `instanceof`, shifts and test files. Nested comparisons
retain JavaScript's native binary grouping (unlike Python's chained-comparison
syntax). No type inference, type checking or exception conversion is introduced.

Ordering inversion describes the token pair; it is not Boolean complementation
for unordered values such as `NaN`. Native string ordering, BigInt comparisons and
object conversion apply. The semantic basis is ECMAScript's
[relational and equality evaluation](https://tc39.es/ecma262/2023/multipage/ecmascript-language-expressions.html#sec-relational-operators)
and [comparison abstract operations](https://tc39.es/ecma262/2025/multipage/abstract-operations.html#sec-islessthan).

Candidates sort by source path, byte offset and operator. Report IDs can therefore
change as the catalog grows; focused reruns match full saved source identities,
not old report-local IDs against a fresh inventory. Readers require the exact
operator/original/replacement triple. Bun's existing recipe hashes discovery
bytes, so older Bun recipes require a fresh scan. Node/Vitest execution recipes
are unchanged. Read-only previews now print the actual operator rather than a
hardcoded boolean label. Existing boolean discovery remains covered.

## Semantic and workflow controls

`src/javascript.rs` has JS/TS grammar controls for all twelve replacements,
precise byte ranges with Unicode/CRLF, independent same-span operators, inert type
syntax and nested comparisons. Its ignored native probe executes all twelve
replacements in both syntaxes on Node and Bun: 48 process cases, each with a
10-second bound and 64 KiB capture budget. Explicit truth tables cover equal
numbers, `NaN`, mixed BigInt/string values, lexicographic strings, signed zero,
null/undefined, operand evaluation and object coercion order. Throwing coercions
preserve the exact exception and evaluate the right operand before conversion;
strict equality avoids conversion. Outer short-circuiting remains intact.
The bounded probe and ten discovery controls pass in 2.08 seconds using direct
pinned executables.

`tests/javascript_comparisons.rs` independently exercises all six combinations:
JavaScript and TypeScript through Vitest, Node and Bun. Each full fixture discovers
14 candidates: 13 kills and one exact-limit boundary survivor. Operator filtering
selects five boundaries; a stronger body assertion kills the survivor in a focused
rerun. Corrupting a loose-equality replacement into strict inequality is rejected.
Sources and saved original report bytes are preserved. The final lifecycle control
uses that boundary to break `beforeEach`, requiring an execution error, not a kill.

`tests/bun_preview.rs` checks JSON operator labels without any runtime on PATH;
`tests/vitest_preview.rs` checks text labels using an identity-only launcher and a
Vitest entry point that must never execute.

Initial Vitest matrix attempts using the host's Vite+ Node shim failed at Node
identification. With the existing direct Node 24.22.0 executable on PATH, both
Vitest syntax workflows passed (104.09 seconds combined). Node and Bun independently
passed both syntax workflows in the initial matrix. The initial native semantic
probe also passed through the shim but took 126.05 seconds; the retained probe uses
bounded execution. These observations do not resolve the deferred host startup
issue or justify relaxed readiness limits.

The final lifecycle matrix passed five combinations but its Vitest TypeScript
full fixture failed: only eight expected kills completed before later commands
hit the unchanged 20-second mutation deadline. An in-progress snapshot retained
four timeouts (IDs 9–12) with empty stdout/stderr, and the test failed its exact
13-kill assertion. The available snapshot is retained at
`/tmp/mutie-js-comparisons-audit/slow-vitest-ts-reports`; it is incomplete,
not a finalized acceptance report. A bounded stack-sampling attempt found no
remaining child by the time it ran. No cause was established. This is recorded
separately from semantic acceptance, and no deadline/assertion was relaxed.
The isolated Vitest TypeScript rerun then passed the complete fixture, filtered
selection, stronger-test rerun, corrupt-identity rejection and setup-error control
in 58.34 seconds. All six combinations therefore have passing final controls,
across the five-pass matrix and this isolated rerun. This does not erase the
failed attempt or establish that intermittent startup/readiness failures are fixed.


## Unchanged public-project validation

Project: [pp-is](https://github.com/carlos-sweb/pp-is), commit
`1d5b3bac804e1e67da289f3d2a14bd0705a383fb`. The original checkout at
`/tmp/mutie-bun-pp-is` remains clean. Its original package manifest, absent
`type`, packaging files, all sources and all tests are retained. This uses the
previously validated narrow implicit-ESM Bun scope; nothing is adapted for this run.
Pinned Bun is `1.3.11+af24e281e`.

Select `src/main/isIPv4.js` and both comparison operators, keeping the entire
58-test baseline and mutation suite. Six candidates complete with four inversion
kills, two boundary survivors, no errors and no timeouts. The original filtered
score is 4/6 (66.67%); this is not a full-project mutation score.

The survivors change `num >= 0` to `num > 0`, and `num <= 255` to `num < 255`.
Existing tests cover an ordinary address, 256, a missing octet and leading zeroes,
but never octets exactly zero or 255. Both endpoints are valid IPv4 octets and are
observable through the public `isIPv4` function.

A separate copy at `/tmp/mutie-js-comparisons-audit/pp-is-stronger` adds only
[one endpoint test](fixtures/javascript-comparisons/pp-is-boundaries.test.js).
It checks each endpoint in each of the four positions. The 59-test pristine suite
passes, and `--rerun-missed` kills both saved survivors. Production source is
unchanged. The original score and original reports are unchanged; the focused
2/2 result does not claim a full stronger-suite score.

The final original run is `final-original/run-1792694141-1MNrdT`; the final stronger
run is `final-stronger/run-1792694212-qEvYH5`, under the audit directory above.
Earlier successful runs are retained too. The final scan was repeated after test
implementation stabilized because Bun's recipe includes discovery source bytes.
[Machine-readable evidence](javascript-comparisons-results.json) includes exact
identities, outcomes, timings and preservation hashes: 63 tracked original files
in both copies, 37 final-original report files and 37 earlier-original report files.
No original report or input was rewritten.

## Reproduction and validation

Use Node 24.22.0 and Bun 1.3.11 executables directly on PATH. Set
`MUTIE_VITEST_FIXTURE` to a prepared audited installation (this audit used the npm
fixture at `/tmp/mutie-package-managers-audit/npm`, Vitest 4.1.9 / Vite 8.1.3).
Mutie does not install dependencies.

```sh
cargo test --bin mutie javascript:: -- --include-ignored
cargo test --test javascript_comparisons -- --ignored --test-threads=1
cargo test --test bun_preview --test vitest_preview
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

Final default suite: **309 passed, 0 failed, 113 ignored**. The six read-only
preview controls pass; formatting, diff whitespace and Clippy with warnings denied
also pass. Native adapter tests remain opt-in so self-mutation cannot recurse.

Public selection:

```sh
mutie /path/to/unchanged/pp-is --lang bun --ui plain \
  --include src/main/isIPv4.js \
  --operator comparison_invert,comparison_boundary \
  --timeout 20 --baseline-timeout 30 --output-dir /outside/original-reports
mutie /path/to/separate/stronger-copy --lang bun --ui plain \
  --rerun-missed /outside/original-reports/RUN \
  --timeout 20 --baseline-timeout 30 --output-dir /outside/stronger-reports
```

This acceptance is macOS evidence within the existing pinned serial scopes.
Broader runtime/version/platform acceptance, additional operators and project
layouts remain on the release roadmap. No lifecycle/protocol classifier changed.
