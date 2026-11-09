# Go numeric literals

Validated on 2026-11-09 with Go 1.27.2 and Rust/Cargo 1.99.1 on macOS arm64,
based on `65ea511`. [Results and hashes](go-numbers-results.json) retain source
identities, full/affected inventories, outcomes and stronger-test evidence.
Project, report-schema and operator-catalog versions are unchanged.

## Scope and recipe

`number_replace` emits one candidate per eligible integer/float literal: exact
zero becomes one, nonzero becomes zero. Integer and float constant kinds stay
distinct (`7 → 0`, `0.0 → 1.0`). Integer radices/separators, decimal floats and
hexadecimal floats are supported. Direct unary `+`/`-` belongs to the same site,
including intervening comments; its child is not duplicated. Parenthesized sign
operands retain the outer sign and mutate the inner literal.

Go constants are exact values, without IEEE negative zero, infinities or NaNs.
Their kind controls the default type of untyped literals; conversion to a
floating-point type introduces rounding. See the Go specification's
[constants](https://go.dev/ref/spec#Constants),
[representability](https://go.dev/ref/spec#Representability) and
[floating-point literals](https://go.dev/ref/spec#Floating-point_literals).

Mutie's native probes distinguish constant `-0.0` from runtime negation of a
zero variable. Zero detection inspects mantissa digits. Nonzero floats that round
to zero as `float32` are conservatively excluded, even where a surrounding
`float64` or exact expression could distinguish them. Decimal checks use f32
rounding; hexadecimal checks compare the exact mantissa/exponent to half the
smallest subnormal, excluding the tie that rounds to zero. Thus `1e-999` and
`0x1p-150` are excluded, while exact `0x0p-999` becomes `1.0`. Large exact
constants remain eligible; Mutie does not infer types or repair invalid code.

Named constant declarations, types and array lengths, keyed composite-literal
keys, expression-switch case values, rune/imaginary literals, tests and generated
files are excluded. Package variable initializers, runtime indexes/slices,
composite values, switch bodies and closures are eligible. These are syntactic
limits, not a proof that every mutation changes observable behavior.

Saved reports must contain an exact regenerated recipe and a `.go` source path.
Fresh discovery also checks the original source context and fingerprint, rejecting
forged excluded-constant spans. All twelve Go operators now run by default;
existing operator filters can select a narrower inventory.

## Native and scored controls

Static tests check UTF-8/CRLF coordinates, exact edits, direct-sign ownership,
constant/type/key/case exclusions, underflow boundaries and malformed recipes.
**33 paired native cases** cover integer width limits, large exact integers,
radices, separators, named/default types, signed zero,
decimal/hex floats, subnormal ties and precision loss. Values beyond f32 range
remain valid f64 values. Two further exact-constant controls exceed f64 range
but become representable through constant division; their zero replacements
must still change the result.

Three original/mutant compiler controls preserve constant division by zero,
out-of-bounds array indexing and shift overflow as build failures. Four tiny
constant controls verify typed f32 zero alongside nonzero exact multiplication.
The generic conversion control runs both f32 and f64 instantiations. Each native
command has a 30-second deadline and a 256 KiB capture bound.

The [scored fixture](../tests/fixtures/go-numbers/README.md) has four candidates:
**one kill, one survivor and two build failures**, for a **50%** raw score.
Full and affected-package results agree. A consumer package supplies the kill;
an unrelated package remains in the full baseline but outside affected mutation
tests. Minimum-score failure exits 3. Dismissal review preserves raw outcomes
and score. A separate stronger copy kills the float survivor, retaining the
source-ID and review mapping. Corrupt recipes/operators/extensions, forged
constant spans and changed sources are rejected. Genuine timeouts and output
overflow remain outside kills; all original inputs and reports are preserved.

## Unchanged godotenv findings

The unchanged [godotenv v1.5.1 checkout](https://github.com/joho/godotenv/tree/3fc4292b58a67b78e1dbb6e47b4879a6cc602ec4)
is an existing validation project. An external config selects `number_replace`
without file filters. The full **43-test baseline passes**.

| Run | Candidates | Killed | Survived | Errors / timeouts / build failures | Seconds |
| --- | ---: | ---: | ---: | --- | ---: |
| Original, full tests | 40 | 25 | 15 | 0 / 0 / 0 | 16.68 |
| Original, affected tests | 40 | 25 | 15 | 0 / 0 / 0 | 15.29 |
| Stronger copy, all prior survivors | 15 | 3 | 12 | 0 / 0 / 0 | 12.34 |

Full and affected runs agree by source identity and outcome. The original
**62.50%** score covers the selected operator only. Timings include other local
validation activity and are not a performance comparison.

Three new public-API tests in a separate copy expose concrete parser behavior:

| Original ID | Changed literal | Stronger assertion |
| --- | --- | --- |
| 8 | CRLF replacement count `-1 → 0` | All CRLFs inside a quoted multiline value normalize to LF. |
| 30 | Missing-newline sentinel `-1 → 0` | An unterminated quoted value at EOF returns its diagnostic without panicking. |
| 39 | Variable-name capture index `4 → 0` | A literal `$` survives parsing when there is no variable name. |

The **46-test stronger baseline passes**. All 15 original survivors are rerun;
these three are killed and 12 remain open: IDs **1, 2, 3, 13, 16, 22, 23, 28,
31, 32, 34, 36**. They include CLI argument handling and parser boundaries or
potentially redundant operations. No equivalence or dismissal is claimed.
The focused score is **20%**, not a full stronger-suite score; the original
reports and scores remain unchanged.

The retained [tests](fixtures/go-numbers/parser_test.go) and
[verifier](fixtures/go-numbers/verify.py) check all **20 tracked inputs** in both
copies, the sole added test, full/affected outcome agreement and source-ID
mapping. All **176 original** and **176 affected** report files retain their
hashes. Raw evidence stays in the temporary `mutie-go-numbers-audit` directory.
The first sandboxed attempt could not access the host Go build cache; its failed
baseline is retained separately and attempted no candidates. The successful runs
used approved cache access.

## Verification and reproduction

The default suite passes **361 tests**, zero failures, with **208 opt-in ignores**.
All **nine real Go workflows**, the real affected-package regression, numeric
native controls, formatting, all-target Clippy with warnings denied, workflow
lint and whitespace checks pass. Default rerun expectations now include the new
numeric sites rather than filtering them out through a helper.
Linux CI includes the native checks and scored workflows; remote validation
remains pending until push. The deferred macOS pre-main startup issue stays open.

```sh
cargo test --locked --bin mutie mutation::number::tests -- --include-ignored
cargo test --locked --test go_workflow --test affected_packages -- --ignored --test-threads=1
```

Place this config beside an unchanged checkout named `original`:

```toml
jobs = 2
timeout = 15
baseline_timeout = 60
output_dir = "original-reports"
[go]
path = "original"
operators = ["number_replace"]
```

```sh
mutie list --config original.toml --json
mutie run --config original.toml --plain
# Repeat with test_scope="affected" and a separate output_dir.
# Copy to stronger/ and add parser_test.go as numeric_parser_test.go.
# In stronger.toml, use path="stronger", output_dir="stronger-reports".
mutie rerun "$ORIGINAL_REPORT" --missed --config stronger.toml --plain
python3 -B benchmarks/fixtures/go-numbers/verify.py "$AUDIT_ROOT" results.json
```

The verifier expects this campaign's artifact layout and preservation manifests
captured before the stronger rerun. Bounded numeric literals now have
cross-language parity; broader numeric recipes and string literals remain open.
