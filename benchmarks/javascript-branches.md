# Scoped JS/TS branch forcing

Shared JS/TS discovery now supports `branch_force_true` and `branch_force_false`
for nonliteral `if` conditions across Vitest, Node and Bun. Existing runner,
layout, dependency, lifecycle and serial-execution limits remain unchanged.
No package, report-schema or operator-catalog version changes.

## Contract

For an original condition `(expression)`, the replacements are:

```js
if (true || (expression)) { /* original body */ }
if (false && (expression)) { /* original body */ }
```

The full original parenthesized condition remains byte-for-byte inside the
replacement. Forcing a branch **skips all condition evaluation**, matching Go's
existing branch-forcing contract. Calls, assignments, getters, exceptions,
`await` and `yield` inside the condition do not run. Bodies, surrounding statements,
labels and dangling-else relationships stay unchanged; exceptions in the selected
body still propagate normally. This is not a mutation that evaluates the original
condition and then ignores its result.

Nested `if`/`else if` conditions are independent sites. Syntactic Boolean literals
behind parentheses/comments are left to Boolean flips. Ternaries, loops,
TypeScript types, text and regular expressions are not branch-forcing sites.
Existing expression mutations inside eligible conditions remain independent.
No constant folding or type inference is performed. TypeScript assertions/casts
inside a runtime condition are retained; forcing a branch can invalidate a
narrowing assumption. Native TS acceptance does not imply type-checked builds.

Saved JS/TS reports reconstruct an `if` statement, require the exact original
condition span and regenerate the replacement. Bare constants, opposite guards,
extra statements and arbitrary edits cannot enter a saved rerun. Fresh source
rediscovery/fingerprints still apply. Bun's discovery-source recipe changes require
fresh original reports. Node/Vitest execution recipes and all classifiers remain
unchanged. Lifecycle/setup failures cannot earn body kills.

## Semantic and scored controls

`src/javascript/branch_tests.rs` covers exact Unicode/CRLF/multiline/comment spans,
nested branches, literal exclusions, TS casts/assertions and wrong-recipe rejection.
Application changes exactly the selected condition bytes and rejects stale source.
Existing shared discovery and Vitest preview tests now assert the expanded
inventory: six Boolean flips plus four branch candidates in the two-syntax fixture.

The native probe executes **80 bounded cases** on pinned Node 24.22.0 and
Bun 1.3.11+af24e281e, independently in JS and TS. Conditions cover calls,
assignments, logical/nullish/sequence expressions, optional chaining and property
access. Throwing condition calls/getters/setters are skipped; the selected branch
returns its original object identity. Body exceptions retain exact identity.
Separate async/generator and dangling-else cases show skipped await/yield and
unchanged branch association. Each case retains a ten-second deadline and 64 KiB
capture budget. All **20 shared JS tests**, including prior comparison, logical and
arithmetic native controls, pass with these pinned runtimes.

`tests/javascript_branches.rs` provides six independent runner/syntax workflows.
Each fixture has five default candidates: four branch edits and one Boolean flip.
Original tests kill four, leaving one force-true survivor. Operator filtering keeps
exactly four branch candidates. A stronger false-input test kills the survivor;
a separate setup-hook failure triggered by the same mutation remains an error
with zero kills. The workflow rejects a corrupted saved replacement and preserves
source plus original run/inventory/outcome bytes.

Initial Bun workflow failures were expected pin rejection: the shell's default
Bun had changed to 1.4.3. Both workflows pass with the retained 1.3.11 binary.
Native probes were also rerun with the pinned binary. This is not new-version
acceptance. Some Vitest attempts timed out at the unchanged 30-second baseline
bound with no candidates executed. Subsequent checks retain every deadline and
assertion; no host security setting, runner instrumentation or retry policy changed.
The persistent JavaScript probe and its report are retained under the audit root.
These passing checks do not resolve the deferred host startup issue.

