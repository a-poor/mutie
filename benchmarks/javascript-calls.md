# Scoped JS/TS standalone-call removal

`call_remove` now supports complete ordinary call statements across Vitest, Node
and Bun, independently for JavaScript and TypeScript. It replaces the complete
statement with `;`, deliberately skipping receiver/property lookup, argument and
spread evaluation, and invocation. Empty statements preserve unbraced if/else,
labels, loop bodies, automatic-semicolon boundaries and directive boundaries.

Direct, method, computed, optional and parenthesized calls are admitted, including
native erasable TypeScript call syntax. Calls used as return values, assignments,
initializers, concise arrow results, arithmetic or sequence operands are excluded.
Constructors, direct `super()`, dynamic imports and tagged templates are excluded.
Any call subtree containing `await` or `yield` is conservatively excluded, including
nested callback bodies. Independent eligible statements elsewhere remain candidates.
No static return-type inference is attempted: ordinary unawaited calls may return
promises. Existing lifecycle attribution continues to reject cleanup/setup failures;
removing a call does not grant new runner compatibility or suppress errors globally.

The saved reader reparses exactly one full statement and requires the exact `;`
replacement. Extra statements, empty replacement and arbitrary expressions are
rejected. Fresh source identity remains mandatory. Bun includes shared discovery
bytes in its execution recipe and therefore requires fresh original reports.
No schema/catalog/package versions, classifiers, deadlines or assertions changed.

## Semantic and scored controls

`src/javascript/call_tests.rs` checks exact UTF-8/CRLF/comment spans, optional and
parenthesized calls, native TS generics/assertions/non-null call targets, exclusions,
corrupt saved recipes and source restoration/staleness. There are 32 native cases:
eight each on Node/Bun × JS/TS. They verify skipped receiver/getter/computed-key/
argument/spread effects, unbraced branches, labels, optional calls, ASI, loop bodies
and unchanged finally execution. Every native case retains a ten-second deadline
and 64 KiB capture limit. All 25 shared JS discovery/native tests pass.

`tests/javascript_calls.rs` runs six independent scored workflows. A checked
array append is killed, while an unchecked append survives. A stronger content
assertion kills that survivor on focused rerun; a setup hook triggered by the same
mutation remains an error with zero kills. Corrupt saved edits are rejected and
original source/report bytes remain unchanged. All six workflows pass with pinned
Node 24.22.0, Bun 1.3.11+af24e281e and Vitest 4.1.9 / Vite 8.1.3.

The default PATH had newer Node/Bun releases, so these controls explicitly use
retained pinned executables. This does not extend runtime-version support.

```sh
export PATH=/path/to/home/.vite-plus/js_runtime/node/24.22.0/bin:/tmp/mutie-bun-pinned-1.3.11/bun-darwin-aarch64:$PATH
cargo test --bin mutie javascript:: -- --include-ignored
MUTIE_VITEST_FIXTURE=/tmp/mutie-package-managers-audit/npm \
  cargo test --test javascript_calls -- --ignored --test-threads=1
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt --check
```

## Repository regression diagnostics

The first default run found a stale Node test that expected `call_remove` to be
unsupported. It now checks that this admitted operator reaches runtime discovery;
invalid jobs/scope/layout checks remain intact. The subsequent run passed 311
checks before all 12 PTY terminal tests timed out. Separate unchanged terminal
runs inside and outside the sandbox each passed four and failed eight; a bounded
sample of one owned stalled CLI showed only `_dyld_start`, before Mutie code.
This matches the previously deferred host-startup symptom but does not establish
the cause of every timeout. Deadlines, assertions, production spawning and host
security settings were not changed. The six trailing Vitest checks passed separately.
Clippy, formatting and whitespace checks pass. Local diagnostic logs and the
sample are retained alongside the acceptance evidence. A final unchanged full-suite
retry passed **329 tests, 167 ignored, zero failures**, including all 12 terminal
tests. This passing retry does not erase the failures or resolve the deferred
startup issue. All diagnostic logs are hashed by the preservation verifier.

## Unchanged-project finding

A fresh unchanged [OpenClaw checkout](https://github.com/yukiharukonishi/openclaw-cc)
at `e323faa446f03b5438fa9edc58781b4ef8dd52df` retains all 63 tracked inputs,
including its manifest and original complete 62-test suite. This revisits an
already validated public project. Earlier temporary original files were no longer
available, so the pinned revision was fetched again without adapting its inputs.

Selecting `src/channel-adapters/telegram.js` and `call_remove` finds six candidates.
All six survive with no errors/timeouts/build failures in 28.15 seconds. The
original file/operator-filtered score remains 0%; it is not a full-project score.

Source IDs 1 and 2 remove the appends that retain the last chunk and full chunks.
The mutant silently loses message text despite returning an array. A separate
copy adds [one test](fixtures/javascript-calls/telegram-chunks.test.js) asserting
exact forced splits, exact-multiple boundaries, newline splitting and a short-input
control. It calls exported `chunkText` without sending messages or using accounts.

The stronger pristine baseline passes 63 tests; the focused rerun kills both
selected survivors with no errors/timeouts in 12.34 seconds. The other four
survivors (result accumulation and diagnostic logging) remain open; none is declared
equivalent or dismissed. Focused 100% is not a full stronger-project score.

The retained [verifier](fixtures/javascript-calls/verify.py) checks the original
clean Git revision, all 63 input hashes, the sole stronger-test addition, source
identity mapping, baseline counts, unchanged execution identity, all original
report files and 468 captured historical report files. The
[result ledger](javascript-calls-results.json) records the evidence and hashes.
Local root: `/tmp/mutie-js-calls-audit`.

```sh
mutie ORIGINAL --lang node --include src/channel-adapters/telegram.js \
  --operator call_remove --timeout 20 --baseline-timeout 30 --ui plain \
  --output-dir /outside/original-reports
mutie STRONGER --lang node --rerun-missed /outside/original-reports/RUN --id 1,2 \
  --timeout 20 --baseline-timeout 30 --ui plain --output-dir /outside/stronger-reports
python3 benchmarks/fixtures/javascript-calls/verify.py \
  /tmp/mutie-js-calls-audit benchmarks/javascript-calls-results.json
```

Rust/Python standalone-call removal, literal replacement and release qualification
remain roadmap work. Passing checks do not resolve the deferred macOS startup issue.
