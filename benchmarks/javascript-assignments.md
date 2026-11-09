# JS/TS property and item assignment removal

`assignment_remove` supports standalone property/index writes across Node, Bun
and Vitest, independently for JavaScript and TypeScript. A complete statement
such as `state.value = next;` or `items[key] = next;` becomes `;`. The mutation
skips receiver, key, RHS and store effects. An empty statement preserves ASI,
unbraced if/else branches, labels, loop bodies and non-directive string positions.

The scope admits ordinary `=` with a member/subscript target, parentheses and
native erasable TS syntax. It excludes bindings/destructuring, value uses,
compound stores, nested/chained assignments, updates, suspension expressions,
optional targets, super/private access and class initialization contexts.
Constructor, field-initializer and static-block ancestors exclude nested callbacks
too. Escaped method names are conservatively excluded. This is not inference about
ordinary functions later invoked with `new`, or about which other methods initialize
state. Those remain candidate-quality limits, not claims of semantic equivalence.

The selector and saved reader share the same parsed recipe. A different operator,
extra statement or non-`;` replacement is rejected. Fresh source/operator discovery
also rejects a forged constructor span with correct source coordinates, because
an isolated statement cannot establish its enclosing context. Bun's execution
fingerprint includes the new selector module. Classifiers, runtime admission,
timeouts and package/report/catalog versions are unchanged.

## Semantic and scored controls

The [ECMAScript assignment algorithm](https://tc39.es/ecma262/multipage/ecmascript-language-expressions.html#sec-assignment-operators)
informed the scope; paired execution on the pinned runtimes is the behavioral
control. Native tests cover 28 original/mutant evaluation pairs and four directive
boundary cases across Node/Bun × JS/TS. Traces check receiver/getter lookup,
computed-key coercion, RHS evaluation and Proxy stores, including unbraced branches,
labels, ASI, loop bodies and unchanged `finally` execution. The pristine controls
initially exposed an incorrect expected coercion order in the test: both pinned
runtimes place computed-key coercion after the RHS. The expectation was corrected;
the replacement still skips all of those effects. Ten-second deadlines and 64 KiB
capture limits remain intact.

Six independent scored workflows verify a kill and a survivor, exact 50% raw
score and minimum-score exit code 3, a stronger-test kill, source drift, corrupt
recipes/operators, forged constructor spans, original-report preservation and
setup failures remaining errors with no passing score. These use Node 24.22.0,
Bun 1.3.11+af24e281e and Vitest 4.1.9 / Vite 8.3.4. Native checks remain opt-in;
each runner's Linux CI job now includes its JS and TS assignment workflows,
and Node/Bun jobs include their own semantic controls.

## Unchanged-project acceptance

The original [OpenClaw checkout](https://github.com/yukiharukonishi/openclaw-cc)
at `e323faa446f03b5438fa9edc58781b4ef8dd52df` retains all 63 tracked files,
including the manifest and complete 62-test suite. No source, tests, configuration
or project layout was adapted. Mutie's config and reports live outside the
checkout. This revisits an existing validation project.

The selected `src/session-manager.js` and `src/workspace-loader.js` files contain
three admitted candidates, all in the session manager. The original full-suite
run completes in **15.57 seconds: one kill, two survivors, no errors or timeouts**.
Its **33.33% score covers this filtered selection**, not the entire project.

Two tests in a separate copy check externally observable session metadata:

- Removing the reset-path `meta[sessionKey] = newEntry` returns a replacement
  session ID without saving it. The stronger test checks persisted metadata and
  that a later resolve reuses the replacement session.
- Removing `entry.updatedAt = Date.now()` leaves a reused session's timestamp
  stale. The stronger test advances a controlled clock and checks the persisted
  timestamp while retaining the session ID and creation timestamp.

The [retained tests](fixtures/javascript-assignments/session-metadata.test.js)
restore the clock and remove their owned temporary directory. They do not send
messages or use accounts. The stronger pristine baseline passes **64 tests**, and
the focused rerun kills both survivors in **12.59 seconds**, without errors or
timeouts. This focused 100% result does not replace the original filtered score
or establish a full stronger-project score.

The [verifier](fixtures/javascript-assignments/verify.py) checks the clean pinned
checkout, all original/stronger source hashes, the sole added test file, all
25 original report files, exact source identities across local mutation IDs,
baseline counts and unchanged Node execution identity. The
[results](javascript-assignments-results.json) contain relative paths and hashes.

```sh
python3 benchmarks/fixtures/javascript-assignments/verify.py \
  /tmp/mutie-js-assignments-audit benchmarks/javascript-assignments-results.json
```

## Validation

- Default suite: **350 passed, 182 opt-in tests ignored**, with two test threads.
- All **29** shared JS/TS discovery/native tests pass on pinned Node and Bun.
- All **six** independent assignment workflows pass; both Bun workflows were
  rechecked after binding the selector module into execution identity.
- Rustfmt, Clippy with warnings denied, Actionlint and whitespace checks pass.

Local logs use `/tmp/mutie-js-assignments-*`; real-project runs and the frozen
campaign CLI are under `/tmp/mutie-js-assignments-audit`. This new slice has not
yet run on remote Linux CI. The preceding Python assignment/cancellation commits
were pushed through `5384542` and passed all nine
[Linux jobs](https://github.com/a-poor/mutie/actions/runs/42648367150).
The separate macOS pre-main startup investigation
remains deferred. Go/Rust assignment removal and broader block deletion remain
separate roadmap work.
