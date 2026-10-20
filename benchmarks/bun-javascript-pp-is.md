# Bun JavaScript acceptance: pp-is

Recorded 2026-10-20 on macOS arm64 with pinned Bun 1.3.11+af24e281e and a debug
Mutie executable. Project: [pp-is](https://github.com/carlos-sweb/pp-is), commit
`1d5b3bac804e1e67da289f3d2a14bd0705a383fb` (package 2.1.0).

## Scope adaptation and controls

This is an **adapted real-project acceptance**, not unchanged-checkout support.
The upstream package has no explicit ESM declaration and contains generated
`dist/`, a root declaration file and a `.mjs` build script. The current validator
rejects it. A temporary scoped copy retains every byte of every upstream `src/`
and `test/` file. Its manifest adds only `type: module` (and JSON formatting).
Other packaging/documentation files are omitted from that copy. No production or
test semantics are rewritten, no dependency is installed and no test is excluded.
The package's `scripts.test` is already `bun test`.

Before adaptation, the original checkout passes 58 native tests across three files
with 177 assertions. The scoped copy passes the same 58-test isolated baseline.
The pinned executable is the separate temporary binary used for the earlier Bun
audits; the user's installed runtime remains unchanged. All input hashes and the
precise omitted-file list are retained in the results record.

Candidate screening also inspected node-diff3/osm-auth (`.mjs`, declarations,
custom commands or test dependencies), mfers (generated dist and JSON/directory
imports), and several dependency/workspace projects. This did not justify quietly
loosening import, discovery or dependency validation. Unchanged package support
for pp-is remains a separate scope-expansion task.

## Full run and useful findings

The full adapted inventory contains 22 boolean flips. All were attempted with a
five-second mutation deadline and 15-second baseline deadline: 12 killed,
10 survived, zero timeouts and zero execution errors, in 228.86 seconds overall.
The full score is 12 / 22 = 54.55%. Individual test commands took roughly
0.06–0.18 seconds; copying, process setup and debug identity hashing are included
in total run time but outside captured test-command durations.

Seven stronger tests address observable boundaries:

- Non-string inputs for `isAlphanumeric`, `isHexColor`, `isIPv4`, `isLowercase`,
  `isUUID` and `isUppercase` must return false.
- IPv4 octets containing letters or empty text must not become valid just because
  the other octets are valid.

The [retained tests](fixtures/bun-pp-is/stronger.test.js) live only in a second
copy. The rerun selects all ten original survivors, preserving saved-ID mappings
and validating runtime/config/recipe identity. The 65-test stronger baseline
passes (185 assertions across four files). The focused rerun kills original IDs
4, 10, 11, 13, 17, 21 and 22, while IDs 6, 7 and 8 survive: seven kills,
three survivors, zero timeouts/errors in 85.96 seconds. Its selected score is
70%; the original full score remains 54.55%. These are separate runs, not a
new full-inventory score.

All 63 original checkout files and all 53 runtime source/test files in both
copies retain their hashes. Every original saved report file is unchanged.
[Machine-readable results](bun-javascript-pp-is-results.json) retain full reports,
identities, inventory, outcome and preservation records. Local report paths are
diagnostic references; retained data and stronger tests are portable evidence.

Three findings intentionally remain open: IDs 6 and 7 change primitive boolean
shortcuts whose ordinary outcomes are also covered by `Object.prototype.toString`;
this is evidence of redundancy under normal built-ins, not universal equivalence
under arbitrary monkeypatching. ID 8 changes `isEmpty`'s fallback for values outside
strings/arrays/objects; the documented contract does not clearly settle that
policy. No dismissal or forced test expectation alters their original scores.

## Acceptance limits

Independent JS and TS fixture review/policy, failure attribution and cleanup
regressions are recorded in [review acceptance](bun-dismissals.md) and
[scored cleanup](bun-scored-cleanup.md). This independent JavaScript project adds
real findings and a stronger-test loop; it does not establish arbitrary package
layouts or native unmodified pp-is compatibility. Existing pinning, serial/full
suite, dependency-free imports, static scope and fail-closed classification remain
unchanged. No versions or production adapter code change for this acceptance.

JavaScript is now independently Validated within the supported explicit ESM
scope, based on this documented adaptation. TypeScript retains its independent
Composer acceptance. Neither promotion broadens accepted checkout layouts.
`git diff --check`, `cargo fmt --check` and results JSON parsing pass; no Rust
behavior changed, so the existing fixture/review/cleanup suite was not rerun.

Follow-up: `benchmarks/bun-unchanged-layout.md` records support for the original
checkout with matching outcomes; the adaptation above remains historical evidence.
