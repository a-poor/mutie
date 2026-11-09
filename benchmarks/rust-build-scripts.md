# Owned Cargo build scripts and unchanged ripgrep

This extends ordinary Rust packages/workspaces to project build scripts, inactive
custom release profiles, and explicit inactive target flags. It follows the
[six-project compatibility audit](compatibility-audit.md), whose ripgrep rejection
and native controls remain historical evidence. No package, report-schema or
operator-catalog version changes were made.

## Execution and identity contract

- `build.rs`, an explicit in-package `package.build` path, `build=true` and
  `build=false` follow Cargo's target inventory. Metadata must match the selected
  script. All workspace members' scripts are recorded, even when mutations target
  only one member. Build scripts and generated code stay outside discovery.
- Cargo runs in the owned source copy with owned target/build directories. Worker
  caches clean **every local package** before each candidate, including build-script
  executables, fingerprints and generated outputs. Registry dependency reuse stays
  worker-local. Compilation and testing share the existing deadline.
- Saved scope records script paths/bytes and a build recipe. A conservative
  fingerprint binds snapshot input files and executable modes. Package `tests/`
  directories are excluded from this initial rerun fingerprint so integration
  tests can be strengthened. Other file changes require a fresh scan, including
  changes to inline tests in production files.
- The pristine baseline records ordered non-warning Cargo directives, hashes of
  declared `rerun-if-changed` files, declared environment values, and generated
  output contents. Owned paths in textual outputs are normalized; only package
  names and hashes are saved. After a rerun baseline, this identity must match
  before mutations begin. An undeclared read of a changed test file is caught if
  it changes the generated output. Explicitly declared test inputs remain bound.
- Successful compilation must leave snapshot input files unchanged. Generated
  evidence is checked again after the test command; changes between compilation
  and tests are errors. Missing or external declared inputs, linked generated
  outputs and incomplete/duplicate Cargo script evidence are rejected. Each input
  or output identity pass is bounded to 256 MiB and 20,000 files.
- A script's runtime failure remains an error. Invalid generated Rust with Cargo
  compiler-error evidence is `build_failed`; test kills still require the standard
  harness failure evidence. Timeouts, capture failures and cancellation retain
  their existing precedence. Running descendants after Cargo exits are errors for
  this scope, and owned process groups are stopped on exit/timeout/interruption.

Cargo executes trusted project and dependency code; this is not an OS sandbox.
Scripts must keep outputs in `OUT_DIR`. Undeclared host files, external tool drift,
network access and nondeterministic generation are not portable build inputs.
Binary outputs containing temporary absolute paths may conservatively invalidate
a rerun. `.git` is omitted from copies. A script that requires Git metadata is
outside this slice; ripgrep's optional Git-version fallback works without it.
No Git hash is invented or injected into the copied project.

Custom profile chains must terminate at `release` or `bench`; dev/test overrides
remain rejected. Cargo config may contain explicit target-triple `rustflags`
arrays only when the actual `rustc -vV` host differs. `cfg(...)` targets, active
flags, custom runners, cross-compilation and proc-macro project members remain
outside scope. These settings are retained and bound, not removed from upstream.

