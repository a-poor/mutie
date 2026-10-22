# Scoped JS/TS arithmetic and remainder mutations

Acceptance on 2026-10-22 extends the shared JS/TS discovery used by Vitest,
Node and Bun. Existing layout/runtime/installation/lifecycle limits remain.
No package, report-schema or operator-catalog version changes.

## Contract

`arithmetic_replace` covers binary `+ ↔ -`, `* ↔ /` and `% → *`. JavaScript `%`
is a remainder operation; the README's modulo row uses the existing catalog name.
Each mutation replaces the complete parsed binary expression with parenthesized
operands and a parenthesized result, retaining inter-operand comments/whitespace.
Nested mutations remain independent. The shared renderer preserves existing
logical-swap replacement bytes.

Native JavaScript arithmetic applies, including string concatenation, changed
conversion hints, signed zero, NaN/Infinity and BigInt. For example, `- → +` may
concatenate numeric strings instead of subtracting, and `* → /` with a zero
right operand can produce Number Infinity/NaN or throw for BigInt. Mixed Number/
BigInt arithmetic can throw. We do not infer types, skip these candidates, wrap
exceptions or change their classification. Applicable semantics are specified by
ECMAScript's [arithmetic evaluation](https://tc39.es/ecma262/2022/multipage/ecmascript-language-expressions.html#sec-applystringornumericbinaryoperator)
and [BigInt operations](https://tc39.es/ecma262/2023/#sec-bigint-objects).

Unary signs, increment/decrement, augmented assignments, exponentiation, shifts,
types, strings, comments and regular expressions are excluded. Runtime expressions
inside template substitutions remain eligible. Source spans/fingerprints always
refer to original bytes. Saved readers reconstruct the exact parsed replacement;
raw ungrouped or arbitrary replacements are rejected before focused rediscovery.
Bun's existing recipe hashes discovery source, so changed recipes require a fresh
scan. Node/Vitest execution and attribution recipes are unchanged.

## Semantic and scored controls

`src/javascript/arithmetic_tests.rs` covers all five substitutions with exact
source spans, Unicode/CRLF, comments, nested arithmetic, type-only exclusions and
invalid-recipe rejection. Its ignored native probe runs each substitution on Node
and Bun, in both syntaxes: **20 bounded process cases**, each containing multiple
independent expected-value assertions. It checks:

- Number and BigInt results, numeric string coercion/concatenation, NaN/Infinity,
  signed zero, zero/zero and negative BigInt truncation.
- BigInt zero division and mixed numeric-type/Symbol exceptions.
- Left/right operand evaluation exactly once, followed by native left/right
  conversion with the expected `default` or `number` hint.
- Exact throwing-coercion identity, right-operand evaluation before left conversion,
  and a throwing left expression preventing right evaluation.

Each process has a ten-second deadline and 64 KiB capture budget. All three
arithmetic unit/native tests pass in 0.82 seconds after adding signed-zero and
negative-BigInt cases. The full shared JS suite also passed all 17 tests, including
native comparison/logical controls, before those additional arithmetic assertions.

`tests/javascript_arithmetic.rs` passes all **six runner/syntax workflows** in
141.83 seconds. Each full fixture has eight candidates: five arithmetic kills,
two arithmetic/remainder survivors and one Boolean kill. Filtering to
`arithmetic_replace` retains all seven arithmetic candidates. Stronger body tests
kill both survivors. A corrupted ungrouped replacement is rejected. A separate
mutation-triggered setup failure produces an error and zero kills. Original source
and saved report bytes stay unchanged through these checks.

These controls retain each adapter's existing body-exception provenance and
lifecycle boundaries. They do not assert that every native exception must be an
execution error: supported body failures follow the adapter's existing classifier,
while setup/lifecycle failures cannot earn kills.

## Unchanged Bragkit finding

Project: [Bragkit](https://github.com/artmsilva/bragkit), commit
`458d31127f1d3a6f3fd28afd2d7a4741ebc648a7`, original clean checkout
`/tmp/mutie-node-ts-bragkit`. All 57 tracked files were checked byte-for-byte
against that Git commit, including source, tests, scripts, manifest and lockfile.
No source, test, metadata or layout adaptation is made. Its original `node --test`
script uses native default discovery with pinned Node 24.22.0; the complete
61-test baseline and mutation suite run unchanged.

Selecting `src/reports/impact.ts` and `arithmetic_replace` discovers four candidates:

| Original | Replacement | Original outcome |
| --- | --- | --- |
| `num(md.additions) / 100` | multiplication | Survived |
| `num(md.reactionCount) * 5` | division | Killed |
| `num(md.storyPoints) * 2` | division | Survived |
| `impactScore(b) - impactScore(a)` | addition | Killed |

The completed selected run takes 6.51 seconds: two kills, two survivors, no errors
or timeouts. Its 50% score is a filtered four-candidate score, not a full-project
score. No remainder site is present in this selection; remainder acceptance comes
from native and scored fixture controls.

Existing tests check relative ranking, a code-volume cap and a reaction lower
bound. They miss the exact uncapped additions weight and the story-point multiplier.
A separate copy adds [one exact-score test](fixtures/javascript-arithmetic/bragkit-weights.test.ts):
a merged PR with 100 additions scores 5, and one with three story points scores 10,
using the original base/type contributions and documented implementation weights.
The unchanged implementation passes the new assertions. The 62-test pristine
baseline passes; a focused rerun kills both original survivors in 4.81 seconds.
Production source is unchanged. The original 50% score remains unchanged; the
focused 2/2 result is not a full stronger-suite score.

Original report:
`/tmp/mutie-js-arithmetic-audit/original-reports/run-1792697053-AlWWOg`.
Stronger report:
`/tmp/mutie-js-arithmetic-audit/stronger-reports/run-1792697111-Q9D6bo`.
[Machine-readable evidence](javascript-arithmetic-results.json) retains identities,
outcomes, timing and preservation hashes. All **57 original input files** remain
unchanged in both copies, and all **29 original report files** retain their hashes.
The original Git checkout remains clean; only the stronger copy has an added test.

## Validation and reproduction

The default suite passes **313 tests, zero failures, 127 ignored**. Formatting and
`cargo clippy --all-targets -- -D warnings` also pass. Native tests
remain opt-in to avoid recursive self-mutation. Use the direct pinned Node 24.22.0
and Bun 1.3.11 executable directories on PATH. `MUTIE_VITEST_FIXTURE` points to the
prepared npm installation at `/tmp/mutie-package-managers-audit/npm`
(Vitest 4.1.9 / Vite 8.1.3). No installer is invoked by Mutie.

```sh
cargo test --bin mutie javascript:: -- --include-ignored
cargo test --bin mutie javascript::arithmetic_tests -- --include-ignored
cargo test --test javascript_arithmetic -- --ignored --test-threads=1
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

```sh
mutie /path/to/unchanged/bragkit --lang node --ui plain \
  --include src/reports/impact.ts --operator arithmetic_replace \
  --timeout 20 --baseline-timeout 30 --output-dir /outside/original-reports
mutie /path/to/separate/stronger-copy --lang node --ui plain \
  --rerun-missed /outside/original-reports/RUN \
  --timeout 20 --baseline-timeout 30 --output-dir /outside/stronger-reports
```

This is macOS evidence within existing pinned serial scopes. The deferred host
startup/readiness issue is not resolved by these passing runs. No deadlines,
assertions, security settings or classifiers were relaxed. JS/TS expression rows
are now covered; Rust/Python expression gaps, control-flow/removal, literal
mutations and public release qualification remain on the roadmap.
