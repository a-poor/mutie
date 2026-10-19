# Second Node project: openclaw-cc

Recorded 2026-10-18 on macOS with audited Node 24.22.0. This acceptance uses
[openclaw-cc](https://github.com/yukiharukonishi/openclaw-cc) at
`e323faa446f03b5438fa9edc58781b4ef8dd52df`, unchanged from upstream, as a second
JavaScript project for the existing experimental Node scope. No dependencies were
installed and no manifest, source layout, production source or original test was
adapted. The package's exact test command is `node --test test/*.test.js`.

## Scope and baseline

Seven test files pass **62 tests**, both directly with native serial Node and
through Mutie's isolated baseline. The isolated baseline takes 5.06s. The tests
inject agent, fetch and message-delivery fakes; file operations use temporary
state directories. The application entry point is discovered but is not imported
by these tests. This validates the package's test workflow, not live Telegram,
Claude CLI execution or the application's unattended service behavior.

The discovery inventory contains 39 boolean flips across polling, scheduling,
delivery, session management, storage, workspace loading and the entry point.
Each mutation command uses the complete suite, one worker, fresh isolated copies,
a 15-second deadline and the unchanged pinned body-attribution recipe.

## Full-inventory result

The original full run completed all 39 candidates in **255.44s**: **22 killed,
15 survivors, two timeouts, zero errors**. The raw boolean-mutation score is
**59.46%** (22/37), excluding the two timeouts. This is a complete discovered
boolean inventory, not coverage of all possible mutation operators or behaviors.

The survivors are source IDs 3, 5, 6, 10, 12, 13, 15, 16, 18, 21, 28 and 34–37.
The review below validates three useful findings. At this checkpoint twelve others remained open;
the [storage/profile follow-up](node-storage-profiles.md) later kills five, leaving
seven unreviewed. None has been dismissed or established equivalent. In particular, the
unimported entry point, storage locking and workspace-profile boundaries should
not be inferred covered from a passing baseline.

## Findings reviewed with stronger tests

[Three retained tests](fixtures/node-openclaw/mutie-stronger.test.js) are added only
to a separate review copy. They exercise public `pollOnce()` with injected fakes:

| Source ID | Original survivor | Observable expectation |
| --- | --- | --- |
| 3 | `processing = true` changed to false | Two overlapping calls issue one outstanding fetch; release the pending fake in `finally` even on assertion failure |
| 5 | `sessionPersistence !== false` changed to compare against true | Explicit false passes `noSessionPersistence: true` to the agent |
| 6 | Empty allowlist returns true | A message from an unlisted chat invokes neither the agent nor message sender |

The empty-list test is a missing boundary test for access control; it does not
establish an upstream vulnerability because the original implementation correctly
rejects the message. Likewise, these tests detect hypothetical regressions, not
three existing production bugs. Test teardown stops the poller and removes its
temporary state; no network or message service is contacted.

The review copy passes a **65-test baseline**. A saved-ID rerun of 3, 5 and 6
then produces **three kills, no survivors/errors/timeouts**, in **24.50s**.
Its 100% score applies only to those three candidates; the original report and
its 59.46% score remain unchanged.

## Timeouts and failure-safe teardown

Source IDs 2 and 4 change the poller's initial or final `processing` flag to true.
The original logs already contain failing assertions, but one upstream test checks
`poller.processing` before calling `poller.stop()`. A failed assertion leaves its
60-second interval alive, so the test process does not complete within 15 seconds.
Mutie's timeout result is correct; a printed assertion alone cannot earn a kill.

A separate cleanup review copy keeps all source, manifests and other original
files unchanged and applies this retained [two-line teardown patch](fixtures/node-openclaw/teardown.patch):
accept the test context and register `t.after(() => poller.stop())` before starting
the poller. The existing assertions stay intact. This copy does not include the
three extra boundary tests, isolating cleanup's effect.

Its **62-test baseline passes**, and a saved-ID full-suite rerun of 2 and 4 finishes
with **two kills, no errors/timeouts**, in **18.01s**. This establishes the leaked
interval as the completion blocker for these runs. Successful hook evidence is
still required. The original full report remains 22/15/2; these two kills are a
separate selection and are not silently merged into a new whole-project score.

## Preservation and checks

All 63 original files retain their hashes in the original and stronger-test
checkouts; the latter adds only the retained three-test file. The cleanup copy
changes only the one named test via the retained patch. Original full-report
hashes are checked after the stronger-test rerun. Native serial Node passes 62
tests; isolated review baselines pass 65 and 62 respectively. The SRI review also
preserves its original files and 442 earlier report files. Formatting and diff
checks pass; no Rust implementation changed, so the default Rust suite was not
rerun for these evidence/test-artifact additions.

## Reproduction

With audited Node on PATH:

```sh
mutie ORIGINAL --lang node --ui plain --timeout 15 --baseline-timeout 30
# Copy the retained test into REVIEW/test/; keep every original file unchanged.
mutie REVIEW --lang node --rerun ORIGINAL_RUN --id 3,5,6 --ui plain --timeout 15
# Separately apply teardown.patch to another original copy:
mutie CLEANUP_REVIEW --lang node --rerun ORIGINAL_RUN --id 2,4 --ui plain --timeout 15
```

[Machine-readable evidence](node-openclaw-results.json) records the pinned package,
runner recipe, complete discovered inventory, outcomes, saved selection and
preservation checks. Focused scores apply only to their selected mutations.
This does not validate TypeScript, installed dependencies, dynamic imports,
additional Node versions or worker parallelism.

## Candidate selection limits

Other candidates were inspected without adapting their manifests or fixtures:
`review-order` is rejected for nested fixture packages (and also contains TypeScript
fixtures); `docex` declares CommonJS; `onp-spec-driven` quotes its test glob and
therefore does not match the current exact-script preflight. These are scope
exclusions, not failing mutation campaigns. The adapter was not broadened merely
to make a validation project fit.
