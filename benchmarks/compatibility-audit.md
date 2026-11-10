# Routine project compatibility audit

Recorded 2026-11-09 against `53968c8`, using one frozen release build on macOS
arm64. This audit changes no production code or support boundaries.

**One of six selected projects completed Mutie's workflow.** The other five
stopped before a baseline. This is a deliberately varied sample, not a population
estimate: one new public project per runner, with real configuration, dependencies,
assets or workspace structure. JavaScript and TypeScript were not independently
sampled for every runner in this pass.

Subsequent bounded work is recorded separately: [Cargo build scripts](rust-build-scripts.md),
[installed Click](python-click-installed.md), [UFO lint/type tests](vitest-ufo.md),
and [Node composed tests](node-eslint.md). The Node follow-up preserves upstream
inputs but needs an explicitly approved addition of missing type declarations.
The table below remains the original audit result.

## Compatibility matrix

Upstream source, tests, manifests and locks were left unchanged. `init` wrote only
`mutie.toml` in complete onboarding copies; original checkouts stayed clean.
Explicit runner selection was tried when automatic detection failed. For UFO,
an external config allowed checking the adapter after init rejected its installer.

| Runner / unchanged project | Init | First observed check result | Later workflow |
| --- | --- | --- | --- |
| Go / [SJSON](https://github.com/tidwall/sjson/tree/f0069f8aa5171ab0fd2ca44b76c8ea243ead411d) | Automatic | Pass after allowing host Go cache access | 36-candidate sample; inspect; stronger tests; survivor rerun |
| Cargo / [ripgrep](https://github.com/BurntSushi/ripgrep/tree/178cc027e283fd965cb72d0a7395b219eada170c) | Automatic | Rejects custom Cargo profiles | Blocked; native full-workspace control passes |
| pytest / [Click](https://github.com/pallets/click/tree/8abb77a8432e03ed14a95f972b44f905279415a7) | Automatic with explicit pinned Python | Rejects development dependency groups | Blocked; installed native pytest control passes |
| Vitest / [UFO](https://github.com/unjs/ufo/tree/f06c800d0c59f2a4a1b9ba65eb6cb61a84419be6) | Rejects pnpm 10.33.2 | External-config check rejects missing `type: module` | Blocked; native suite not attempted |
| Node / [eslint-node-test](https://github.com/sindresorhus/eslint-node-test/tree/dcb850e2bd01534792fd67bcec4747828fa96df9) | Needs explicit runner | Rejects composed `scripts.test` | Blocked; native suite not attempted |
| Bun / [Elysia](https://github.com/elysiajs/elysia/tree/1cf5ce15dc4de23a96c769b5500a8d6fac8a8373) | Needs explicit runner | Rejects 40,379,030-byte test video | Blocked; native suite not attempted |

Automatic init succeeded for three projects; explicit selection produced configs
for two more. Init reports that it does not certify compatibility. No project
was trimmed, relabeled as dependency-free, upgraded, or assigned a replacement
test script to make admission pass. JS installations were unnecessary to establish
these static rejections, so they were not downloaded. Their native suite health
remains unmeasured.

## Blockers behind the first diagnostic

These are observations from the pinned manifests and adapter validation code,
not successful executions after removing the first obstacle:

- **ripgrep:** `profile.release-lto` inherits `release`, and `profile.deb` inherits
  `release-lto`. There is no custom dev/test profile in its manifest, so the current
  error text is misleading. The root's `build.rs` is a second explicit rejection.
  It reads Git revision information and emits platform-specific linker settings.
  The workspace has registry and internal path dependencies and an existing lock.
- **Click:** ordinary development/test/docs dependency groups fail the narrow
  metadata allowlist. Flit module metadata, uv/typing settings, pytest warning and
  marker configuration, skips, and CLI/filesystem/environment behavior also exceed
  current admission. Ignoring development metadata alone would not admit its suite.
- **UFO:** pnpm 10.33.2, absent root `type`, and locked Vitest 4.1.5 / Vite 8.0.10
  differ from accepted metadata/runtime recipes. The project uses TypeScript tests
  and a lint-plus-typecheck test script. Its recorded package versions were retained;
  installing Mutie's preferred versions would be an adapted-project experiment.
- **eslint-node-test:** `scripts.test` orchestrates lint and `test:*`; `test:js`
  invokes Node with `test/*.js test/unit/*.js`. Runtime/peer dependencies, root and
  `rules/` source layout, and JSON module imports add separate scope requirements.
  Selecting Node explicitly resolves detection, not execution compatibility.
- **Elysia:** the video exceeds the existing 8 MiB per-input limit. Composed Bun/
  typecheck/Node tests, runtime/peer dependencies, nested package manifests and
  TypeScript configuration are additional barriers. Raising the size limit alone
  would not make this framework supported.

## Native controls

Go 1.27.2, Rust/Cargo 1.99.1, Node 24.22.0, Bun 1.3.11+af24e281e and CPython
3.14.7 / pytest 9.0.2 were selected. Most rejected adapters stopped before launching
their test runtime. The host's newer default Node/Bun executables were not used
for adapter probes.

Ripgrep's unchanged tracked inputs pass
`cargo test --locked --workspace --tests --no-fail-fast`: **1,209 passed, one
ignored**, across 14 targets. A source copy without Git metadata passes in 18.82s;
its build script explicitly warns that the revision is unavailable. A second
control with Git metadata and a fresh target directory passes in 17.86s. This
demonstrates native test health and a Git/snapshot concern, not Mutie support.
The test command retains Cargo's ordinary omission of doctests and ignored tests.

Click's initial source-path-only control fails collection because a test reads
installed distribution metadata. Installing that unchanged copy into its own
temporary environment resolves this: **2,622 passed, 25 skipped, 31,000 deselected,
one xfailed**, in 5.40s including startup. The deselection is Click's existing
`not stress` configuration; no tests or settings were removed. The first failed
control is retained. These native skips/xfails and application effects remain
outside Mutie's current Python acceptance.

## SJSON: run, inspect, improve, rerun

SJSON supplies an ordinary Go module with `gjson`, `pretty` and indirect `match`
dependencies. Its unchanged **14-test** baseline passes. All-operator discovery
finds **646 candidates**. Before observing outcomes, the audit selected
`string_replace` and `negation_remove`: **36 candidates**, two workers, ten seconds
per candidate, full test scope and a 60-second baseline limit. No file filters
or test exclusions were used.

| Run | Tests | Killed | Survived | Build failed | Seconds | Selection score |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Original operator selection | 14 | 23 | 11 | 2 | 34.56 | 67.65% |
| Stronger survivor rerun | 19 | 5 | 6 | 0 | 11.92 | 45.45% |

No candidate timed out or became an execution error. The two build failures empty
parts of a `fmt.Sprintf` format string; normal Go vet checks reject the resulting
argument/directive mismatch. They stay outside the score denominator. This small
sample's invalid-candidate rate is 2/36; it does not measure all 646 candidates.

Inspecting saved diffs and outcomes led to five
[public-API tests](fixtures/compatibility-audit/sjson_contracts_test.go) in a
separate complete copy:

| Original ID | Assertion that kills the survivor |
| --- | --- |
| 16 | Setting index `0` on JSON `null` creates `[7]`. |
| 27 | Optimistic raw replacement writes a nested object rather than returning the old value. |
| 28 | Optimistic string replacement writes the shorter value. |
| 31 | Unsupported complex-path deletion returns a nonempty diagnostic and preserves input. |
| 36 | Setting a Boolean to false writes JSON `false`. |

All 11 survivors were rerun. IDs **9, 14, 15, 18, 26, 34** remain open; some
involve fallback/optimization or diagnostic paths, but none is declared equivalent
or dismissed. The original 67.65% score and reports remain unchanged. Neither
score is a full-project score. Timings are single local observations, not controlled
performance comparisons; the upstream suite also contains randomized probes.

Initial sandboxed Go check/list attempts could not access module/build caches.
Their failures are retained, followed by successful host-access retries. Those
permission failures are environmental, not project incompatibility.

## Recommended next implementation goal

**Admit ordinary Cargo build scripts and inactive custom release profiles, using
unchanged ripgrep as the acceptance project.** This is the most contained route
from a demonstrated rejection to another complete workflow in this sample.
Rust's native tests already pass, and its Cargo execution path already handles
registry build dependencies. The other blocked projects require several more
independent runner/configuration changes.

The bounded acceptance contract should cover:

1. Profiles unused by `cargo test`, including inheritance between custom release
   profiles, with accurate diagnostics for unsupported active test settings.
2. Project build-script targets, inputs and outputs in owned workspaces and saved
   execution identity; source/input drift and failed scripts must stay explicit.
   Keep build scripts outside mutation discovery for this first slice.
3. Fresh and worker-cached builds, generated output cleanup, rerun invalidation,
   and an explicit Git/environment contract. Cargo can retain `OUT_DIR` between
   builds, so merely removing the current manifest rejection is insufficient.
4. Ripgrep's full workspace baseline, a bounded member/operator selection, useful
   separate-copy stronger tests and a focused rerun, with original inputs/reports
   preserved and no changes to its manifest or suite.

Cargo documents [profile selection and inheritance](https://doc.rust-lang.org/cargo/reference/profiles.html)
and [build-script lifetime/output behavior](https://doc.rust-lang.org/cargo/reference/build-scripts.html).
General arbitrary profiles, proc-macro project members, out-of-tree dependencies,
cross-compilation and Windows remain separate scopes. This audit does not implement
the proposed goal or claim it will resolve every possible ripgrep admission check.

Follow-up: [owned Cargo build-script acceptance](rust-build-scripts.md) now
implements that slice, plus the explicit inactive target-config check discovered
next. Unchanged ripgrep completes its full workspace baseline and a bounded
mutation/stronger-test/rerun workflow. The table above remains the original audit
result at its recorded revision.

## Evidence and limits

The [results ledger](compatibility-audit-results.json) records exact upstream
commits, command outcomes/timings, source identities, all **1,239 tracked input
hashes**, raw-log hashes, and the **160 original / 60 stronger** SJSON report-file
hashes. The [verifier](fixtures/compatibility-audit/verify.py) confirms clean
original checkouts, unchanged onboarding/native copies, exact source-ID mapping,
and the sole added SJSON test file. It exports no machine-specific paths.
Raw checkouts, environments, logs and reports stay in the temporary audit directory;
no upstream codebase or dependency installation is vendored here.

[Reproduction instructions](fixtures/compatibility-audit/README.md) retain the
bounded harness and original failure attempts. Scripts were exercised against
all recorded artifacts; the final preservation verifier and whitespace/format
checks pass. Production code is unchanged, so the earlier 369-test unit/integration
suite was not needlessly repeated. This is macOS evidence; no remote CI ran.

The routine compatibility milestone remains open. Later work still needs broader
JS/TS dependency/configuration admission, independent syntax samples, supported
runtime-version evidence, representative regression automation and Linux/macOS
release qualification. Completing operator parity alone does not establish those.
