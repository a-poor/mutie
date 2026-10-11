# fzf v0.65.2 branch-forcing validation

Validated on 2026-10-11 with the branch-forcing changes based on Mutie commit
`0e0cf1d`. A fixed sample of **24 mutations across 12 conditions** produced
identical full-module and affected-package outcomes: **1 killed, 23 missed,
0 build failures, 0 timeouts, 0 tool errors**. Added tests killed four of those
survivors in both scopes, covering two behaviors.

The [data](fzf-v0.65.2-branches.json) records exact identities, selection, commands,
binary/report hashes, package targets, outcomes, and failed-test evidence.
This evaluates fzf's Go tests; its separate Ruby/tmux integration suite was not run.

## Source and method

Used the same official fzf v0.65.2 source and cached dependencies as the
[earlier application validation](fzf-v0.65.2.md): commit
`416aff86e9d550d0a34af9d3cc12dfede25354c3`, 130 files, 1,344,688 logical bytes.
All work used disposable copies. The source archive hash is in the data.
The host was macOS arm64 with Go 1.27.2, APFS, `GOMAXPROCS=8`,
`CGO_ENABLED=0`, `GOWORK=off`, `GOENV=off`, `GOTOOLCHAIN=local`, and `GOPROXY=off`.
Cleared CI and `FZF_*` overrides and used `/bin/zsh` as `SHELL`.

A seed run passed its full-module baseline and published the branch-only
inventory, then was interrupted before any mutation completed. Discovery found
**1,987 eligible conditions**, each with true/false variants: 3,974 mutations.
Before inspecting outcomes, selected condition index `floor(i * (N - 1) / 11)`
for `i = 0..11` in discovery order, keeping both directions. This spans seven
files and deliberately samples source positions; it is not a random sample.

Reran the same 24 seed IDs with full-module tests, then affected-package tests.
Each run required a fresh full baseline. Used two workers, 30-second mutation
timeouts, 120-second baseline/discovery timeouts, 65,536 retained bytes per log
artifact, and 16,777,216 combined raw capture bytes per command. No limit was hit.
Source fingerprints, original byte ranges, report checkpoints, completed states,
diffs, and log artifacts were checked. Temporary roots were empty after every run.

| Selection | Full outcomes | Affected outcomes |
| --- | --- | --- |
| Original 24 mutants | 1 killed, 23 missed | 1 killed, 23 missed |
| Four survivors after added tests | 4 killed | 4 killed |

All four runs had zero build failures, timeouts, and tool errors. Run durations
are retained for traceability, but this is not a timing comparison: there was
one trial per scope, no controlled warm-up, and Rust checks and real-Go fixture
tests overlapped part of validation.

## Useful surviving mutations

Added only these two test files to another disposable source copy:

- [Ellipsis argument tests](fzf-v0.65.2-ellipsis_test.go.txt), installed as
  `src/mutie_branch_test.go`. Seed IDs **1083/1084** (sample IDs **7/8**) force
  `err != nil` at `src/options.go:2837`. Forcing true returns before applying a
  supplied `--ellipsis ...` value; forcing false accepts a missing argument.
  The new test verifies both value application and missing-argument rejection.
- [Up-arrow modifier tests](fzf-v0.65.2-arrow_test.go.txt), installed as
  `src/tui/mutie_branch_test.go`. Seed IDs **3611/3612** (sample IDs **21/22**)
  force `alt` at `src/tui/light.go:555`. Forcing true decodes Shift/Alt-Shift up
  arrows as Alt-Up; forcing false decodes Alt-Up as Shift-Up. Tests check four
  escape sequences' decoded event types and consumed byte counts without a TTY.

Both fresh baselines passed with the additions. Focused `--rerun-missed` runs
selected sample IDs `7,8,21,22` from the original full report, and all four became
killed in both scopes. Diagnostics identify the assertions above, not build errors.
Original source files and the source report remained unchanged. The tests were
not sent upstream.

## Limits and next decisions

Zero unbuildable mutants in this sample supports retaining condition references
with short-circuit wrappers, but does not establish a general noise rate. The
separate real-Go fixture also verifies imports/locals used only in conditions,
initializer effects, named/generic booleans, and shadowed boolean names.

Not every survivor is a useful test gap. For example, seed ID 362 forces the
ASCII shortcut in `src/ansi.go:216` false. Inspection shows that decoding an ASCII
rune through the general UTF-8 path still consumes one byte, so this change has
equivalent output with extra decoding work. Most remaining survivors were not
reviewed in depth; a high missed count must not be presented as a count of bugs.

The sample supports keeping the new operators and supplies two concrete test
improvements. It does not justify a project-wide mutation score, general
full/affected equivalence, named operator profiles, or claims about the separate
interactive integration suite. Broader operator and equivalent-mutation review
remain follow-up work.
