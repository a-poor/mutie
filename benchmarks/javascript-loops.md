# Scoped JS/TS conditional-loop skipping

`loop_skip` now supports ordinary `while` and explicitly conditional `for` loops
across Vitest, Node and Bun. Each runner retains its existing JS/TS admission,
dependency, lifecycle and execution limits. No versions or classifiers changed.

## Contract and exclusions

The selected condition becomes `(false && (ORIGINAL))`. For `while`, ORIGINAL
includes its source parentheses; for `for`, it is the parsed condition expression.
Extra grouping preserves sequence expressions, logical combinations, comments and
TypeScript assertions. Original condition bytes remain inside the wrapper.

A `for` initializer still executes once. Its assignments, lexical bindings, effects
and exceptions remain native. The condition, loop body and update do not execute.
Thus condition calls, assignments, getters, exceptions, `await` and `yield` are
skipped. Labels, lexical scope and following statements remain intact. A thrown
initializer exception is not swallowed. This follows the existing Go loop-skip
contract rather than evaluating a condition and ignoring its result.

Literal `true`/`false` conditions behind parentheses/comments stay with boolean
flipping. No constant folding or type inference is attempted. Conditionless
`for`, `for…in`, `for…of`, `for await…of` and `do…while` are excluded. Do-while
needs a separate contract because its body runs before its condition. Existing
operators inside excluded constructs retain their own eligibility. Type syntax,
strings, regexes and test files do not gain loop candidates. Native TypeScript
support does not establish type-checked builds or broader runner compatibility.

Saved edits reconstruct a conditional `for`, require an exact condition span,
and regenerate the exact replacement. Bare constants, extra statements and
arbitrary guards are rejected. Fresh source identity remains mandatory. Bun's
shared-discovery recipe changes require fresh original reports; Node and Vitest
execution recipes are unchanged.

## Controls and fixture validation

`src/javascript/loop_tests.rs` checks Unicode/CRLF/multiline/comment spans,
sequence grouping, nested loops, TypeScript expressions, exclusions, corrupt
recipes and stale-source rejection. The native probe runs **32 bounded cases**:
eight cases on each combination of pinned Node/Bun and JS/TS. They cover:

- While conditions, calls, getters, assignments and body effects being skipped.
- For initializers running once, updates being skipped, `var` remaining visible,
  and a shadowing `let` preserving its outer binding and label.
- Exact initializer exception identity and skipped await/yield, retaining async
  and generator behavior.

Every native case has a ten-second deadline and 64 KiB capture limit. All **23
shared JS tests**, including earlier arithmetic/comparison/logical/branch probes,
pass on Node 24.22.0 and Bun 1.3.11+af24e281e.

`tests/javascript_loops.rs` runs six independent scored workflows. The while
fixture has a nonempty-input kill; the for fixture initially tests only empty
input and survives. Explicit `loop_skip` selection produces exactly two sites.
Adding a nonempty-input assertion kills the survivor on a focused rerun. A setup
hook triggered by that same survivor remains an error with zero kills. Corrupt
saved edits are rejected, and source/original-report bytes are preserved.
All six workflows pass, including independent JavaScript and TypeScript on Vitest
4.1.9 / Vite 8.1.3, Node and Bun. No deadline or assertion was weakened.

The full default suite passes **326 tests**, with **156 opt-in ignores** and no
failures. All-target Clippy with warnings denied, formatting and diff checks pass.
These passing checks do not resolve the separately deferred host startup issue.

## Unchanged-project finding

The clean [OpenClaw checkout](https://github.com/yukiharukonishi/openclaw-cc) at
`e323faa446f03b5438fa9edc58781b4ef8dd52df` retains all **63 tracked inputs**.
Its original manifest, explicit Node test script and complete **62-test suite**
are unchanged. This revisits an existing validated JavaScript project.

Selecting `src/channel-adapters/telegram.js` and `loop_skip` finds one candidate:
skip the chunking loop at line 52. It **survives** the complete original suite,
with no errors/timeouts, in a 7.79-second run. The original filtered score is 0%;
this says nothing about a whole-project mutation score.

The mutant returns an empty chunk list for messages longer than the requested
maximum, silently discarding their text. The short-message fast path still works.
A separate copy adds [one test](fixtures/javascript-loops/telegram-chunks.test.js)
for exact forced splits, newline-preferred splits, the default 4,096-character
boundary, and a short-input control. It exercises exported `chunkText` without
network calls, external accounts or original test changes.

The **63-test stronger baseline passes**, and the saved-survivor rerun **kills the
candidate** in 7.89 seconds, with no errors/timeouts. The focused 100% result is
not a full stronger-project score. The original 0% report remains unchanged.

## Preservation and reproduction

Audit root: `/tmp/mutie-js-loops-audit`. The retained
[verifier](fixtures/javascript-loops/verify.py) confirms the original Git checkout
is clean, all 63 original inputs match, and the stronger copy adds only
`test/telegram-chunks.test.js`. All **1,362 historical report files** and the new
original report retain their pre-rerun hashes. The
[result ledger](javascript-loops-results.json) records inventory, outcomes,
runtime identity, baseline counts, source-identity mapping, scores and hashes.
Timings are observations, not controlled benchmarks.

```sh
mutie ORIGINAL --lang node --include src/channel-adapters/telegram.js \
  --operator loop_skip --timeout 20 --baseline-timeout 30 --ui plain \
  --output-dir /outside/original-reports
mutie STRONGER --lang node --rerun-missed /outside/original-reports/RUN \
  --timeout 20 --baseline-timeout 30 --ui plain --output-dir /outside/stronger-reports
python3 -B benchmarks/fixtures/javascript-loops/verify.py \
  /tmp/mutie-js-loops-audit benchmarks/javascript-loops-results.json
```

For opt-in controls, put the retained pinned runtime directories on PATH and use
`MUTIE_VITEST_FIXTURE=/tmp/mutie-package-managers-audit/npm`. Mutie invokes
no installer.

```sh
cargo test --bin mutie javascript:: -- --include-ignored
cargo test --test javascript_loops -- --ignored --test-threads=1
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt --check
git diff --check
```

Rust/Python conditional-loop skipping, standalone-call removal, literal mutations
and public macOS/Linux release qualification remain separate roadmap work.
