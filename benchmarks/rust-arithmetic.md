# Scoped Rust arithmetic and remainder mutations

Rust's `arithmetic_replace` now supports binary `+ ↔ -`, `* ↔ /`, and `% → *`
within the existing ordinary package/workspace scope. No package, report-schema or
operator-catalog version changes. Unary signs, dereferences, compound assignments,
macro tokens, test-only/cfg subtrees, constant/type contexts and array repeat
lengths remain outside this operator's scope.

## Replacement and semantic controls

Each pair has the same precedence and associativity. Discovery replaces only the
parsed operator token, retaining grouping, trivia, operand evaluation order and
native ownership/trait dispatch. No casts, clones, borrows or arithmetic helpers
are introduced. A division replacement immediately before a comment includes a
separating space: `a*/* comment */b` becomes `a/ /* comment */b`, not a line comment.
Both block- and line-comment adjacency have discovery controls. Saved reports
accept only exact token pairs (including this `/ ` separator); focused reruns
still rediscover against source identity. Corrupt arithmetic recipes are rejected.

The [Rust expression reference](https://doc.rust-lang.org/reference/expressions.html#expression-precedence)
defines grouping and left-to-right operand evaluation. Native
[arithmetic rules](https://doc.rust-lang.org/reference/expressions/operator-expr.html#arithmetic-and-logical-binary-operators)
apply, including overloaded traits, integer truncation and floating-point results.
Test-profile integer overflow and zero division can panic. A missing replacement
trait or compile-time arithmetic error stays `build_failed`; a runtime test panic
uses the existing Cargo test-failure classification. Aborts/incomplete evidence
stay errors. There is no type inference or equivalence claim, and arithmetic
mutations can break invariants inside unsafe project code.

`src/rust/arithmetic_tests.rs` checks discovery/exclusion boundaries, Unicode/CRLF,
exact source preservation and parsed edits. Five opt-in native cases compile and
run all replacements across integers, floats, owned overloads and borrowed
overloads (four disjoint edits per semantic case, never a scored multi-mutation
run). Assertions cover values, negative integer truncation, checked overflow,
integer division by zero, signed zero, NaN/Infinity, operand evaluation, dispatch,
drop order and continued use of borrowed values. These native controls pass in
2.99 seconds. The compiler/run bounds are 30 seconds and 64 KiB each.

## Scored fixture

`tests/fixtures/rust-arithmetic` is an independent library with five original
integration tests. Its opt-in workflow uses two worker caches and the existing
30-second mutation / 60-second baseline bounds. Ten candidates produce seven
kills, two survivors and one build failure. Integer zero division and overflow
are kills; `String + &str` becoming subtraction is a build failure. The
multiplication case includes an adjacent comment to exercise the separating space.
A stronger test kills the weak addition/remainder survivors through a focused
rerun; source/report bytes, saved-ID mapping and invalid-recipe rejection are
checked. Root default tests do not recursively execute this Cargo workflow.

The first fixture attempt produced five kills, two survivors, one build failure
and two timeouts while other builds ran. Its unchanged-limit repeat passed in
42.10 seconds. This does not prove the deferred macOS startup/readiness issue is
fixed, nor establish the cause of those timeouts. Final post-separator validation
passes in 27.19 seconds, with the same expected outcomes.

## Unchanged public project and useful finding

[UCD Generate](https://github.com/BurntSushi/ucd-generate), commit
`e8fa937f0cac643669dcaf5edac2785b15cab917`, is the unchanged public workspace.
All 108 tracked files match the commit; no source, tests, manifests, lockfile or
configuration were adapted. Mutation selection is `ucd-util`; all four workspace
members run for each baseline/mutation. Original baselines pass 140 tests
(3/84/40/13). This revisits the workspace-validation project; it is not acceptance
of a new project or a full-workspace mutation score.

The initial 16-candidate selection stops with 11 kills, two survivors, one error
and two unfinished candidates. Both Hangul remainder replacements are killed.
Candidate 14 changes the ASCII case-offset subtraction into addition inside
unsafe normalization code. That violates the UTF-8 invariant and aborts while
formatting a panic. The failed report retains **no score**; the abort is not a
kill, and the two unattempted/unfinished sites are not counted as passing.

A completed unchanged focused selection replays original IDs **3, 5, 9, 11**:

| Original ID | Site | Outcome |
| --- | --- | --- |
| 3 | Hangul `s_index % N_COUNT` → multiplication | Killed |
| 5 | Hangul `s_index % T_COUNT` → multiplication | Killed |
| 9 | Hyphen's next-character lookup `i + 1` → subtraction | Survived |
| 11 | Tail boundary `i + 2 >= slice.len()` → subtraction | Survived |

This takes 34.46 seconds: two kills/two survivors, no build failures/errors/timeouts,
and a 50% **rerun-selection** score. It preserves the failed exploratory report.

A separate copy adds only
[`arithmetic_hyphen.rs`](fixtures/rust-arithmetic-ucd/arithmetic_hyphen.rs), checking
`A-1 → a-1`, `A- → a-`, and the medial control `A-B → ab`.
The original algorithm passes. The next-character mutant instead inspects the
previous letter and removes a non-medial hyphen. The tail-boundary mutant
underflows on the short name's index. The 141-test stronger baseline passes, and
both survivors are killed in 28.11 seconds, without classification/deadline changes.
The focused 100% result is not a replacement score for the original run.
These arithmetic findings overlap the non-medial-hyphen behavior explored by the
prior workspace boolean review, while demonstrating distinct operator/index bugs.

[Machine-readable evidence](rust-arithmetic-results.json) retains identities,
input/report hashes, all outcomes, baseline counts and command settings.
Local evidence is under `/tmp/mutie-rust-arithmetic-audit`; original,
focused-original and stronger reports remain separate. All original tracked files
match in both copies; all 109 original/exploratory and focused-original report files
retain their hashes, and the original Git checkout stays clean.

## Final checks

The final default suite passes **315 tests, zero failures, 129 ignored**. Formatting,
diff checks and all-target Clippy with warnings denied pass. The earlier
configuration/reader/browser test failures were corrected to reflect newly
supported arithmetic and valid synthetic operator recipes; no assertions were
removed. Native/scored checks were repeated after the comment-separator fix.

## Reproduction and limits

```sh
cargo test --bin mutie rust::discovery::arithmetic_tests -- --include-ignored
cargo test --test rust_workflow arithmetic:: -- --ignored --test-threads=1
cargo test
cargo fmt --check
cargo clippy --all-targets -- -D warnings
```

```sh
mutie /path/to/unchanged/ucd-generate --lang rust --rust-package ucd-util \
  --operator arithmetic_replace --rust-build-cache worker --jobs 2 \
  --timeout 60 --baseline-timeout 120 --ui plain --output-dir /outside/original
# The exploratory abort remains an error; select the recorded review sites.
mutie /path/to/unchanged/ucd-generate --lang rust --rust-package ucd-util \
  --rerun /outside/original/RUN --id 3,5,9,11 --rust-build-cache worker \
  --jobs 1 --timeout 60 --baseline-timeout 120 --ui plain --output-dir /outside/focused
# Add the retained integration test only to a separate copy.
mutie /path/to/stronger-copy --lang rust --rust-package ucd-util \
  --rerun-missed /outside/focused/RUN --rust-build-cache worker --jobs 1 \
  --timeout 60 --baseline-timeout 120 --ui plain --output-dir /outside/stronger
```

This is macOS Rust/Cargo 1.99.1 evidence within the existing scope. No Linux,
new compiler-version, broader Cargo-feature or unsafe-code soundness guarantee is
inferred. Rust comparison inversion/negation removal and the remaining Python,
control-flow, literal and release-qualification gaps remain separate work.
