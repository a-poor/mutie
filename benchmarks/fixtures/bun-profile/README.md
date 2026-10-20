# Bun feedback profiling

Use unchanged pp-is at `1d5b3bac804e1e67da289f3d2a14bd0705a383fb` and put the
separate pinned Bun 1.3.11+af24e281e runtime first on PATH. Do not install project
dependencies. Reports must be outside the project.

Build all artifacts before measuring, then run the following sequentially without
concurrent Cargo checks or other benchmark runs:

Current development/test builds optimize `sha2`. To reproduce the historical
fully unoptimized debug measurements, use
`cargo --config 'profile.dev.package.sha2.opt-level=0' build --offline` and the
same override for the debug test builds/probes below. Preserve that binary before
switching profiles. Ordinary builds now measure the adopted optimization.

```sh
cargo build --offline
cargo build --offline --release
cargo test --offline --no-run --bin mutie
cargo test --offline --release --no-run --bin mutie
python3 benchmarks/fixtures/bun-profile/run.py \
  --project /path/to/pp-is \
  --debug target/debug/mutie \
  --release target/release/mutie \
  --output /path/to/new-profile-directory
MUTIE_BUN_PROFILE_PROJECT=/path/to/pp-is cargo test --offline --bin mutie \
  bun::profile::real_feedback_phase_profile -- --ignored --exact --nocapture
MUTIE_BUN_PROFILE_PROJECT=/path/to/pp-is cargo test --offline --release --bin mutie \
  bun::profile::real_feedback_phase_profile -- --ignored --exact --nocapture
```

The script creates a new output directory, runs one full inventory per build,
then three serial one-candidate selections per build in alternating order. Each
selection includes a full baseline and a full test suite for the candidate.
It asserts matching inventory, outcomes and runtime identity between builds and
checks all source hashes after every invocation. It saves reports and exact CLI
commands, with external wall time excluding compilation and preservation checks.

The ignored test prints three `bun-profile` JSON lines. Actual adapter phases are
measured by test-only timers; they exclude initial scope resolution, the separate
warm-file read/hash calibration, and CLI coordinator/report work. Phase times
are components of adapter total, not additional work to sum with it. The first
scope resolution is recorded once (repeated in each line for context). The
calibration separates file read, executable SHA-256 and recipe SHA-256 costs but
is not a cold-cache experiment. Profile tests require a passing complete baseline
and preserve original source fingerprints.

No production timers, profile CLI flags, report schema changes, identity caching
or reduced validation are introduced. The Bun recipe hashes `bun.rs` source bytes,
so adding test-only timer call sites changes the recipe fingerprint; use fresh
reports from the same source revision when comparing builds.

For the historical hash-only compiler experiment, preserve the fully unoptimized debug
binary first, build with `cargo --config 'profile.dev.package.sha2.opt-level=3'
build --offline`, and copy that binary to a temporary path. Run the comparison
script with that path as `--debug`, plus `--focused-only --modes debug` and a new
output directory. Restore the ordinary build with `cargo build --offline` after
measuring. The level-3 setting is now the repository default; the command-line
override does not edit project configuration.
