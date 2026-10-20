# Python process-lifetime observer

Recorded 2026-10-20, following scope-audit commit `efb5650`. The native observer
uses the same CPython 3.14.7 / pytest 9.0.2 environment. Python was Planned at this stage:
this adds an experiment and evidence reader, not a Rust adapter or scored run.
Later [scoped milestone acceptance](python-scrubify-findings.md) records current
support; this document preserves the original lifecycle evidence.

## Evidence boundaries

The [initial audit](python-initial-scope.md) found an atexit exception that pytest
reported as success while the process exited zero. The new
[`lifetime_driver.py`](fixtures/pytest-audit/lifetime_driver.py) installs lifetime
observation before importing pytest:

- Unraisable, uncaught and threading exception hooks retain failure evidence.
- A raw file descriptor stays open through interpreter teardown. JSONL records
  include a per-run nonce, writer PID and consecutive sequence numbers.
- An atexit checkpoint is registered before application callbacks. Normal later
  registrations execute first; the checkpoint verifies hook ownership and the
  absence of other live Python threads.
- A separate return checkpoint follows `pytest.main`. Neither checkpoint alone
  means the process succeeded.

The parent waits for process exit, checks for surviving process-group members,
then reads the entire bounded evidence and uncaptured stderr. A positive result
requires complete collection and ordered setup/call/teardown evidence, a passing
setup and teardown for each test, consistent pytest/process statuses, intact
checkpoints and no failure events. An assertion type is checked by identity and
inheritance in the observer; a user exception merely named `AssertionError` is
not positive assertion evidence.

The [reader](fixtures/pytest-audit/lifetime_protocol.py) rejects unknown/extra
fields, duplicate JSON keys, malformed/truncated/oversized streams, invalid
types, missing/reordered/duplicate completion, mismatched PID/nonce, changed hooks
and exit-code disagreement. It deliberately preserves the initial conservative
assertion-only classification; arbitrary body runtime exceptions remain errors.

## Cleanup after pytest

Paired controls cover both a passing body and a failing body followed by:
atexit exceptions/assertions/SystemExit, `os._exit(0)`, removed callbacks, stderr,
destructor errors, weakref finalizer errors and hook replacement. A cleanup error
overrides a body assertion; it cannot become a kill.

In particular, module-destructor failures can be recorded **after** the atexit
checkpoint. The reader waits for the process and inspects the full stream, so it
does not accept the earlier checkpoint as a final success marker.

Uncaptured stderr is conservatively an error in this proposed scope. This
provides an additional failure path when interpreter shutdown cannot reliably
write structured evidence. It is not traceback string matching. Consequently,
even harmless diagnostics written to stderr after pytest capture ends are
unsupported. Ordinary stderr captured inside a passing test and clean stdout
from an atexit callback have positive controls.

CPython documents [unraisable hooks](https://docs.python.org/3.14/library/sys.html#sys.unraisablehook)
and [atexit ordering and its limits](https://docs.python.org/3.14/library/atexit.html).
The observed pinned-runtime results, rather than an assumption about exit status,
determine these controls.

## Unittest and thread scope

Unittest-derived items are explicitly rejected during collection, so the original
`setUp` assertion misclassification no longer produces a positive result.

Thread exceptions deserve a narrower boundary than the initial hooks suggest.
A real background-thread failure can be retained by pytest's own thread hook
after its final warning drain, yielding neither a warning nor stderr. The probe
rejects that invocation because a thread is still alive at return. That timing
does not prove all possible races are covered.

The observer therefore records starts through `threading.Thread.start` as
unsupported, including clean joined threads, and verifies that the installed
start wrapper is intact. It delegates to the original method so the test's actual
execution and cleanup still occur. This rejects the unproven boundary rather than
classifying a suppressed failure as a pass or a kill. Low-level thread APIs,
custom start implementations, async runtimes and arbitrary hook manipulation
remain outside the proposed scope and require static scope enforcement.

## Limits and next implementation gate

The harness applies a 4 MiB per-file limit to evidence and captures, retains
stdout/stderr separately, kills the process group on timeout, and rejects/cleans
up a surviving ordinary child. These are native controls, not validation of
Mutie's full cancellation or racing-descendant guarantees. They are not an
aggregate disk quota or an adversarial sandbox.

The hooks do not defend against deliberately disabling all observation or
escaping the process group. The next gate is a strict project/import/config
validator and isolated baseline integration using the existing Rust process
controls, with independent inventory/identity checks. Do not enable scored Python
runs from this experimental classifier. Doctests are exercised only for the
adapted Inflection control; production admission remains a separate decision.

## Validation

The final retained observer passes **54 native controls and 66 replay-rejection
checks**. This includes clean/body-failure pairs, late cleanup mixtures, explicit
pytest body failure, same-named exception rejection, unittest/thread exclusions,
timeout during the test and during atexit, output overflow and an ordinary
surviving subprocess.

Inflection preserves the prior direct workflow:

| Control | Tests | Result | Wall time |
| --- | ---: | --- | ---: |
| Original selected baseline | 467 | Passed | 2.318s |
| Manual Unicode-digit mutation | 467 | Survived | 1.871s |
| Original source plus stronger test | 468 | Passed | 2.672s |
| Same mutation plus stronger test | 468 | Body assertion failure | 2.019s |

These are individual native samples, not scored Mutie reports or a performance
comparison. Test-ID sets agree within the original and stronger pairs. All 17
original project hashes remain unchanged. Both passing-body and failing-body
module-destructor controls explicitly record an error after the atexit checkpoint.

[Compact results](python-process-lifetime-results.json) retain environment,
observer/reader hashes, native commands, phase and lifetime evidence summaries,
inventory hashes and full-artifact hashes. Full logs/evidence remain at the
recorded temporary output path and can be regenerated. Python syntax and
repository whitespace checks also pass; no unrelated Rust tests were rerun.

## Reproduction

Prepare the pinned environment and Inflection checkout as described in the
[audit instructions](fixtures/pytest-audit/README.md), then run:

```sh
python3 -B benchmarks/fixtures/pytest-audit/lifetime_audit.py \
  --python /path/to/audit-env/bin/python \
  --project /path/to/inflection \
  --output /path/to/new-lifetime-output
```

The original audit driver/results stay unchanged as negative controls. The new
harness creates fresh owned copies, records full evidence and checks original
input hashes. No Rust production code or support level changes.
