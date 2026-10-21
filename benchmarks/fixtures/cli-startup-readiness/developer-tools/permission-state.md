# Temporary Developer Tools experiment

The user reported that Codex was absent from Developer Tools, then added and
enabled it before the enabled workload on 2026-10-21. This state is user-reported;
UI automation is unavailable. The enabled phase ran with ordinary sandboxed
command execution. No test deadlines or assertions were changed for this phase.

Three full suites passed 278 tests each (93 ignored each); ten focused rounds
passed all 110 tests. CLI and selected test source hashes matched before/after.

Removal of the added Codex entry was requested immediately after completion.
The user instead confirmed disabling it and authorized the comparison. The entry
remains present: the original absent-entry configuration was not exactly restored.
No independent UI verification or restart confirmation is available.

The `restored` directory names the disabled-permission comparison, not a verified
restoration of the absent entry. All three full suites passed 278 tests each
(93 ignored), and all ten focused rounds passed 110 tests. Hashes matched within
both phases and between them. The same overlapping workload ran in both phases.

Both conditions passed; this experiment is inconclusive about the intermittent
stall and establishes no remedy. It does not justify retaining the permission.
Caching, prior launches and unverified effective permission state remain limits
of this sequential, user-observed comparison.
