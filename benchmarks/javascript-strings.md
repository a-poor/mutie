# JS/TS string literals

Validated on 2026-11-09 from `f7e3968`, using Node 24.22.0, Bun
1.3.11+af24e281e and Vitest 4.1.9 / Vite 8.3.4 on macOS arm64.
[Results and hashes](javascript-strings-results.json) retain the unchanged-project
inventory, original outcomes and separate stronger-test evidence.
Project, report-schema and operator-catalog versions are unchanged.

## Scope and recipe

`string_replace` emits one candidate per eligible ordinary quoted runtime string:
nonempty becomes empty, empty becomes `"x"`. Parenthesized replacements preserve
expression/member syntax: `'text' → ("")`, `'' → ("x")`. Original quoting,
escaping, byte ranges and source hashes remain the saved identity.

Emptiness follows the string value, not the number of source characters. A line
continuation contributes no UTF-16 code units, whereas NULs and Unicode escapes
do. The scanner validates strict-mode escape forms without evaluating source or
converting lone surrogates into Rust strings. See ECMAScript's
[string value rules](https://tc39.es/ecma262/multipage/ecmascript-language-lexical-grammar.html#sec-string-literals).
Native controls independently verify these decisions.

The first slice excludes:

- All standalone string statements, including directive prologues and
  parenthesized/non-prologue statements. The broader exclusion avoids mutating
  inert statements; [directives have distinct syntax semantics](https://tc39.es/ecma262/multipage/ecmascript-language-source-code.html#sec-directive-prologues-and-the-use-strict-directive).
- Imports, direct dynamic-import arguments, re-export paths/names and import
  attributes; declaration keys, including entire computed-key expressions.
- Type-only strings/enums, template literals and their substitutions, tests and
  generated files. Existing unsupported JSX/decorator/ambient rules remain.
- Legacy octal/decimal escapes and malformed escapes. The pinned grammars still
  reject export attributes; this slice does not broaden parser/runtime scope.

Ordinary values, defaults, call arguments, runtime property lookups, switch case
values, class field values and exported values are eligible. There is no type
inference or promise that every changed value is observable in a given context.

Saved readers reparse the exact snippet and regenerate the replacement. Fresh
full-source discovery must also admit its context: a valid literal recipe forged
into a declaration key is rejected. Bun's recipe fingerprint includes
`javascript/string.rs`, requiring fresh Bun reports after this change. Explicit
string selection is rejected for Go, Rust and Python; their discovery is unchanged.
All 13 JS/TS operators run by default, with config filters for narrower selections.

## Native and scored controls

Static tests verify UTF-8/CRLF positions, exact edits/restoration, directives,
module syntax, declaration/computed keys, type strings, generated/test exclusions,
source NUL handling, line continuations and strict saved recipes.

**30 paired native cases per runtime and syntax** pass on Node/Bun × JS/TS:
**120 pairs, 240 executions**. They cover both quote styles, raw/escaped NULs,
control characters, identity escapes, quote/backslash escaping, astral characters,
lone high/low surrogates, the maximum Unicode code point, LF/CR/CRLF/LS/PS
continuations, mixed content, raw LS/PS, and string member access. Each side checks
`Object.is` and the resulting type. Commands retain ten-second deadlines and
64 KiB capture bounds.

All **six scored workflows** pass independently for Node, Bun and Vitest, each
with JS and TS. Three candidates yield **two kills, one survivor** and a raw
**66.67%** score. Minimum-score failure exits 3. A stronger value assertion kills
the empty-string survivor while preserving its dismissal mapping. Disposable
reports exercise corrupt replacement/operator rejection and forged key spans;
source drift is rejected. Original source/report bytes remain unchanged.
Separate setup failures remain unscored errors, taking precedence over score
policy; a three-second deliberate hang remains a timeout, excluded from kills.

## Unchanged OpenClaw findings

The unchanged [OpenClaw checkout](https://github.com/yukiharukonishi/openclaw-cc/tree/e323faa446f03b5438fa9edc58781b4ef8dd52df)
retains all **63 tracked files** and its full **62-test baseline**. External
configuration selects `src/utils/session-key.js` and `string_replace`; no
manifest, layout, source or test adaptation is needed. This revisits an existing
validation project, not a new compatibility claim.

All **13 selected candidates** run in **66.06 seconds**: **two kills, 11 survivors,
zero errors/timeouts/build failures**. Existing tests kill the builder's `"dm"`
and `"group"` branch strings. The original **15.38% score covers this filtered
file/operator selection**, not the whole project.

Five [new public-API tests](fixtures/javascript-strings/session-keys.test.js) in
a separate copy exercise the remaining behavior:

| Original IDs | Behavior checked |
| --- | --- |
| 1 | Missing routing fields produce an actionable diagnostic naming the required fields. |
| 4–7 | Cron parsing recognizes the prefix/type and retains colons inside a job ID. |
| 8–10 | Direct-message parsing retains type, agent, channel and peer fields. |
| 11–12 | Group parsing retains the group and topic fields. |
| 13 | An unknown key retains both its classification and original text. |

The stronger **67-test baseline passes**. A focused rerun kills all **11 original
survivors in 46.97 seconds**, with no errors or timeouts. The diagnostic assertion
is an explicit test decision about useful error messages; the routing tests check
structured public results. All selected candidates now have observable
stronger-test distinctions. This small sample does not establish a general noise
rate or justify broader string/template transformations. The focused 100% is not
a full stronger-suite score, and the original 15.38% is preserved.

The [verifier](fixtures/javascript-strings/verify.py) checks the pinned clean
checkout, all 63 input hashes in both copies, the sole extra test, exact source
identities across report-local IDs, baseline counts, unchanged Node execution
identity and all **65 original report hashes**. The frozen CLI and raw evidence
remain in the temporary `mutie-js-strings-audit` directory. Timings were recorded
during other local validation and are not a speed comparison.

## Verification and reproduction

All **364 default tests** pass across the final unit run and full integration run,
with **216 opt-in ignores**. The six new string workflows, both native semantic
tests and all **18 existing Node/Bun/Vitest workflows** pass. Rustfmt, Clippy with
warnings denied, Actionlint and whitespace checks pass. Default discovery tests
now account for string sites, including literal NULs; the invocation helper does
not hide them with a global operator filter.

Linux CI includes the new native tests and all six runner/syntax workflows.
Remote validation awaits a push. The deferred macOS pre-main startup issue stays
open; no classifier, evidence rule or deadline was relaxed.

```sh
cargo test --locked --bin mutie javascript::string_tests -- --include-ignored
MUTIE_VITEST_FIXTURE="$FIXTURE" cargo test --locked --test javascript_strings -- --ignored --test-threads=1
```

Put this config beside an unchanged checkout named `original`:

```toml
timeout = 15
baseline_timeout = 30
output_dir = "original-reports"
[js]
path = "original"
runner = "node"
operators = ["string_replace"]
include = ["src/utils/session-key.js"]
```

```sh
mutie list --config original.toml --json
mutie run --config original.toml --plain
# Copy original to stronger and add the retained session-keys.test.js under test/.
# In stronger.toml, change path and output_dir to the corresponding stronger paths.
mutie rerun "$ORIGINAL_REPORT" --missed --config stronger.toml --plain
python3 -B benchmarks/fixtures/javascript-strings/verify.py "$AUDIT_ROOT" results.json
```

The verifier expects this campaign's artifact layout and preservation manifests
captured before the stronger rerun. Python/Rust/Go strings, template literals and
broader replacement recipes remain separate roadmap slices.
