# Rust assignment removal

Validated on 2026-11-09 with Rust/Cargo 1.99.1 on macOS arm64, based on Go commit
`db2cb5d`. [Results and hashes](rust-assignments-results.json) retain the complete
selected inventory and original, unchanged-control and stronger-test outcomes.
No package, report-schema or operator-catalog version changes are needed.

## Scope and semantics

`assignment_remove` replaces a complete semicolon-terminated field/index `=`
statement directly in a block with `{}`. Named and tuple fields, indexed places,
and parenthesized assignments/targets are supported. The empty block retains a
statement boundary without adding redundant-semicolon warnings.

Rust evaluates an assignment's RHS before its destination; assignment also
destroys an initialized overwritten value. Removing the whole statement skips
RHS, place/index/trait evaluation, overwrite and old-value destruction. A value
no longer moved by the assignment follows the remaining scope's drop behavior.
See the Rust Reference's [assignment rules](https://doc.rust-lang.org/reference/expressions/operator-expr.html#assignment-expressions)
and [destructors](https://doc.rust-lang.org/reference/destructors.html).

Plain bindings, destructuring, pointer-only stores, compound/nested assignments,
value-used assignments and semicolon-free tails are excluded. The entire candidate
subtree must be free of macros, attributes, `await`, `?`, `yield`, `return`, `break`
and `continue`, including inside nested closures. Eligible inner statements remain
independent candidates. Existing cfg/test, generated, constant/type and macro
exclusions remain. No type inference or constructor-name heuristic is added.

Compiler checks stay authoritative. Removing a field reinitialization after a
partial move, the only type constraint, or the only mutable use under denied
warnings can make a mutant invalid. Such results remain build failures; no
reference repair, warning suppression or unreachable wrapper is synthesized.
Saved readers require the exact complete statement and `{}` recipe. Fresh source/
operator discovery rejects forged test-only spans even when the snippet alone
looks eligible. Execution, cleanup, classifiers and report formats are unchanged.

## Controls and fixture

Static tests cover exact UTF-8/CRLF spans and edits, comments, nesting, deterministic
discovery, exclusion boundaries, statement reparsing and corrupted saved recipes.
Fifteen paired native controls compile and run original/mutated programs, checking:

- RHS-before-place order; named/tuple fields, arrays and parenthesized stores.
- Overloaded `IndexMut` and `DerefMut`, mutable references, and temporary receivers.
- Owned-value moves and delayed destruction, old-value destruction, and RHS temporaries.
- RHS/place/index/bounds panics, cleanup of unconsumed RHS values, and a panicking
  old-value destructor, including the changed scope at which destruction occurs.

The trace programs allow ordinary unused warnings in both originals and mutants;
the operator adds no allow attributes. Three additional original/mutant compiler
controls explicitly verify partial-move, missing-type-inference and denied-warning
failures. Every native command retains a 30-second deadline and 64 KiB capture bound.

The independent [Cargo fixture](../tests/fixtures/rust-assignments/README.md) has
four candidates: **one kill, one survivor, two build failures**. Fresh serial and
two-worker cached execution agree. Build failures stay outside the 50% score;
minimum-score failure exits 3. Dismissal review suppresses the finding policy
without changing raw outcomes/score. A separate stronger copy kills the survivor
and preserves its review/source-ID mapping. Forged recipes/operators/paths,
test-only spans and stale sources are rejected. Caller artifacts, original source
and every original report file are preserved.

Separate fixtures verify genuine timeout and capture overflow remain non-kills.
The overflow control writes directly to stdout because libtest normally captures
`println!` output from passing tests. Overflow produces a failed report, no passing
score and exit 1 even with a score policy; no production capture flag is changed.

## Unchanged UCD Parse acceptance

The existing public [UCD Generate workspace](https://github.com/BurntSushi/ucd-generate/tree/e8fa937f0cac643669dcaf5edac2785b15cab917)
at `e8fa937f0cac643669dcaf5edac2785b15cab917` is copied into a fresh unchanged
checkout. This revisits an accepted project, not a new layout. All **108 tracked
paths** (including symlinks) remain unchanged. External config selects `ucd-parse`
and `assignment_remove`, with no source-file filter; every candidate still runs
the complete four-member workspace suite.

The **140-test baseline passes**. All **ten eligible assignments** run: six kills,
four survivors, zero build failures/errors/timeouts, in **53.74 seconds**. The
60% score is filtered to the chosen package/operator, not a workspace-wide score.
The zero invalid-candidate rate applies only to these ten candidates; the fixture
demonstrates that ownership/inference failures remain possible.

| Original ID | Surviving store | Stronger public behavior check |
| --- | --- | --- |
| 1 | `common.rs:333`, `err.line = Some(line_number)` | Malformed input after comments/blank lines reports physical line 4. |
| 5 | `unicode_data.rs:172`, `data.bidi_mirrored = …` | Both `Y` and `N` field values survive parsing correctly. |
| 7 | `unicode_data.rs:174`, `data.iso_comment = …` | An accepted nonempty legacy field is preserved. Modern UCD data leaves this field empty; this is a compatibility boundary. |
| 9 | `unicode_data.rs:276`, `self.mapping[self.len] = cp` | Decomposition contents equal the supplied codepoints in order, not merely the right length. |

An unchanged-copy focused run, completed before stronger-test installation,
reproduced **all four survivors** with the original 140-test baseline in 28.59
seconds. Its report is retained under `control-reports`; relocating that artifact
directory did not rewrite its saved metadata or original paths.

The separate stronger copy adds only
[`assignment_effects.rs`](fixtures/rust-assignments-ucd/assignment_effects.rs), with
four tests and no new dependencies. Its **144-test baseline passes**, and the
focused rerun kills **all four** in **36.44 seconds**. There are no remaining
survivors in this ten-candidate selection. The focused 100% score does not replace
the original 60% score, and no equivalence or general noise-rate claim is made.
Timings are observations under concurrent local validation, not speed comparisons.

The [verifier](fixtures/rust-assignments-ucd/verify.py) matches all 108 original
paths in both copies, the sole added test, original manifest/lockfile identity,
full-workspace test counts and exact report-local mutation identities. All **56
original report files** and **32 unchanged-control report files** retain their
hashes. Raw copies/reports remain outside the repository in the temporary
`mutie-rust-assignments-audit` directory.

## Verification and reproduction

The default suite passed **354 tests**, zero failures, with **189 opt-in ignores**.
Native semantic/compiler controls, both scored assignment workflows and the
existing Rust binary, workspace and call-removal workflows pass. Formatting,
all-target Clippy with warnings denied, workflow lint and whitespace checks pass.
The Linux Rust job now includes assignment semantics and workflows. This slice is
not yet committed/pushed, so its remote CI is pending. The separately deferred
macOS pre-main startup investigation remains open.

```sh
cargo test --locked --bin mutie rust::discovery::assignment_tests -- --include-ignored
cargo test --locked --test rust_workflow assignments:: -- --ignored --test-threads=1
```

Place external config beside an unchanged checkout named `original`:

```toml
jobs = 2
timeout = 60
baseline_timeout = 180
output_dir = "original-reports"
[rust]
path = "original"
packages = ["ucd-parse"]
operators = ["assignment_remove"]
build_cache = "worker"
```

```sh
mutie list --config original.toml --json
mutie run --config original.toml --plain
# Copy the checkout to stronger/ and add the retained test under ucd-parse/tests/.
# Use path="stronger" and output_dir="stronger-reports" in another config.
mutie rerun "$ORIGINAL_REPORT" --missed --config stronger.toml --plain
python3 -B benchmarks/fixtures/rust-assignments-ucd/verify.py \
  "$AUDIT_ROOT" results.json
```

The verifier expects this campaign's original/control/stronger artifact layout
and preservation manifests recorded before the stronger run.
