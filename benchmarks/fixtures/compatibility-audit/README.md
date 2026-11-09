# Compatibility audit reproduction

These scripts retain the 2026-11-09 audit of six unchanged projects. They are
manual, bounded probes, not part of the default self-test suite. Upstream source
and dependencies stay outside this repository. `sjson_contracts_test.go` is an
original stronger-test addition, copied only to the separate SJSON test project.

Use the commits recorded in [the results ledger](../../compatibility-audit-results.json):

| Directory | Repository | Commit |
| --- | --- | --- |
| sjson | tidwall/sjson | f0069f8aa5171ab0fd2ca44b76c8ea243ead411d |
| ripgrep | BurntSushi/ripgrep | 178cc027e283fd965cb72d0a7395b219eada170c |
| click | pallets/click | 8abb77a8432e03ed14a95f972b44f905279415a7 |
| ufo | unjs/ufo | f06c800d0c59f2a4a1b9ba65eb6cb61a84419be6 |
| eslint-node-test | sindresorhus/eslint-node-test | dcb850e2bd01534792fd67bcec4747828fa96df9 |
| elysia | elysiajs/elysia | 1cf5ce15dc4de23a96c769b5500a8d6fac8a8373 |

Create an empty external `AUDIT_ROOT`; clone each project at its recorded commit
into `$AUDIT_ROOT/NAME/original`. Each checkout must be clean. Build Mutie with
`cargo build --release --locked`. `prepare` freezes that executable, records every
tracked input and creates onboarding copies. It refuses to overwrite a frozen CLI.

Set `MUTIE_AUDIT_NODE`, `MUTIE_AUDIT_BUN`, and `MUTIE_AUDIT_PYTHON` to the audited
Node 24.22.0, Bun 1.3.11+af24e281e, and CPython 3.14.7 / pytest 9.0.2 venv launchers.
Go 1.27.2 and Rust/Cargo 1.99.1 must be on PATH with writable caches. The harness
does not install or alter JS project dependencies to bypass admission.

```sh
python3 -B benchmarks/fixtures/compatibility-audit/run.py "$AUDIT_ROOT" prepare
python3 -B benchmarks/fixtures/compatibility-audit/run.py "$AUDIT_ROOT" admission
python3 -B benchmarks/fixtures/compatibility-audit/run.py "$AUDIT_ROOT" sjson-list
python3 -B benchmarks/fixtures/compatibility-audit/run.py "$AUDIT_ROOT" sjson-run
python3 -B benchmarks/fixtures/compatibility-audit/run.py "$AUDIT_ROOT" sjson-rerun
python3 -B benchmarks/fixtures/compatibility-audit/run.py "$AUDIT_ROOT" native-rust
python3 -B benchmarks/fixtures/compatibility-audit/run.py "$AUDIT_ROOT" native-rust-git
python3 -B benchmarks/fixtures/compatibility-audit/run.py "$AUDIT_ROOT" native-python
```

`retry-check sjson` and `sjson-list-host` retain separately named retries when a
sandbox blocks Go caches. They grant no permissions themselves. The recorded run
needed host access; don't manufacture failures on a host without that restriction.
Repeated stage labels are refused to preserve evidence.

The first native Python control deliberately uses the supplied pinned environment
and `PYTHONPATH=src`. For Click's subsequent installed-project control, create a
separate `$AUDIT_ROOT/click/native-env` with CPython 3.14.7, then install pytest
9.0.2 and the unchanged `$AUDIT_ROOT/click/native` project editable with its declared
Flit backend. The original audit used `uv pip install` and left its resolver output
in `native-install-host.log`. This environment is never passed to Mutie.

```sh
python3 -B benchmarks/fixtures/compatibility-audit/run.py "$AUDIT_ROOT" native-python-installed
python3 -B benchmarks/fixtures/compatibility-audit/verify.py "$AUDIT_ROOT" results.json
```

Mutie commands have 90–180 second outer bounds and a 16 MiB per-file limit in the
harness. Native controls have a 240-second outer bound and no inherited file-size
limit, allowing ordinary compiler output artifacts. Timeout cleanup terminates
the owned process group. Raw stdout/stderr and exit/timing records remain external.

`verify.py` audits this specific campaign, including its retained failed/retry
attempts. A fresh campaign with different environmental outcomes needs its own
expected-stage assertions; it must not rewrite the historical ledger. New inputs
or stronger tests likewise require a new campaign rather than updating these hashes.
