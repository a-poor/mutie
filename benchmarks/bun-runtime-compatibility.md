# Bun 1.4.4 alongside 1.3.11

Mutie accepts two exact Bun runtime pairs: **1.3.11+af24e281e** and
**1.4.4+38afee6c8**. This expands the runtime set for the existing scoped JavaScript
and TypeScript runner. It does not add dependency imports, package layouts,
custom commands, skips or general Bun configuration.

The [Elysia native baseline](bun-elysia-baseline.md) motivated this work: four
unchanged tests fail on the older pinned runtime but pass on 1.4.4. Elysia itself
still fails Mutie's project admission. Its broader compatibility is separate.

## Attribution and identity

Bun 1.3.11 labels every reported body exception `AssertionError`. Bun 1.4.4 retains
the exception's name and includes its message/stack in JUnit. Merely changing the
accepted version left 18 otherwise valid body-failure controls classified as
errors. The older reader remains strict about its original report format; the
newer reader accepts nonempty exception labels only alongside complete positive
body/checkpoint evidence. Labels never supply phase attribution.

A body exception named `TimeoutError` is a kill when its body failure completes.
An actual timeout has an unfinished body and no provisional failure checkpoint.
Hook failures, pending rejections, process errors, unsupported APIs, truncated
records and incomplete inventories remain errors. The observer itself is unchanged.

`bun/runtime.rs` defines the exact version/revision pairs. Private runtime probes,
saved identities and observer initialization must agree on the selected pair;
accepting any known pair independently at each stage would be insufficient.
Executable bytes remain checked before and after execution. Changing the runtime
or recipe requires a fresh run. Existing report schemas and historical scores are
unchanged; saved text summaries now display the observed runtime version.

## Native controls and scored workflows

The retained [audit script](fixtures/bun-runtime/audit.py) runs the original
56 cases plus six exception-name controls independently for JS and TS, both with
and without the production observer. The production Rust reader replays all
**124 cases on each runtime**. Each runtime yields 16 passes, 30 body failures,
two timeouts and 76 conservative errors, with matching native/observed exit codes.
These are expected classifications of controls, not a project mutation score.

The [1.3.11 records](fixtures/bun-runtime/bun-1.3.11.json) and
[1.4.4 records](fixtures/bun-runtime/bun-1.4.4.json) preserve expected outcomes,
observer events, complete native reports and sanitized captures. Raw-record hashes
refer to pre-sanitization evidence. New negative controls reject mixed runtime
evidence, unknown revisions, unfinished checkpoints, contradictory terminals and
process errors. The prior 112-case replay and malformed-evidence tests still pass.

Both runtimes pass the real adapter's JS/TS outcome matrix and resource/process
cleanup controls, plus all five scored workflow groups: reports/stronger tests,
review/policies, lifecycle failures, capture/file limits and descendant shutdown.
No deadlines, output limits or cleanup assertions were relaxed.

## Real projects on Bun 1.4.4

The full original tests run for every candidate. Only mutation sources/operators
are selected; no tests, packaging files or configurations are trimmed. These are
fresh reports with the new runtime identity, not reruns of historical 1.3.11 reports.

| Project / selection | Killed | Survived | Errors | Original selection score |
| --- | ---: | ---: | ---: | ---: |
| pp-is: all 22 boolean sites | 12 | 10 | 0 | 54.55% |
| Composer: two `src/utils.ts` boolean sites | 1 | 1 | 0 | 50% |

Unchanged pp-is at `1d5b3bac804e1e67da289f3d2a14bd0705a383fb` passes its 58-test
baseline. A separate copy with the [retained seven stronger tests](fixtures/bun-pp-is/stronger.test.js)
passes 65 tests; a focused rerun kills seven of the ten survivors and leaves three
open. The counts and stronger-test findings match previous records. This does not resolve
the ambiguous `isEmpty` fallback or redundant primitive-boolean cases.

Unchanged Composer at `3de15b846e3f5ef46cf8aebd5d136908baa43b54` passes 272 tests.
The `configurable: true` mutation survives. The separate copy adds the
[middleware-renaming test](fixtures/bun-audit/composer-stronger.test.ts), passes
273 tests, and kills that survivor. This two-site selection is not a full Composer
mutation score or a re-review of its other historical findings.

Attempting each new report's rerun under 1.3.11 fails identity validation before
baseline or mutation execution. The failed diagnostic report has no score. All
63 pp-is and 35 Composer original inputs, all original report files and their
scores are preserved. [Results](fixtures/bun-runtime/results.json) retain run IDs,
source identities, exact timings, report hashes and the preservation checks.

## Verification and remaining work

The ordinary suite passes 390 tests; formatting and Clippy pass. The Linux workflow
now has separate Bun 1.3.11 and 1.4.4 matrix entries, with the existing workflow,
operator and native semantic checks on both. The new matrix is configured locally
and has not run remotely. Runtime support does not broaden the separately audited
Bun 1.3.11 **package-manager** installation contract.

Next for Elysia: complete build/command attribution and rebuilt-artifact identity,
owned dependency/nested-package support, the large inert asset, JSONC/bundler TS
configuration and explicit original skip dispositions. Its native success is a
prerequisite for that work, not scored Mutie acceptance.
