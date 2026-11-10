# Unchanged Click: installed package and CLI fixtures

Click at `8abb77a8432e03ed14a95f972b44f905279415a7` now completes Mutie's
baseline, sampled mutation execution and a separate-copy stronger-test rerun.
All 179 tracked input files retain their original SHA-256 hashes. The stronger
copy adds one test file; it changes no original source, tests or configuration.
The [results ledger](fixtures/python-click/results.json) retains per-report
execution identity, source-line outcomes and preservation digests for all 119
original report files. Those reports retain their original scores.

This closes Click's admission blocker from the [compatibility audit](compatibility-audit.md).
It validates one dependency-free Flit package on macOS, using CPython 3.14.7,
pytest 9.0.2 and Flit Core 3.12.0. Linux fixture coverage is configured in CI;
it has not been run remotely for this change.

## Installation and selection

The explicit Flit module and static project metadata select the installed scope.
The audited backend builds a pure-Python wheel in an owned directory, without
importing the application, resolving dependencies or invoking project hooks.
Wheel paths, RECORD hashes and the complete package payload must match audited
source files. An owned virtual environment contains that wheel and a copied,
pinned pytest runtime. Child commands using `sys.executable` see the same real
installation, including distribution metadata. Runtime/package bytes, launcher
target and environment configuration are verified before and after execution.

The original `testpaths`, strict warning configuration and `-m 'not stress'`
selection are preserved. Native and Mutie baselines both report:

- 2,622 passed, 25 skipped, one expected failure.
- 31,000 deselected by upstream's own stress marker configuration.
- All 2,648 selected node IDs and the ordered deselection digest are bound to
  the pristine run. Selected plus deselected files must cover the audited suite.

The installed scope uses pytest's prepend import mode, which Click needs for
ordinary test-to-test imports. Documentation, examples and explicitly declared
typing fixtures remain fingerprinted and non-importable. Native test files and
conftests cannot be hidden there. Other backends, dynamic metadata, application
dependencies, arbitrary pytest options/plugins and async execution remain excluded.

## Failure boundaries

The observer records requested/running/finished thread states. Exceptions from
ordinary thread bodies are retained independently of temporary exception-hook
replacement. Raw thread starts, incomplete threads, fixture errors and late
unraisable failures remain errors. Joined threads do not relax body attribution.

Owned temporary files, pipes and directory-relative descriptors are supported.
The `os.open` wrapper carries `dir_fd` into the audit check because CPython's
`open` audit event omits it. Controls retain source-write, external-file,
symlink-escape and changed-HOME rejection. These checks supplement static
admission and post-run hashes; they are not an OS security sandbox.

Setup/call skips and marked call xfails require positive exception evidence and
exact pristine dispositions. New skips, removed skips, changed xfail exception
types, XPASS, fixture/setup/cleanup failures and incomplete phases remain errors.
An assertion elsewhere cannot conceal them. The bounded evidence reader and Unix
per-file limit are now 16 MiB, accommodating Click's roughly 6 MiB ledger. Capture
budgets remain separate; this is not an aggregate disk quota. Historical 4 MiB
reports remain readable; changed execution recipes require fresh reports for reruns.

## Findings

Discovery lists 315 boolean sites. The sample selects all 16 in
`src/click/shell_completion.py`, while every candidate runs the original full
selected test suite. Three reports account for all candidates:

| Report | Attempted | Killed | Missed | Errors | Wall time |
| --- | ---: | ---: | ---: | ---: | ---: |
| Original sample | 12 | 5 | 6 | 1 | 248.33 s |
| First remaining selection | 1 | 0 | 0 | 1 | 67.91 s |
| Final remaining selection | 3 | 0 | 2 | 1 | 95.95 s |

This is combined inventory accounting, **not a full score**. All three reports
remain failed/unscored. The errors contain uncaught Click exceptions such as
`NoSuchOption`, `BadOptionUsage`, `MissingParameter` and `NoArgsIsHelpError`;
the classifier was not broadened to turn them into kills.

Four retained [stronger tests](fixtures/python-click/stronger_tests.py) expose:

| Original line | Observable boundary |
| --- | --- |
| 48 | Completion source output must equal the template bytes without an added newline. |
| 419 | An old-Bash warning belongs on stderr, leaving stdout available for completion source. |
| 424 | The missing-Bash warning also belongs on stderr. |
| 619 | Completion word splitting must preserve punctuation in `--value=a:b`. |

The separate copy passes 2,626 tests with the same skips/xfail/stress selection.
Its full-suite focused rerun kills all four selected survivors in 114.53 seconds;
the recorded pytest commands take 8.78–10.48 seconds each. This focused 100% is
not a full stronger-suite score. Four original survivors remain open: two
TypeVar covariance sites (including the inactive typing branch) and two chained
command parsing settings. None is declared equivalent or dismissed.

## Regression coverage

- The installed CLI fixture checks real metadata, child imports, joined threads,
  filesystem fixtures, saved scores, stronger reruns and mutation-dependent skips.
- Native observer tests cover bodies, fixtures, unittest, threads, cleanup,
  descendant shutdown, captures and file limits. Source and external-file
  controls include directory-relative opens.
- Static controls reject hidden tests, unknown dependencies, dynamic code,
  unsupported selection options, runtime-name collisions and packaging paths
  outside audited inputs. Child interpreters cannot inherit host Python paths.
- Protocol controls reject missing, reordered or forged thread/phase evidence;
  saved identity retains exact selection and historical limit validation.

Validation passed: 376 default tests, 86 static Python controls, nine native
lifecycle groups, the installed scoring/rerun fixture and scored resource-limit
controls. Formatting, strict Clippy and actionlint pass. The default suite needed
process-inspection permission for its existing cancellation tests; the unrestricted
rerun passed. The final unchanged Click check passes in 9.43 seconds of recorded
pytest command time. End-to-end runs also include validation, installation and
reporting overhead.

Use the [pinned requirements](fixtures/python-click/requirements.txt) for an
installed-scope environment. No upstream project code is vendored here.
