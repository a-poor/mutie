# Go assignment removal

Validated on 2026-11-09 with Go 1.27.2 on macOS arm64 and the working tree based
on `1316dfc`. [Results and hashes](go-assignments-results.json) retain every candidate.

## Scope and semantics

`assignment_remove` deletes an entire single-target selector/index `=` statement
directly in a block or case body. This covers fields, slice/array elements and map
entries, including parenthesized targets. All receiver, key, RHS and store effects
are skipped, consistent with the two evaluation phases in the
[Go assignment specification](https://go.dev/ref/spec#Assignment_statements).

Names, declarations, multiple targets, compound assignments, increments,
pointer-only stores, labels, headers and select receive clauses are excluded.
Go selectors are syntactic: a package-qualified variable can qualify, and ordinary
initialization functions have no special constructor exclusion. Removing the only
use of an import or local can make a mutant invalid. We retain that build failure
instead of inserting references or changing other statements.

Saved reports require a complete eligible snippet and an exact empty replacement.
Reruns also rediscover the full original source, rejecting a valid snippet forged
into an excluded header. Other operators, source hashes and byte coordinates stay
part of identity. No report, catalog or package version changed.

## Regression controls

- Static controls cover eligible block/case statements, exclusions, UTF-8/CRLF,
  exact edits/diffs and wrong saved recipes.
- Sixteen native original/mutant pairs cover receiver/key/RHS call order, struct
  and generic fields, arrays, array pointers, slices, maps, aliases, nil/bounds/
  unhashable-key panics, receiver/key/RHS panics, buffered channel receives and
  deferred cleanup inside an RHS closure. Mutations skip every removed effect.
- Scored fixtures produce one kill, one survivor and two build failures (unused
  local/import). Full and affected selection agree; build failures do not inflate
  the 50% score. A minimum-score policy exits 3. Review and a separate stronger
  copy preserve original reports and kill the survivor. Altered recipes/operators,
  a forged header and stale source are rejected.
- Deliberate timeout and output overflow remain non-kills; overflow produces a
  failed report, no passing score, and exit 1 despite the score policy. Go's
  existing panic/test-failure classification is unchanged.

The full default suite passed **352 tests** (185 opt-in tests ignored). The native
assignment controls and all **eight real-Go workflow/selection tests** passed.
Formatting, Clippy with warnings denied, and workflow lint passed. Linux CI now
includes the native pairs; scored assignment workflows run in the existing Go job.
The preceding JS/TS commit passed [all nine Linux jobs](https://github.com/a-poor/mutie/actions/runs/42652787604).
These new Go changes have local validation; their remote CI is pending commit/push.

## Unchanged godotenv v1.5.1

The fresh upstream checkout is pinned to
[`3fc4292`](https://github.com/joho/godotenv/tree/3fc4292b58a67b78e1dbb6e47b4879a6cc602ec4).
All 20 tracked inputs, its manifest and its full 43-test baseline are unchanged.
External config selects only `assignment_remove`, without file filters, using two
workers and a 20-second mutation deadline. Both full-module and affected-package
runs keep a full baseline.

| ID | Assignment removed | Full / affected |
| --- | --- | --- |
| 1 | `godotenv.go:101`, `envMap[key] = value` | Build failed / build failed |
| 2 | `godotenv.go:138`, `command.Stdin = os.Stdin` | Survived / survived |
| 3 | `godotenv.go:139`, `command.Stdout = os.Stdout` | Survived / survived |
| 4 | `godotenv.go:140`, `command.Stderr = os.Stderr` | Survived / survived |
| 5 | `godotenv.go:194`, `currentEnv[key] = true` | Build failed / build failed |
| 6 | `parser.go:40`, `out[key] = value` | Build failed / build failed |

The three invalid candidates leave `key` and/or `value` unused. That is **50%
build-failure noise** in this six-candidate inventory, not evidence of coverage.
The original filtered score is 0/3, with no errors or timeouts. Full and affected
runs took 2.11 and 2.59 seconds; these are warm-cache observations, not a benchmark.

The [new test](fixtures/go-assignments/streams_test.go) calls the public `Exec` API
with the current test binary as a helper child. It verifies forwarded input and
both output streams through temporary files, restores process-global streams,
and requires neither a shell nor a terminal. It lives only in a separate copy.
The 44-test stronger baseline passes; all three survivors are killed in a focused
rerun taking 1.70 seconds. Its 100% focused score does not replace the original
0% filtered score. No equivalence or broader effectiveness claim is made.

All original input hashes and all 40 original report-file hashes still match.
The stronger copy differs only by the added test; the verifier also matches saved
source identities across the full, affected and focused reports.

## Reproduce

In an external `campaign.toml`, set `[go].path` to an unchanged v1.5.1 checkout:

```toml
jobs = 2
timeout = 20
baseline_timeout = 60
output_dir = "reports"
[go]
path = "original"
operators = ["assignment_remove"]
test_scope = "full"
```

```sh
mutie list --config campaign.toml --json
mutie run --config campaign.toml --plain
```

Repeat with `test_scope = "affected"` and another output directory. Copy the
checkout, add `streams_test.go` as `assignment_streams_test.go`, and point a second
external config at that stronger copy. Then:

```sh
mutie rerun "$ORIGINAL_REPORT" --missed --config stronger.toml --plain
python3 benchmarks/fixtures/go-assignments/verify.py "$AUDIT_ROOT" results.json
```

The retained verifier expects the original/affected/stronger artifact layout and
pre-run input manifest used by this campaign. Raw checkouts/reports stay outside
the repository under the temporary `mutie-go-assignments-audit` directory.
