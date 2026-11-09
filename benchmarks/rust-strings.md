# Rust string literals

`string_replace` now supports ordinary and raw Rust runtime string literals.
Each eligible literal produces one change: a nonempty value becomes `""`, an
empty value becomes `"x"`. Both replacements retain `&str` semantics. Raw strings
use the same canonical quoted replacement; source identity uses the original
bytes, not a normalized spelling.

## Scope and semantic controls

`rust/string.rs` validates the complete token without evaluating source. It checks
quote/raw delimiters, Rust escapes, Unicode scalar values and decoded emptiness.
String-continuation escapes remove their following HT/LF/CR/space characters;
other whitespace, NULs, Unicode escapes and raw backslashes remain nonempty.
CRLF normalization does not change the source span. See the Rust Reference's
[literal grammar](https://doc.rust-lang.org/reference/tokens.html#string-literals)
and [continuation semantics](https://doc.rust-lang.org/reference/expressions/literal-expr.html#string-continuation-escapes).

Discovery retains the existing function-body boundary, including closures, match
guards, arguments and method receivers. Macros, attributes, constants/statics,
inline const blocks, types, patterns, array lengths and cfg/test/generated code
stay excluded. Characters, byte strings and C strings are separate types and
are not selected. No macro expansion, type inference or warning repair is added.

The pinned Tree-sitter Rust grammar still rejects valid Unicode escapes containing
underscores. A native control confirms Rust accepts `"\u{0_0_}"` while discovery
rejects that source; this existing parser limitation is not silently broadened.
Malformed scalar/ASCII escapes, unsupported prefixes/suffixes, token injection,
no-op recipes and raw delimiters above Rust's 255-hash limit remain rejected.

**32 cases under LF and CRLF give 64 native original/mutant pairs**, checking
values and `&'static str` types through real `rustc` compilation and execution.
They cover ordinary/raw emptiness, multiline strings, raw delimiters, actual and
escaped NULs, Unicode boundaries, continuations, retained non-Rust whitespace,
and method-call syntax. Seven additional grammar/compiler controls cover the
255/256-hash boundary, Unicode underscore compatibility and malformed escapes.
Commands retain a 30-second deadline and 64 KiB capture bound. Native continuation
warning cases compile with Rust's normal warning policy; production warnings
and user lint settings are unchanged.

Default discovery tests verify context exclusions, UTF-8/CRLF source coordinates,
exact writes, double-application rejection and saved recipes. Saved Rust string
records require a `.rs` source and the exact regenerated replacement. Fresh
discovery rejects forged cfg-test spans; stale source fingerprints reject reruns.
Rust's existing manifest/lockfile/target/toolchain checks remain in force. No
package, schema or catalog version was bumped.

## Scored Cargo workflows

The [independent fixture](../tests/fixtures/rust-strings/README.md) has an ordinary
string killed by its assertion and an empty raw-string survivor. Fresh serial
and two-worker cached runs agree at **one kill, one survivor, 50% raw score**.
Minimum-score failure exits 3. Dismissal policy passes without changing raw
outcomes or score. A separate stronger copy kills the survivor with both fresh
and worker-cached Cargo builds, retaining its source ID and dismissal mapping.

Corrupt recipes/operators/extensions, forged test-only contexts and source drift
are rejected. Original reports, project files, caller build artifacts and worker
cleanup are checked. Separate real Cargo probes keep a five-second timeout and
capture overflow outside kills and passing scores. Existing compiler-failure,
process/error and cancellation controls also pass; classification was unchanged.

## Unchanged UCD Parse audit

The [UCD Generate workspace](https://github.com/BurntSushi/ucd-generate/tree/e8fa937f0cac643669dcaf5edac2785b15cab917)
at `e8fa937f0cac643669dcaf5edac2785b15cab917` is an unchanged checkout of an
existing validation project. All **108 original paths**, including symlinks,
are retained. External config selects `ucd-parse` and `string_replace` without
source-file filters. The full four-member workspace remains the test scope;
the pristine **140-test baseline passes**.

All **112 candidates** complete in **586.39 seconds**: **48 kills, 64 survivors**,
with no timeouts, build failures or errors. The **42.86%** score applies to this
package/operator selection, not all workspace mutations. Timings include local
validation running concurrently and are not a performance comparison.

Four [retained tests](fixtures/rust-strings-ucd/string_contracts.rs) in a separate
complete copy check observable public behavior:

| Original IDs | Missing contract |
| --- | --- |
| 2–7 | All six joining-type labels round-trip through a parsed Arabic-shaping row. |
| 94–109 | All 16 decomposition tags round-trip together with their codepoint mapping. |
| 1 | Directory parsing reads `DerivedAge.txt` and returns its row. |
| 20 | Directory-version lookup reads the `PropList.txt` header. |

The stronger baseline passes **144 tests**. Its **131.53-second** focused rerun
kills all **24 selected survivors**, with no errors, timeouts or build failures. The focused
100% is not a full stronger-workspace score and does not replace the original
42.86%. The remaining **40 survivors stay open**: 36 file-selection paths and
four UnicodeData behaviors (First/Last markers, mirroring parsing and the empty
expander sentinel). No equivalence or dismissal is claimed.

## Preservation and verification

The [verifier](fixtures/rust-strings-ucd/verify.py) checks all 108 original paths
in both copies, the sole added test module, clean original Git state, execution
scope, source identity across report-local IDs and full-workspace test counts.
All **464 original report files** retain their hashes. Final-CLI discovery agrees
with the campaign inventory. The [results ledger](rust-strings-results.json)
records relative paths, timings, outcomes and hashes.

Both campaign runs use the same frozen CLI. The final source includes an
equivalent Clippy correction to byte slicing; final native/scored checks and
discovery validate the unchanged recipe behavior. Historical reports were not
rewritten.

- Default suite: **367 passed**, no failures.
- All **32 existing Rust workflows** and both new string workflows pass.
- Both native string suites, default discovery/recipe controls, Rustfmt, Clippy
  with warnings denied, Actionlint and whitespace checks pass.
- Linux CI includes native and scored string tests. Remote validation requires
  a push; the separately deferred macOS startup investigation remains open.

```sh
cargo test --locked --bin mutie rust::discovery::string_tests -- --include-ignored
cargo test --locked --test rust_workflow strings:: -- --ignored --test-threads=1
python3 -B benchmarks/fixtures/rust-strings-ucd/verify.py \
  /tmp/mutie-rust-strings-audit benchmarks/rust-strings-results.json
```

Artifacts and the frozen CLI are under `/tmp/mutie-rust-strings-audit`; logs use
`/tmp/mutie-rust-strings-*`. Go strings, templates and broader recipes remain
separate roadmap work.
