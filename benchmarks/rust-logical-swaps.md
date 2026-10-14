# Rust logical swaps

Follow-up to `8fb84d6`, after the parallel score-rerun reliability check.
`logical_swap` reuses the shared operator catalog, CLI/config filters, reports,
identities and reruns. No package, schema or catalog version changes.

## Eligibility and replacement

Swap lazy boolean `&&` and `||` in eligible function/method bodies, closures and
match guards. Replace the parsed binary expression so parentheses can preserve
operand grouping when precedence changes. Preserve existing comments/trivia and
operand order; include each operand once. The changed operator intentionally
changes whether right-hand operands execute. This follows the
[Rust lazy boolean rules](https://doc.rust-lang.org/reference/expressions/operator-expr.html#lazy-boolean-operators).

The grammar's `let_chain` connectors are excluded and reported. Ordinary boolean
operands inside a chain remain eligible; a replacement disjunction is grouped
when necessary to remain a valid condition. Bindings and their scope remain
intact. Reference syntax, bitwise operators and closure delimiters are not
candidates. Existing exclusions for tests, macros, cfg, generated and constant
contexts apply unchanged. Logical replacements do not claim type analysis or
prove equivalent mutations.

The discovery regression checks exact edits for nested conjunctions/disjunctions,
comments, references, bitwise expressions, closures, guards and let-chains. It
applies every replacement, reparses it, and verifies unchanged source outside the
selected range. The comparison-boundary regression retains its operator-specific
assertions while allowing the newly discovered logical expression.

## Real Cargo fixture

`tests/fixtures/rust-logical` is an independent binary package. Its three ordinary
CLI tests check side-effect traces, three-operand truth tables, let-chain behavior,
and a deliberately weak disjunction with only equal-input coverage.

The opt-in `logical::rust_logical_swaps_preserve_grouping_and_find_missing_mixed_inputs`
workflow passed in 13.53 seconds. It selects the operator through `mutie.toml`,
runs eight mutations with two worker caches, and records **seven killed and one
missed**, with no build failures, timeouts or errors. Its 100% score policy fails
at 87.5% as expected. Every mutation targets `src/logic.rs`.

Adding a mixed-input test outside the production file, then selecting the saved
survivor with `--rerun-missed --operator logical_swap`, maps source ID 3 to new
ID 1 and kills it through `mixed_inputs_are_accepted`. The rerun's 100% policy
passes. Input hashes, the source report and caller artifacts remain intact, owned
temporary directories are empty, and saved reports pass strict retention preview.

Validation commands:

```sh
cargo test --locked --manifest-path tests/fixtures/rust-logical/Cargo.toml
cargo test --locked --test rust_workflow logical:: -- --ignored --nocapture
cargo test --locked
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
```

The fixture's three tests pass directly. The default suite passes 200 tests, with
26 opt-in tests ignored. Formatting, diff checks and Clippy pass. The other ignored
real-Cargo workflows were not repeated; existing Go default regressions remain
green. Root default tests never execute the new real-Cargo mutation workflow.


## Mutie self-validation

A current isolated Mutie copy selects the single logical conjunction in
`src/operator_summary.rs`:

```diff
- s.is_finite() && *s >= 0.0
+ s.is_finite() || *s >= 0.0
```

The command uses `--include src/operator_summary.rs --operator logical_swap
--rust-build-cache worker --jobs 1 --baseline-timeout 300 --timeout 600
--min-score 100 --ui plain`, with external report storage and caller artifact
sentinels. Its baseline passes all 200 default tests. The mutation is killed
only by `operator_summary::tests::distinguishes_missing_invalid_and_zero_durations`;
all integration tests pass. The 100% score policy passes, with zero missed,
timeouts, build failures or tool errors. This confirms existing invalid-duration
coverage; it is not a newly discovered test gap.

The whole run takes 285.88 seconds. With only one mutation, its worker cache is
initially empty; this run makes no dependency-reuse speedup claim. Input files and
caller artifacts are preserved, owned temporary storage is empty, and the saved
report passes strict retention preview. Local evidence is under
`/tmp/mutie-logical-self-2h264bba`, report
`reports/run-1792009924-UNZezw`, with command/preview logs, result JSON and input
hash maps. This is a focused real-project sample, not a broad finding-yield or
build-failure-rate estimate.
