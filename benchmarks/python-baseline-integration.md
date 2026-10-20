# Python scope and baseline integration

This records the first CLI integration, before scored-workflow acceptance.
It exposes read-only `python-list` and isolated `python-check`. Neither
command produces scores, saved reports or review decisions.
Current scored support is recorded in [milestone acceptance](python-scrubify-findings.md).

## Admission

The initial layout is one flat package with `__init__.py`, a named PEP 621
`pyproject.toml`, and `test_*.py` files under `tests/`. Application dependencies,
namespace packages, executable files elsewhere, conftest, extra pytest settings,
startup hooks, user plugins, async code and thread/process APIs are excluded.
The static AST reader admits a small standard-library import set and scoped
pytest APIs; it never imports the project. It is a supported-scope validator,
not a security sandbox for hostile code.

The [case-converter audit](python-case-converter.md) additionally admits
`io.StringIO`, basic logging stream setup, ordinary `super(...).__init__` calls,
and local parameters named `input` (not interactive calls). Logging submodules,
file handlers, shutdown and other reflective attributes remain rejected.
The later [Scrubify acceptance](python-scrubify-findings.md) admits named
`fnmatch` and `urllib.parse` string helpers, without networking imports.
Shallow `copy.copy` and explicit index-protocol access are also audited;
deep copy, copy internals and general reflective attributes remain rejected.

Boolean discovery uses UTF-8 byte offsets, skips annotations, and retains input
hashes. Source bytes, tests and configuration are compared across original and
isolated workspaces. Symlinks, standard-library/dependency shadowing and legacy
sourceless bytecode are rejected. Existing `__pycache__` files are retained as
inputs but execution uses a fresh external cache prefix.

The explicit `--python` launcher must preserve its virtual-environment path.
CPython 3.14.7 and the versions in
[`requirements.txt`](fixtures/pytest-audit/requirements.txt) are pinned. The
environment permits pip but no additional distributions. Runtime identification
uses `-I -S`, rejects startup hooks/editable installs, and fingerprints the
executable, Python shared library, standard library, package files and venv
configuration. These identities are rechecked after work.

## Baseline

```sh
cargo run -- python-list tests/fixtures/python \
  --python /path/to/pinned-venv/bin/python --json
cargo run -- python-check tests/fixtures/python \
  --python /path/to/pinned-venv/bin/python --json
MUTIE_PYTHON=/path/to/pinned-venv/bin/python \
  cargo test --test python_workflow -- --ignored --test-threads=1
```

The runner owns the pytest config, home, temporary directory, observer, evidence
and cache paths. Plugin autoload and inherited pytest options are disabled.
A separate collection pass must match the full run's ordered test IDs.
The Rust reader requires successful setup/call/teardown for every collected test,
successful session/return events and intact lifetime checkpoints. It binds
evidence to the actual child PID and a per-command token and reads only after
the shared process runner has waited and shut down its process group. Unknown
events, warnings, uncaptured stderr, malformed evidence and incomplete runs fail.
Each subprocess has the requested deadline and capture budget; the observer
file shares the 4 MiB per-file resource limit. This is not a total disk quota.

The macOS arm64 fixture passes two tests and discovers two source flips.
Native CLI regressions cover original-input preservation, inherited pytest
options, rejection before project import, assertion/runtime failures, unsupported
skips, deadlines and an equal-size/backdated stale-bytecode negative control.
Static regressions cover scope exclusions, Unicode spans and annotation exclusion.

Validation on 2026-10-20: all five opt-in Python workflow tests passed (39.17s),
including nine static-scope tests. The default `cargo test` suite,
`cargo clippy --all-targets -- -D warnings`, `cargo fmt --check` and
`git diff --check` passed. The two-test direct baseline spent 2.67s in collection
and test subprocesses; this excludes runtime fingerprints and scope validation.

The [process integration follow-up](python-process-integration.md) completes
the initial production cancellation/descendant/resource-limit probes and adds
typed execution identity. [Shared-run fixture acceptance](python-shared-runs.md)
now covers scored execution, saved identity, focused reruns and review/policies.
Scored cleanup and the [Scrubify stronger-test loop](python-scrubify-findings.md)
now complete the scoped milestone. The earlier adapted Inflection audit alone
did not establish acceptance for this narrower CLI layout.