## Unchanged Bragkit finding

The clean [Bragkit](https://github.com/artmsilva/bragkit) checkout remains at
`458d31127f1d3a6f3fd28afd2d7a4741ebc648a7`. All **57 tracked inputs** are unchanged,
including the native `node --test` script, metadata and original tests. Pinned
Node 24.22.0 runs the complete **61-test baseline** and every mutation suite.

Selecting `src/reports/impact.ts` and the two branch operators produces 16 sites:
**nine kills, seven survivors, no errors/timeouts**, in **24.84 seconds**. The
56.25% score is a filtered selection, not a full-project score.

| Original IDs | Branch | Surviving change |
| --- | --- | --- |
| 1 | Kudos impact bonus | Apply to every achievement |
| 3, 4 | Blog impact bonus | Always apply / never apply |
| 5, 6 | Merged-PR impact bonus | Always apply / never apply |
| 9, 10 | Closed-issue count | Count every achievement / count none |

Existing impact tests compare rankings and broad lower bounds, allowing wrong
bonuses to survive. The metric test does not assert `issuesClosed`. A separate
copy adds [two tests](fixtures/javascript-branches/bragkit-type-branches.test.ts):
exact bonus-only scores for each achievement type and a count that includes only
closed issues in a mixed list. No original test or implementation is changed.

The **63-test stronger baseline passes**, and the focused rerun kills **all seven
survivors** in **13.74 seconds**, without errors/timeouts. The original 56.25% score
and reports remain unchanged; the focused 100% result is not a full stronger-suite
score. Original source IDs are mapped to fresh local IDs through source/edit identity.

## Regression results

The final default suite passes **322 tests**, with **145 opt-in ignores** and zero
failures. All-target Clippy with warnings denied, Rust formatting and diff checks
pass. All six scored workflows have passing results: Node JS/TS in the first
matrix, Bun JS/TS with the corrected pinned PATH, Vitest TS on the first recheck
and Vitest JS on its final focused recheck. Earlier failures remain recorded in
the audit logs; no deadline or assertion was weakened.

## Preservation and reproduction

Local audit: `/tmp/mutie-js-branches-audit`. The
[machine-readable ledger](javascript-branches-results.json) records input/report
hashes, runtime identity, selections, source spans, outcomes and timings. All 57
original inputs, **295 historical report files**, and every file in the new
original report remain unchanged. The original checkout is clean; the stronger
copy adds only `test/type-branches.test.ts`. The frozen CLI is stored as `mutie`
in the audit directory. Timings are observations, not controlled benchmarks.

```sh
mutie /path/to/unchanged/bragkit --lang node --ui plain \
  --include src/reports/impact.ts --operator branch_force_true,branch_force_false \
  --timeout 20 --baseline-timeout 30 --output-dir /outside/original-reports
mutie /path/to/separate/stronger-copy --lang node --ui plain \
  --rerun-missed /outside/original-reports/RUN \
  --timeout 20 --baseline-timeout 30 --output-dir /outside/stronger-reports

python3 -B benchmarks/fixtures/javascript-branches/verify.py \
  /tmp/mutie-js-branches-audit benchmarks/javascript-branches-results.json
```

The verifier checks the pre-run preservation manifests, original/stronger bytes,
original report preservation, seven-survivor identity mapping and actual scores.
Use the direct pinned Node/Bun directories on PATH for opt-in tests;
`MUTIE_VITEST_FIXTURE=/tmp/mutie-package-managers-audit/npm` supplies the
prepared Vitest 4.1.9 / Vite 8.1.3 installation. Mutie invokes no installer.

```sh
cargo test --bin mutie javascript:: -- --include-ignored
cargo test --test javascript_branches -- --ignored --test-threads=1
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

This is scoped macOS evidence. Rust/Python branch forcing, loop/call removal,
literal mutations and public macOS/Linux qualification remain separate goals.
