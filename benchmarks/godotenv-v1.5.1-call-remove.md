# godotenv v1.5.1 call-removal validation

Validated on 2026-10-11 using the working tree based on `aa2a1ee`, with the new
`call_remove` operator and fresh-run operator selection. This checks findings and
outcome consistency; it is not a performance benchmark. Exact source hashes,
effective settings, mutation identities, and outcomes are in the
[validation data](godotenv-v1.5.1-call-remove.json).

## Method

- Copied the existing local `github.com/joho/godotenv` v1.5.1 validation checkout
  into a new temporary directory. Every upstream file matched the SHA-256 manifest
  from the earlier v1.5.1 validation; its existing `mutie.toml` was also copied.
- Used Go 1.27.2 on macOS arm64, the debug Mutie binary, two workers, plain output,
  a ten-second mutation timeout, and a warm reusable Go build cache. Network module
  downloads were disabled. Cargo tests overlapped some runs; no timing claim is made.
- Listed and executed only `call_remove`, first with full-module tests, then with
  affected-package tests. Both runs used a full-module baseline.
- Added CLI behavioral tests to a second isolated copy and reran the four missed
  source IDs with fresh full-module tests. Implementation files were unchanged.

## Results

| Source ID | Removal | Full / affected outcome |
| --- | --- | --- |
| 1 | `autoload/autoload.go:14`, `godotenv.Load()` | Build failed / build failed |
| 2 | `cmd/godotenv/cmd.go:15`, register `-h` | Missed / missed |
| 3 | `cmd/godotenv/cmd.go:17`, register `-f` | Missed / missed |
| 4 | `cmd/godotenv/cmd.go:19`, register `-o` | Missed / missed |
| 5 | `cmd/godotenv/cmd.go:21`, `flag.Parse()` | Missed / missed |
| 6 | `cmd/godotenv/cmd.go:38`, print usage | Build failed / build failed |
| 7 | `cmd/godotenv/cmd.go:54`, `log.Fatal(err)` | Build failed / build failed |
| 8 | `godotenv.go:173`, `sort.Strings(lines)` | Build failed / build failed |

Both original runs completed with **0 killed, 4 missed, 0 timeouts, 4 build
failures, and 0 tool errors**. The CLI package had no tests. The four build
failures were caused by imports becoming unused; usage-print removal also left
the `usage` local unused. These failures do not demonstrate test coverage and
remain separate from killed outcomes. No cleanup edits were applied to mutants.

The [added tests](godotenv-v1.5.1-call-remove_test.go.txt) run the CLI entry point
in a helper subprocess. One checks that documented help accepts `-h`, `-f`, and
`-o` and prints the program's usage; the other checks that an unknown option fails.
They use observable output/exit behavior, without inspecting flag registration
internals. The new baseline passed, and the focused rerun killed **all four**
previously missed mutations, mapping source IDs 2–5 to new IDs 1–4.

## Reproduce

With a writable v1.5.1 checkout in `PROJECT`, a writable Go build cache, and a
built Mutie binary:

```sh
target/debug/mutie "$PROJECT" --operator call_remove --list
target/debug/mutie "$PROJECT" --operator call_remove --jobs 2 --timeout 10 --ui plain --output-dir "$REPORTS/full"
target/debug/mutie "$PROJECT" --operator call_remove --test-scope affected --jobs 2 --timeout 10 --ui plain --output-dir "$REPORTS/affected"
```

Copy the supplied `.go.txt` file to `cmd/godotenv/call_remove_test.go` in a separate
copy of the checkout, then use its path as `IMPROVED` and the first run directory
as `FULL_RUN`:

```sh
target/debug/mutie "$IMPROVED" --rerun-missed "$FULL_RUN" --operator call_remove --jobs 2 --timeout 10 --ui plain --output-dir "$REPORTS/improved"
```

All three reports had matching checkpoints and completed execution states.
Every original source file in both copies retained its SHA-256 hash. All eight
saved diffs were applied with `git apply` to disposable files and matched the
recorded byte replacement exactly. Raw reports and copies remain under
`/tmp/mutie-call-remove-8pz0ds75`, outside version control.

This small project demonstrates actionable CLI gaps and substantial build-failure
noise. It does not establish general call-removal effectiveness or justify broad
import/local rewriting. Multiline/parenthesized removals, precise boundaries,
Unicode/CRLF diffs, config overrides, dismissals, and stale reruns are additionally
covered by repository regression tests and the opt-in real-Go fixture.
