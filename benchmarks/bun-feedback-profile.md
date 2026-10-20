# Bun execution overhead profile

Recorded 2026-10-20 on macOS arm64, Rust 1.99.1. Uses unchanged pp-is commit
`1d5b3bac804e1e67da289f3d2a14bd0705a383fb`, pinned Bun 1.3.11+af24e281e and
Mutie based on `44b109d` plus test-only phase observations. No production
optimization, identity caching or default build-profile change is made.

## Build-mode comparison

All binaries were built before timing. Runs were serial, without concurrent
Cargo tests or compilation. The full 22-candidate inventory ran once per build;
the one-candidate selection (`src/main/isAlphanumeric.js`) ran three times per
build in alternating order. Every invocation has a complete fresh baseline and
full test suite, five-second mutation timeout and 15-second baseline timeout.
External wall times exclude builds and hash-preservation checks.

| Workload | Debug | Release |
| --- | ---: | ---: |
| All 22 mutations (one run each) | 306.646s | 19.763s |
| One candidate, repeat 1 | 24.077s | 2.113s |
| One candidate, repeat 2 | 23.118s | 2.261s |
| One candidate, repeat 3 | 23.114s | 2.132s |
| One-candidate median | 23.118s | 2.132s |

Release is 15.5× faster for this full run and 10.8× faster for the median focused
selection. These are build-mode comparisons, not a before/after code optimization
or universal speed guarantee. The full run has one sample per build; focused
repeats reduce order effects but do not control unrelated host activity, CPU
frequency or cold filesystem caches.

Both full runs match the earlier unchanged-layout inventory and all outcomes:
12 killed, ten survivors, zero errors/timeouts, 54.55% score. All focused runs
reproduce the one survivor and zero-percent selected score. Inventories, execution
identities and all 63 original file hashes match across builds. Reports are fresh;
historical reports were not rerun with a different recipe or modified.

## Actual baseline phases

`bun::profile::real_feedback_phase_profile` runs three real isolated baselines per
build. Each passes the complete 58-test suite with matching identity and original
input hashes. Test-only timers split the actual adapter path; no timing fields
are added to production CLI output or saved report schemas.

| Actual adapter phase, median | Debug | Release |
| --- | ---: | ---: |
| Original project validation | 0.0503s | 0.0142s |
| Isolated workspace copy | 0.0166s | 0.0165s |
| Copied-project validation | 0.0484s | 0.0210s |
| Private environment setup | 0.0006s | 0.0008s |
| Runtime identity subprocess | 0.0301s | 0.0371s |
| Identity capture before test | 4.4221s | 0.2838s |
| Observer setup | 0.0004s | 0.0003s |
| Test process and capture | 0.0984s | 0.0841s |
| Identity capture after test | 4.4310s | 0.2778s |
| Evidence classification | 0.0049s | 0.0008s |
| Diagnostic rendering | 0.0005s | 0.0005s |
| Owned workspace cleanup | 0.0048s | 0.0042s |
| **Adapter total** | **9.1957s** | **0.7432s** |

Phases are components of adapter total; do not add them to it. Independently
computed medians need not sum exactly. CLI scope resolution, coordinator/report
writes and outer worker copies are outside these adapter totals. The first scope
resolution is recorded separately in the results, not repeated work in each
adapter sample.

A separate warm-file calibration measures the same 61,069,216-byte executable:

| Calibration median | Debug | Release |
| --- | ---: | ---: |
| Read executable bytes | 0.0149s | 0.0110s |
| SHA-256 executable bytes | 4.4419s | 0.2539s |
| Recipe fingerprint | 0.0170s | 0.0008s |

The dominant cost is executable SHA-256 computation, not copying or process
startup. Before/after identity captures account for roughly 96% of debug adapter
time and 76% of release adapter time. This successful full workload performs one
initial scope fingerprint plus two per baseline/mutation command: 47 complete
executable fingerprints. They are existing drift checks, not profiling overhead.
Calibration is separate from adapter timing and warms filesystem caches; these
numbers do not estimate cold-disk latency.

## Hash-only compiler experiment

To check the cause without removing any verification, built a temporary debug
binary with `cargo --config 'profile.dev.package.sha2.opt-level=3' build --offline`.
Only the SHA-256 dependency received a compiler optimization override. No
`Cargo.toml` or local persistent config was edited. Three focused runs took
3.595s, 2.425s and 2.439s (median 2.439s), about 9.5× faster than ordinary debug.
All retained the identical survivor, inventory and execution identity. The first
sample was slower; three samples do not establish a general timing distribution.
The ordinary default debug binary was subsequently restored and its bytes checked
against the original benchmark binary.

**Recommendation:** use release builds for real mutation feedback now. If faster
development-mode execution is needed, the measured next change is a separately
reviewed `sha2` development-profile override. Runtime identity resolution reuse
would save much less on this project. Keep executable byte checks and full suite
scope; this evidence does not justify metadata-only caching or fewer validations.
No production cache, parallel worker, schema/catalog/package version change or
permanent profile override is part of this profiling work.

## Reproduction and validation

The [serial comparison script and instructions](fixtures/bun-profile/README.md)
retain commands and timing boundaries. [Machine-readable results](bun-feedback-profile-results.json)
include every CLI timing/outcome, phase sample, runtime/binary identity and input
hash. Local paths identify diagnostic artifacts, not portable dependencies.

Adding test-only timer call sites changes the source-hashed Bun recipe. Both
builds and all experiments used the same updated recipe; saved runs from the
prior recipe remain valid historical records but cannot be used as current rerun
inputs. The instrumentation is compiled out of ordinary binaries.

Validation passed: the ignored phase probe in debug and release, `cargo test
--offline`, `cargo clippy --offline --all-targets -- -D warnings`, and `cargo fmt
--check`. The serial comparisons verified matching inventories, outcomes and
execution identities, plus preservation of all 63 project input hashes.
