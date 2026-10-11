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
- [x] Add persistent per-mutation dismissals with required reasons and optional
  notes in `mutie.dismissals.toml`; support add/list/update/remove CLI commands.
- [x] Match dismissals against rediscovered mutations using source-aware identity;
  surface stale, missing, or ambiguous matches with `dismiss check` for review
  after source changes.
- [x] Show dismissal reasons separately from actual execution outcomes in reports
  and result views; allow dismissed findings to be included in inspection and reruns.
  Capture portable review snapshots, add browser review filters and badges, and
  expose unresolved entries without applying them or changing outcome counts.
- [x] Test dismissal persistence, matching/invalidation, removal, and preservation
  of execution outcomes and counts.
- [x] Validate findings and practical runtime on a small real Go project.
- [x] Extend validation to `go-cmp` v0.7.0: all 1,450 mutations across ten packages,
  workspace/command timings, saved-report audits, and source preservation.
  See `benchmarks/go-cmp-v0.7.0.md` for evidence and limitations.
- [x] Repeat a fixed 32-mutation sample with one/four workers; investigate the
  killed/timeout variation with controlled diff-direction probes. Add isolated
  NaN comparison tests and verify a missed mutation becomes killed on a focused rerun.
- [x] Validate a fixed 32-mutation application sample on `fzf` v0.65.2 across all
  seven operators. Full/affected outcomes matched in four measured trials;
  affected scope averaged 16.5% less run time. Record workspace/temp/report sizes,
  review build failures, and prove two call-removal survivors become killed with
  isolated test additions. See `benchmarks/fzf-v0.65.2.md` for evidence and limits.
- [ ] Validate a large application or asset-heavy tree before drawing broader
  scaling conclusions. The measured go-cmp tree is 470 KB and fzf is 1.34 MB;
  the larger Hugo candidate could not pass its baseline on the validation host.
- [x] Record workspace timings alongside existing command durations. Count successful
  CoW clones and fallback copies/bytes for snapshot, baseline setup, discovery reset,
  worker setup, and worker reset; retain partial counts on failure/interruption and
  expose metrics in saved reports and the final summary.
- [x] Document workspace placement via the existing `TMPDIR` override, outside the
  source tree on a chosen filesystem, and record the resolved temporary root.
- [ ] Compare representative runs with workspaces on a reflink-capable filesystem
  versus `/tmp` on tmpfs. The go-cmp run verified explicit reflinks on APFS;
  ordinary-copy and Linux tmpfs measurements remain pending.
- [x] Own and clean up Go temporary build directories after timeout/interruption.
  The go-cmp full run left 27 `go-build*` directories (566 MB logical bytes) even
  though all Mutie workspaces were removed. Each Go command now gets a private
  `GOTMPDIR` below the caller's temporary parent; cleanup preserves unrelated
  files and the reusable Go build cache. Regression tests cover success,
  tool/startup errors, concurrent timeouts, and SIGINT/SIGTERM. A focused go-cmp
  timeout and interruption both left zero owned temporary build directories.
- [x] Own general test temporary directories per Go command, in addition to
  `GOTMPDIR`, and clean them after completion, timeout, and cancellation. Set child
  `TMPDIR`/`TMP`/`TEMP` without changing the caller's environment. Verify separate
  concurrent lifetimes, startup/tool errors, signals, relative parent placement,
  nested files, symlinks, and preservation of caller-owned files and caches.
  The real-Go fixture deliberately leaves temporary files before its timeout;
  baseline, mutation, and rerun cleanup checks pass. Repeated fzf full/affected
  samples left zero temporary files, with all 32 outcomes unchanged in each scope.
- [x] Add opt-in affected-package testing with a full-module baseline/default,
  transitive reverse dependencies and test imports, per-mutation saved selections,
  and fresh graph discovery on reruns. Verify against real multi-package tests.
  The controlled go-cmp sample matched all 32 outcomes in four paired runs;
  affected scope averaged 25.3% less run time (see benchmark notes/data).
