# Rust numeric literals

Validated on 2026-11-09 with Rust/Cargo 1.99.1 on macOS, based on `1cf04b7`.
[Results and hashes](rust-numbers-results.json) retain the original inventory,
outcomes, source identities and stronger-test evidence. No project, report-schema
or operator-catalog version changes are needed.

## Scope and recipe

`number_replace` emits one candidate per eligible runtime integer/float literal:
zero becomes one, nonzero becomes zero. Explicit integer and `f32`/`f64` suffixes
stay attached; integer/float categories stay distinct (`7u8 → 0u8`,
`0.0f32 → 1.0f32`). Radix digits and underscores are accepted without truncating
large integers. Hexadecimal digits ending in `f32`/`f64` are not float suffixes.
See the Rust Reference's [literal expressions](https://doc.rust-lang.org/reference/expressions/literal-expr.html)
and [numeric tokens](https://doc.rust-lang.org/reference/tokens.html#literals).

A directly attached negative sign owns the literal, including intervening
comments. Its child is not a second candidate; negative zero becomes positive
one. Parenthesized operands retain the outer sign and mutate only the inner
eligible literal. Replacements are atomic literals without extra parentheses,
preserving method-call syntax without introducing redundant-parenthesis warnings.

Explicit float suffixes select the rounding width when deciding whether a value
is zero. Unsuffixed floats are eligible only when f32 and f64 agree on zero/nonzero:
`1e-100` is excluded, while `1e-100f32 → 1.0f32` and
`1e-100f64 → 0.0f64` are separate valid recipes. Discovery does not infer types or
repair invalid programs; compiler diagnostics and execution classification remain
authoritative. This is a syntactic literal operator, not an equivalence proof
for later casts, wrapping arithmetic or custom surrounding behavior.

Existing test/cfg/generated/macro exclusions remain. Constants/statics, types,
generic arguments, patterns, array repeat lengths and tuple field selectors are
excluded; runtime array elements/indexes, match guards and closures are eligible.
The saved reader reparses the exact original and regenerates its replacement.
Fresh discovery rejects forged test-only spans and stale source identities.
Cargo target/manifest/lockfile/toolchain validation is unchanged.

## Native and scored controls

Static tests check UTF-8/CRLF spans, comments, exact edits, exclusions, suffixes,
ambiguous float widths, injected snippets and corrupted recipes. **41 paired
native cases** compile/run both versions with warnings denied, checking:

- Integer width boundaries through u128/i128, signed minima, radices and separators.
- Signed zero, direct versus parenthesized signs, and numeric method-call syntax.
- f32/f64 subnormal rounding boundaries, explicit underflow and f64 precision loss.
- Unsuffixed underflow that is zero at both inferred widths.

Two additional original/mutant compiler controls preserve constant division by
zero and out-of-bounds indexing as build failures. An inferred-width control
compiles the same excluded `1e-100` literal as f32 and f64, verifying the differing
zero decisions. Each native command has a 30-second deadline and 64 KiB bound.

The independent [Cargo fixture](../tests/fixtures/rust-numbers/README.md) has four
candidates: **one kill, one survivor, two build failures**. Fresh serial and
two-worker cached results agree; the raw score stays **50%**. Minimum-score
failure exits 3. Dismissal review preserves raw outcomes/score; a separate stronger
copy kills the float survivor and preserves its source-ID/review mapping.
Corrupted recipes/operators/extensions, forged cfg-test spans and stale sources
are rejected. Input files, caller artifacts and original saved reports stay intact.
Separate native workflows keep genuine timeouts and capture overflow outside
kills, preserving failed-report scores and error precedence.

The original binary fixture now explicitly selects its three Boolean sites in
`mutie.toml`. Unfiltered discovery independently checks all five sites, including
the two new numeric literals in `src/main.rs`; the invocation helper does not
silently filter the new operator.

## Unchanged UCD Parse findings

The public [UCD Generate workspace](https://github.com/BurntSushi/ucd-generate/tree/e8fa937f0cac643669dcaf5edac2785b15cab917)
at `e8fa937f0cac643669dcaf5edac2785b15cab917` is a fresh unchanged checkout of an
existing validation project. External config selects `ucd-parse` and
`number_replace`, without source-file filters. All four workspace members remain
in the test scope; the **140-test baseline passes**.

All **39 candidates** run in **257.27 seconds**: **26 kills, 12 survivors, one
timeout**, no build failures or errors. The **68.42%** score covers this selected
package/operator only and excludes the timeout. This does not establish a general
invalid-candidate rate or a workspace-wide score.

Four tests in a separate copy exercise public behavior:

| Original IDs | Mutation | Stronger assertion |
| --- | --- | --- |
| 2, 3, 4 | Version regex group indexes become zero | Header `PropList-15.1.2.txt` parses as `(15, 1, 2)`. |
| 5, 6 | Physical-line initial value/increment changes | Malformed input after a comment and blank line reports line 4. |
| 7 (timeout) | EOF check `n == 0` becomes `n == 1` | A leading blank line does not discard the following valid row. |
| 10 | Codepoint iterator increment becomes zero | Bounded consumption of a single codepoint yields exactly one value. |

The iterator test uses `take(2)`, so repeating values fail an assertion without
waiting indefinitely. The blank-line assertion would expose the changed EOF
decision, but Cargo never reaches this integration test: the preceding library test binary hangs. The
retained stdout/stderr confirms successful compilation and incomplete library
tests. The mutation remains a timeout; no timeout/classifier was relaxed.

The stronger **144-test baseline passes**, and the focused rerun kills all six
selected survivors in **72.11 seconds**. The selected prior timeout still times out.
The original score/report stays unchanged; the focused 100% excludes that timeout
and is not a full stronger-suite score. Six original survivors remain open (IDs 1, 26, 30, 34, 35,
37). These include the version-header error-line convention, mirroring, case
mapping indexes, decomposition length and the initial empty expander range.
No equivalence or dismissal is claimed. In particular, preserving a zero-based
header location versus using one needs a contract decision, not an assertion
copied blindly from the implementation.

The retained [tests](fixtures/rust-numbers-ucd/numeric_contracts.rs) and
[verifier](fixtures/rust-numbers-ucd/verify.py) check all **108 original paths**
(including symlinks) in both copies, the sole added test, source/manifest/lockfile
identity and full-workspace test counts. All **172** original report files
retain their hashes. Raw evidence stays outside the repository in the temporary
`mutie-rust-numbers-audit` directory. Timings were measured during other local
validation and are not a speed comparison.

## Verification and reproduction

The default suite passes **359 tests**, zero failures, with **204 opt-in ignores**.
Native numeric and compiler controls, both scored numeric workflows, formatting,
all-target Clippy with warnings denied, workflow lint and whitespace checks pass.
The full Rust workflow run completed 29 tests successfully and exposed three
legacy expectations: metadata operations bypassing malformed config, implicit
language selection in a mixed-project wrapper, and the old signal-failure label.
The tests now use grouped-config/public-CLI selection and require invalid signal
evidence to remain unscored. Focused rechecks of those three workflows pass.
Together the checks cover all 32 Rust workflows without changing production
execution/classification behavior.
The Linux Rust job now includes the numeric native and scored workflows. Remote
CI for this slice remains pending until push. The previously deferred
macOS pre-main startup investigation remains open.

```sh
cargo test --locked --bin mutie rust::discovery::number_tests -- --include-ignored
cargo test --locked --test rust_workflow numbers:: -- --ignored --test-threads=1
```

Place this config beside an unchanged checkout named `original`:

```toml
jobs = 2
timeout = 30
baseline_timeout = 180
output_dir = "original-reports"
[rust]
path = "original"
packages = ["ucd-parse"]
operators = ["number_replace"]
build_cache = "worker"
```

```sh
mutie list --config original.toml --json
mutie run --config original.toml --plain
# Copy to stronger/ and add numeric_contracts.rs under ucd-parse/tests/.
# Use path="stronger", output_dir="stronger-reports" in stronger.toml.
mutie rerun "$ORIGINAL_REPORT" --id 2,3,4,5,6,7,10 --config stronger.toml --plain
python3 -B benchmarks/fixtures/rust-numbers-ucd/verify.py "$AUDIT_ROOT" results.json
```

The verifier expects this campaign's artifact layout and preservation manifests
recorded before the stronger rerun. Go numeric literals, broader recipes and
string-literal mutations remain separate roadmap slices.
