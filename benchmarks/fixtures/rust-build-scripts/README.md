# Cargo build-script acceptance

`run.py` bounds and retains real Mutie commands. It expects a frozen release CLI,
complete tracked-source copies of the ripgrep revision recorded in the benchmark,
and external TOML configuration. It does not edit upstream code or install tools.
Copy `ripgrep.toml` next to the source copies as `config.toml`; the stronger copy
uses `path = "stronger"`, and the fresh control uses `build_cache = "fresh"`.
`verify.py` checks complete source inventories, the original saved report hashes,
matching execution identity, and original-source fresh/cached outcomes before
exporting the portable result ledger.

`command_reader_contracts.rs` is the test-only addition to a separate ripgrep
copy. It checks command-reader EOF, partial close and stderr diagnostics through
public APIs. No upstream production code is included here.

See [acceptance and limits](../../rust-build-scripts.md). Native regressions live
in `tests/support/rust_build_scripts.rs` and `tests/fixtures/rust-build/`; the
Linux Cargo CI job runs them explicitly. The recorded real-project timings are
observations from a shared development host, not isolated performance benchmarks.
