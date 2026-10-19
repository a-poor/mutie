# Native TypeScript on Node: first scoped execution

Recorded 2026-10-18 on macOS with Node **24.22.0**. Native TypeScript is now
**Experimental** through `mutie PATH --lang node`. This record is independent of
Node JavaScript's Validated milestone and Vitest's TypeScript acceptance. No
package, report-schema or operator-catalog version was changed.

## Execution and syntax contract

[Node's pinned-major TypeScript documentation](https://nodejs.org/docs/latest-v24.x/api/typescript.html)
was served as v24.22.0 during this audit. Native execution removes erasable types;
it does not type-check, read tsconfig transforms, or rewrite import paths. This
adapter uses that default mode, without loaders or experimental transforms.

- One dependency-free ESM package on the exact audited Node runtime, no installed
  `node_modules`. Unused build-only devDependencies remain allowed.
- `.ts` and `.js` under `src/` and `test/`, static literal relative imports with
  real extensions or `node:` built-ins. Both `import type` and inline type imports
  are supported; erased local imports must still resolve within the audited set.
- Exact test script `node --test test/*.test.ts` or the existing `.js` variant.
  Only the declared immediate-directory glob is selected. JS tests may import TS
  source, and TS tests may import JS helpers. Files and Mutie workers remain serial.
- Boolean flips use the shared TypeScript grammar and original byte ranges.
  Types, interfaces, generic arguments/constraints and return annotations produce
  no candidates. `as`, `satisfies`, non-null assertions and erasable function/class
  syntax execute natively.
- Enums, all namespaces (including type-only ones), parameter properties, import
  aliases, angle-bracket assertions, decorators and ambient declarations are
  rejected by the Node-specific audit. `.d.ts`, TSX, MTS/CTS and CommonJS remain
  outside this initial scope. These are adapter limits, not claims that all such
  features are impossible in Node. Vitest's syntax contract is unchanged.
- Dynamic imports, runtime packages, custom runner flags, native default recursive
  test discovery and build steps remain unsupported.

An optional root `tsconfig.json` must be a regular file containing plain JSON.
Accepted top-level keys are `compilerOptions`, `include` and `exclude`; config
extends/references, aliases, plugins and nested configurations are rejected.
Supported options cover NodeNext module/resolution settings, ES2023/ESNext target,
`types`/`lib` string arrays and the explicit type-checking boolean allowlist in
`src/node/typescript.rs`. `noEmit`, `allowImportingTsExtensions`,
`erasableSyntaxOnly` and `verbatimModuleSyntax`, when present, must be true.
This is an audit of ignored type-checking metadata: neither includes/excludes nor
compiler options alter Mutie's execution, discovery or full declared test suite.
No `tsc`, install or package script is executed.

The native TS identity records the test glob and optional config fingerprint;
config absence is significant too. It has a distinct runner recipe while retaining
the same exact native hook hash, observer and reporter. Config is checked again
inside each command snapshot. Source/manifest/config changes invalidate focused
reruns; test-only edits remain allowed. Saved commands select the correct glob,
and new text summaries explicitly state that no type checking occurs. The existing
JavaScript recipe and serialized records stay compatible.

## Fixture and failure evidence

[The typed fixture](../tests/fixtures/node-typescript/README.md) contains exactly
three runtime booleans: two kills and one survivor. Aliases, interfaces, generics
and literal return types add no candidates. The fixture imports a JS helper and
contains an intentionally ill-typed `const runtime: true = false` in its test:
the baseline passes, proving that type errors are not being treated as compiler
failures. Mutating a `(): true` function to return false is judged by its executed
assertion. A stronger fallback assertion kills the saved survivor.

The opt-in workflow verifies reports, noninteractive browser validation, saved
command globs, dismissal CRUD/check without Node on PATH, separate raw-score and
findings policies, test-only reruns, original source/report preservation, and
rejection of changed manifests or edited/deleted configs. A corrupted TS report
cannot be relabeled with the legacy JS identity to admit TS mutation records.
A mixed JS-test/TS-source run also produces two kills and one survivor. Importing
a type as a runtime value fails the baseline and earns no kills.

All **37** existing mutation-triggered attribution cases are repeated with TS
entry points and typed production functions. Five additional cross-file/late
mixtures bring this to **42** TS cases: body assertion/generic/async/callback
failures can kill; plans, static module evaluation, hooks, suppressed body-plus-
cleanup failures, unhandled process errors, early exits and incomplete/empty
results cannot. No change was made to the fail-closed observer/protocol.

Independent TS cleanup exercises timeout, cancellation and capture overflow with
a spawned descendant, checking process termination, owned workspace/temp deletion,
source preservation and classifier precedence. Real Node tests remain ignored in
the default suite to avoid recursive self-mutation campaigns.

## Real projects and adaptation

[Machine-readable results](node-typescript-results.json) retain commits, complete
file hashes, execution identities, mutation ranges/outcomes, durations and report
paths.

1. [dependency-atlas](https://github.com/SamGo1006/dependency-atlas) at
   `4ee7e870f273fa068bf37d9fa81a84ff36fa5eda` fits the exact native scope unchanged.
   Its original nine tests pass through Mutie's isolated baseline. Discovery finds
   zero runtime boolean literals, so the report correctly has no mutation score.
   This is baseline/import/config acceptance, not useful-findings acceptance.
2. [Bragkit](https://github.com/artmsilva/bragkit) at
   `458d31127f1d3a6f3fd28afd2d7a4741ebc648a7` provides a useful candidate set.
   It has no runtime dependencies; original TS sources, tests and config were
   preserved. Its original test script is `node --test`, which this adapter still
   rejects. In a separate copy, only `scripts.test` was changed to
   `node --test test/*.test.ts`. No package tooling was installed or built.

Direct serial runs on unchanged Bragkit using native default discovery and the
explicit file list both pass the same **61 named tests** across ten test files.
The names are retained and compared, not merely the counts. This establishes
suite equivalence for this checkout; it does not implement general default
Node discovery. Other candidate projects had runtime package imports, dynamic
imports, incompatible layouts, or no boolean sites; their existence does not
broaden the chosen contract.

The adapted Bragkit full scan passes its 61-test baseline and attempts all **23**
candidates in **31.17 seconds** (saved run duration): **2 killed, 21 missed,
0 errors, 0 timeouts**. Its raw score is **8.70%**. Existing tests kill the
missing-user-mention branch in Slack's kudos helper (ID 18) and recursive config
directory creation (ID 19). Native TS types alone neither kill nor rescue them.

## Inspect, strengthen, rerun

The three PDF-module list-state survivors have visible HTML consequences without
launching a browser. [Two retained tests](fixtures/node-bragkit/mutie-html.test.ts)
exercise the public `htmlWrap` function in a separate review copy:

| Original ID | Mutation | Observable regression |
| --- | --- | --- |
| 20 | Initial `listOpen: false → true` | A heading/paragraph closes a list that was never opened |
| 21 | Closing a list sets `listOpen: false → true` | A later list fails to open its container |
| 22 | Opening a list sets `listOpen: true → false` | Consecutive items open repeated containers and omit closing tags |

The stronger full-suite baseline passes **63 tests**. A saved-ID rerun of 20–22
kills **all three** in **11.00 seconds** (saved duration), with no errors/timeouts.
Its 100% score belongs only to this three-candidate rerun; it is not a replacement
full-project score. These timings overlapped other checks and are not a benchmark
comparison. Eighteen other original survivors—CLI argument/default/directory
behavior, collector settings/error responses and browser fallback—remain
unreviewed; none is labeled equivalent.

Hash checks preserve all **57** original Bragkit files, all 57 adapted input files
in both the scoped and review copies (the review adds only the retained test),
and all **105** original report files. The only difference between original and
scoped input is the declared test script. The unchanged dependency-atlas checkout
also has a clean Git status. Local reports live under the paths in the JSON record;
the repository retains the evidence and stronger test, not generated run folders.

## Reproduction and remaining gate

Use audited Node 24.22.0 on PATH. For Bragkit at the recorded commit, copy the
checkout, change only the exact test script as described above, then:

```sh
mutie /path/to/scoped-bragkit --lang node --list
mutie /path/to/scoped-bragkit --lang node --ui plain --timeout 10 --baseline-timeout 15
# Copy the retained test into a separate review copy's test/ directory:
mutie /path/to/review --lang node --ui plain --timeout 10 --baseline-timeout 15 --rerun ORIGINAL_RUN --id 20,21,22
cargo test --test node_workflow -- --include-ignored --test-threads=1
cargo test --bin mutie node::tests -- --include-ignored --test-threads=1
cargo test
cargo fmt --check
cargo clippy --all-targets -- -D warnings
```

The default suite passes **247 tests**, with **51 ignored**. Node workflow and
cleanup checks pass separately for both syntaxes, alongside formatting and strict
Clippy. The retained evidence predates the final text-summary wording correction;
execution recipes, saved TS identity and outcomes are unchanged, and automated
workflow checks verify the corrected summary.

Keep TypeScript **Experimental**. The next gate is safe native `node --test`
discovery plus unchanged Bragkit acceptance, or another unchanged project with
useful mutation findings. Continue the 18-survivor review separately. Runtime
versions, installed dependencies, loaders/transpilation, declaration files and
wider layouts each need explicit acceptance; JavaScript evidence does not grant it.
