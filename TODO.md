# Progress

Track implementation here; keep design decisions and rationale in
[DESIGN.md](DESIGN.md) and current usage in [README.md](README.md). Update this
checklist as work lands, checking items off after appropriate verification.

## Current direction

The Go MVP and the initial Rust binary-package milestone are complete for their
supported scopes. The next objective is shorter focused Rust feedback, driven by
measured full-suite runtime on Mutie. Preserve outcome correctness, source/artifact
isolation and the full test scope while identifying the dominant costs. Explicit
user priorities and demonstrated reliability blockers take precedence.

## Target support tracking

[SUPPORT.md](SUPPORT.md) tracks maturity, scope and evidence independently for each
language/test runner. The planned targets below are explicit roadmap targets;
their ordering and initial scopes are not yet selected.

- [x] Define support levels and record current Go and Rust evidence and limits.
- [ ] Define and implement the first JavaScript/TypeScript + Vitest slice.
- [ ] Define and implement the first JavaScript/TypeScript + Node test runner slice.
- [ ] Define and implement the first JavaScript/TypeScript + Bun test runner slice.
- [ ] Define and implement the first Python + pytest slice.

Each new slice starts with a fixture and a real validation project, then advances
through the support gates recorded in the ledger. Tracking these targets does not
change the current focused-feedback work queue.

## Next objective: shorter focused Rust feedback

- [x] Profile compilation, adapter phase overhead and individual test-target
  execution in repeated serial/parallel cold/warm Mutie runs. See
  [the feedback profile](benchmarks/rust-feedback-profile.md). The residual outside
  harness timings bounds further investigation; it does not isolate Cargo startup.
- [x] Measure prebuilt CLI startup and full dismissal-harness execution with
  default/one/two Tokio threads in interleaved repetitions. All nine harnesses
  pass; the small 2–3% median difference does not justify changing runtime defaults.
  See [the runtime experiment](benchmarks/rust-runtime-startup.md).
- [x] Attribute dismissal workflow time to command execution, report persistence
  and cleanup using isolated diagnostics. Focused and complete harnesses identify
  report persistence as substantial; observer overhead is quantified. See
  [workflow costs](benchmarks/rust-workflow-costs.md). Extend instrumentation to
  terminal/log-limit targets if needed after evaluating this candidate.
- [ ] Evaluate avoiding unchanged derived-report writes while retaining matching
  authoritative checkpoints, atomic durable publication for changed files, missing
  file/error handling and identical final reports. Keep full tests, package rebuilds,
  deadlines and strict rerun matching; measure actual write reduction and runtime.
- [ ] Compare unchanged saved selections before/after using runtime distributions,
  exact failure sets, cancellation checks and source/artifact preservation.

See [the acceptance review](benchmarks/rust-milestone-acceptance.md) for evidence,
limits and the reasoning behind this priority.

## Completed milestone: initial Rust support