- [ ] Evaluate coverage-based test selection and result reuse against full-module
  testing, with explicit invalidation rules. Keep affected-package comparisons
  against full-module results as projects and operators expand.
- [ ] Future exploration, not the next implementation task: investigate Tree-sitter
  structural fingerprints and Merkle dependency hashes for durable mutation
  identity and incremental result reuse. See
  [design notes](DESIGN.md#future-exploration-structural-identities-and-result-reuse)
  for dependency tracking, invalidation, and validation requirements.
- [x] Add standalone call removal with explicit statement eligibility and multiline
  diffs. Support operator selection for fresh runs, previews, and reruns through
  CLI/config, preserving dismissal matching and source identity. Verify real-Go
  outcomes and four missed-to-killed CLI gaps on `godotenv`; full/affected outcomes
  match. See `benchmarks/godotenv-v1.5.1-call-remove.md` for evidence and noise limits.
- [x] Add `branch_force_true` / `branch_force_false` for complete Go `if` and
  `else if` conditions. Use short-circuit wrappers to preserve name references
  without evaluating the original condition; preserve initializers and bodies,
  exclude bare boolean conditions, and retain independent nested operators.
  Verify multiline diffs, config/CLI selection, real-Go types and side effects,
  dismissals, source preservation, and stronger-test reruns. A fixed 24-mutation
  fzf sample matched full/affected outcomes with no build failures; added tests
  killed four survivors in both scopes. See `benchmarks/fzf-v0.65.2-branches.md`
  for useful findings and equivalent-mutation limits.
- [ ] Expand return/loop and further branch mutations and add named operator profiles based on
  useful findings; evaluate call-removal build-failure noise on more projects.
  The fzf sample added two actionable gaps and two unbuildable call removals;
  eight stratified removals do not establish a general noise rate.
- [x] Add explicit report listing and retention: status/age/logical size, count/age
  previews, and `--apply` deletion with writer locks and revalidation. Protect active
  and unfinished reports, unsupported metadata, symlinks, and unrelated contents.
- [x] Add opt-in per-artifact log limits with CLI/config overrides, head/tail
  excerpts, byte-count metadata, and browser notices. Keep complete event parsing
  and discovery independent of retention; preserve noisy outcomes, Unicode,
  partial diagnostics, reruns, and cleanup after timeout/interruption.
- [x] Add opt-in per-command raw capture limits with CLI/config/clear overrides.
  Bound combined stdout/stderr writes before they reach disk; stop the command
  group on overflow and retain diagnostics as a tool error. Preserve exact-limit
  success, discovery completeness, saved reports, policy precedence, and reruns;
  verify noisy streams, blocked observers, concurrent workers, signals, and cleanup.
- [ ] Evaluate archival/compression and run-wide disk/memory budgets. Per-command
  raw captures and per-artifact saved logs can be bounded; rendered logs, decoded
  Go records, workspace/test files, and caches have separate resource costs.
- [x] Add opt-in `--fail-on-missed` / repository configuration, with a local
  disable override. Evaluate current undismissed missed findings after successful
  execution, save the decision separately from execution status, and return exit
  `3` only after report persistence. Cover current/stale dismissals, focused/empty
  runs, listing, failures, cancellation, and terminal restoration.
- [x] Define raw mutation scores with explicit exclusions, unchanged dismissal
  counts, and selection scope. Add opt-in `--min-score` / config with exact
  comparisons, explicit empty/incomplete-run handling, and execution/persistence
  error precedence. Surface scores in summaries, saved JSON, and browser `m`;
  validate saved decisions and cover threshold boundaries, configuration, combined
  policies, filtered/focused runs, compact/wide frames, terminal restoration, and
  real-Go killed/missed/timeout/build-failure outcomes.
- [ ] Add integration/report exports for concrete consumers.

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
