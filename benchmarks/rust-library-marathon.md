# Ordinary Rust libraries: Marathon validation

## Result and scope

On 2026-10-15, Mutie completed the scoped run → inspect → improve tests → rerun
workflow on Marathon, an ordinary library-plus-binary package. The library-only
fixture and library acceptance regressions supply complementary layout, review,
failure and cancellation evidence. This completes the ordinary-library milestone
at **Validated**, not Established. Mutie itself remains the binary-only control.

Marathon revision: `6d16015fc9747a8b84e9cfd286b230ce091f1a7f` (clean checkout).
Mutie revision: `da1b318`. Host: macOS arm64, Rust/Cargo 1.99.0. Cargo ran offline
using available registry dependencies. Every execution used default features and
`cargo test --locked --tests --no-fail-fast`, with compilation separated using
`--no-run`; doctests and ignored tests stayed excluded.

All work ran on a copy of Marathon's tracked files under
`/tmp/mutie-marathon-validation-_g9qup2n`. Neither the original checkout
nor its tests were edited. Both `src/lib.rs` and `src/main.rs` appear in the saved
production-target identity. Discovery found 170 mutations; this validation ran
all 15 candidates in `src/book.rs`, covering parsing, script construction,
environment validation and input editing. It did **not** execute all 170
mutations. The full test suite ran for every selected mutation.

## Execution evidence

| Run | Selection | Baseline tests | Outcomes | Total | Baseline |
| --- | --- | --- | --- | --- | --- |
| Initial, worker cache | 15 book mutations | 182 pass | 11 killed, 4 missed | 156.68s | 25.30s |
| Stronger tests, worker cache | 4 saved survivors | 184 pass | 3 killed, 1 missed | 83.24s | 24.57s |
| Stronger tests, fresh artifacts | Same 4 survivors | 184 pass | Same outcomes and exact failed-test sets | 106.53s | 24.35s |

All runs used two workers, a 180-second baseline timeout and a 120-second
mutation timeout. There were no timeouts, build failures, execution errors or
unfinished mutations. The initial filtered score was 11/15 (73.33%); the focused
rerun score was 3/4 (75%). These scores describe different selections, not a
before/after whole-project score. The fresh/cached comparison is one sequential
pair, not a general performance guarantee.

The [machine-readable results](rust-library-marathon.json) retain per-mutation
outcomes, timings, test failures, source IDs, toolchain identity and discovery
exclusions. Raw reports and logs remain at the temporary evidence root:

- Initial: `reports/run-1792072971-22vWH7`.
- Stronger worker-cache rerun: `reports/run-1792073179-5ayM0a`.
- Fresh rerun: `reports/run-1792073293-rQNV3X`.

## Finding usefulness

| Saved ID | Location | Review and stronger-test result |
| --- | --- | --- |
| 1 | `book.rs:97`, logical swap | Ordinary `json` blocks were treated as input prompts. A public-API regression using valid display-only JSON now rejects this mutation. |
| 2 | `book.rs:98`, boolean flip | Missing `mrthn=input` metadata defaulted to input behavior. The same JSON regression kills this mutation. |
| 4 | `book.rs:252`, logical swap | An after-hook gained an extra leading newline or an extra newline after an already terminated body. A script-construction regression kills this mutation. This establishes an observable text contract, not a claim that every extra blank line changes shell execution. |
| 11 | `book.rs:913`, comparison boundary | Reviewed as equivalent for reachable editor states; intentionally remains missed. |

For ID 11, when the cursor is below the character count, both versions delete the
same character. Above the count, both return. At exactly the count, the original
returns while the mutation replaces the empty end-of-string range with an empty
string and leaves the cursor unchanged. Adding an artificial assertion to kill
this mutation would not improve the behavioral tests.

The [two-test patch](marathon-stronger-tests.patch) was applied only in the
isolated copy, adding `tests/mutation_regressions.rs`; production source and
Cargo manifests/lockfile stayed byte-identical. It passes `git apply --check`
against the original revision. The stronger tests are proposed changes for
Marathon, not changes applied to its working checkout.

A dismissal for saved ID 11 was added and checked in the copy after both reruns.
Full rediscovery matched it as current ID 12 (the earlier run had a file filter):
`1 matched; 0 require review`. This verifies identity-based review across changed
selection IDs. Previously saved reports and their raw scores remained unchanged;
dismissal does not erase the survivor from mutation-score arithmetic.

## Isolation and rebuild checks

- SHA-256 checks preserve every original tracked file in both the real checkout
  and the validation copy. The copy's only additions are the stronger test file
  and intentional dismissal metadata. The original Git status remains clean.
- Every compilation rebuilt all local package artifacts: eight before the new
  integration target and nine afterwards, including library, linked binary and
  test outputs. Execution-phase reuse of those just-built artifacts is expected.
- Each worker's first mutation built 134 dependency artifacts; subsequent worker
  mutations reused those dependencies while rebuilding the local package. Fresh
  runs rebuilt all 134 dependencies for each mutation.
- The caller target directory retained only its unchanged sentinel. Owned
  temporary storage was empty after runs and dismissal checking.
- SHA-256 inventories of both the initial and worker rerun report trees remained
  unchanged after subsequent work. Final run/outcome checkpoint numbers match.

## Reproduction and limits

On an isolated copy of the pinned revision, run Mutie with `--include src/book.rs
--rust-build-cache worker --jobs 2 --baseline-timeout 180 --timeout 120 --ui plain`.
Retain the printed report directory, add the two-test patch, then use
`--rerun-missed <initial-report>` with the same timeouts/jobs and worker caching.
Repeat the saved selection with `--rust-build-cache fresh`. Use owned temporary
and caller artifact paths outside the copied package, as recorded in the JSON
and raw command records. No full project mutation score is inferred.

The saved inventory includes 36 exclusion messages: test-only/conditional and
remapped source, macro tokens/expansions, and let-chain connectors. These are
existing discovery limits, not evidence of complete Rust syntax coverage. This
is one macOS library-plus-binary project; library-only coverage is fixture-based.
Windows, workspaces, local dependencies, multiple binaries, proc macros, other
library crate types, custom harnesses, feature selection and doctests remain
outside the supported scope. No Rust adapter fix was needed during this run.
