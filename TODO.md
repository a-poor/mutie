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

## Remaining MVP work

### 1. Mutation inventory and reports — next

- [ ] Add named operators, source fingerprints, and operator catalog versioning.
- [ ] Save the complete mutation inventory and diffs before workers start.
- [ ] Separate run metadata, inventory, and outcomes using the design's report
  layout; add text lists grouped by outcome.
- [ ] Represent pending/running mutations and interrupted runs explicitly.
- [ ] Preserve active diagnostics and completed results during cancellation;
  coordinate worker shutdown and cleanup across run phases.
- [ ] Test incomplete runs and the new report contract; update usage docs while
  preserving existing saved reports.

### 2. Repository configuration

- [ ] Load `mutie.toml` from the selected project root.
- [ ] Merge defaults, repository config, and explicit CLI overrides in that order.
- [ ] Define path resolution, list replacement, and CLI list-clearing behavior.
- [ ] Validate unknown keys and invalid values; save the effective configuration.
- [ ] Test precedence and defaults, and document working configuration examples.

### 3. Default terminal status view

- [ ] Introduce shared coordinator state/events for reports and presentation.
- [ ] Select the terminal implementation and backend; Ratatui is the candidate.
- [ ] Add automatic interactive/plain mode selection and an explicit override.
- [ ] Show phase, elapsed time, progress, outcome counts, active workers, and
  recent results in the minimal interactive view.
- [ ] Handle resizing, small terminals, cancellation, and terminal restoration;
  print a final plain summary and report path.
- [ ] Test rendering and terminal lifecycle, including non-interactive use.

### 4. MVP validation

- [ ] Verify the complete workflow with config, parallel execution, both terminal
  modes, empty/failed/interrupted runs, saved artifacts, and source preservation.
- [ ] Reconcile README examples and design status with the completed MVP.

## Follow-up work

- [ ] Select and rerun mutations by saved-run ID, file/operator, or missed outcome.
- [ ] Rediscover and validate rerun targets against current source, reject stale
  or ambiguous matches, and require a fresh baseline.
- [ ] Add interactive result filtering and diff/log browsing.
- [ ] Validate findings and practical runtime on a real Go project.
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
