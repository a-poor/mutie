# Temporary Developer Tools experiment

This comparison exercised enabled and disabled Developer Tools permission states
on macOS. The enabled phase used ordinary sandboxed command execution. Permission
state was reported manually and was not independently verified; no test deadline
or assertion changed.

Three full suites passed 278 tests each (93 ignored each); ten focused rounds
passed all 110 tests. CLI and selected test source hashes matched before/after.

The second phase disabled the permission. It did not establish exact restoration
of the pre-experiment state, and no independent restart verification was recorded.

The `restored` directory names the disabled-permission comparison, not a verified
restoration of the absent entry. All three full suites passed 278 tests each
(93 ignored), and all ten focused rounds passed 110 tests. Hashes matched within
both phases and between them. The same overlapping workload ran in both phases.

Both conditions passed; this experiment is inconclusive about the intermittent
stall and establishes no remedy. It does not justify retaining the permission.
Caching, prior launches and unverified effective permission state remain limits
of this sequential, user-observed comparison.
