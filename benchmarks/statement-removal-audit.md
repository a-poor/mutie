# Statement/block-removal assessment

The existing branch, conditional-loop and call operators now have scoped parity.
This assessment adds one useful form beyond them: Python `assignment_remove`
for single-target attribute/item writes. Arbitrary statement or block deletion
is not implemented. Go and Rust assignment removal are subsequently validated in
[Go acceptance](go-assignments.md) and [Rust acceptance](rust-assignments.md).
JS/TS support was subsequently validated in its own
[assignment-removal acceptance](javascript-assignments.md); this assessment's
scored evidence remains specific to Python.

## What is worth removing?

| Form | Assessment |
| --- | --- |
| Standalone calls | Already supported; no additional blanket statement candidate needed. |
| Entire branch/loop bodies | Overlaps existing forcing/skipping, but is not equivalent: keeping condition effects, declarations and exits needs a separate recipe. |
| Local declarations/name bindings | Removing them can change lookup, initialization or type checking. The Python native control demonstrates a local read silently becoming a global read. Excluded from this slice. |
| Attribute/item assignments | Individual state updates are not represented by changing a value or deleting a whole enclosing call. Implemented for scoped Python statements, with concrete rotation findings. |
| Constructor stores | All 16 tested Route Mini deletions errored. Excluded from final discovery, including nested constructor subtrees. |
| Return/break/continue/raise and general blocks | Changes reachability, termination and binding/lifetime structure. Requires independent candidate-quality and semantic evidence. |
| Cleanup/exception/context-manager structure | General deletion could change lifecycle evidence. No observer, classifier or scope restrictions were relaxed to admit it. |

