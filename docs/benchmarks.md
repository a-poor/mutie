# Benchmark records

The [benchmarks directory](../benchmarks/) records what was tested, what failed,
and which stronger tests caught previously surviving mutations. Each acceptance
note links its scripts, fixtures and results. Upstream projects are fetched
separately; retained excerpts are covered by the [third-party notices](../THIRD_PARTY_NOTICES.md).

Published records have been sanitized: home and checkout paths use `/path/to/`
placeholders, temporary paths use `/tmp`, and hostnames, hardware counts, exact
host OS builds, disk identifiers and sampled memory addresses are redacted.
Platform families and toolchain versions remain where they describe compatibility.
Test outcomes, scores, timings, relative source paths and mutation identities
are retained.

These are historical evidence summaries, not portable saved runs. Recorded hashes
refer to the original audit inputs, reports and captures. A sanitized transcript
will no longer match its original byte hash; those hashes have not been rewritten
to imply a new execution. Reproduction scripts should be run with fresh checkouts,
explicit local paths and fresh output directories. Historical absolute paths and
run IDs are not expected to exist on another machine.

Some real-adapter checks are ignored by the default Cargo test suite. See the
[development guide](development.md) and the individual acceptance notes for their
required runtimes and environment variables. An unresolved failure stays recorded
even if a later retry passes.
