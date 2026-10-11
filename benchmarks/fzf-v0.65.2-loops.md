# fzf v0.65.2 loop-skipping validation

Validated on 2026-10-11 with the `loop_skip` changes based on Mutie commit
`773cf8a`. A fixed sample of **16 loop mutations** produced identical full-module
and affected-package outcomes: **5 killed, 11 missed, 0 build failures,
0 timeouts, 0 tool errors**. Added tests killed two selected survivors in both
scopes, exposing gaps in tail truncation and display-width trimming tests.

The [data](fzf-v0.65.2-loops.json) includes exact identities, selection, commands,
binary/report hashes, package targets, outcomes, and failed-test evidence.
Only fzf's Go tests were exercised; its Ruby/tmux integration suite was not run.

## Source and method

Used disposable copies of the same official fzf v0.65.2 source as the
[earlier application validation](fzf-v0.65.2.md): commit
`416aff86e9d550d0a34af9d3cc12dfede25354c3`, 130 files, 1,344,688 logical bytes.
The source archive hash is recorded in the data. The host was macOS arm64,
Go 1.27.2, APFS, `GOMAXPROCS=8`, `CGO_ENABLED=0`, `GOWORK=off`, `GOENV=off`,
`GOTOOLCHAIN=local`, and `GOPROXY=off`, with cached dependencies. Cleared CI and
`FZF_*` overrides and used `/bin/zsh` as `SHELL`.

A seed run passed its full-module baseline and published the loop-only inventory
of **107 mutations**, then was interrupted before any mutant completed. Before
inspecting outcomes, selected 16 mutations at discovery indices
`floor(i * (N - 1) / 15)` for `i = 0..15`. This spans ten files, sampling source
positions rather than selecting randomly. Range loops, missing conditions, and
bare boolean conditions were outside discovery's `loop_skip` eligibility rules.

Reran those same seed IDs with full-module tests, then affected-package tests,
each with a fresh full baseline. Used two workers, 30-second mutation timeouts,
120-second baseline/discovery timeouts, a 65,536-byte retention limit per log
artifact, and a 16,777,216-byte combined raw capture limit per command.

| Selection | Full outcomes | Affected outcomes |
| --- | --- | --- |
| Original 16 mutants | 5 killed, 11 missed | 5 killed, 11 missed |
| Two survivors after added tests | 2 killed | 2 killed |

All four runs had zero build failures, timeouts, and tool errors. Checked source
fingerprints and original byte ranges, report checkpoints, complete execution
states, and the existence of diffs and logs. Temporary roots were empty after
each run. Source files and the original full report were preserved.

Durations are retained for traceability, not as a speed comparison: there was
one trial per scope, no controlled warm-up, and Rust checks overlapped part of
validation.

## Useful surviving mutations

Added [these tests](fzf-v0.65.2-loops_test.go.txt) as `src/mutie_loop_test.go` in
another disposable copy, without changing production sources:

- **Partial tail chunks:** seed ID **22**, sample ID **4**, skips the loop at
  `src/chunklist.go:108` that trims a partially retained chunk. Existing tests
  used tail lengths aligned with the retained chunks. The new test checks tails
  of 3 and 103 items after inserting 205 items, including exact retained contents
  and preservation of an earlier snapshot. The mutant instead retains 5 and 105
  items, failing the new assertions.
- **Display-width trimming:** seed ID **57**, sample ID **9**, skips the loop at
  `src/terminal.go:3337` that removes leading runes until the remaining text fits
  alongside an ellipsis. New cases cover wide characters, tabs, ASCII, and text
  that already fits. For a five-column space with a two-column ellipsis, the
  mutant retains `界界界a` instead of `界a`, `a\tb` instead of `b`, and `bcdef`
  instead of `def`, with incorrect removed-rune counts.

Both baselines passed with the added tests. Focused `--rerun-missed` runs selected
sample IDs `4,9` from the original full report, and both became killed in each
scope. Saved diagnostics show the expected assertion failures. The test file was
not sent upstream.

## Limits

The sample shows useful findings and no build-failure noise for these 16 mutants;
it does not establish a general noise rate, project-wide score, or equivalence
between full and affected testing. The separate real-Go fixture verifies
initializer preservation, skipped condition/body/post effects, locals/imports
used only in conditions, named/generic booleans, and shadowed boolean names.

Not every survivor is a useful test gap. Seed ID 8 affects `debugV2` diagnostic
printing. Seed ID 92 skips an eight-byte ASCII fast path; inspection shows that
the later four-byte and byte-at-a-time loops still scan the input, so survival
can reflect a performance change rather than broken character conversion.
Remaining survivors were not all reviewed or demonstrated killable. Range-loop
mutation, unconditional-loop mutation, and named operator profiles remain future
work.
