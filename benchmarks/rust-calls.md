# Scoped Rust standalone-call removal

Rust now supports `call_remove` within the existing package/workspace scope.
No package, report-schema or operator-catalog version changes are needed.
Python call removal and arbitrary statement/block removal remain separate work.

## Scope and semantics

A whole semicolon-terminated ordinary call statement becomes `{}`. This includes
free functions, associated functions, methods, function pointers and closures,
including parenthesized calls. The empty block preserves a statement boundary
without adding redundant-semicolon warnings. Calls used as values and tail calls
without semicolons are excluded. See the Rust Reference's
[expression statements](https://doc.rust-lang.org/reference/statements.html#expression-statements),
[call expressions](https://doc.rust-lang.org/reference/expressions/call-expr.html)
and [method calls](https://doc.rust-lang.org/reference/expressions/method-call-expr.html).

Removing the whole statement skips receiver, argument and invocation effects.
It does not preserve argument evaluation or insert a replacement invocation.
Values no longer consumed by the call follow Rust's remaining scope/drop rules;
for example, removing `drop(value);` can delay destruction until scope exit.
Native compiler checks still apply. Removing the only type constraint or a needed
initialization can produce a build failure; warnings promoted to errors also
remain build failures. No type inference, casts, warning suppression or outcome
reclassification is introduced.

The entire call subtree must be free of macro invocations, attributes, `await`,
`?`, `yield`, `return`, `break` and `continue`. This deliberately conservative
rule also applies inside closure arguments. Macro expansion could hide those
constructs, and suspension/error propagation/control transfers need separate
acceptance. Existing macro-token, cfg/test, generated-source, constant/type and
other discovery exclusions remain. A call containing `vec![]` is excluded even
when that macro looks harmless; no allowlist or expansion is inferred.

Saved readers reparse the complete original statement and require the exact `{}`
recipe, a Rust source path and normal source-identity rediscovery. Arbitrary
replacements, multiple statements, prefixes/suffixes and unsupported calls cannot
enter focused reruns. Shared scheduling, classification, cleanup and report
formats are unchanged.

## Controls and fixture

`src/rust/call_tests.rs` checks exact spans, comments, Unicode/CRLF, nested blocks,
qualified/parenthesized/closure calls, exclusions, deterministic ordering, reparsing,
source restoration and corrupted saved recipes. Eight paired native controls
compile and execute original and mutated programs with warnings denied. They
cover receiver/argument/body effects, owned values, explicit drop, FnOnce captures,
function pointers, generics, ignored Result values and a following closure-call
statement. Every command has a 30-second deadline and a 64 KiB capture bound.

The independent [fixture](../tests/fixtures/rust-calls/README.md) has three original
tests and three candidates: **one kill, one survivor and one build failure**.
The build failure removes the only element-type constraint from a vector.
Fresh serial execution and two-worker dependency-cache execution agree exactly;
build failures are excluded from the 50% score. A stronger `weak(true)` assertion
kills the survivor in a focused rerun with a 100% minimum-score policy.
The workflow also rejects corrupt recipes/paths and changed source identities,
preserves caller artifacts and all original report bytes, and verifies source-ID
mapping. It passes in 23.31 seconds; timing is an observation, not a performance
comparison.

## Unchanged real-project acceptance

The unchanged [UCD Generate](https://github.com/BurntSushi/ucd-generate) workspace at
`e8fa937f0cac643669dcaf5edac2785b15cab917` provides real-project evidence. This
revisits the previously validated public workspace rather than claiming a new
project layout. All **108 tracked paths** remain unchanged.

The original full-workspace baseline passes **140 tests**. Selecting
`ucd-parse/src/common.rs` in member `ucd-parse` finds five call-removal candidates.
Every candidate still tests all four workspace members. Two workers with
worker-local dependency reuse finish in **33.38 seconds**: **three kills, two
survivors**, no build failures, errors or timeouts. The **60% score** applies only
to the selected file/operator.

| Original ID | Removed call | Missing behavior check |
| --- | --- | --- |
| 1, line 23 | `xs.push(x);` | The file-level parser must retain parsed rows in order, rather than returning an empty vector. |
| 2, line 42 | `map.insert(cp, x.clone());` | The codepoint map must contain each single codepoint and every member of a range, with its parsed record. |

A separate copy adds only
[`file_collections.rs`](fixtures/rust-calls-ucd/file_collections.rs). Its two tests
create owned temporary UCD input directories, call public parsing APIs, and check
exact row and map contents. They add no dependencies or production changes and
clean up only their own directories.

The stronger **142-test baseline passes**. A focused rerun kills both survivors
in **19.21 seconds**, without build failures, errors or timeouts. Its focused
100% score does not replace the original filtered 60% score. The three other
original candidates were already killed; there are no unresolved survivors in
this five-candidate selection.

The [verifier](fixtures/rust-calls-ucd/verify.py) checks all original tracked paths
in both copies, the clean original checkout, the sole added test, all **36 original
saved-report files**, full-workspace test counts and report-local source identities.
The [results ledger](rust-calls-results.json) retains inventory, outcomes, scores,
timings and hashes without workstation-specific paths. Local evidence is under
`/tmp/mutie-rust-calls-audit`.

## Verification and reproduction

The default suite passes **342 tests**, zero failures, with **170 opt-in ignores**.
Native call controls, the scored call fixture and the existing Rust binary-package
and workspace workflows pass. Formatting, all-target Clippy with warnings denied and whitespace
checks pass. The Linux Rust job now includes the native call controls and scored
fixture; the new CI configuration has not yet run remotely. Recorded native
acceptance is on macOS with Rust/Cargo 1.99.1. Passing tests do not resolve the
explicitly deferred macOS startup investigation.

```sh
cargo test --locked --bin mutie rust::discovery::call_tests -- --include-ignored
cargo test --locked --test rust_workflow calls:: -- --ignored --test-threads=1
cargo test --locked --no-fail-fast -- --test-threads=2
cargo clippy --locked --all-targets -- -D warnings
cargo fmt --check
git diff --check
```

For the real-project run, place this config outside the original checkout (paths
are relative to the config):

```toml
jobs = 2
timeout = 60
baseline_timeout = 180
output_dir = "original-reports"
[rust]
path = "original"
packages = ["ucd-parse"]
include = ["ucd-parse/src/common.rs"]
operators = ["call_remove"]
build_cache = "worker"
```

```sh
mutie run --config /path/to/audit/original.toml --plain
# Copy the unchanged project to stronger/, adding only the retained test.
# Use another config with path="stronger" and output_dir="stronger-reports".
mutie rerun /path/to/audit/original-reports/RUN --missed \
  --config /path/to/audit/stronger.toml --plain
# With the preservation manifests recorded before the stronger run:
python3 -B benchmarks/fixtures/rust-calls-ucd/verify.py \
  /tmp/mutie-rust-calls-audit benchmarks/rust-calls-results.json
```
