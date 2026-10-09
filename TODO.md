# Progress

Track implementation here; keep design decisions and rationale in
[DESIGN.md](DESIGN.md) and current usage in [README.md](README.md). Update this
checklist as work lands, checking items off after appropriate verification.

## Completed foundation

- [x] Build the Rust CLI with initial Go module support.
- [x] Run a baseline and stop on failed or timed-out tests.
- [x] Discover active Go files and simple Tree-sitter mutations.
- [x] Support include/exclude globs and mutation listing without running tests.
- [x] Run parallel workers in isolated copies with full workspace restoration.
- [x] Support configurable workers and baseline/mutation timeouts.
- [x] Distinguish killed, missed, timed-out, unbuildable, and tool-error results.
- [x] Save summaries, JSON results, diffs, and logs, including partial results.
- [x] Add unit tests and a Go fixture covering outcomes, isolation, and reports.
- [x] Document the MVP and longer-term direction in `DESIGN.md`.

## MVP milestones

### 1. Mutation inventory and reports — completed

- [x] Add named operators, source fingerprints, and operator catalog versioning.
- [x] Save the complete mutation inventory and diffs before workers start.
- [x] Separate run metadata, inventory, and outcomes using the design's report
  layout; add text lists grouped by outcome.
- [x] Represent pending/running mutations and interrupted runs explicitly.
- [x] Preserve active diagnostics and completed results during cancellation;
  coordinate worker shutdown and cleanup across run phases.
- [x] Test incomplete runs and the new report contract; update usage docs while
  preserving existing saved reports.

### 2. Repository configuration — completed

- [x] Load `mutie.toml` from the selected project root.
- [x] Merge defaults, repository config, and explicit CLI overrides in that order.
- [x] Define path resolution, list replacement, and CLI list-clearing behavior.
- [x] Validate unknown keys and invalid values; save the effective configuration.
- [x] Test precedence and defaults, and document working configuration examples.

### 3. Default terminal status view — completed

- [x] Introduce shared coordinator state/events for reports and presentation.
- [x] Choose Ratatui for a live, in-place status view inspired by the Vitest CLI.
- [x] Integrate Ratatui 0.30.1 with Crossterm 0.29 and an inline viewport.
- [x] Add automatic interactive/plain mode selection and an explicit override.
- [x] Show phase, elapsed time, progress, outcome counts, active workers, and
  recent results in the minimal interactive view; redraw in place and keep time
  and activity indicators updating while commands run, not just on completion.
- [x] Handle resizing, small terminals, cancellation, and terminal restoration;
  print a final plain summary and report path.
- [x] Test rendering and terminal lifecycle, including non-interactive use.

### 4. MVP validation — completed

- [x] Verify the complete workflow with config, parallel execution, both terminal
  modes, empty/failed/interrupted runs, saved artifacts, and source preservation.
- [x] Reconcile README examples and design status with the completed MVP.
- [x] Validate on `godotenv` v1.5.1: 77 mutations with identical outcomes in plain
  and interactive modes; verify four surviving mutations become test failures
  after adding tests to an isolated copy. See `DESIGN.md` for evidence and limits.
- [x] Fix narrow-terminal outcome clipping and use all four worker rows when
  space permits; verify the fixes against the real-project run and frame tests.

## Follow-up work

- [x] Rerun completed missed mutations from a saved run with `--rerun-missed`;
  preserve the source report and record old-to-new ID mappings.
- [x] Rediscover and validate rerun targets against current source, reject stale
  or ambiguous matches, and require a fresh baseline.
- [x] Test rerun report validation, source matching, previews, empty selections,
  baseline failures, source preservation, and missed-to-killed results after test changes.
- [x] Refine the live Ratatui run view before adding saved-report browsing: keep
  stable rows ordered by worker number, showing each worker's state, current
  mutation, and elapsed time, including idle workers.