These checks follow Cargo's documented [build-script output/lifetime rules](https://doc.rust-lang.org/cargo/reference/build-scripts.html)
and [profile selection/inheritance](https://doc.rust-lang.org/cargo/reference/profiles.html).

## Native regression evidence

`tests/fixtures/rust-build/` generates Rust, cfg and env settings from data and
environment inputs. Its script rejects a marker left in `OUT_DIR` by a previous
build. Fresh, serial cached and two-worker cached runs agree on one kill and one
survivor; a new integration test kills the survivor on a focused rerun. Script
and data drift reject before mutation execution; environment and test-dependent
generated-output drift reject after the pristine baseline.

The ignored `build_scripts::` tests also cover an unselected workspace member's
custom script path, source/report/artifact preservation, script panics, output
overflow, generated compiler failures, declared external input, linked output and
source-write rejection, plus a sparse oversized-output control that exercises
the identity-reader budget without allocating a large buffer. Separate real-process timeout/SIGINT probes observe the
script, shell and descendant live before checking their shutdown and cleanup.
They are opt-in to prevent recursive mutation runs during Mutie's self-tests.
Linux CI now invokes these controls explicitly; this change does not claim a
completed hosted CI run.

## Unchanged ripgrep

Revision: [`178cc027e283fd965cb72d0a7395b219eada170c`](https://github.com/BurntSushi/ripgrep/tree/178cc027e283fd965cb72d0a7395b219eada170c).
All 226 tracked inputs are retained, including the root build script, release
profile inheritance, platform-specific Cargo config and full workspace suite.
The initial audit had already run native controls with and without Git metadata;
both passed 1,209 tests, with one ignored test across 14 targets.

The new isolated `mutie check` passes in 18.80 seconds. A bounded selection of
`boolean_flip`, `number_replace` and `string_replace` in `grep-cli`'s
`human.rs`/`process.rs` yields 16 candidates. Every candidate runs the complete
workspace suite. The worker-cached run completes in 329.77 seconds: six kills,
ten survivors, no timeouts, build failures or errors. Its 37.5% score describes
only this filtered selection, not ripgrep as a whole.

Four public-API integration tests in a separate complete copy then check trimmed
stderr framing, partial close/idempotence, nonzero exit at EOF, and last-byte/EOF
behavior. The pristine baseline passes 1,213 tests with one ignored test across
15 targets. The ten-survivor focused run takes 192.70 seconds (30.89-second
baseline), killing seven with no errors, timeouts or compiler failures:

| Original IDs | Finding caught by the stronger tests |
| --- | --- |
| 8, 9 | Lost stderr framing/separator width |
| 10 | Treating a partial read as EOF makes close report an error |
| 11 | A closed stream reports an extra byte |
| 12 | A one-byte read is mistaken for EOF |
| 13 | Nonzero exit with empty stderr is suppressed at EOF |
| 14 | EOF reports a phantom byte |

Original IDs 7, 15 and 16 remain survivors: an I/O-error fallback and two internal
panic-message changes. No equivalence or dismissal claim is made. The original
37.5% score remains unchanged; the focused 7/10 result is not a full stronger-test
score for the 16 candidates or for ripgrep.

A fresh-artifact rerun on the unchanged copy selects original IDs 1, 8, 10 and
13. It takes 100.42 seconds and exactly matches the cached outcomes: one kill,
three survivors, no other outcomes. After the final shared-build-script discovery
guard was added, the final release binary repeats original ID 10 against the
stronger copy: a 1,213-test baseline and a kill in 42.63 seconds. Execution scope,
including build inputs and outputs, matches all four reports.

The [portable result ledger](rust-build-scripts-results.json) records commands,
binary hashes, baseline counts, source identities, outcomes and report hashes.
Verification checks all 226 original inputs, all original paths in the stronger
copy, its one additional test file, and all 80 files in the original report.
The earlier six-project audit was reverified separately: 1,239 upstream inputs
and all 160 original / 60 stronger-test SJSON report files remain unchanged.

## Validation

The default suite passes 374 tests (232 opt-in tests ignored). All five native
build-script workflows pass, along with the existing backdated-source Cargo cache
regression. Targeted Rust unit tests pass after the final discovery guard. Formatting,
Clippy with warnings denied, workflow validation with actionlint, and the Python
evidence scripts' syntax checks pass. Native tests remain opt-in:

```sh
cargo test --locked --test rust_workflow build_scripts:: -- --ignored --test-threads=1
```

Tools for the real-project evidence: Rust/Cargo 1.99.1 on macOS arm64. The full
workspace command remains `cargo test --locked --tests --no-fail-fast --workspace`
with Cargo JSON messages, ordinary default features, and no doctests/ignored tests.
Linux execution is delegated to the added CI step and is not inferred from macOS.

Raw artifacts are kept outside the repository. The retained
[runner, verifier and stronger tests](fixtures/rust-build-scripts/README.md)
make the input/report preservation checks reproducible. Timings include other
development-host work and are not controlled performance comparisons.
