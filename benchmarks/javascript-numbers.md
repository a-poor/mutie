# JS/TS numeric literals

`number_replace` adds one candidate per eligible runtime Number/BigInt literal
across Node, Bun and Vitest, independently for JavaScript and TypeScript. Zero
becomes one; nonzero becomes zero. The recipe deliberately avoids a replacement
cross-product and preserves Number/BigInt: `0 → (1)`, `7n → (0n)`.
This completes the first slice of the numeric/string milestone. Numeric literals
in other languages and string literals remain open.

## Scope and semantics

The [ECMAScript numeric grammar](https://tc39.es/ecma262/multipage/ecmascript-language-lexical-grammar.html#sec-numeric-literals)
informed the selector; paired runs on the pinned runtimes check behavior.
Decimal, radix, separator, fraction, exponent and BigInt forms are eligible.
A directly attached unary sign and its literal form one candidate, including
intervening comments. Direct unary plus on BigInt is excluded because removing
it could erase an existing runtime exception. Parentheses around replacements
retain grouping and allow numeric member syntax such as `7..toString()`.

Decimal Number parsing identifies underflow to zero, including the rounding
boundary at half the smallest subnormal. Signed zero becomes positive one;
overflow to Infinity is nonzero and becomes zero. Integer radix/BigInt zero
detection uses digits without truncating large values. These are literal value
changes, not inferred values of variables or expressions. There is no TypeScript
type checking or failure reclassification.

Types/enums, module syntax, declaration keys (including computed names), legacy
leading-zero forms and tests are excluded. Runtime subscript indexes and property
values remain eligible. Numeric text inside strings, templates and regexes does
not create candidates; runtime expressions inside template substitutions can.
The original literal bytes and span remain the source identity. Saved readers
reconstruct the exact operator/category/replacement recipe; fresh discovery
rejects valid numeric snippets forged into excluded property-key contexts.
Bun's execution fingerprint includes the selector module, requiring fresh Bun
reports after this recipe change. Package/report/catalog versions are unchanged.

## Controls and CI

Native Node 24.22.0 and Bun 1.3.11+af24e281e checks run **33 original/mutant
expression pairs per syntax and runtime**: 132 pairs, 264 executions. Cases cover
signed zero, signs with comments, bases, separators, fractions/exponents,
underflow/overflow, the half-subnormal rounding boundary, precision beyond 2^53,
large BigInts and Number/BigInt member syntax. Each side checks `Object.is` and
the result type. Ten-second deadlines and 64 KiB capture bounds are unchanged.

Six independent scored workflows cover Node/Bun/Vitest × JS/TS. The fixture has
three numeric candidates (two kills, one BigInt survivor), with an excluded
numeric property key. They check the raw 66.67% score and minimum-score exit 3,
review identity, a stronger-test kill, corrupt operator/category/replacement
rejection, forged property-key contexts, source drift and report preservation.
Separate setup failures remain errors with no score. A three-second deliberate
hang remains a timeout, excluded from the scored denominator. Vitest uses its
pinned 4.1.9 / Vite 8.3.4 installation. Linux CI now includes each runner's JS/TS
numeric workflows and Node/Bun native controls; remote execution awaits a push.

## Unchanged-project findings

The unchanged [OpenClaw checkout](https://github.com/yukiharukonishi/openclaw-cc)
at `e323faa446f03b5438fa9edc58781b4ef8dd52df` retains all 63 tracked files and
the complete 62-test suite. No manifest, layout, source or test adaptation was
needed. Config and reports live outside the checkout. This revisits an existing
validation project and does not establish new project compatibility.

All five numeric candidates in `src/session-manager.js` were attempted:

| Original ID | Location | Change | Original result | Stronger-test observation |
| --- | --- | --- | --- | --- |
| 1 | line 48 | `86_400_000 → (0)` | Survived | A recently used session must remain reusable under the default idle window. |
| 2 | line 126, seconds argument | `0 → (1)` | Survived | Reset must happen at the exact daily minute, without one extra second. |
| 3 | line 126, milliseconds argument | `0 → (1)` | Survived | Reset must happen at the exact daily minute, without one extra millisecond. |
| 4 | line 129 | `1 → (0)` | Killed | Existing tests detect failure to advance the reset date. |
| 5 | line 214 | `0 → (1)` | Survived | A newly created session must report zero consumed tokens. |

The original run completes in **23.99 seconds: one kill, four survivors, no errors,
timeouts or build failures**. Its **20% score covers this filtered file/operator
selection**, not the whole project. No admitted candidate in this selection was
discarded or left unresolved; all four survivors have observable stronger-test
distinctions. This small selection does not establish a general candidate-noise
rate or justify additional replacement variants.

Three [retained tests](fixtures/javascript-numbers/session-numbers.test.js) in
a separate copy check the public `resolve`/`listSessions` behavior. They restore
the clock and remove their owned temporary directories. A controlled local date
keeps the exact reset-boundary check independent of timezone offset. The stronger
baseline passes **65 tests**; its focused rerun kills all four selected survivors
in **26.24 seconds**, without errors or timeouts. The focused 100% result does not
replace the original score or establish a full stronger-project score.

The [verifier](fixtures/javascript-numbers/verify.py) checks the clean pinned
checkout, all 63 original and stronger-copy input hashes, the sole extra test,
all 33 original report files, baseline counts, exact source identities across
report-local IDs and unchanged Node execution identity. The
[results ledger](javascript-numbers-results.json) retains relative paths and hashes.

```sh
python3 benchmarks/fixtures/javascript-numbers/verify.py \
  /tmp/mutie-js-numbers-audit benchmarks/javascript-numbers-results.json
```

## Validation

- Default suite: **356 passed, 197 opt-in tests ignored**, with two test threads.
- Both native numeric tests pass on pinned Node/Bun, independently for JS/TS.
- All six numeric workflows pass, including deliberate setup-error/timeout cases.
- All **18 existing Node/Bun/Vitest workflows** pass, retaining their original
  report, review, lifecycle, resource-limit and shutdown checks.
- Rustfmt, Clippy with warnings denied, Actionlint and whitespace checks pass.

Campaign artifacts and the frozen CLI are under `/tmp/mutie-js-numbers-audit`;
local validation logs use `/tmp/mutie-js-numbers-*`. The deferred macOS pre-main
startup investigation remains open. No classifier, lifecycle or timeout boundary
was relaxed for this slice.