The language review used the primary specifications for
[Python assignment/pass](https://docs.python.org/3.14/reference/simple_stmts.html#assignment-statements),
[Rust assignment](https://doc.rust-lang.org/reference/expressions/operator-expr.html#assignment-expressions),
[Go assignment](https://go.dev/ref/spec#Assignments), and
[JavaScript assignment](https://tc39.es/ecma262/multipage/ecmascript-language-expressions.html#sec-assignment-operators).
Evaluation order, binding and overloaded/property effects differ by language;
there is no shared text-deletion rule justified by this review.

## Implemented Python scope

An ordinary single-target `Assign` to an attribute or subscript becomes `pass`.
Examples are `node.parent = parent`, `items[key] = value` and slice assignments.
The whole RHS, target lookup, index/slice and store are skipped. Parentheses,
multiline byte spans, surrounding comments, semicolon neighbors, empty suites and
docstring positions remain valid.

Excluded forms are local-name bindings, tuple/list unpacking, chained targets,
annotated/augmented assignments, test and annotation subtrees, `__init__`/`__new__`
subtrees, and assignments containing named expressions, yield/yield-from or await.
The constructor rule is syntactic and conservative, including nested functions.
Other methods may still initialize state: a missing attribute remains an error.
No type inference, runtime admission or exception reclassification is added.

Native discovery marks assignment context separately from calls, which share
the exact `pass` wire recipe. Rust validates the marker/recipe, and fresh
AST/source/operator matching rejects a forged name assignment or a store labeled
as a call before execution. Recipe bytes bind reruns to the final scope. Package,
report and catalog versions remain unchanged.

```toml
[python]
# Set executable to the pinned environment as usual.
operators = ["assignment_remove"]
```

## Candidate-quality measurement

The audit used unchanged
[Binary Tree Library](https://github.com/burpeesDaily/sphinx-sample-code) at
`68d688fb4fe9c867c77a9d54f4fdb46a58dea34e` and
[Route Mini](https://github.com/JustinJLeopard/route-mini) at
`35a3db157731ba0073d66f432b69b997ce2e9f01`. Both were already admitted projects;
no upstream source, tests, manifests or layout were adapted.

The draft selector included constructors. Each first run stopped on a phase
error; remaining candidates ran as independent focused selections with fresh
full-suite baselines. No continuation rule was relaxed. Every original candidate
is accounted for exactly once in the combined counts:

| Selected source | Tests | Candidates | Killed | Survived | Timed out | Errors |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Binary Tree Library `avl_tree.py` | 7 | 32 | 16 | 7 | 6 | 3 |
| Route Mini application files | 68 | 16 | 0 | 0 | 0 | 16 |

These are combined inventory counts, **not full mutation scores**. All original
failed reports stay failed/unscored. Tree removals have substantial residual noise:
9/32 timed out or errored, and survivors require review. Timeouts retain the
10-second bound; no hang is counted as a kill. All Route Mini candidates were
constructor stores, motivating their exclusion rather than shipping 16 guaranteed
errors for that unchanged suite.

With the final recipe, Route Mini's 68-test baseline passes and discovers zero
assignment candidates. The tree inventory stays exactly the same 32 source
identities; its final full run again stops after one kill and one setup error,
leaving 30 pending. This remains an honest failed report.

## Useful findings under the final recipe

Two independent stronger tests in a separate tree copy verify a six-key insertion
sequence that triggers left rotation with a nonempty moved subtree:

- Source ID 8, line 232: removing `node_y.left.parent = node_x` leaves the moved
  subtree attached to the wrong parent. The added test checks both directions of
  the parent/child relationship through objects returned by the public API.
- Source ID 10, line 237: removing `self.root = node_y` leaves the old root exposed.
  The added test checks the expected public root and its absent parent.

The final original focused run preserves both survivors (7-test baseline,
16.35 seconds). The stronger copy passes nine pristine tests and kills both
in 14.68 seconds, without errors or timeouts. The focused 100% result is not a
full stronger-project score. Five other draft survivors and all six timeouts
remain open; no equivalence or dismissal is claimed.

The [retained tests and verifier](fixtures/python-assignments-tree/verify.py)
check all 33 tree and 25 Route Mini tracked files, clean original commits,
the sole stronger-test addition, complete draft inventory accounting, draft and
final recipe hashes, and source identity across report-local IDs. They preserve
1,013 draft report files (including the preliminary stronger run) and 94 final
original-report files. The previous call-removal campaign's 44 original report
files were also rehashed unchanged. [Results](python-assignments-results.json)
retain relative run paths, inventories, per-report digests and outcome accounting.

```sh
python3 benchmarks/fixtures/python-assignments-tree/verify.py \
  /tmp/mutie-statements-audit /tmp/mutie-statements-route-audit \
  benchmarks/python-assignments-results.json
```

The verifier reconstructs the draft selector by removing only the documented
constructor exclusion. It requires the preserved draft scope bytes to match;
draft evidence is not relabeled as execution under the final recipe.

## Validation and remaining reliability finding

- 81 native/static Python controls, including assignment inventory, UTF-8/CRLF
  spans, native AST equivalence, RHS-before-target order, skipped descriptors and
  item stores, constructor/binding/suspension exclusions and generator/docstring
  preservation.
- Two new scored workflows: kill/survivor/stronger rerun, review versus raw score,
  corrupt recipes, forged bindings and cross-family identities, source drift,
  missing initialization and mixed body/fixture-cleanup errors. Both existing
  call workflows and the static suite pass too (five opt-in workflow tests).
- Final `cargo test --locked -- --test-threads=2`: 345 passed, 174 ignored.
  Clippy with warnings denied, Rustfmt and whitespace checks pass.
- The pushed call-removal commits passed all nine
  [Linux CI jobs](https://github.com/a-poor/mutie/actions/runs/42637937662).
  The new assignment tests are wired into the Python operator partition locally;
  after pushing this slice with the cancellation fix, all nine
  [Linux CI jobs](https://github.com/a-poor/mutie/actions/runs/42648367150) passed.

An earlier unrestricted-concurrency local suite failed the existing PTY Ctrl-C
test: one worker's SIGINT exit was saved as an error while another was interrupted,
making the report `failed` instead of `interrupted`. The isolated recheck and final
two-thread suite passed unchanged. At that point this remained an unresolved
timing-sensitive classification observation, distinct from the deferred pre-main
loader issue. No cancellation deadline/assertion or production process code changed
in the assignment-removal slice. Local logs
are `/tmp/mutie-assignments-full.log`, `mutie-assignments-cancellation-recheck.log`
and `mutie-assignments-full-final.log`.

The subsequent [cancellation investigation](cancellation-exit-race.md) reproduces
the child-exit race and fixes it without relaxing deadlines or hiding genuine
errors. That later validation does not replace this audit's original results.
