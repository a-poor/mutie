# Scoped Rust branch forcing

Rust now supports `branch_force_true` and `branch_force_false` for ordinary Boolean
`if` conditions in its existing package/workspace scope. Source selection narrows
mutations, not the full test suite. No package, schema or operator version changes.

## Contract and binding audit

The condition alone becomes `true || (ORIGINAL)` or `false && (ORIGINAL)`.
The original bytes, references, parentheses and comments remain inside the guard;
the condition is deliberately **not evaluated**. Calls, assignments, moves,
borrows, comparisons, temporary construction and panics inside it are skipped.
Branch bodies and surrounding expression context remain unchanged. Native Boolean
keywords cannot be shadowed. No named binding, cast, clone or trait call is added.

`if let` and let-chains are explicitly excluded. Their pattern bindings can be
required by the consequence body; short-circuiting away the pattern does not
provide those bindings. Discovery reports this exclusion rather than generating
predictably invalid branch recipes. Nested ordinary `if` conditions within an
`if let` body remain eligible because the outer binding is preserved. Ordinary
Boolean subexpressions retain their existing expression-operator eligibility.

Syntactic Boolean literals behind parentheses/comments stay with `boolean_flip`.
Types, constants/const blocks, tests/cfg-excluded source, macro tokens and loop or
match-guard conditions remain outside branch forcing. A complete macro call used
as an ordinary condition remains intact; compilation still expands it, but the
forced condition skips its runtime evaluation. No macro-token mutation is added.

Rust still type-checks retained expressions and selected bodies. Changes that
violate ownership, borrowing or definite initialization remain build failures.
The fixture demonstrates this concretely: a value initialized only inside the
condition may be uninitialized after forcing either result. These are not kills.
No error classifier, timeout, process recipe or execution-identity rule changes.

Saved reports require a `.rs` path, an exact parsed original condition span and
reconstruction of the same operator-specific recipe. Boolean literals, binding
conditions, arbitrary guards and injected extra syntax are rejected. Fresh source
identity and rediscovery remain required for reruns.

## Native and fixture controls

`src/rust/branch_tests.rs` validates Unicode/CRLF/comment spans, deterministic
operator pairs, parenthesized literal exclusions, `if let`/let-chain exclusions,
nested ordinary branches and intact macro-call conditions. Mutation application
changes only its span and rejects stale sources. Invalid saved recipes fail.

Ten opt-in native compile/run cases cover both force directions with warnings
denied, a 30-second deadline and 64 KiB capture bound per command:

- Skipped condition panic and skipped assignment with the original value retained.
- Non-Copy String comparisons whose values remain available after the condition.
- Skipped consuming calls: the owned value is not moved at runtime and drops once
  at its original scope exit, after the selected body.
- Skipped mutable-borrow calls: the original String remains unchanged.
- Original selected branch result and trace for both force directions.

All ten cases pass. The final two unit/native tests finish in 2.69 seconds.
No compiler warnings or semantic assertions were disabled for the mutation recipe.
Test helper functions unused in individual probes have local `allow(dead_code)`
annotations; compiler warnings remain denied for each compiled probe.

The independent [fixture](../tests/fixtures/rust-branches/README.md) has three
original tests and six candidates. Its scored workflow, with two workers and
worker-local Cargo caches, produces **three kills, one survivor and two build
failures**, no errors/timeouts. Both build failures are definite-initialization
violations. A stronger false-input test kills the force-true survivor in a focused
rerun. The workflow rejects corrupt recipes for both operators and preserves
source, original report bytes and source-ID mapping. It passes in 16.61 seconds.

## Unchanged public-project acceptance

The unchanged [UCD Generate](https://github.com/BurntSushi/ucd-generate) workspace
remains at `e8fa937f0cac643669dcaf5edac2785b15cab917`, with all **108 tracked inputs**
unchanged. This revisits the validated workspace; it is not a new compatibility
claim. The selection covers `ucd-parse/src/unicode_data.rs` in member `ucd-parse`,
while every candidate tests all four workspace members.

The original baseline passes **140 tests** (3/84/40/13). All 26 branch candidates
complete in **153.45 seconds**: **15 killed, 11 survived**, with no build failures,
errors, timeouts or unfinished candidates. The 57.69% score is filtered to this
file and these operators, not a full-project score.

Four force-false survivors silently drop optional parsed fields:

| Source ID | Line | Field lost |
| ---: | ---: | --- |
| 4 | 145 | Decimal numeric value |
| 6 | 157 | Digit numeric value |
| 10 | 175 | Simple uppercase mapping |
| 14 | 181 | Simple titlecase mapping |

A separate copy adds only [two direct parser-contract tests](fixtures/rust-branches-ucd/optional_fields.rs).
A DIGIT ZERO record must retain `Some(0)` in both numeric fields. A small-letter
record with uppercase/titlecase mapping columns must retain U+0041 in both fields.
Original tests and source are untouched. These assertions catch successful parsing
that silently loses data, without changing expected-error handling or classification.

The stronger baseline passes **142 tests** across the workspace. A focused rerun
of IDs 4, 6, 10 and 14 kills **all four**, with no errors/timeouts/build failures,
in **32.14 seconds**. The focused 100% score does not replace the original 57.69%
filtered score. Seven original survivors remain open: IDs 15, 16, 18, 20, 21, 22
and 24 (decomposition formatting, capacity guard and empty-input guards). They are
not declared equivalent, dismissed or resolved by this goal.

## Preservation, reproduction and regressions

The [results ledger](rust-branches-results.json) records original/stronger reports,
source/edit identities, execution metadata, hashes and timings. Local audit:
`/tmp/mutie-rust-branches-audit`. All 108 originals match in both copies,
all **225 historical report files** remain unchanged, and a pre-rerun manifest
verifies every file in the new original report. The original checkout remains
clean; only `ucd-parse/tests/optional_fields.rs` is added in the stronger copy.

The [verifier](fixtures/rust-branches-ucd/verify.py) checks preserved hashes, exact
selected-source identity mapping, original score, actual outcomes and full-workspace
baseline counts. Timings are observations, not controlled performance measurements.

```sh
mutie /path/to/unchanged/ucd-generate --lang rust --rust-package ucd-parse \
  --include ucd-parse/src/unicode_data.rs --operator branch_force_true,branch_force_false \
  --rust-build-cache worker --jobs 2 --timeout 60 --baseline-timeout 120 \
  --ui plain --output-dir /outside/original-reports
mutie /path/to/separate/stronger-copy --lang rust --rust-package ucd-parse \
  --rerun /outside/original-reports/RUN --id 4,6,10,14 \
  --rust-build-cache worker --jobs 2 --timeout 60 --baseline-timeout 120 \
  --ui plain --output-dir /outside/stronger-reports
python3 -B benchmarks/fixtures/rust-branches-ucd/verify.py \
  /tmp/mutie-rust-branches-audit benchmarks/rust-branches-results.json
```

The default suite passes **323 tests**, with **147 opt-in ignores** and zero
failures. Native controls, scored fixture, all-target Clippy with warnings denied,
Rust formatting and diff checks pass. Real Cargo mutation tests remain opt-in to
avoid recursive self-mutation.

```sh
cargo test --bin mutie rust::discovery::branch_tests -- --include-ignored
cargo test --test rust_workflow branches:: -- --ignored --test-threads=1
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

This is macOS evidence on the existing supported Rust toolchain. Public Linux
qualification remains separate, and passing runs do not resolve the deferred host
startup issue. Python branch forcing, loop/call removal and literal mutations
remain roadmap work.
