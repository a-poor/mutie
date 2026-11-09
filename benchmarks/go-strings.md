# Go string literals

`string_replace` now supports interpreted and raw Go runtime string literals.
Each eligible literal produces one change: nonempty becomes `""`, empty becomes
`"x"`. Replacements retain Go string-constant semantics. Raw literals use the
canonical quoted replacement; identity and writes preserve original source bytes.

## Scope and semantic controls

`mutation/string.rs` validates the complete token without evaluating source.
Interpreted escapes follow Go's byte and Unicode rules; NULs, arbitrary byte
escapes and valid Unicode scalars are nonempty. Raw literals discard carriage
returns, so a backtick literal containing only CRs is empty. An actual CR inside
an interpreted string remains nonempty. See the [Go string rules](https://go.dev/ref/spec#String_literals).

Imports, struct tags, named constants, types/array lengths, keyed composite keys,
switch case values, rune literals, tests and generated files remain excluded.
Variable initializers, arguments, returns, closures, composite values and runtime
lookups remain eligible. Numeric and string discovery share the same value-context
helper; existing numeric semantic controls still pass.

**32 cases under LF and CRLF give 64 native original/mutant pairs**, checking exact
bytes through real Go compilation and execution. Ordinary and defined string
return types, slicing, raw multiline strings, CR deletion, NULs, invalid UTF-8 byte
values and Unicode boundaries are covered. Two original/mutant compiler controls
keep empty-string indexing and division by `len("")` as build failures. Eight
malformed-source controls reject literal NUL/BOM, invalid scalar/octal escapes,
escaped apostrophes and line continuation. Commands have a 30-second deadline
and 256 KiB capture bound. No compiler, vet or warning policy is changed.

Static tests check context exclusions, UTF-8/CRLF coordinates, exact writes,
double-application rejection and strict recipes. Saved Go string records require
a `.go` path and the exact regenerated replacement. Fresh discovery rejects
forged excluded constant spans; changed source fingerprints reject reruns.
No package, report-schema or catalog version was bumped.

## Scored Go workflows

The [fixture](../tests/fixtures/go-strings/README.md) has four candidates:
one kill from a dependent package, one raw-empty-string survivor and two compiler
failures. Full and affected modes agree at **50% raw score**. Affected selection
includes the root and its reverse dependent while preserving the full pristine
baseline. Minimum-score failure exits 3; dismissing the survivor satisfies findings
policy without changing raw outcomes or score. A separate stronger copy kills
the survivor in a focused affected run and preserves its source/dismissal mapping.

Corrupt replacements/operators/extensions, forged contexts and source drift are
rejected. Project files and original reports remain preserved. Separate probes
keep a four-second timeout and capture overflow outside kills and passing scores.
The original Go workflow now includes its newly discovered greeting string,
including updated default counts and saved source IDs; no helper hides the operator.

## Unchanged godotenv audit

[godotenv v1.5.1](https://github.com/joho/godotenv/tree/3fc4292b58a67b78e1dbb6e47b4879a6cc602ec4)
at `3fc4292b58a67b78e1dbb6e47b4879a6cc602ec4` is an unchanged checkout of an
existing validation project. External configuration selects `string_replace`
without source-file filters. All **20 tracked inputs** are preserved; the
**43-test baseline passes**. One frozen CLI is used for every successful run.

| Run | Candidates | Killed | Survived | Build failed | Seconds |
| --- | ---: | ---: | ---: | ---: | ---: |
| Original, full | 39 | 16 | 19 | 4 | 12.18 |
| Original, affected | 39 | 16 | 19 | 4 | 9.81 |
| Stronger survivor rerun, full | 19 | 4 | 15 | 0 | 6.70 |
| Stronger survivor rerun, affected | 19 | 4 | 15 | 0 | 6.07 |

There are no timeouts or execution errors. Full/affected inventories, identities
and per-candidate outcomes match. All four build failures come from format
strings emptied while arguments remain: Go's normal test/vet checks reject them.
They stay outside the denominator. The original **45.71%** score applies only
to this operator selection, not all project mutations. Timings are single local
runs with other verification active, not a performance comparison.

Four [retained public-API tests](fixtures/go-strings/contracts_test.go) in a
separate complete copy establish observable missing assertions:

| Original ID | Missing contract |
| --- | --- |
| 11 | `Write` terminates the final serialized line with a newline. |
| 21 | CRLFs inside quoted multiline values normalize to LF rather than disappear. |
| 26 | An unquoted empty value at EOF stays empty and retains its key. |
| 39 | A standalone dollar sign survives parsing without a variable name. |

The stronger **47-test baseline passes**. All 19 original survivors are rerun in
both test scopes; four are killed and **15 remain open**: IDs **1–10, 22, 24, 25,
28, 38**. They include untested CLI behavior and parser error-return/expansion
branches. No equivalence or dismissal is claimed. The focused **21.05%** score
is not a full stronger-suite score; original reports and scores remain unchanged.

The [verifier](fixtures/go-strings/verify.py) checks all original input hashes in
both copies, the sole added test file, all **172 original** and **172 affected**
report hashes, full/affected outcomes and report-local source-ID mapping.
The [results ledger](go-strings-results.json) retains these checks and hashes
without machine paths. Raw evidence remains in the temporary
`mutie-go-strings-audit` directory. A sandboxed initial attempt could not access
the Go build cache; its failed baseline is retained separately, with no candidates
attempted and no score. Successful runs used approved host-cache access.

## Verification and reproduction

The default suite passes **369 tests**, with **227 opt-in ignores**. All **11
real Go workflows**, the affected-package regression, **22 Go discovery/native
tests** (including existing numeric/assignment controls), formatting, all-target
Clippy with warnings denied and workflow lint pass locally. Sandbox process
inspection restrictions initially blocked two cancellation controls; both pass
with host access, as does the final complete suite. Linux CI now includes the string
native controls and scored workflows; remote validation awaits a push. The
deferred macOS pre-main startup investigation remains open.

```sh
cargo test --locked --bin mutie mutation::string::tests -- --include-ignored
cargo test --locked --test go_workflow --test affected_packages -- --ignored --test-threads=1
```

Place this external config beside the unchanged checkout named `original`:

```toml
jobs = 2
timeout = 15
baseline_timeout = 60
output_dir = "original-reports"
[go]
path = "original"
operators = ["string_replace"]
```

```sh
mutie run --config original.toml --plain
# Repeat with test_scope="affected" and a separate output_dir.
# Copy all inputs to stronger/ and add contracts_test.go as string_contracts_test.go.
# In stronger.toml, use path="stronger", output_dir="stronger-reports".
mutie rerun "$ORIGINAL_REPORT" --missed --config stronger.toml --plain
# Repeat the same saved selection with affected mode and its own output_dir.
python3 -B benchmarks/fixtures/go-strings/verify.py "$AUDIT_ROOT" results.json
```

The verifier expects this artifact layout and preservation manifests captured
before stronger runs. Bounded numeric and string literal parity is complete.
Templates and broader recipes remain separate; routine project compatibility
and feedback are the next roadmap milestone.
