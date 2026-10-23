# Scoped Python logical swaps

Python now supports `logical_swap` in binary `and`/`or` expressions, including
nested binary nodes. Discovery replaces only the native keyword, preserving
operand bytes, comments and parentheses. It introduces no helpers, Boolean
coercion or duplicated operands. Short-circuit decisions intentionally change;
results can be raw objects, containers, numbers or strings. Newly evaluated
operand failures and truthiness exceptions retain the existing error policy.

## Scope and semantics

Only `ast.BoolOp` nodes with exactly two values are eligible. Flat chains such as
`a and b and c` are excluded at the chain node. They require a separate grouping
policy: swapping one join must not silently regroup the chain, and synthesizing a
binary fold can introduce different truthiness calls. An eligible nested binary
node inside a longer chain remains independently eligible. Bitwise `&`/`|`, test
and conftest modules, annotations and type-parameter expressions are excluded.

For the binary scope, changing a keyword can merge neighboring same-operator
nodes in the reparsed AST. Native controls compare the edited source against the
intended original AST operator change rather than assuming textual precedence is
safe. Across 13 nested/mixed expression forms and four execution contexts, they
compare complete operand/truthiness traces and returned raw values for 16 truth
assignments and four state/exception modes. More than 4,000 paired executions
pass, including stateful/toggling truthiness, second-call failures and a failing
right operand. Contexts cover direct values, values inside lists, conditions and
outer negation. Both versions use the pinned CPython compiler.

The complete 59 scope/semantic tests pass. Additional controls cover UTF-8 and
CRLF byte spans, comments, adjacent punctuation, f-string expressions, raw-value
identity, annotation/test exclusions and flat-chain exclusion. Rust unit tests
validate exact `and ↔ or` pairs, variable-length byte edits and stale-source
rejection. The shared recipe matcher also validates saved reports; focused reruns
still require fresh AST discovery and execution identity. No schema/catalog or
package version changes are made.

## Scored fixture

The [fixture](../tests/fixtures/python-logical/README.md) has five original tests
and six swaps: **four kills and two survivors**, with no errors or timeouts. The
survivors exercise equal operands, so `and` and `or` return the same value. Two
stronger tests distinguish the values and fallback behavior; the seven-test
baseline passes and the focused rerun kills both survivors.

The production workflow rejects corrupted saved operator labels and keyword
replacements, and preserves source bytes and original report outcomes. A separate
scored control makes a formerly skipped right operand raise ValueError: it remains
an error with zero kills. Both ignored workflow tests pass in 55.08 seconds.

## Unchanged Route Mini findings

The unchanged original remains `/tmp/mutie-python-route-mini-original`.
The selected file is `src/route_mini/router.py`; every run executes all 68 original
tests. Four logical sites are selected. Across the initial run and separate
original-input reruns for its pending candidates, the inventory accounts for
**two survivors and two runtime errors**, without timeout or build failure.
This combined accounting is not a full score. The first error remains fatal even
with bounded continuation because the existing policy does not admit that failure
as safely continuable; no classification is broadened to finish the inventory.

| Original ID | Line | Change | Original outcome |
| ---: | ---: | --- | --- |
| 1 | 16 | Exception diagnostic `decision.error or fallback` | Survived |
| 2 | 31 | Default fallback object | Error |
| 3 | 32 | Default telemetry object | Error |
| 4 | 105 | Last provider error `last_error or fallback` | Survived |

Both survivors discard a specific provider diagnostic and substitute the generic
`fallback exhausted` message. The original tests check error type, reason and
attempt count but miss the diagnostic text. This matters to callers displaying or
logging the exception and to consumers of the saved decision's error field.

A separate copy adds only
[diagnostic contract tests](fixtures/python-logical/test_fallback_diagnostics.py).
They check preservation of a supplied message and decision object, fallback text
for None/empty messages, and a real route ending after one provider-down failure.
The route test asserts one provider call and one attempt before checking both the
decision diagnostic and exception text. No original test/assertion is changed.

## Execution identity and host caveats

The first stronger-copy attempt was rejected before candidates because a Rust
unit-test edit changed `project.rs`, which participates in Python's execution
recipe. That rejected report and the preceding original reports are preserved.
Final acceptance uses a frozen final CLI and fresh original reports; old reports
are not rewritten or grandfathered through the identity check.

Some validation launches produced no immediate output. One bounded sample caught
the stronger run after it had entered main; it does not prove a pre-main stall for
that launch. The existing host startup investigation remains deferred. No security
setting, test deadline, assertion or production spawning rule was changed.

## Final acceptance and preservation

The frozen final build repeats the original four-site accounting with the same
outcomes: ID 1 survives, ID 2 is a fatal error, and separate original-input reruns
classify IDs 3/4 as error/survivor. The original selection takes 22.11 seconds;
the pending-ID reruns take 15.98 and 16.54 seconds. IDs in the focused reports are
local; the saved source IDs and source identities map them back to the table.
The failed reports remain unscored. The successful ID 4 original-input rerun has
its own selected 0% score; it does not turn combined accounting into a full score.

The final stronger copy passes **72 baseline tests** and kills **both saved
survivors (original IDs 1 and 4)** in **16.39 seconds**, without error or timeout.
Its 100% selected score does not replace any original report. The two constructor
runtime errors remain unresolved; there are no unreviewed survivors in this
four-site selection. This is not whole-project logical coverage.

All **25 original tracked inputs** remain Git-clean and byte-identical to the
pre-run snapshot. The stronger copy retains every original input and adds just
one test file. All **145 earlier report files**, including exploratory and rejected
reports, match their hashes from before the final stronger rerun. Current recipe
bytes match the final original, pending and stronger execution identities exactly.
[Machine-readable evidence](python-logical-results.json) retains hashes, commits,
inventories, outcomes, source selections and timings.

Validation completed:

- 320 default tests passed (136 opt-in tests ignored).
- All 18 configuration tests passed after consolidating the identical operator
  admission branches for Python and JS/TS.
- 59 Python scope/semantic tests and both scored logical workflows passed.
- `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and
  `git diff --check` passed.

The initial unit build caught a test-only call to a nonexistent restoration API;
the test now checks the actual apply/stale-source contract. The corrected full
suite passed. Long silent startup delays did not require relaxing any assertions.

## Reproduction

All recorded artifacts are under `/tmp/mutie-python-logical-audit/`.
`mutie-final` is the frozen final CLI; `final-original-reports`,
`final-pending-3-reports`, `final-pending-4-reports` and `final-stronger-reports`
contain the final execution evidence. Earlier directories retain the exploratory
recipe and its correctly rejected stronger rerun.

```sh
target/debug/mutie /tmp/mutie-python-route-mini-original \
  --lang python --python /tmp/mutie-pytest-audit-env/bin/python \
  --include src/route_mini/router.py --operator logical_swap \
  --continue-on-error 10 --timeout 20 --baseline-timeout 30 --ui plain \
  --output-dir /tmp/new-python-logical-original

# Use the newly saved report for pending IDs 3 and 4 individually, then use
# its IDs 1 and 4 against a copy with the retained diagnostic tests added.
target/debug/mutie /tmp/mutie-python-logical-audit/stronger \
  --lang python --python /tmp/mutie-pytest-audit-env/bin/python \
  --rerun /tmp/mutie-python-logical-audit/final-original-reports/run-1792715914-7qQQHi \
  --id 1,4 --timeout 20 --baseline-timeout 30 --ui plain \
  --output-dir /tmp/new-python-logical-stronger
```

Use a fresh original report if the recipe changes; old identity checks remain
mandatory. No project source or original tests need adapting.
