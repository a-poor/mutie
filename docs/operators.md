# Mutation operators

[Documentation index](README.md) · [Project overview](../README.md)

Support differs by language and applies within each runner's validated scope:

See the [operator table](../README.md#status) for availability by language.

JS/TS comparison inversion also supports `=== ↔ !==`, preserving strictness:
`==` never becomes `!==`. Both comparison operators replace only the parsed
operator token, preserving grouping and operand evaluation. JavaScript's native
coercion, BigInt, string and `NaN` behavior applies; ordering inversion is not a
Boolean negation for unordered values. Nested comparisons remain native binary
expressions, and type syntax, `in`, `instanceof`, shifts and tests are excluded.
No type checking or exception reclassification is added. See
[semantic controls, six runner/syntax workflows and unchanged pp-is findings](../benchmarks/javascript-comparisons.md).

JS/TS logical swaps replace `&& ↔ ||` while parenthesizing the parsed operands
and result to preserve grouping. Short-circuit decisions intentionally change;
operands still run only as required by the swapped operator and may return
non-boolean values. Negation removal replaces `!value` with `(value)`, preserving
its raw value and evaluating it once. `!!value` has two independent candidates.
TypeScript postfix non-null and definite-assignment assertions, nullish operators,
logical assignments and bitwise operators are excluded. See
[semantic controls and unchanged-project findings](../benchmarks/javascript-logical.md).

JS/TS `arithmetic_replace` supports binary `+ ↔ -`, `* ↔ /` and `% → *`
(JavaScript `%` is remainder). Parsed operands and results are parenthesized,
retaining grouping, comments and evaluation order. Native string concatenation,
coercion, BigInt and Number semantics apply: zero division may produce Infinity,
NaN or a BigInt exception. No numeric type inference or exception reclassification
is added. Unary signs, updates, augmented assignments, powers and types remain
excluded. See [semantic controls and unchanged Bragkit findings](../benchmarks/javascript-arithmetic.md).

Python comparison inversion replaces `== ↔ !=`, `< ↔ >=`, and `<= ↔ >` tokens
in single comparisons. Native rich-comparison/reflected dispatch and raw results
are preserved; ordering inversions need not complement NaN results. `not` removal
parenthesizes the original operand, returning its raw value without truthiness
conversion and evaluating it once. Nested `not` expressions are separate sites.
Chained comparisons, `is`/`in` operators, bitwise `~`, annotations and tests remain
excluded. Runtime exceptions remain errors. See
[semantic controls and unchanged-project validation](../benchmarks/python-inversions.md).

Python `logical_swap` replaces the `and`/`or` keyword in binary Boolean
expressions. Native grouping, short-circuit evaluation and raw operand values
are retained; newly evaluated operands may raise errors. Nested binary expressions
are independent candidates. Flat chains of three or more operands (`a and b and c`)
are excluded at that chain node; eligible nested binary expressions remain eligible.
Bitwise operators, annotations and tests are excluded. See
[semantic controls and unchanged Route Mini diagnostic findings](../benchmarks/python-logical.md).

JS/TS `call_remove` replaces a complete ordinary call statement with `;`.
It skips receiver/property/argument evaluation and the call itself. Parenthesized,
method, computed and optional calls are supported; surrounding branches, labels
and statement boundaries remain intact. Calls used as values, `new`, direct
`super()`, dynamic import, tagged templates, and call subtrees containing `await`
or `yield` are excluded. Unawaited ordinary calls may return promises: no static
return-type inference is attempted, and lifecycle failures remain errors. See
[controls and unchanged-project finding](../benchmarks/javascript-calls.md).

JS/TS `assignment_remove` replaces a standalone property/index `=` statement
with `;`, such as `state.value = value` or `items[key] = value`. It skips the
receiver, key, RHS and store effects, preserving branches, labels, ASI and
directive boundaries. Parenthesized statements and native erasable TS expressions
are supported. Bindings, destructuring, compound/chained assignments, updates,
value-used assignments, optional targets, `super`, private fields, and subtrees
containing another assignment, update, `await` or `yield` are excluded.
Class constructor, field-initializer and static-block subtrees are excluded,
including nested callbacks; escaped method names are conservatively excluded too.
This does not infer whether an ordinary function is used as a constructor or
whether other methods initialize state. Existing runner failure attribution is
unchanged. See [semantic controls and session-persistence findings](../benchmarks/javascript-assignments.md).

Rust `call_remove` replaces a whole semicolon-terminated call statement with
`{}`. Free, associated, method, function-pointer and closure calls are eligible,
including parenthesized calls. Receiver, argument and invocation effects are
skipped; an owned value that is no longer consumed can drop later in its scope.
Compiler checks remain authoritative: losing a type constraint or required
initialization can produce a build failure. There is no type inference or
warning suppression in the operator.

Calls used as values, tail expressions without semicolons, macro invocations and
call subtrees containing macros, attributes, `await`, `?`, `yield`, `return`,
`break` or `continue` are excluded. This subtree rule also applies inside closure
arguments. Existing test/cfg/generated/constant/type exclusions remain. This
removes call statements only, not arbitrary statements or blocks. Saved reports
require the exact parsed statement and `{}` recipe. See
[native controls and unchanged UCD findings](../benchmarks/rust-calls.md).

Python `call_remove` replaces a complete ordinary call statement with `pass`,
including outer parentheses. It skips receiver/property lookup, argument and
unpacking effects, and invocation. `pass` preserves empty suites, semicolon
neighbors and docstring positions. Calls used as values, annotations and tests
are excluded, as are call subtrees containing `:=`, `yield`, `yield from` or
`await`, even inside nested closures. Ordinary constructors and callable objects
have no separate static type restriction. Runtime and lifecycle failures remain
errors. Saved recipes require exact `pass`; focused reruns also require fresh
AST/source/operator identity. See
[native controls and unchanged Route Mini findings](../benchmarks/python-calls.md).

Python `assignment_remove` replaces a single-target attribute/item assignment
with `pass`: for example, `node.parent = parent` or `items[key] = value`.
The complete RHS, target lookup, index/slice and store effects are skipped.
Local-name bindings, unpacking/chained targets, annotated and augmented
assignments, tests and annotation subtrees are excluded, along with constructor
(`__init__`/`__new__`) subtrees and assignments containing `:=`, `yield`,
`yield from` or `await`. Constructors were excluded after an unchanged-project
audit produced 16 errors from 16 deletions. Other methods can still initialize
state; missing initialization, setter errors and lifecycle failures remain errors.
This is a scoped assignment operator, not arbitrary statement/block deletion.
See the [assessment and stronger-test findings](../benchmarks/statement-removal-audit.md).

Python `loop_skip` wraps an ordinary `while` condition as
`(False and (original))`, skipping all condition effects, truthiness and body
execution. Native zero-iteration semantics apply: `while…else` still runs.
Literal Boolean conditions and conditions containing assignment expressions
(`:=`) are excluded, as are iterator `for` loops, annotations and tests.
Retained syntax preserves generator classification; subsequent runtime failures
remain errors. See [controls and unchanged-project finding](../benchmarks/python-loops.md).

Rust `loop_skip` wraps an ordinary Boolean `while` condition as
`false && (original)`, skipping condition evaluation and the body. Values and
references remain in the source for Rust's native type and ownership checks;
initialization or ownership violations remain build failures. Literal Boolean
conditions, `while let`, let-chains, iterator `for`, unconditional `loop`, macro
tokens, constants and tests are excluded. Nested ordinary loops remain eligible.
See [controls and unchanged-project finding](../benchmarks/rust-loops.md).

JS/TS `loop_skip` replaces the condition of an ordinary `while` or conditional
`for` with `(false && (original))`. A `for` initializer still runs once, including
its effects and exceptions; condition evaluation, body and update are skipped.
Lexical scope, labels and code after the loop stay intact. Literal Boolean
conditions belong to boolean flipping. Conditionless `for`, `for…in`, `for…of`,
`for await…of`, and `do…while` are excluded. TypeScript syntax remains subject to
each runner's existing scope, without added type checking. See
[semantic controls and unchanged-project finding](../benchmarks/javascript-loops.md).

Python `branch_force_true`/`branch_force_false` wrap ordinary `if`/`elif`
conditions as `(True or (condition))` / `(False and (condition))`. Condition
execution and truthiness conversion are deliberately skipped; branch bodies stay
unchanged. Literal `True`/`False` conditions belong to boolean flipping. Any
condition containing an assignment expression (`:=`), including nested scopes,
is excluded to avoid skipping a binding expected by later code. Ternary expressions,
loop conditions, comprehension guards, annotations and tests are excluded. Skipping
other condition effects can still invalidate body assumptions; runtime exceptions
remain errors. See [controls and unchanged-project validation](../benchmarks/python-branches.md).

Rust `branch_force_true`/`branch_force_false` wrap ordinary Boolean `if`
conditions with `true || (condition)` or `false && (condition)`. Condition
execution is deliberately skipped; the branch bodies are unchanged. `if let`
and let-chains are excluded because branch bindings must remain available.
Literal Boolean conditions stay with `boolean_flip`; types, macro bodies, constants,
test-only code, match guards and loop conditions are outside branch forcing. A
macro call used as the complete condition stays intact and is skipped at runtime.
Rust still type-checks retained expressions: ownership, borrowing and definite
initialization violations remain build failures. See
[semantic controls and unchanged-project validation](../benchmarks/rust-branches.md).

JS/TS `branch_force_true` and `branch_force_false` force nonliteral `if`
conditions, including nested `else if` branches. The condition is deliberately
**not evaluated**: calls, assignments, getters, exceptions, `await` and `yield`
inside it are skipped. The original parenthesized condition is retained behind
`true ||` or `false &&`; the branch bodies are unchanged. Literal `true`/`false`
conditions (including parentheses/comments) stay with `boolean_flip`. This does
not mutate ternaries, loop conditions or TypeScript types. Native TypeScript runs
remain untypechecked; forcing a branch can invalidate a narrowing assumption and
cause an ordinary runtime failure. See
[semantic controls and unchanged-project validation](../benchmarks/javascript-branches.md).

Python binary arithmetic supports `+ ↔ -`, `* ↔ /` and `% → *`, preserving native
grouping and operand evaluation. Unary signs, augmented-assignment operators, floor
division, powers and annotations are excluded. Operand types are not
inferred: unsupported operations and division by zero remain errors. See
[semantic controls and unchanged-project validation](../benchmarks/python-arithmetic.md)
and the [complete 194-candidate review](../benchmarks/python-arithmetic-full.md).
Modulo replacement also includes string/bytes formatting and custom `%` dispatch:
the replacement uses native multiplication, including sequence repetition or
TypeError as appropriate. It does not infer numeric types. See
[modulo controls and the unchanged-project finding](../benchmarks/python-modulo.md).
Python comparison boundaries exclude chained comparisons such as `0 < x < 10`.

For larger Python inventories, `--continue-on-error MAX_ERRORS` can continue
verified type/value/arithmetic/lookup exceptions from ordinary pytest bodies,
up to a positive error limit. Reaching the limit stops the run; omitted or `1`
stops at the first error. Lifecycle/resource/infrastructure errors still stop
immediately. Runs containing errors remain failed (exit 1) with no score, even
when all candidates finish. This option is Python-only and invocation-only;
reruns must opt in again. See [continuation boundaries and validation](../benchmarks/python-candidate-continuation.md).

Call removal deletes eligible standalone call statements; it excludes calls used
as values, `go`/`defer` calls, initializers and loop updates. Boolean literal
flipping does not force arbitrary conditions. Branch forcing and loop skipping
have additional eligibility and evaluation rules described below.
See [proposed operator expansion](../DESIGN.md#proposed-operator-expansion) for ideas
that are not yet implemented.

All ten Go operators run by default; Rust runs `boolean_flip`, `comparison_invert`,
`comparison_boundary`, `logical_swap`, `negation_remove`, `arithmetic_replace`,
`branch_force_true`, `branch_force_false`, `loop_skip`, and `call_remove`. Python runs `boolean_flip`,
`comparison_invert`, `comparison_boundary`, `logical_swap`, `negation_remove`,
`arithmetic_replace`, `branch_force_true`, `branch_force_false`, `loop_skip`,
`call_remove`, and `assignment_remove`. Vitest, Node and Bun run
`boolean_flip`, `comparison_invert`, `comparison_boundary`, `logical_swap`,
`negation_remove`, `arithmetic_replace`, `branch_force_true`, `branch_force_false`,
`loop_skip`, `call_remove`, and `assignment_remove`.
Set `operators` under the language section in `mutie.toml` to select a subset
for fresh runs, listings and reruns:

```toml
[go]
operators = ["arithmetic_replace", "call_remove"]
```

Then run `mutie list` or `mutie run --jobs 2`. Duplicates are removed;
unknown or unsupported names are errors. Omit the list (or use `operators = []`)
for all supported operators. Operator selection
intersects with file scope and, for reruns, saved IDs and outcome selection.
An empty selection completes normally. IDs are assigned after selection, and
reports record the effective operator list.

`branch_force_true` and `branch_force_false` force an entire `if` or `else if`
condition to take or skip its branch. They preserve the initializer (if any)
and both bodies, but skip evaluation of the original condition, including its
side effects. For example, `if ready()` becomes `if (0 == 0) || (ready())` or
`if (0 != 0) && (ready())`. Keeping the original expression as a short-circuited
operand keeps its imports and local variables used, avoiding unused-name build
failures. The constant comparisons work even when code shadows `true` or `false`.

These operators include nested, labeled, multiline, and initializer-bearing
`if` statements. They exclude loop/switch conditions and conditions consisting
only of `true` or `false` (possibly parenthesized), which remain covered by
`boolean_flip`. This is a syntax rule, without name resolution or general constant
folding; named constants and equivalent branches can still produce unkillable
mutations. Other mutations inside the original condition remain independent.

The [fzf branch sample](../benchmarks/fzf-v0.65.2-branches.md) matched all 24 outcomes
between full and affected tests with no build failures. Added tests killed four
survivors, covering ellipsis arguments and up-arrow modifiers. This sample does
not establish a general noise rate or make every survivor a test gap.

`loop_skip` makes a conditional `for` loop execute zero iterations. It supports
both `for condition { ... }` and `for init; condition; post { ... }`, replacing
only the condition with `(0 != 0) && (condition)`. The initializer still runs
once; the original condition, body, and post statement are not evaluated.
References remain in the source, keeping imports and local variables used.
Nested, labeled, parenthesized, and multiline conditions are supported.

Range loops and loops without a condition are excluded. Bare `true`/`false`
conditions (including parentheses/comments) remain with `boolean_flip`, following
the same syntax rule as branch forcing. Other operators in headers and bodies
remain separate candidates, including inside loops excluded from `loop_skip`.
Skipping an already empty loop can be equivalent; a missed result alone does not
prove that a test is missing.

The [fzf loop sample](../benchmarks/fzf-v0.65.2-loops.md) matched all 16 outcomes
between full and affected tests with no build failures. Added tests killed two
survivors covering partial tail chunks and display-width trimming.

Go `call_remove` deletes an entire standalone function or method call, including
multiline and parenthesized calls, from a block or case body. Arguments are also
removed, so their side effects no longer execute. It excludes calls used as
values (including `_ = call()`), `go`/`defer` calls, labeled calls, and calls in
control-flow headers. Surrounding comments and separators are preserved; comments
inside the removed statement are removed with it. Other mutations inside a call
remain separate candidates.

Removal can leave unused imports or locals, or remove a required terminating
`panic`. These are recorded as build failures; Mutie does not make extra edits
to repair them. Dismissals still match against discovery before operator
selection, so valid decisions for omitted operators are not reported as stale.
`dismiss check` checks all operators within the configured file scope.

Validation on `godotenv` found four surviving CLI mutations. Added behavioral
tests in an isolated copy killed all four; see the
[call-removal validation](../benchmarks/godotenv-v1.5.1-call-remove.md) for evidence
and the observed build-failure noise.