- [x] Add live worker progress events for preparing/building/testing/idle states;
  use "running" when the phase is unknown. Observe captured Go JSON output live
  for build/test evidence while preserving full logs and command cleanup.
- [x] Show a bounded results tail below the workers, newest completions at the
  bottom, with killed/missed/build failed/timeout/error labels. Redraw in place,
  retain progress and outcome totals, and keep the full history in saved reports.
- [x] Verify worker transitions, results ordering, resizing, and compact layouts
  for the refined live view.
- [x] Investigate and fix the preceding shell prompt disappearing after the live
  view closes. Distinguish content pushed into scrollback from content erased;
  audit inline viewport sizing, Ratatui's full-screen clear on horizontal shrink,
  and exit cleanup so pre-run terminal content is preserved.
- [x] Add terminal regression coverage with a pre-existing prompt and output;
  verify preservation through normal completion, cancellation, and resizing,
  including narrow/short terminals and worker views taller than the screen.
- [x] Extend focused reruns to individual saved IDs and operator selection.
- [x] Verify filter intersections, duplicate/invalid selectors, previews, empty
  selections, source validation, and report mappings; exercise focused reruns
  against the real-Go fixture with a fresh baseline.
- [x] Add interactive result filtering and diff/log browsing with `mutie report`;
  mark saved IDs and preview/export focused rerun commands without executing them.
- [x] Verify browser filtering, Unicode/paste input, bounded/missing artifacts,
  compact/wide frames, and shell restoration on quit/export/Ctrl-C/SIGTERM.
- [x] Fix the live-view scrollback race: queued frames used stale absolute rows
  after resizing and moved the cleanup cursor above restored history. Use relative
  frame/allocation movement and guard line wrapping; verify queued output through
  growth/shrink, completion, and cancellation with deterministic regression coverage.
- [ ] Add persistent per-mutation dismissals with required reasons and optional
  notes in repository-owned metadata; support inspecting, changing, and removing them.
- [ ] Match dismissals against rediscovered mutations using source-aware identity;
  surface stale, missing, or ambiguous matches for review after source changes.
- [ ] Show dismissal reasons separately from actual execution outcomes in reports
  and result views; allow dismissed findings to be included in inspection and reruns.
- [ ] Test dismissal persistence, matching/invalidation, removal, and preservation
  of execution outcomes and counts.
- [x] Validate findings and practical runtime on a small real Go project.
- [ ] Extend validation to a larger Go project before drawing scaling conclusions.
- [ ] Measure workspace and command costs before choosing performance changes.
- [ ] Make reflink use observable: count successful CoW clones and ordinary-copy
  fallbacks, and record snapshot/reset timings. Compare representative runs with
  workspaces on a reflink-capable filesystem versus `/tmp` on tmpfs.
- [ ] Support/document workspace placement on the same reflink-capable filesystem
  as the input project, outside its source tree. `TMPDIR` already provides an
  override; the default `/tmp` on tmpfs prevents reflink savings on this machine
  despite the repository living on Btrfs.
- [ ] Evaluate affected-package/coverage-based test selection and result reuse
  against full-module testing, with explicit invalidation rules.
- [ ] Expand mutations, including standalone call removal and return/branch/loop
  changes; add operator selection and profiles based on useful findings.
- [ ] Decide and implement report retention/archival without pruning active runs
  or unrelated files.
- [ ] Add CI failure policies, score definitions, and integration/report exports.

## Longer-term options

- [ ] Pick and implement a second language, extracting the shared language boundary.
- [ ] Add Rust, JavaScript/TypeScript, and Python support one language at a time;
  choose the order based on concrete projects.
- [ ] Extend project support to workspaces, multiple modules, and additional test
  frameworks as needed.
- [ ] Consider HTML and standard interchange exports for concrete consumers.
- [ ] Explore external language plugins, possibly using WASM, after multiple
  built-in languages establish the interface. Design the protocol, host controls,
  packaging, and versioning before choosing a runtime.
