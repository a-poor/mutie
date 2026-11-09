# Python string literals

Python `string_replace` now changes an ordinary runtime `str` literal from
nonempty to empty or empty to `"x"`: `'value' → ("")`, `'' → ("x")`.
Each eligible AST string value produces one candidate. This extends the existing
JS/TS operator; Go/Rust strings and broader replacement recipes remain separate.

## Scope and native semantics

The pinned CPython 3.14.7 AST decodes ordinary quoted, raw, Unicode-prefixed and
triple-quoted literals without importing or executing the project. Adjacent
literals form one combined span, including intervening comments and line
continuations. See Python's [literal and concatenation rules](https://docs.python.org/3.14/reference/lexical_analysis.html#string-literal-concatenation)
and [AST positions](https://docs.python.org/3.14/library/ast.html#ast.AST).

Decoded value determines emptiness. Nonraw physical line continuations emit
nothing; raw continuations retain characters. Escaped NULs, octal/hex/Unicode
escapes, named Unicode characters and lone surrogates are nonempty. Parenthesized
replacement literals preserve grouping and member access. The original bytes
remain the source fingerprint and mutation span.

Standalone string statements, including module/class/function/additional/attribute
docstrings, are excluded. Annotations, type parameters/aliases, match patterns,
bytes, f-strings and template strings are excluded too, including string literals
inside formatted/template expressions. Runtime mapping keys, lookups, defaults,
arguments and match guards remain eligible. This does not change other operators'
eligibility inside f-string expressions. Tests remain outside discovery.

`python/string.rs` independently checks lexical forms, concatenation trivia,
emptiness and exact replacement recipes. It does not evaluate Python or duplicate
the Unicode name database: native parsing resolves named escapes, and fresh AST
discovery validates source context. Saved readers reject incorrect/no-op recipes
and fresh reruns reject forged annotation spans. The reader joins Python's byte-based
execution recipe; versions and report schemas remain unchanged.

The native controls execute **46 cases under LF and CRLF: 92 original/mutant
pairs**, checking exact `str` type and value. Both 46-site inventories also pass
through CLI discovery and the Rust reader. They cover quoting/prefixes, Unicode,
surrogates, escaping, raw/multiline strings, implicit concatenation, comments,
continuations and member syntax. Static controls verify exclusions and original
bytes. Malformed Unicode escapes and byte/string concatenations remain native
errors; named Unicode sequences are not valid `\N{...}` string escapes.

CR-only physical line endings exposed the existing AST/source coordinate mismatch.
They remain outside the supported source-coordinate scope, with a CLI rejection
control. No coordinate check or parser-warning rejection was relaxed; LF and CRLF
are the accepted line endings for this slice.

Three opt-in workflows cover native discovery; a scored kill/survivor with 50% raw
score, minimum-score exit 3, dismissal policy, stronger-test reruns, corrupt
recipes/operators, forged contexts and source drift; and runtime/setup errors plus
a bounded timeout. Errors and timeouts never become kills or passing scores.
Linux CI includes all three in its Python operator partition.

## Unchanged Scrubify audit

The [Scrubify checkout](https://github.com/MartiAble/scrubify-py) at
`558265805d942c03d2e6d8522baa0ca2992ec067` retains all **nine tracked inputs**,
including the original manifest, `src/` layout and complete **six-test suite**.
No adaptation, dependency installation or suite trimming was needed. This
revisits an existing validated project; it adds no runtime or layout support.

All **34 package string candidates** were attempted using the same frozen CLI,
pinned interpreter and complete original suite:

| Run | Attempted | Killed | Survived | Errors | Seconds | Saved score |
| --- | ---: | ---: | ---: | ---: | ---: | --- |
| Original | 32 | 7 | 24 | 1 | 127.09 | Unavailable; failed with two pending |
| Pending selection | 2 | 1 | 1 | 0 | 12.93 | 50%, rerun selection only |

Combined accounting is **eight kills, 25 survivors and one error**, with no
timeouts or build failures. It is not a full score. Emptying the default `"***"`
mask produces assertion failures plus `IndexError` in length-preserving redaction;
the conservative mixed-failure classification remains an error. Its failed report
stays unscored. The total saved original-run duration is **140.02 seconds**,
including repeated baselines.

## Stronger public-API tests

The [retained test module](fixtures/python-strings/test_redaction_defaults.py)
adds **26 cases** in a separate complete copy: 19 sensitive key names, six header
names and one hyphenated key. It checks redacted values, preserved public fields
and unchanged caller input through `redact` and `redact_headers`.

The stronger pristine baseline passes **32 tests**. A focused rerun kills all
**19 implementation survivors** in **79.05 seconds**:

- Fourteen uncovered default key patterns: passwd, secret, token-prefix, apikey,
  auth/session variants, cookie, credential, private-key and access-key names.
- Four uncovered headers: proxy authorization, API key, auth token and cookie.
- Hyphen-to-underscore normalization, observed through an uppercase private-key
  name that otherwise exposes its value.

The six survivors in package exports and version metadata remain open. They are
not declared equivalent or dismissed. The mask error was not selected or
reclassified. Focused 100% does not establish a full stronger-suite score and
does not replace any original score.

## Preservation and validation

The [verifier](fixtures/python-strings/verify.py) checks all original and stronger
input hashes, the sole added test module, clean pinned Git state, all **170
original report files**, baseline counts, execution recipes/runtime identity and
exact source identities across report-local IDs. Final-CLI discovery matches the
original 34-site inventory. The [results ledger](python-strings-results.json)
contains relative paths and hashes; reports were not rewritten.

```sh
python3 benchmarks/fixtures/python-strings/verify.py \
  /tmp/mutie-python-strings-audit benchmarks/python-strings-results.json
```

- Default suite: **365 passed, 219 opt-in tests ignored**.
- The string Rust recipe unit test, three native/scored string workflows and
  **81 static Python scope controls** pass on CPython 3.14.7 / pytest 9.0.2.
- All **46 existing Python workflows** pass across the full run and targeted
  rechecks. The root-layout review fixture's version string became a new survivor;
  its shared Boolean review contract now explicitly selects `boolean_flip`.
  All five affected layout/review workflows pass with that selection.
- Rustfmt, Clippy with warnings denied, Actionlint and whitespace checks pass.

Artifacts and the frozen CLI are under `/tmp/mutie-python-strings-audit`; logs use
`/tmp/mutie-python-strings-*`. Remote Linux CI validation still requires a push.
The separate macOS startup investigation remains deferred. No classification,
lifetime check, resource limit or deadline was relaxed.
