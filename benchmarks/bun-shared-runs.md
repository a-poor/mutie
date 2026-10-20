# Bun shared scored runs and focused reruns

Recorded 2026-10-19. JavaScript and TypeScript are now independently Experimental
within the pinned dependency-free scope. This is not Validated support.

## Implementation and fixture evidence

Explicit `--lang bun` connects serial full-suite boolean discovery, isolated
execution, saved reports, scores and focused reruns. Runtime bytes, package/config
and the observer/execution recipe are persisted and checked on reruns. Stronger
tests, executable relocation and budget changes are allowed; every command within
a run must still match its independently collected test inventory. Original and
snapshot executable inputs must agree, so an output-directory exclusion cannot
hide a selected test. Raw capture has a mandatory 4 MiB default budget; existing
Unix 8 MiB per-file limits are not a total disk quota.

`tests/bun_workflow.rs` independently exercises JS and TS: each fixture produces
one kill and one survivor, saves a completed report, and returns the missed-policy
exit code. A new test file kills the survivor through `--rerun-missed` with a 100%
selection score. Original production bytes and the source report are preserved.
Missing saved identity and changed config are rejected. A separate mutation that
triggers body and cleanup failures produces an error, zero kills and an unavailable
score. Default tests reject unsupported options without launching Bun and reject
report exclusions that would hide tests. Native workflow tests remain opt-in.

Validation: `cargo test --offline`, the ignored JS/TS workflow and cleanup tests,
`cargo fmt --check`, and `cargo clippy --offline --all-targets -- -D warnings` pass.
The initial native test invocation exposed two test assertion mistakes (conflicting
preview/output flags and an incorrect score-field name); both were corrected and
the affected tests rerun successfully.

## Unchanged Composer and stronger tests

Project: GramIO Composer at `3de15b846e3f5ef46cf8aebd5d136908baa43b54`.
The installed Bun had changed to 1.4.3, which the adapter correctly rejected.
These runs used a separate official Bun 1.3.11+af24e281e executable whose SHA256
matches the earlier audit; the user's installed runtime was unchanged.

The full CLI run used `--lang bun --timeout 8 --baseline-timeout 15`. All 272
baseline tests passed. All 12 candidates were attempted in 106.14 seconds:

- Killed: IDs 1, 2, 3, 7, 10, 12.
- Survived: IDs 4, 5, 6, 8, 11.
- Timed out: ID 9 (`src/queue.ts`, initial active flag).
- Execution errors: zero. Score: 6 / 11 = 54.55%; the timeout is excluded.

A separate copy added only the retained
[`composer-stronger.test.ts`](fixtures/bun-audit/composer-stronger.test.ts).
Its 273-test baseline passed and `--rerun RUN_DIR --id 11` killed the
`src/utils.ts:7` name-configurability survivor in 15.86 seconds overall.
The selected score is 100%; the original full-run score remains unchanged.

All 35 original project file hashes match in both original and stronger-test
copies. Every original report file hash is unchanged after the rerun.
[Machine-readable evidence](bun-shared-runs-results.json) retains identities,
outcomes, timings and preservation hashes. Local report paths are diagnostic
references, not portable artifacts. Timings include debug-build identity hashing;
individual captured command durations exclude preparation/identity overhead.

## Remaining acceptance

Four Composer survivors and the queue timeout remain unreviewed. Independent
real JavaScript acceptance, Bun-specific review/dismissal/policy coverage and
scored cancellation/capture/descendant-cleanup regressions remain gates. Existing
baseline diagnostic and observer probes do not substitute for those scored-path
checks. No parallel workers, installed dependencies, custom Bun configuration,
preloads or multi-package support is claimed.
