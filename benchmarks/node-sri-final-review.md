# SRI remaining-findings review

Recorded 2026-10-18, using Node 24.22.0 and the unchanged body-attribution recipe.
Project: [sri-dynamic-loader](https://github.com/subresource-integrity/sri-dynamic-loader)
at `370652c304d6fee0c59cfb51bf097e72f6ebf13a`. Source IDs refer to
`run-1792371294-hkKpgW` in [the preceding audit](node-body-attribution.md).

## Cleanup guard (source ID 9)

`src/loader.js:322` changes cleanup's `settled = true` to false. Five additional
[public-behavior tests](fixtures/node-sri/mutie-cleanup.test.js) cover:

- Script and stylesheet success followed by an error event, abort and deadline.
- Error, abort and timeout failure followed by later load/error events.
- Preserved successful elements, removed failed elements and detached listeners.

The original 60 tests, earlier five stronger tests and these five tests pass as a
70-test baseline. A saved-ID rerun still reports **one survivor, no errors or
timeouts** (5.70s saved elapsed; baseline 2.08s, mutant 2.10s). Separate direct Node
runs of these five tests plus the completion probe pass on both pristine source
and the exact ID 9 mutation.

Cleanup removes the load/error/abort listeners and clears timeout/poll handles.
That prevents normal later events from reaching the callbacks whose guard was
changed. Resolving a Promise again is also unobservable through its result, but
`fail()` additionally removes the element, so Promise idempotence alone is not
an equivalence proof. No removed callback was captured and invoked artificially,
and no cleanup API was replaced with a no-op to force a kill.

**Disposition: reviewed survivor, equivalence unproven.** These DOM-stub probes do
not prove behavior for every browser, queued-task ordering or reentrant host
object. No dismissal was created and the raw result remains missed. A browser
ordering/reentrancy investigation would be a separate validation scope, not a
reason to broaden the Node runner here.

## Initially settled flag (source ID 8)

`src/loader.js:315` changes `let settled = false` to true. Both `succeed()` and
`fail()` immediately return; neither resolves/rejects the promise nor runs cleanup.
The original full suite still times out with a **10-second** command budget,
compared with the prior 5-second result. The focused run took 12.46s including a
passing 0.96s baseline and a 10.02s timed-out mutant. It has no scored candidates.

The retained [standalone completion probe](fixtures/node-sri/mutie-completion.test.js)
queues a successful load event, uses a 20ms loader timeout and observes the promise
with a separate 200ms deadline. Pristine source completes; the exact ID 8 mutation
fails an assertion with `still pending` instead of `loaded`. Both processes exit
normally after their timers finish, without force-exit or unhandled failures.
The ID 9 control also completes normally. This isolates a semantic hang from
ordinary machine slowness without relying on elapsed-time precision.

**Disposition: explained timeout, not a scored kill.** The standalone diagnostic
runs only its own file; the full original suite remains hung. Adding a test that
fails earlier does not provide full-suite completion. Mutie correctly preserves
timeout precedence; no runner classification change is warranted.

## Reproduction and preservation

Copy `mutie-cleanup.test.js` into a review checkout's `test/` directory alongside
the earlier `mutie-stronger.test.js`, then run with the audited Node on PATH:

```sh
mutie REVIEW --lang node --rerun ORIGINAL_RUN --id 9 --timeout 5 --ui plain
mutie ORIGINAL --lang node --rerun ORIGINAL_RUN --id 8 --timeout 10 --ui plain
# In separate disposable copies, apply each exact saved mutation to source,
# copy the completion probe into test/, and run only that diagnostic:
node --test test/mutie-completion.test.js
```

[Machine-readable evidence](node-sri-final-review.json) records report identities,
results, source hashes and diagnostic exits. Original source and original tests
remain unchanged; earlier reports are preserved. No Mutie production code,
classification rule, support scope or version changed for this review.
