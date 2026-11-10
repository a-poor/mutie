# Unchanged UFO: pinned tools, lint and type tests

UFO at [`f06c800`](https://github.com/unjs/ufo/tree/f06c800d0c59f2a4a1b9ba65eb6cb61a84419be6)
now completes discovery, isolated runs, report review and a stronger-test focused
rerun without changing its source, existing tests, manifest, lockfile or configuration.
Two added public-API tests catch previously surviving hostname-encoding boundaries.

The [results](fixtures/vitest-ufo/results.json) retain input and report hashes,
mutation identities, outcomes and timings. The [reproduction script](fixtures/vitest-ufo/reproduce.py)
uses an already installed checkout and separate original/stronger copies.
The [stronger tests](fixtures/vitest-ufo/stronger-host.test.ts) are new Mutie test code;
UFO's codebase is fetched separately, not vendored here.

## Scope

The unchanged checkout uses Node 24.22.0, pnpm 10.33.2, Vitest 4.1.5, Vite 8.0.10,
TypeScript 6.0.3, ESLint 10.2.1, eslint-config-unjs 0.6.2 and Prettier 3.8.3.
Dependencies were installed with the original frozen lockfile and scripts disabled.
The native `pnpm test` command passes lint/formatting and reports 491 tests across
14 files, including two type-test cases. Its test recipe is:

```json
{
  "test": "pnpm lint && vitest run --typecheck",
  "lint": "eslint . && prettier -c src test"
}
```

Mutie recognizes this complete recipe and runs its commands directly in each
owned copy. It also accepts `vitest run --typecheck` with the audited compiler.
It does not execute arbitrary package scripts or install dependencies. Lint and
type failures remain errors; a formatting-sensitive mutation can therefore stop
a run before runtime tests. This is not general shell-command support.

The package has no `type` declaration. This extension admits `.ts` sources and
explicit `.mjs` tooling while rejecting `.js`, `.cjs`, `.cts`, JSX and TSX inputs
in that implicit TypeScript layout. The pnpm version, local isolated store,
manifest/lock agreement and complete installed bytes remain checked. The new
runner pair is exact; existing Vitest 4.1.9 / Vite 8.1.3 or 8.3.4 behavior remains
covered separately. Unknown runner bytes still fail before instrumentation.

Vitest 4.1.5's worker differs from the audited 4.1.9 worker at two concurrent
scheduling sites. Its failure/fixture dispatch anchors are identical. An
additional exact SHA-256 admits those bytes; body, hook, cleanup, nested fixture
and mixed-failure controls run against the real older runner.

The ESLint configuration must be a static `eslint-config-unjs` import and literal
`unjs({ignores:[],rules:{...}})` call. Numeric rule levels are admitted; additional
code and file ignores are rejected. An empty JSON `.prettierrc` and one global
EditorConfig `root = true` prevent inherited formatting settings. All three files
join execution identity. Other lint configurations and tools require new evidence.

## A passing type-test label was insufficient

UFO's original tsconfig includes only `src`. Native Vitest collects and labels
the two `test/types.test-d.ts` cases as passing even though the compiler omits that
file. A separate-copy negative control changes one expected result from its actual
object type to `number`; unchanged `pnpm test` still exits successfully with all
491 labels passing. This is a native configuration gap, not proof that the assertion
is valid. Vitest documents [type tests](https://vitest.dev/guide/testing-types) and
their separate [selection options](https://vitest.dev/config/typecheck).

Mutie creates an owned supplemental tsconfig that extends the original and adds
every selected `.test-d.ts` / `.spec-d.ts` file. It retains the original compiler
options and source checks. Completion requires a clean compiler exit, no source
errors, matching scheduled/completed type modules and compiler `--listFiles`
evidence for every type-test file. The same false assertion now rejects the
baseline; no mutations run. Type-test failures cannot supply body-kill evidence.

The copied pnpm `tsc` launcher contains absolute `NODE_PATH` entries pointing at
the original installation. Mutie bypasses it with an owned launcher using the
pinned Node executable and isolated compiler. Build-info output uses a fresh
private path. A wrapper bounds combined compiler output at 1 MiB before Vitest
can buffer it internally. Lint, formatting and Vitest share one command timeout
and external capture budget. Compiler exit, signal, timeout and output controls
remain errors/timeouts as appropriate, with owned process-group cleanup.

## Run, inspect, improve, rerun

These final runs use the same production recipe fingerprint recorded in the
results. Each mutation retains the full suite, including lint and actual type-test
compilation. Selection filters restrict mutation sources, not tests.

| Selection | Attempted | Killed | Survived | Errors | Pending | Wall time |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| All boolean flips | 4 | 4 | 0 | 0 | 0 | 119.77 s |
| `src/punycode.ts` comparison boundaries | 17 | 8 | 9 | 0 | 0 | 336.38 s |
| `src/query.ts` comparisons | 4 of 9 | 2 | 1 | 1 | 5 | 80.58 s |
| Two stronger-test boundary reruns | 2 | 2 | 0 | 0 | 0 | 69.11 s |

The original punycode selection score remains **47.06%**. The query run stops on
an actual source type error after comparison inversion; its score stays unavailable
and its five unattempted mutations stay pending. These selections are not a full
project score. Wall times include snapshots/validation and overlapped other local
acceptance checks; they are not a controlled performance comparison.

The two findings are the inclusive high-surrogate checks at `src/punycode.ts:51`:

- Original report ID 5 changes `>= 55296` to `> 55296`. A test of
  `encodeHost("\u{10000}.example")` expects `xn--2n7c.example`.
- Original report ID 6 changes `<= 56319` to `< 56319`. A test of
  `encodeHost("\u{10FC00}.example")` expects `xn--4s22g.example`.

The expected encodings agree with Python's punycode codec and Node's punycode
implementation. The second value is a private-use code point: this checks UFO's
raw hostname encoding, not IDNA domain validity. Both tests exercise the exported
API and pass against the unchanged implementation. The stronger native suite has
493 test labels, and Mutie's focused rerun also compiles the two type tests.
Both original survivors become kills. Focused 100% does not replace the original
score or establish a full stronger-suite score. Seven other punycode survivors,
the query survivor, its type error and five pending query candidates remain open;
none is dismissed or declared equivalent.

All 39 original tracked inputs match in the upstream checkout, native baseline,
original and stronger copies. All 254 report files present before the stronger rerun are unchanged; a final
query confirmation brings the retained original report inventory to 288 files.
Earlier diagnostic runs remain preserved but are not substituted for the final
rows above. Source fingerprints and original byte spans map the two findings to
their new report-local IDs.

## Verification and CI

Local validation passed on macOS:

- 380 ordinary Rust tests, formatting and Clippy with warnings denied.
- Five Vitest 4.1.5 CLI/report/review workflows, two lifecycle groups, native
  outcome controls and timeout/cancellation/capture/isolation controls.
- All four existing Vitest 4.1.9 native execution groups.
- Three recipe-control groups covering compiler participation and failure,
  lint/configuration drift, combined capture limits and a shared deadline.
- The new small fixture's frozen install, native command, and production check;
  workflow YAML parsing and syntax checks for all 35 shell steps.

The sandbox denied two signal operations in the ordinary cancellation suite.
The authorized run with process permissions passed; deadlines/assertions were
not relaxed. The separate, previously deferred macOS startup investigation remains
unresolved.

Pushing the preceding work exposed one stale Python CI control: it wrote 5 MiB
after the production file limit had become 16 MiB. Commit `afb59d6` changes that
control to 17 MiB; its local regression and all nine jobs in
[Linux CI](https://github.com/a-poor/mutie/actions/runs/42729147740) pass.
This local acceptance record predates the new Vitest compatibility job's first
remote run. No project, schema or catalog versions were changed.
