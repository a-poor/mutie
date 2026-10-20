# Python feedback profiling

Use the pinned CPython 3.14.7 / pytest 9.0.2 venv and Scrubify commit
`558265805d942c03d2e6d8522baa0ca2992ec067`. Prepare a fresh original six-test
project with `../python-scrubify/prepare.py`; do not add the stronger tests to
that input. The comparison script copies it into separate owned projects.

Preserve the pre-change development executable before building the candidate.
For the historical comparison the reference is commit `24aff2b`; both builds
use the repository's existing optimized `sha2` development profile.
Build all executables first, then measure serially without concurrent Cargo
checks or other profiling processes:

```sh
python3 benchmarks/fixtures/python-profile/run.py \
  --project /path/to/prepared-scrubify \
  --before /path/to/reference-mutie \
  --after target/debug/mutie \
  --python /path/to/pinned-venv/bin/python \
  --output /path/to/new-comparison-directory
```

Three pairs alternate build order. Each pair runs the full three-mutation
inventory, adds the retained blank-query tests and runs `--rerun-missed` using
that build's own full report. It verifies matching inventories, outcomes,
runtime hashes and pristine test IDs across builds; it checks every input and
all original report files after the stronger-test rerun. Recipe hashes may
differ: observer implementation bytes remain part of execution identity.
Cross-recipe reruns are intentionally rejected; start a fresh run after upgrading.

`results.json` retains external wall time, exact commands, report paths,
identities, inventories and preservation hashes. Compilation, copying the
comparison projects and verification are outside the timed CLI invocation.
These are repeated warm-filesystem samples, not cold-cache or large-project
benchmarks. The source checkout and prepared input are never edited.

An ignored test measures actual adapter phases, including every repeated runtime
and project validation, the isolated copy, collection and test processes:

```sh
MUTIE_PYTHON=/path/to/pinned-venv/bin/python \
MUTIE_PYTHON_PROFILE_PROJECT=/path/to/prepared-scrubify \
  cargo test --offline --bin mutie python::profile::real_feedback_phase_profile \
  -- --ignored --exact --nocapture
```

It prints three `python-profile` JSON records and verifies passing baselines,
matching identities and preserved inputs. Timers exist only in test builds.
The adapter total includes cleanup on return; phases stop at diagnostics, so
the small remaining time is return/cleanup overhead. This excludes the CLI's
initial admission, coordinator and saved-report work. To compare the prior
walker with identical timer sites, build a test binary with these timer sites
and the `runtime.py` from `24aff2b`, preserve it, then restore/build the candidate.
Use each binary's own identity; the recipe is expected to change.

The standard-library profiler can identify work within one runtime helper:

```sh
/path/to/pinned-venv/bin/python -I -S -B \
  -X pycache_prefix=/path/to/new-empty-cache \
  -m cProfile -s cumulative src/python/runtime.py
```

Profiling itself adds overhead. Do not compare its elapsed time with unprofiled
CLI runs or run it concurrently with the measurements above.

To isolate traversal from subprocess startup, export the previous trusted helper
and alternate both walkers within one pinned isolated interpreter:

```sh
git show 24aff2b:src/python/runtime.py > /path/to/reference-runtime.py
/path/to/pinned-venv/bin/python -I -S -B \
  -X pycache_prefix=/path/to/new-empty-cache \
  benchmarks/fixtures/python-profile/runtime_profile.py \
  --before /path/to/reference-runtime.py --after src/python/runtime.py \
  --output /path/to/runtime-profile.json
```

This requires equal complete runtime identities first, then checks equal stdlib
and package hashes on each of five alternating pairs. Only the two tree scans
are timed; import/startup, executable/library/config hashing and distribution
metadata are outside this calibration. Run it separately from the CLI samples.
