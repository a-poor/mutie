# Initial Rust milestone acceptance

Review of `69bcac0` on 2026-10-14. The selected real project remains Mutie itself.
Decision: accept the initial Rust milestone for the agreed binary-package scope.
This does not imply general Cargo compatibility or parity with Go's operator catalog.

## Completion criteria and evidence

| Criterion | Verified implementation and evidence |
| --- | --- |
| Concrete package scope | `rust/project.rs` validates one ordinary binary package on Unix, a lockfile, host toolchain and standard tests. Mutie needs no workspace-member selection. See the [scope audit](rust-initial-scope.md). |
| Shared execution workflow | `language.rs` dispatches discovery/test commands through shared scheduling, workspace, capture, status and reporting paths. [First-slice runs](rust-first-slice.md) exercise baseline, outcomes and preservation. |
| Reliable classification and shutdown | Compilation is separate from test execution, with one deadline and Cargo completion evidence. Fault, capture, cancellation and cleanup tests cover partial reports and errors. [Process fixes](process-reliability.md), [survivor isolation](rust-survivor-isolation.md) and [parallel reruns](rust-parallel-score-reruns.md) retain failures and subsequent evidence. |
| Small useful operator set | `boolean_flip`, `comparison_boundary` and `logical_swap` have syntax/eligibility tests and real-Cargo fixtures. [Boundaries](rust-comparison-boundaries.md) and [logical swaps](rust-logical-swaps.md) verify operand behavior and stronger-test reruns. Parity with Go was not a completion criterion. |
| Inspect, review and rerun | [Focused reruns](rust-focused-reruns.md) verify saved-ID matching and PTY browser export. [Dismissals](rust-dismissals.md) verify review decisions, unchanged raw outcomes, score policies and historical report preservation. |
| Real-project test improvement | Missing/unexpected saved percentages and the exact tolerance endpoint yielded stronger tests in Mutie. The endpoint's saved rerun fails only its new assertion. The logical self-run confirms existing invalid-duration coverage; it does not claim another new gap. |
| Parallelism and artifacts | Six score mutations produce six intended kills with identical failure sets in fresh and cached modes. All 14 local targets rebuild; later cached mutants reuse 155 dependency records. Sources, reports and caller artifacts are preserved and owned storage is cleaned. |
| Runtime and usefulness | Focused self-reruns take 256.60–285.88 seconds; the parallel six-mutation pair takes 686.16/608.64 seconds. These are recorded samples, not latency guarantees or broad finding-yield estimates. |
| Go compatibility | Shared default regressions remain green; a real-Go workflow is included in this acceptance check. No Go discovery/execution behavior changes are needed for acceptance. |

The three operators meet the scoped requirement: boolean flips found a real
saved-score validation gap; comparison boundaries found the tolerance endpoint;
logical swaps cover compound conditions with verified grouping and short-circuit
behavior. Additional operators may be useful later, but are not required to
complete this specific milestone.

## Explicit limits

- Exactly one ordinary binary package on Unix. Libraries/proc macros, multiple
  packages, enclosing workspaces and workspace-member selection remain unsupported.
- Registry dependencies, default features, host compilation, standard test profile
  and harness. Project build scripts, local/git dependencies, custom runners and
  semantic Cargo overrides are outside the contract.
- Full `cargo test --tests` scope: no affected-test selection, test-name filtering,
  doctests, ignored tests, examples or benchmarks in the supported execution path.
- Discovery excludes test-only, cfg/cfg_attr, generated, macro-token and constant/
  type-level code. Let-chain connectors are excluded; eligible boolean operands
  remain mutable. Discovery does not perform type analysis or establish equivalence.
- Reruns require strict source identities and compatible Cargo/toolchain settings.
  Stronger tests in another file can retain the selected source's identity.
- Fresh artifacts remain the default; dependency reuse is opt-in and worker-local.
  No cross-run cache or automatic previous-result reuse is claimed.
- Evidence comes from focused Mutie samples and fixtures on one host. Historical
  intermittent failures remain recorded; successful rechecks do not erase them
  or prove all scheduling interleavings safe.

## Acceptance checks

The logical-swap implementation passed 200 default tests (26 opt-in tests ignored),
its real-Cargo fixture workflow, formatting and Clippy before this review. Its
Mutie self-run independently passed the same baseline and killed only the intended
invalid-duration assertion. This review changes documentation and fixture ignore
rules; it does not alter execution behavior.

The current acceptance checks both passed:

```sh
cargo test --locked --test rust_workflow -- --ignored --test-threads=1
# 17 passed, 0 failed; 304.80 seconds
cargo test --locked --test go_workflow go_workflow -- --ignored --exact
# 1 passed, 0 failed; 27.09 seconds
```

This includes the cache/restoration and shared-deadline cases that had previously
failed intermittently in a full suite. All 17 now pass together without relaxing
their deadlines. Local logs are `/tmp/mutie-rust-acceptance.log` and
`/tmp/mutie-rust-acceptance-go.log`. These supplement the historical
validation; they do not erase earlier failures. Formatting, diff checks, local
document links and absence of tracked generated artifacts were checked as well.
No package, report-schema or operator-catalog version was changed.

## Next objective: shorten focused Rust feedback

The next measured obstacle is feedback time within the supported scope. Dependency
reuse cuts warm compilation to roughly 17–19 seconds in the latest pair, yet whole
mutations still take roughly 153–157 seconds. A one-finding self-rerun takes over
four minutes. Prioritize understanding and reducing that cost before automatically
adding more operators, languages, general workspaces or UI refinements.

Start with controlled timing of the unchanged full suite: separate compilation,
Cargo launch and individual test-target execution; record repeated samples under
known worker/load conditions. Use the existing saved score selection as an outcome
control. Select an optimization from the measured bottleneck, then compare fresh
and cached runs with the same source, tests, deadlines and expected failure sets.
Keep source/artifact preservation and cancellation checks in the acceptance gate.
Do not silently omit tests, extend deadlines, reuse local package artifacts or
weaken source matching to make the timing look better. Changes to supported test
scope or persistent caching require a separate explicit design decision.
