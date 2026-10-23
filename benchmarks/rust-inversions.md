# Scoped Rust comparison inversion and Not removal

Rust supports `comparison_invert` (`== ↔ !=`, `< ↔ >=`, `<= ↔ >`) and
`negation_remove` within its existing package/workspace scope. Together with
booleans, boundaries, logical swaps and arithmetic, this covers the Rust
expression rows in the README. No package, schema or operator-catalog version
changes; control-flow/removal and literal mutations remain separate work.

## Semantics and replacement contract

Comparisons replace only the parsed token. Native implicit borrowing and exact
`PartialEq`/`PartialOrd` dispatch remain intact. Inversion selects the corresponding
method, not a Boolean negation of the old result: unordered float values and
custom overrides can distinguish those operations. See the
[Rust comparison rules](https://doc.rust-lang.org/reference/expressions/operator-expr.html#comparison-operators).
Inversion and boundary edits share source spans; sorting/deduplication includes
operator identity so neither is lost. Existing logical edits retain outer-before-inner
order, and the older boundary workflow now explicitly selects its operator.

Rust `!` supports Boolean negation, integer bitwise inversion and overloaded
`Not`. All three are eligible; there is no static type inference. Removing the
operator also removes its overloaded call and effects. See the
[Rust negation rules](https://doc.rust-lang.org/reference/expressions/operator-expr.html#negation-operators).
A full unary expression `!operand` becomes `(operand,).0`, retaining trivia between
the bang and operand. Tuple construction ensures the operand is evaluated once
in value context; projecting its only field returns that value without an added
trait call, clone, cast or named binding. This matters for `&!x`: plain `&x` could
borrow the original place instead of moving/copying into a temporary. The tuple
recipe retains that value evaluation and avoids unnecessary-parentheses warnings
in the native `-D warnings` controls. Changed custom output types can still produce
build failures. Nested `!!x` yields two independent candidates.

Never types, macro bangs, unary minus and dereference syntax are not `Not` nodes.
Existing macro-token, cfg/test, generated, constant/type and array-repeat-length
exclusions remain. Saved readers require exact comparison pairs or reparse the
original unary expression and reconstruct the tuple recipe exactly. Source-identity
rediscovery remains mandatory for focused reruns. Compiler failures, test failures,
process errors and timeouts retain their existing classifications.

## Controls and scored fixture

`src/rust/inversion_tests.rs` checks exact pairs/recipes, comments, Unicode/CRLF,
nested edits, reparsing, unchanged bytes outside the selected span, deterministic
ordering and both operators at shared comparison spans. Saved-report controls
reject altered comparison/negation recipes and non-Rust paths.

Nine opt-in native compile/run cases pass in 1.97 seconds, using a 30-second and
64 KiB bound per command with compiler warnings denied:

- Six comparison cases cover numeric values, signed zero, NaN on either side,
  infinities, non-Copy generic operands retained after comparison, evaluation
  order and exact overloaded method dispatch. Deliberately independent custom
  methods ensure inversion is not implemented as `!old_comparison`.
- One case covers six disjoint negation edits: Boolean/integer values, a once-only
  closure, owned/borrowed custom Not, and operand drop timing beneath an enclosing
  borrow. The custom Not call is absent, as intended.
- Two cases independently remove each bang in a nested `!!x`.

These semantic controls apply disjoint edits together only where stated; scored
workflows always execute one mutation at a time.

`tests/fixtures/rust-inversions` is an independent library with four original
integration tests. Its workflow selects both operators, executes 11 candidates
with two worker caches, and records **eight kills, two survivors and one build
failure**, no errors/timeouts. A `Not<Output = bool>` input whose type is not bool
becomes a build failure when the call is removed. Stronger ordered-input and
observed-result assertions kill the two survivors in a focused rerun. The workflow
also rejects both corrupt recipe types and preserves inputs, report bytes, source
IDs and caller artifacts. The final run passes in 25.85 seconds. The prior boundary
workflow passes in 38.79 seconds with its original five outcomes and stronger rerun.

## Unchanged public project

The unchanged [UCD Generate](https://github.com/BurntSushi/ucd-generate) workspace,
commit `e8fa937f0cac643669dcaf5edac2785b15cab917`, provides the real-project evidence.
This selection targets `ucd-parse/src/unicode_data.rs` in member `ucd-parse`, unlike
the prior arithmetic goal's `ucd-util` selection. It revisits the same public
workspace, not a new-project compatibility claim.

All 108 tracked source/test/config/manifest/lock inputs match the commit. Every
candidate tests all four workspace members; the original baseline passes 140 tests
(3/84/40/13). Thirteen selected candidates complete in **81.29 seconds**:
**11 killed, two survived**, zero build failures/errors/timeouts. The 84.62% score
is filtered to this file and these operators, not a full-project score.

| Original ID | Mutation | Missing assertion |
| --- | --- | --- |
| 9 | Canonical mapping equality `==` → `!=` | A nontrivial canonical decomposition must be retained when serializing a UnicodeData record |
| 11 | Remove `!first` in decomposition formatting | Codepoint strings need exactly one separator between entries and no leading separator |

Original tests exercise parsing and range expansion; they do not catch these
formatting changes. The separate stronger copy adds only
[`decomposition_display.rs`](fixtures/rust-inversions-ucd/decomposition_display.rs).
One test parses the canonical decomposition of U+00C0, checks its serialized
`0041 0300` mapping, and reparses the record for equality. The other checks exact
single/multiple-codepoint and tagged formatting. The original implementation
passes the new assertions: the stronger baseline is **142 tests**.

A focused rerun kills both original survivors in **22.58 seconds**, without errors,
timeouts, classifier changes or weaker assertions. The focused 100% score does not
replace the original filtered score. All 108 tracked originals match in both copies,
all **68 original report files** retain their hashes, and the original checkout is
clean. The stronger test file is retained separately from original inputs.

[Machine-readable evidence](rust-inversions-results.json) includes source and
execution identities, hashes, outcomes, baseline counts and command settings.
Local reports are under `/tmp/mutie-rust-inversion-audit`:
`original-reports/run-1792713767-N9PQmS` and
`stronger-reports/run-1792713896-hLv8ZH`.

## Validation and reproduction

The final default suite passes **317 tests, zero failures, 132 ignored**. Formatting,
diff checks and all-target Clippy with warnings denied pass. Initial unit regressions
caught shared-span boundary duplication in an operator-specific assertion and a
changed nested-logical ordering; filtering the assertion and retaining the old
outer-first ordering resolved them. No existing semantic assertion was weakened.
Root default tests never recursively run real Cargo mutation workflows.

```sh
cargo test --bin mutie rust::discovery::inversion_tests -- --include-ignored
cargo test --test rust_workflow inversions:: -- --ignored --test-threads=1
cargo test --test rust_workflow boundaries:: -- --ignored --test-threads=1
cargo test
cargo fmt --check
cargo clippy --all-targets -- -D warnings
```

```sh
mutie /path/to/unchanged/ucd-generate --lang rust --rust-package ucd-parse \
  --include ucd-parse/src/unicode_data.rs --operator comparison_invert,negation_remove \
  --rust-build-cache worker --jobs 2 --timeout 60 --baseline-timeout 120 \
  --ui plain --output-dir /outside/original-reports
# Add the retained tests only to a separate copy.
mutie /path/to/stronger-copy --lang rust --rust-package ucd-parse \
  --rerun-missed /outside/original-reports/RUN --rust-build-cache worker --jobs 1 \
  --timeout 60 --baseline-timeout 120 --ui plain --output-dir /outside/stronger-reports
```

This is macOS Rust/Cargo 1.99.1 evidence in the existing scope, not Linux or broader
compiler/Cargo-layout qualification. Passing runs do not resolve the deferred host
startup/readiness issue. Python expression gaps and release qualification remain.
