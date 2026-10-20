# Native pytest feasibility audit

This is a direct Python experiment, not a Mutie language adapter. It intentionally
retains two **known misclassifications** as negative controls: an `atexit` failure
looks passed, and a `unittest.setUp` assertion looks like a body failure. Do not
use this classifier to compute mutation scores.

Use CPython 3.14.7 and a dedicated virtual environment. The lock here records the
pytest 9.0.2 environment actually tested; no upstream project installation or
editable install is needed. From the Mutie repository:

```sh
python3 -m venv /path/to/audit-env
/path/to/audit-env/bin/python -m pip install -r \
  benchmarks/fixtures/pytest-audit/requirements.txt
git clone https://github.com/jpvanhal/inflection.git /path/to/inflection
git -C /path/to/inflection checkout 88eefaacf7d0caaa701af7c8ab2d0ab3f17086f1
python3 -B benchmarks/fixtures/pytest-audit/audit.py \
  --python /path/to/audit-env/bin/python \
  --project /path/to/inflection --output /path/to/new-audit-output
```

The output directory must not exist. Every probe copies its source to an owned
directory, runs a fresh interpreter with `-I -B`, and (except the negative cache
control) uses a fresh `-X pycache_prefix`. The driver inserts only the owned
project root for application imports. Plugin autoload is disabled and ambient
pytest variables are removed. An explicit empty config, importlib test imports,
no conftest loading and no pytest cache define this experimental recipe.

The Inflection selection is `test_inflection.py inflection --doctest-modules`.
It includes all tests in that file and all package doctests, but **does not run
the full upstream tox/mypy recipe or scan packaging/documentation modules**.
Source, tests and configuration are copied unchanged for the baseline.

`driver.py` records collection, each setup/call/teardown report, warnings, session
finish, return status and selected imported module paths. `audit.py` checks
ordered complete per-item evidence, exit agreement and conservative assertion
failure attribution. Skip/xfail, runtime exceptions, missing evidence and
non-call failures remain errors. These checks are experimental and incomplete.

The matrix covers ordinary and parametrized tests, fixtures/finalizers, collection,
syntax/import errors, skips, xfail/xpass, exits, timeout, thread/unraisable errors,
and the known unittest/atexit gaps. Synthetic fixture controls demonstrate a
boolean survivor and stronger-test kill. Cache controls deliberately retain valid
timestamp bytecode while changing source to the same byte count.

The real project probes retain a boolean-default mutation, an ASCII-digit
regression already caught upstream, and a manual Unicode-digit regex mutation
that survives until `inflection-stronger.py` is added. The regex edit is a
hand-authored diagnostic, not a proposed or implemented mutation operator.

Raw logs, JSONL evidence, owned projects and full `results.json` remain under the
output directory. Source hashes are checked after every run. Wall times include
process startup/wait; they are small local samples, not performance guarantees.
The helper kills its process group on timeout, but bounded capture, cancellation,
racing descendants and resource-limit reliability still require dedicated work.