See [the Rust milestone](DESIGN.md#completed-milestone-initial-rust-support)
for the agreed scope and completion criteria. Acceptance applies to one ordinary
binary package on Unix, three operators, and the verified test-improvement workflow;
it does not imply general Cargo/workspace support.

- [x] Select Rust as the second language and the next major product milestone.
- [x] Choose a small fixture and a representative maintainer-owned Rust project:
  `tests/fixtures/rust` and Mutie itself. Define the initial binary-package/default
  feature Cargo scope, source exclusions, unsupported layouts, and owned artifact
  directories in DESIGN. Verify the fixture's two detected boolean flips and one
  survivor, plus a stronger-test probe; verify Mutie's baseline in an isolated
  copy (178 passed, 6 ignored). See `benchmarks/rust-initial-scope.md`. These are
  direct Cargo checks recorded before implementing mutation execution.
- [x] Build the first end-to-end Rust slice: project detection/configuration,
  source discovery, one operator, passing baseline, isolated mutation execution,
  and saved results for one Cargo binary package. Share command captures,
  classification, test dispatch, workers, status and reports with Go. Verify the
  three-mutation fixture, stronger tests through a fresh run, failures, empty
  selections, repeated/parallel execution, timeout/cancellation cleanup, and
  caller artifact preservation. Persist Cargo scope/tool versions and keep
  unsupported Rust rerun/dismissal actions explicit. Mutie's serial two-mutation
  score sample finished with one kill and one survivor in 429.59 seconds. See
  `benchmarks/rust-first-slice.md`, including the noisy parallel-run result.
- [x] Turn the first real Rust survivor into a test improvement: add saved-report
  regressions for missing/unexpected percentages, then run the same score sample
  through Mutie again. The fresh serial run passed all 189 baseline tests and
  killed both mutations; the former survivor failed exactly the two new cases.
  See the stronger-test follow-up in `benchmarks/rust-first-slice.md`.
- [x] Harden the PTY self-tests under parallel Cargo load: retain failure
  diagnostics, prevent inherited terminal descriptors, and allow enough time
  for a whole test workflow to finish. The descriptor regression fails before
  the fix; 12 paired terminal/browser runs (216 tests) pass afterward, including
  overlap with fresh compilation. A fresh two-worker self-run passes all 189
  baseline tests and kills both score mutations through score-validation failures;
  every terminal/browser test passes. See `benchmarks/terminal-reliability.md`.
- [x] Verify cancellation during Rust mutation test execution with real Cargo:
  serial SIGTERM and parallel SIGINT after a completed mutation, using both
  file-backed and pipe-backed captures. Keep completed results/logs, distinguish
  interrupted and pending work, validate the partial report, stop Cargo/test/
  helper processes, and preserve sources and caller artifacts. See the
  cancellation follow-up in `benchmarks/rust-first-slice.md`.
- [x] Verify compilation-stage cancellation/timeouts for Rust baselines and
  parallel mutations, using real Cargo and a live rustc checkpoint. Preserve
  partial reports and build diagnostics, stop compiler/helper descendants, and
  prevent tests from starting after compilation stops. Verify one deadline spans
  compilation and testing; deliberately resetting the test deadline makes the
  regression fail. Keep source/caller artifacts intact and owned directories
  cleaned up. See `benchmarks/rust-first-slice.md`.
- [x] Verify Cargo tool/protocol failures and raw-output limits: 17 injected fault
  scenarios cover startup, unexpected exits/signals, completion evidence,
  discovery/listing and baseline failures, and parallel overflow/peer cleanup.
  Preserve failure reasons independently of tiny saved logs; fix metadata
  overflow diagnostics that previously lost the capture-limit explanation.
  Document separate per-invocation capture budgets for compilation and testing.
- [x] Make Rust execution reliable for the initial binary-package scope:
  distinguish build/test/tool failures, enforce
  timeouts and cancellation, preserve partial results, and define build-cache
  and target-directory ownership. Verify repeated/parallel mutations execute the
  intended tests without stale artifacts, source edits, or artifact collisions.
  The first slice covers ordinary Cargo evidence, fresh artifacts, repeated/
  parallel runs, compilation/test timeouts, shared deadlines, and baseline/
  mutation cancellation with partial results. Tool/protocol failures and raw
  capture limits are now covered too; broader Cargo layouts remain unsupported.
  The PTY follow-up fixes reproduced harness defects; the original exit-only
  failure lacks enough diagnostics to establish its specific cause.
- [x] Add a small useful Rust operator set and document eligibility for test-only
  code, macros, generated sources, and conditional code. Support the chosen real
  project's workspace/package selection if required by the agreed scope. The
  accepted set is boolean flips, comparison boundaries and logical swaps; Mutie
  requires no workspace-member selection. See the acceptance review.
- [x] Add Rust comparison-boundary mutations (`<` ↔ `<=`, `>` ↔ `>=`), preserving
  operands and source exclusions. Include executable match guards while keeping
  patterns excluded. Verify exact edits, generic borrowing, floats, operand
  effects and a fixture survivor killed by an equality test through focused rerun.
  Mutie's six score-boundary mutations finish with five kills and one survivor
  at the saved-percentage tolerance endpoint in 780.31 seconds; retain that narrow
  review finding and the runtime evidence in `benchmarks/rust-comparison-boundaries.md`.
- [x] Add Rust `logical_swap` for lazy boolean operators, preserving grouping,
  comments and single operand occurrences. Exclude let-chain connectors while
  retaining valid boolean operands. Verify short-circuit effects, nested truth
  tables, source exclusions, configuration, parallel caches, reports and a
  stronger-test saved rerun in `tests/fixtures/rust-logical` (seven kills and one
  survivor, then one intended kill). A focused Mutie self-run passes 200 baseline
  tests and kills its invalid-duration conjunction solely through the existing
  timing regression, preserving sources/artifacts (285.88 seconds); see
  `benchmarks/rust-logical-swaps.md`.
- [x] Validate opt-in per-worker Cargo dependency reuse against fresh builds on
  the same six Mutie score-boundary mutations. Force package recompilation with
  Cargo cleanup; verify backdated source, restoration, stronger-test reruns,
  parallel isolation, failure/timeout/cancellation cleanup and source preservation.
  Fresh remains the default. Both self-runs produce five kills and one survivor;
  warm compilation falls from 66–73 to 22–46 seconds, with 155 dependency artifact
  records reused and all 14 local targets rebuilt. Whole runs take 846.13 versus
  651.97 seconds, but baseline variation accounts for nearly half the difference.
  See `benchmarks/rust-build-cache.md` for costs, limits and preservation evidence.
- [x] Reproduce and fix capture-shutdown and retention-lock assertion failures
  exposed during the paired self-runs. Retry process-group shutdown to catch
  racing forks and explicitly release report locks before closing their owning
  descriptors. The isolated EOF regression passes 100/100 runs; all 198 default
  tests pass. See `benchmarks/process-reliability.md` for evidence and validation
  limits. Keep cache reuse opt-in.
- [x] Stabilize nested parallel Mutie self-testing before accepting new runtime
  or finding-quality evidence. The reliability follow-up passes 198 default
  tests; all 16 Rust workflows pass either in the suite or focused rechecks, but
  the full Rust suite has two intermittent timeout/checkpoint failures. The
  six-mutation self-run produced four timeouts and an unrelated kill of the
  previous survivor despite a clean baseline. Original capture/lock assertions
  did not recur; other cancellation, log-limit and terminal tests stalled or
  failed. A fixture loader stall is observed, not established as every failure's
  cause. Preserve deadlines, source isolation and diagnostic logs; see
  `benchmarks/process-reliability.md`.
  Unmutated controls now pass all 198 tests in one and two isolated Cargo workers;
  repeated cancellation suites also pass directly and through Mutie's subprocess
  runner. The fixture now records shim startup and includes checkpoint/output
  diagnostics on readiness timeouts. The complete Cargo adapter also passes all
  nine focused executions (63 cancellation-test passes) across fresh, cold-cache
  and warm-cache modes with one/two workers; local targets rebuild every time.
  No watcher trigger or artifact leak occurs. The subsequent paired survivor
  comparison passes all six focused executions, but the full pristine cold-cache
  control reproduces capture EOF. Pipe diagnostics establish cross-command
  descriptor inheritance after the original group is gone. Serialize spawning
  alongside the group retry; the combined fix passes 100/100 capture stress runs.
  A new exact-tolerance test kills the score survivor through its intended boundary
  assertion and passes before/after restoration. With both changes, all six
  two-worker full adapter controls pass 199 tests across fresh/cold/warm caches;
  source and caller artifacts are preserved and owned temporary storage is empty.
  The stronger test now lives outside score.rs to preserve strict rerun identity.
  A saved-ID rerun passes all 199 baseline tests and kills only the selected
  tolerance mutation through that assertion, with the 100% policy passing and
  source/report/caller artifacts preserved (256.60 seconds); see
  `benchmarks/rust-survivor-isolation.md`. The subsequent two-worker six-mutation
  comparison passes in both fresh and cached modes: six intended kills each,
  identical per-ID failing tests, no timeouts/build failures/errors, and preserved
  source/report/caller artifacts. Both baselines pass 199 tests; local targets
  rebuild and warm dependencies are reused. Whole runs take 686.16/608.64 seconds.
  This closes the reproduced blocker for this sample, not every possible
  scheduling issue. See `benchmarks/rust-parallel-score-reruns.md`. The acceptance
  review subsequently passes all 17 real-Cargo workflows together.
- [x] Complete the shared user workflow for Rust: filters/configuration,
  live/plain status, reports/browser, scores/policies, dismissals, and focused
  reruns. Record language and execution scope for correct rerun validation;
  preserve existing Go reports and reject unsupported language-specific options.
- [x] Add focused Rust reruns by saved IDs/operators/missed outcomes and browser
  command export. Validate Cargo scope, manifest/lockfile identity, toolchain and
  current source; preserve legacy report browsing and Go reruns. Verify previews,
  partial/empty runs, moved checkouts and a stronger-test fixture rerun.
- [x] Add Rust dismissal CRUD, fresh rediscovery, saved review and focused rerun
  matching. Preserve raw outcomes/scores and legacy Go decisions; distinguish
  language mismatches. Verify isolated metadata side effects, output limits,
  timeout/cancellation cleanup, stronger-test reruns and historical browser review.
  A focused Mutie self-run keeps its dismissed survivor in the raw score while
  passing the findings policy and failing the minimum-score policy. See
  `benchmarks/rust-dismissals.md`.
- [x] Verify fixture outcomes, failed/empty runs, interruption, source/artifact
  preservation, and stronger-test reruns, with Go regressions remaining green.
- [x] Validate a useful focused rerun on Mutie itself: reproduce the known score
  survivor in a temporary copy, restore its two regression tests, and kill only
  the saved survivor through `--rerun-missed`. Verify browser export, ID mapping,
  a passing fresh baseline, source/report/artifact preservation, and measured
  runtime. See `benchmarks/rust-focused-reruns.md`.
- [x] Exercise the complete workflow on the selected Rust project with ordinary
  CLI commands. Review survivors, verify a useful stronger-test rerun, and record
  time to an actionable finding, run/build/workspace costs, and noise/limitations.
  Saved-percentage presence/absence regressions now catch the first real survivor
  in `MutationScore::matches_saved` through fresh and focused runs. Dismissal
  review, policies, browser export and all three initial operators are verified.
- [x] Document verified Rust usage and limits in README/design and accept the
  initial milestone. The evidence review is in `benchmarks/rust-milestone-acceptance.md`.
  Choose shorter focused Rust feedback as the next objective: full-suite runtime
  remains the dominant observed cost after worker-local dependency reuse.

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

## Go follow-up history and deferred backlog

Checked items below are implemented. Unchecked items remain available for later
work; their order is not a priority queue and they do not block the Rust milestone.

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
- [x] Add `loop_skip` for explicit conditions in single-condition and three-part
  Go `for` loops. Preserve initializer execution and name references; skip the
  original condition, body, and post statement. Exclude range/missing/bare-boolean
  conditions, retain nested discovery, and verify exact diffs, types, effects,
  config/CLI selection, reports, dismissals, and stronger-test reruns. A fixed
  16-mutation fzf sample matched full/affected outcomes without build failures;
  added tests killed two survivors in both scopes. See
  `benchmarks/fzf-v0.65.2-loops.md` for findings and limitations.
- [x] Add per-operator planned/completed counts, outcomes, unfinished states, and
  cumulative command time to saved summaries and the browser's `t` view. Derive
  totals from existing inventory/execution records so older reports work without
  rewriting them. Keep totals scoped to the saved selection across filters, marks,
  and dismissals; distinguish missing timings and unpublished/empty inventories.
  Verify aggregation, shuffled legacy records, saved-summary parity, wide/compact
  snapshots, navigation/resizing, and report preservation in a pseudo-terminal.
- [ ] Expand return and further loop/branch mutations and add named operator profiles based on
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

- [ ] Prioritize the planned JavaScript/TypeScript and Python runner slices using
  concrete projects and the largest remaining obstacles to routine use; see
  [target support tracking](#target-support-tracking).
- [ ] Extend project support beyond the chosen Rust milestone scope to additional
  workspace/multi-module layouts and test frameworks as needed.
- [ ] Consider HTML and standard interchange exports for concrete consumers.
- [ ] Explore external language plugins, possibly using WASM, after multiple
  built-in languages establish the interface. Design the protocol, host controls,
  packaging, and versioning before choosing a runtime.
