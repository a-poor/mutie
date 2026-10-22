# Bounded Python candidate-error continuation

The [194-candidate arithmetic review](python-arithmetic-full.md) needed 35 stopped
runs to execute the 164 candidates not covered by the initial Kelly acceptance.
Ordinary Python body exceptions stopped the entire run even when the complete
suite, interpreter lifetime and isolated inputs could be verified.

`--continue-on-error MAX_ERRORS` now opts a **serial Python run** into bounded
continuation. For example:

```sh
mutie /path/to/project --lang python --python /path/to/pinned/venv/bin/python \
  --operator arithmetic_replace --continue-on-error 64
```

The positive limit counts eligible errors across this invocation. Reaching the
limit stops execution; `1` therefore stops at the first error. With no flag,
stop-on-error behavior is unchanged. This is an invocation-only setting, recorded
in effective configuration, not a repository setting. Focused reruns must opt in
again. Other languages and read-only listing reject the option.

## Eligibility and failure boundaries

The observer records positive type evidence for ordinary pytest body exceptions
that derive from `TypeError`, `ValueError`, `ArithmeticError` or `LookupError`.
Application subclasses are eligible. Names alone are not evidence. Exceptions
also deriving from `OSError`, `MemoryError`, `SystemError` or `RecursionError` are
excluded. Assertions retain their existing classification, including multiply
inherited assertion/value exceptions. Arbitrary exceptions and unittest body
exceptions remain fatal in this first scope.

A body exception does not short-circuit evidence validation. The adapter must
verify every setup/body/teardown phase, fixture finalization, session status,
returned/atexit checkpoints, exact collected inventory and actual process exit.
It rejects remaining descendants, uncaptured stderr, capture failures and
incomplete evidence. Observer/config bytes, copied/original inputs and runtime
identity must pass the existing post-execution checks. Only then does the adapter
emit an internal isolated-candidate-error marker.

The shared classifier **still returns an error**. The worker scheduler uses the
marker solely to decide whether it may schedule another candidate within the
explicit limit. Setup, teardown, fixture, collection, baseline, runtime drift,
workspace, persistence and infrastructure failures remain fatal. Cancellation
still drains workers and takes precedence over previously deferred body errors.
Timeout outcomes retain their existing handling and deadlines.

Any run with candidate errors still exits **1**, is saved as **failed**, and has
**no score**, even if every candidate completed. Findings/minimum-score policy
cannot turn it into exit 0 or 3. Each error retains its diagnostics, original
outcome and source identity. A run without errors retains normal score/exit rules.

The observer/protocol recipe changed, so historical execution identities are
not silently accepted for focused execution. Historical reports remain readable
and unchanged; fresh runs are compared by mutation source identity. There is no
report-schema or operator-catalog version bump.

## Verification

Fixture controls cover default behavior, limits 1/2/10, two body errors followed
by a kill and survivor, mixed assertion/runtime failures, bounded saved logs,
explicit rerun opt-in, no-error scoring and pristine-baseline rejection. Separate
controls retain a pending candidate after cleanup, capture and per-file failures.
A cancellation probe first completes an eligible error, then interrupts a running
candidate and verifies the interrupted report and removal of owned workspaces.

Protocol controls corrupt and remove later evidence after an eligible exception
so positive body evidence cannot conceal a broken lifetime. Native exception
controls distinguish real subclasses, same-named unrelated classes, resource
exceptions, multiple inheritance and genuine assertions. Existing native body,
unittest, fixture, shared-lifetime, capture/file-limit and descendant-cleanup
controls remain in place.

The default suite passes **282 tests** with 99 opt-in ignores. The three new
real continuation workflows pass (123.83 seconds). Six existing native
lifecycle/resource workflows pass (90.99 seconds), and the additional seven-case
exception-type workflow passes (5.27 seconds). The latter adds one opt-in ignore
after the full-suite run. Formatting and all-target Clippy checks also pass.

## Unchanged-project comparison and reproduction

The unchanged 198-test betting-math-kit project completed **all 194 arithmetic
candidates in one run**: **61 kills, 97 survivors, 36 errors, no timeouts and no
unfinished candidates**. Every outcome matches the preserved segmented review by
exact source identity. The run took **843.768 seconds**, exited 1, and retained a
failed status and null score despite complete inventory accounting. Its effective
error limit was 64, with minimum-score and missed-finding policies both enabled.

The [results ledger](fixtures/python-candidate-continuation/results.json) and
[saved summary](fixtures/python-candidate-continuation/summary.txt) retain the
accounting and checks. Full runtime, scope, test inventory and limit metadata
match the previous acceptance; only the execution recipe differs. An attempted
historical focused rerun correctly failed before any candidate executed
(2.881 seconds). All original inputs, prior stronger copies, historical ledgers
and finalized reports passed independent preservation verification.

The acceptance driver initially checked for the word “recipe” in the rejection
diagnostic. The actual message is “Python execution identity changed; start a
fresh run.” Its text check was corrected; the independent verifier separately
establishes that the recipe is the only identity difference. No CLI behavior,
classification, assertions or report results were changed for this correction.


The [driver](fixtures/python-candidate-continuation/run.py) runs the complete
198-test unchanged betting-math-kit suite for all 194 arithmetic candidates. It
copies the built CLI into the output directory to keep the validation binary
stable. It compares each completed outcome with the preserved previous review by
file, fingerprint, operator, span and replacement identity, not report-local IDs.

```sh
python3 -B benchmarks/fixtures/python-candidate-continuation/run.py \
  --project /tmp/mutie-python-betting-math-original \
  --python /tmp/mutie-pytest-audit-env/bin/python \
  --mutie target/debug/mutie \
  --output /tmp/new-python-candidate-continuation \
  --previous /tmp/mutie-python-arithmetic-full/results.json \
  --initial /tmp/mutie-python-arithmetic-acceptance/results.json
```

The driver checks original Git cleanliness and file hashes, all previous ledgers,
original report bytes and prior stronger copies before/after commands. It also
checks historical recipe rejection and retains the new finalized report hashes.
The independent [verifier](fixtures/python-candidate-continuation/verify.py) takes
the new output directory, original project and prior full ledger as positional
arguments. It reopens actual report files, compares full runtime/scope/inventory
and limit metadata (only the recipe may differ), and verifies each outcome.
Candidate/baseline timeouts remain 30/60 seconds. Comparisons are single observed
runs, not controlled performance claims; no historical outcomes or scores are
rewritten.
