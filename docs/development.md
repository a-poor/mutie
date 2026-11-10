# Development

[Documentation index](README.md) · [Project overview](../README.md)

## Linux CI

[Linux CI](https://github.com/a-poor/mutie/actions/workflows/ci.yml) runs on pushes
to `main`, pull requests and manual dispatches. It uses Ubuntu 24.04, Rust 1.99.1
and Go 1.27.2. Separate jobs run:

- Rustfmt, Clippy with warnings denied, the normal test suite and crate packaging.
- Real Go mutation workflows, including affected-package selection, numeric/string
  native semantics, compiler boundaries and stronger-test reruns.
- Selected real Rust package and workspace workflows, including reports, source
  preservation and reruns. The Rust job also runs native call/assignment removal
  and numeric-literal controls, serial/cached scored workflows and stronger-test reruns, plus native
  serial/parallel cancellation and descendant cleanup.
- Node 24.22.0 JavaScript and native TypeScript workflows, review/policies,
  lifecycle failures, default discovery, timeout/cancellation and capture limits.
- Bun 1.3.11 JavaScript and TypeScript workflows, review/policies, native outcomes,
  resource limits and descendant cleanup.
- Vitest 4.1.9 JavaScript and TypeScript workflows, review/policies, lifecycle
  attribution and cleanup, using Node 24.22.0 and the frozen pnpm 11.10.0 fixture.
- A separate Vitest compatibility job is configured for 4.1.5 / Vite 8.0.10 /
  pnpm 10.33.2, including the same runtime evidence controls plus lint/typecheck
  completion and failure controls. Its frozen tools live in
  `tests/fixtures/vitest-typecheck`; set `MUTIE_VITEST_RECIPE_FIXTURE` to an
  installed temporary copy to run `vitest::recipe_tests::` with `--ignored`.
  The [local acceptance record](../benchmarks/vitest-ufo.md) predates this job's
  first remote run.
- Scoped JS/TS assignment removal: independent scored workflows in each runner's
  job and paired native semantics on Node/Bun, including exact saved identities,
  constructor exclusions, stronger-test reruns, policy scores and lifecycle errors.
- JS/TS numeric and quoted-string literals: six scored runner/syntax workflows
  per operator, plus paired Node/Bun native value, Unicode and escaping controls.
- CPython 3.14.7 with pytest 9.0.2: scored workflows and operators (including
  standalone-call and assignment semantics, review/reruns and runtime/cleanup error controls), flat/src/root-test
  layouts, review/policies, unittest/conftest attribution, resource limits and cleanup.
  The workflow job also installs Flit Core 3.12.0 for owned-wheel metadata,
  child-interpreter, skip/xfail selection and stronger-test rerun controls.

The normal suite runs with two test threads; adapter tests run serially in separate
jobs. Python splits its workflow, operator and native lifecycle tests across three
jobs to bound feedback time. Individual test deadlines stay unchanged. Package
verification uses a separate temporary Cargo target directory. Vitest dependencies are installed
with scripts disabled in a temporary fixture copy; Python uses a dedicated temporary
venv with the pinned audit requirements. Neither installation modifies the checkout.
The native tests remain ignored by ordinary `cargo test` and are explicitly enabled
in CI, so local self-mutation cannot recursively launch runner campaigns.

macOS CI, the full package-manager matrix, the other separate JS/TS operator campaigns,
additional Rust probes and external-project/profile probes remain outside this
suite. The CI badge reports this Linux coverage, not complete release qualification.

## Local checks

```sh
cargo test
cargo fmt --check
cargo test --test go_workflow -- --ignored
cargo test --test affected_packages -- --ignored
cargo clippy --all-targets -- -D warnings
```

The Go integration test checks all four outcomes, repository configuration and
scope, baseline failure and timeout, workspace resets, Go build-directory cleanup,
saved reports, and preservation of input files. It needs Go on `PATH` and a writable
Go build cache, and is opt-in because it runs real builds and deliberately waits
for a timeout. Non-ignored Unix tests use a fake Go command to verify SIGINT/SIGTERM
during baseline, discovery, and parallel execution, preserved diagnostics, pending
work, process and build-directory cleanup, and source preservation. They require
an environment that delivers those signals.
The additional real-Go call-removal test covers multiline and parenthesized calls,
killed/missed/build-failed results, dismissal identity, and a stronger test killing
a survivor on an affected-package rerun.
The opt-in affected-package fixture compares full/affected outcomes and verifies
transitive and internal/external test imports, unrelated-package omission, and
fresh selections after adding tests to a focused rerun.
Configuration tests use the public CLI directly and also cover root help,
report-free baseline checks, multi-language selection, strict schemas, defaults, language-section inheritance, explicit runtime overrides,
invalid settings, path resolution, listing without reports, and saved effective
settings using a fake Go command. Adapter regression suites use `tests/support/invocation.rs`
to materialize their execution settings in temporary external config files while
preserving project bytes and source/report identity.
Rerun tests cover saved-report validation, stale/missing/ambiguous targets, fresh
baselines, ID/operator/outcome intersections, invalid selectors, selection
previews, new ID mappings, empty selections, source report preservation, and
improved tests turning a previous missed result into killed. The opt-in real-Go
test also selects a killed and a dismissed missed mutation by ID and operator,
checks the dismissal against real discovery, and verifies that a test-only change
preserves its identity while the new tests kill the previously missed mutation.
The real-Go branch fixture covers locals/imports used only in conditions,
initializer and condition side effects, named/generic booleans, shadowed boolean
names, config/CLI selection, diffs, dismissals, and affected-package reruns after
stronger tests. Parser tests cover nested and multiline conditions, comments,
Unicode, CRLF, literal exclusion, and strict source matching.
The real-Go loop fixture verifies both header forms, unused-name preservation,
named/generic booleans, initializer/condition/body/post effects, saved selections,
dismissals, and a stronger-test rerun. Parser tests check excluded loop forms,
nested discovery, exact condition ranges, and multiline diffs.
Dismissal tests cover persistence, independent IDs, reason/note edits, removal,
strict rediscovery, moved checkouts, stale/missing/ambiguous entries, malformed
metadata, concurrent-writer exclusion, and preservation of reports, source files,
execution outcomes, counts, and rerun selection. Report tests cover frozen review
data, rerun ID remapping, omitted valid decisions, invalid snapshot rejection,
and compatibility with older reports. Browser snapshots and PTY tests cover review
filters, badges, reasons/notes, hidden marks, export, and checkout-free viewing.
Browser tests cover saved IDs, intersecting filters, marks hidden by filters,
quoted command export, missing/bounded artifacts, Unicode input, and frame
snapshots at wide and compact sizes. Unix PTY tests open reports with no Go or
checkout, exercise paste and resize, and verify shell/termios restoration after
quit, export, Ctrl-C, and SIGTERM without changing report files.
Renderer tests check complete frames, styles, clipping, elapsed-time updates, and
resizing, stable idle/busy rows, and results ordering. Progress tests cover partial
output lines, interleaved packages, stale events after worker reuse, full capture
preservation, and cancellation/deadlines during blocked progress observation.
Unix pseudo-terminal tests exercise live phases before command completion and
redraws during a blocked baseline, Ctrl-C, resizing, terminal initialization
failure, empty/failed runs, terminal restoration, and plain output in CI. A
terminal screen/scrollback model checks that pre-existing output and prompts
survive completion, cancellation, width/height changes, and oversized worker
views. A regression deliberately delivers frames prepared at the old size after
a resize, verifying history preservation and normal line wrapping afterward.
Temporary workspace paths are
canonicalized so macOS path aliases agree with Go discovery and internal symlinks.

## Other real-toolchain workflows

```sh
cargo test --test rust_workflow -- --ignored --test-threads=1
cargo test --test node_workflow -- --ignored --test-threads=1
cargo test --test bun_workflow -- --ignored --test-threads=1
cargo test --test python_workflow -- --ignored --test-threads=1
```

These need the pinned runtimes and fixture paths described in their acceptance
notes. Vitest workflows additionally use `MUTIE_VITEST_FIXTURE`; Python workflows
use `MUTIE_PYTHON`. Check the test's requirements before running it. The ordinary
suite keeps real Rust mutation workflows opt-in to avoid recursive self-testing.

When publishing benchmark results, remove personal paths and host metadata as
described in [the benchmark guide](benchmarks.md). Keep outcomes and unresolved
failures intact.
