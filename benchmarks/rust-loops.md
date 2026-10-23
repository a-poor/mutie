# Scoped Rust conditional-loop skipping

Rust `loop_skip` now supports ordinary Boolean `while` conditions in the existing
package/workspace scope. Mutation selection never narrows the project's test suite.
No package, report-schema or operator-catalog version changes.

## Contract

Only the condition changes, to `false && (ORIGINAL)`. Original bytes, references,
comments and grouping stay in the guard. Condition evaluation and the loop body
are deliberately skipped. Calls, assignments, temporary construction, consuming
calls, mutable-borrow calls and panics in the condition do not execute. Following
statements and surrounding scopes remain unchanged.

Rust still type-checks retained syntax. Ownership, borrowing and initialization
violations remain build failures, not kills. No named bindings, cloning, casts or
trait calls are introduced. Native Boolean keywords cannot be shadowed.

`while let` and let-chains are excluded and reported because their bindings may be
needed by the body. Nested ordinary loops inside a binding loop remain eligible.
Parenthesized/commented Boolean literals stay with boolean flipping. Iterator
`for`, unconditional `loop`, types, constants/const blocks, tests/cfg-excluded code
and macro tokens retain their exclusions. A whole macro invocation used as an
ordinary condition remains intact; its tokens are not mutated.

Saved Rust loop edits require a `.rs` file, an exact parsed `while` condition span
and reconstruction of the same recipe. Literals, bindings, injected extra syntax
and arbitrary replacements are rejected. Fresh source identity and rediscovery
remain mandatory. Execution recipes, classifiers and deadlines are unchanged.

## Controls and scored fixture

`src/rust/loop_tests.rs` checks Unicode/CRLF/comment spans, literal and binding
exclusions, nested loops, whole macro conditions, saved-recipe validation and
stale-source rejection. Six opt-in native compile/run cases use warnings denied,
a 30-second command deadline and 64 KiB capture budget:

- A panicking condition and an assignment are skipped.
- Non-Copy String comparisons leave the original values usable.
- A consuming call does not run; the owned value drops once at its original scope
  exit, after the following statement.
- A mutable-borrow call leaves its String unchanged.
- A temporary in the condition is never created or dropped.

All six cases pass. Helper functions unused in individual probes have local
`allow(dead_code)` attributes; compiler warnings remain denied.

The [fixture](../tests/fixtures/rust-loops/README.md) has three original tests and
three loop candidates: **one kill, one survivor and one build failure**, with no
errors/timeouts. The build failure comes from skipping the only initialization
of a value used after the loop. A stronger nonempty-input test kills the survivor
on focused rerun. Two-worker, worker-local-cache execution, operator configuration,
saved-edit corruption rejection, source-ID mapping and original-report preservation
all pass in the opt-in Cargo workflow.

## Unchanged public-project acceptance

The clean [Marathon](https://github.com/a-poor/marathon) checkout at
`750e21cf7778f0b59c19df194cdb85d15afe2963` has **86 tracked entries**, including an
internal skills-directory symlink. Manifest, lockfile, sources and original tests
remain unchanged. This revisits the previously validated library/binary project
at its recorded revision; it is not a claim about all Ratatui applications.

The complete original baseline passes **194 tests**. Selecting `src/tui.rs` and
`loop_skip` finds two candidates, both survivors, with no errors, timeouts or build
failures. The run takes **88.62 seconds** and retains its **0% filtered score**.
All library, binary and integration tests execute for each candidate.

Source ID 1 at line 123 skips the interactive event loop entirely. The `run`
command can then exit successfully without displaying its document. Existing
state/render unit tests do not verify that the real CLI enters its event loop.
Source ID 2 at line 205 skips horizontal search scrolling; it remains open.

A separate copy adds only
[`tests/interactive_loop.rs`](fixtures/rust-loops-marathon/interactive_loop.rs).
The test starts the real Cargo-built binary on an 80×20 pseudoterminal with a
read-only Markdown document, requires its unique title to render within ten
seconds, verifies the viewer stays alive for input, sends `q`, and requires a
successful exit within five seconds. Captures are capped at 64 KiB. An owned
child guard kills/reaps the process group on failure. The test does not run shell
cells, modify production code, install dependencies or use external services.

The test follows the local Ratatui testing guidance and pinned Ratatui 0.30.1 /
Crossterm 0.29.0. It validates real event-loop entry and quit, not all terminal
rendering, resize or restoration behavior. Native PTY evidence is macOS-only.
An initial stronger baseline failed to compile because macOS `openpty` requires
mutable pointers; the test helper was corrected. Two subsequent PTY attempts
were stopped for diagnosis and remain saved without any attempted mutations.
A one-second stack sample located a test cleanup stall in `waitpid`; an earlier
pre-spawn-stall inference was not established. The final harness drains terminal
restoration output while waiting for exit and closes the PTY before failure
cleanup/reaping. It also uses preallocated argv/environment and native fork/exec
so its unchanged ten-second readiness deadline includes executable startup. No
allocation or locks are used between fork and exec. A direct pristine control
passes in 0.47 seconds after these harness corrections. All failed attempts and
the diagnostic logs remain separate from acceptance; no production-spawn,
classifier, host-security or deadline change was made.

The final stronger **195-test baseline passes**, and a focused rerun kills ID 1
with no errors, timeouts or build failures in **49.70 seconds**. The original 0% report stays unchanged;
the focused result is not a full stronger-project score. The second survivor is
not declared equivalent or dismissed.

## Preservation and regressions

Audit root: `/tmp/mutie-rust-loops-audit`. The
[verifier](fixtures/rust-loops-marathon/verify.py) checks all original tracked
bytes and symlink targets in both copies, clean original Git status, all **557
historical report files**, and every new original report file against pre-rerun
hashes. The stronger copy adds only the retained integration test. The
[results ledger](rust-loops-results.json) includes actual outcomes, selections,
execution identity, baseline counts, source-ID mapping and hashes, including the
failed helper-compilation attempt. Timings are observations, not controlled
performance measurements.

The default Mutie suite passes **327 tests**, with **158 opt-in ignores** and no
failures. Native controls, scored Cargo fixture, all-target Clippy with warnings
denied, formatting and whitespace checks pass. Real Cargo mutation workflows
remain opt-in to avoid recursive self-mutation. Passing checks do not resolve
the deferred macOS startup issue; public Linux qualification remains separate.

```sh
cargo test --bin mutie rust::discovery::loop_tests -- --include-ignored
cargo test --test rust_workflow loops:: -- --ignored --test-threads=1
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt --check
git diff --check

mutie ORIGINAL --lang rust --include src/tui.rs --operator loop_skip \
  --rust-build-cache worker --jobs 2 --timeout 120 --baseline-timeout 240 \
  --ui plain --output-dir /outside/original-reports
mutie STRONGER --lang rust --rerun /outside/original-reports/RUN --id 1 \
  --rust-build-cache worker --jobs 2 --timeout 120 --baseline-timeout 240 \
  --ui plain --output-dir /outside/stronger-reports
python3 -B benchmarks/fixtures/rust-loops-marathon/verify.py \
  /tmp/mutie-rust-loops-audit benchmarks/rust-loops-results.json
```

Python conditional-loop skipping, standalone-call removal, literal mutations and
public release qualification remain roadmap work.
