# Vitest dismissal review and CI policies

Following focused-rerun commit `9632c91`, the scoped Vitest workflow uses the
shared dismissal store, saved review snapshots and CI policy evaluation. This is
fixture acceptance for the pinned single ESM/pnpm package, serial full-suite
execution and JS/TS boolean flips; Vitest remains Experimental.

## Behavior

```sh
mutie /path/to/project --lang vitest --fail-on-missed
mutie dismiss --module /path/to/project --lang vitest add /path/to/saved-run --id 3 --reason 'accepted test gap'
mutie dismiss --module /path/to/project --lang vitest check
mutie /path/to/project --lang vitest --rerun-missed /path/to/saved-run --fail-on-missed --min-score 80
```

`add`, `list`, `update`, and `remove` operate on review metadata. `check` snapshots
the project, validates its supported package/dependency metadata, and rediscovers
source mutations with the Vitest adapter. It does not invoke Node, pnpm, Vitest,
package scripts, a baseline or tests. An empty store needs no discovery. As with
other languages, add/list do not claim current source has been checked.

Decisions carry the `vitest` runner identity and exact source identity. A decision
for a JS source mutation does not apply to a TS source mutation or another
language. Source edits invalidate its whole-file fingerprint; a stale decision
appears under `needs_review` and no longer excuses a missed finding. Test edits
do not invalidate production-source review. Reruns capture current matching
review, remap it to selected IDs, and preserve the original saved reports.

`--fail-on-missed` evaluates undismissed survivors only. `--min-score` always
uses raw killed/(killed + missed), including dismissed findings. Dismissals neither
change execution outcomes nor suppress mutation execution: stronger tests can
still kill a dismissed survivor. Exit 3 means a completed run failed a policy;
infrastructure errors use exit 1 and leave both enabled policies `not_evaluated`.
Configuration and explicit CLI overrides use the existing common precedence.
Saved report reading validates these review snapshots without running tools.

## Automated acceptance

The ignored `tests/support/vitest_dismissals.rs` test runs independent copies of
the pinned fixture through the real CLI:

- A JS scan produces two kills/one miss and exits 3 for an undismissed survivor.
- Add, duplicate rejection, update, list, check, remove and re-add preserve metadata
  correctly. CRUD and nonempty `check` run with an empty `PATH`, proving no runner
  or package-manager executable is required for review.
- With the survivor dismissed and both policies configured, the same raw outcomes
  and score remain. Findings policy passes; minimum score 100 fails with exit 3.
- A missed-only rerun with `--clear-min-score` still executes the dismissed
  survivor, saves it as missed with raw score zero, and passes findings policy.
- A source edit makes the dismissal stale. Check fails with a review message;
  a fresh scan counts the surviving finding as undismissed and exits 3.
- After restoring source, a stronger JS test kills the dismissed survivor in a
  focused rerun; raw score 100 and both policies pass. Prior report bytes and
  production source are preserved.
- A separate TS scan remains undismissed by the JS decision. After its own
  dismissal, a focused TS rerun retains a raw survivor/score zero while passing
  findings policy.
- A runner exiting zero without completion fails the baseline with exit 1;
  both configured policies remain unevaluated.

The original shared CLI/rerun fixture stays opt-in. Default tests cover empty
review without runner tools and the remaining unsupported execution options;
existing common tests cover score arithmetic, review validation and policy logic.

```sh
MUTIE_VITEST_FIXTURE=/path/to/prepared-fixture cargo test --test vitest_workflow review:: -- --ignored
cargo test
cargo fmt --check
cargo clippy --all-targets -- -D warnings
```

## Remaining gates

Broader hook/fixture lifecycle attribution is still conservative. Parallel
workers remain rejected. Real-project usefulness and feedback time, unmodified
Pathe configuration support, and independent JavaScript project acceptance are
not established by these fixture tests. Installed dependencies retain metadata
and containment checks rather than a cryptographic inventory of all bytes.

Final validation (2026-10-15): all 231 default tests passed, with 36 opt-in tests
ignored. The real Vitest review/policy test passed, covering the JS and TS cases
above. Formatting and all-target Clippy passed. No package, report-schema or
operator-catalog version changed.
