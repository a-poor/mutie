"""Run reproducible native probes in fresh copies; no production support claim."""
import argparse
import ast
import hashlib
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess
import tempfile
import time

HERE = Path(__file__).resolve().parent


def hashes(root):
    return {str(p.relative_to(root)): hashlib.sha256(p.read_bytes()).hexdigest()
            for p in root.rglob("*") if p.is_file() and ".git" not in p.parts}


def classify(events, code):
    """Conservative audit policy; deliberately excludes skip/xfail/runtime errors."""
    if not events or events[0]["event"] != "start":
        return "error"
    if sum(e["event"] == "start" for e in events) != 1:
        return "error"
    if any(e["event"] not in {"start", "collection", "phase", "finish", "returned"}
           for e in events):
        return "error"
    inventories = [e["nodes"] for e in events if e["event"] == "collection"]
    if len(inventories) != 1 or not inventories[0]:
        return "error"
    nodes = inventories[0]
    if len(nodes) != len(set(nodes)):
        return "error"
    if len(events) < 2 or [e["event"] for e in events[-2:]] != ["finish", "returned"]:
        return "error"
    if sum(e["event"] == "finish" for e in events) != 1:
        return "error"
    if sum(e["event"] == "returned" for e in events) != 1:
        return "error"
    if code not in (0, 1) or any(e["status"] != code for e in events[-2:]):
        return "error"
    phases = [e for e in events if e["event"] == "phase"]
    if [e["node"] for e in phases] != [n for n in nodes for _ in range(3)]:
        return "error"
    failed = False
    for i in range(0, len(phases), 3):
        group = phases[i:i+3]
        if [e["when"] for e in group] != ["setup", "call", "teardown"]:
            return "error"
        for e in group:
            if e["xfail"] or e["outcome"] not in ("passed", "failed"):
                return "error"
            if e["outcome"] == "failed":
                if e["when"] != "call" or e["exception"] not in (
                    "AssertionError", "DocTestFailure", "Failed",
                ):
                    return "error"
                failed = True
    if (code == 1) != failed:
        return "error"
    return "body_failure" if failed else "passed"


def run(python, source, output, name, selection, *, timeout=15, prefix=True):
    work = output / name
    work.mkdir()
    root = work / "project"
    shutil.copytree(source, root, ignore=shutil.ignore_patterns(".git"))
    before = hashes(source)
    config = work / "pytest.ini"
    config.write_text("[pytest]\n")
    evidence = work / "evidence.jsonl"
    command = [str(python), "-I", "-B"]
    if prefix:
        command += ["-X", f"pycache_prefix={work / 'cache'}"]
    command += [str(HERE / "driver.py"), str(root), str(evidence), str(config), *selection]
    env = {k: v for k, v in os.environ.items()
           if not k.startswith(("PYTHON", "PYTEST"))}
    env["PYTEST_DISABLE_PLUGIN_AUTOLOAD"] = "1"
    # Negative controls: -I and explicit environment ownership must defeat these.
    env["PYTHONPATH"] = str(source)
    env["PYTHONHOME"] = "/nonexistent/mutie-poison"
    env["PYTHONPYCACHEPREFIX"] = str(source / "__poison_cache__")
    started = time.monotonic()
    with (work / "output.log").open("w") as log:
        process = subprocess.Popen(command, cwd=work, env=env, stdout=log,
                                   stderr=subprocess.STDOUT, start_new_session=True)
        timed_out = False
        try:
            code = process.wait(timeout=timeout)
        except subprocess.TimeoutExpired:
            timed_out = True
            os.killpg(process.pid, signal.SIGKILL)
            code = process.wait()
    events = [json.loads(line) for line in evidence.read_text().splitlines()] if evidence.exists() else []
    result = "timeout" if timed_out else classify(events, code)
    returned = next((e for e in events if e["event"] == "returned"), {})
    for filename in returned.get("imports", {}).values():
        assert Path(filename).is_relative_to(root)
    assert hashes(source) == before
    return dict(name=name, classification=result, exit_code=code,
                wall_seconds=time.monotonic()-started, command=command,
                events=events, input_sha256=before)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--python", type=Path, required=True)
    parser.add_argument("--project", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    # Keep the venv launcher path; resolving its symlink would bypass the venv.
    python = args.python.absolute()
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    records = []
    cases = {
        "pass": ("def test_ok(): assert True\n", "passed"),
        "assertion": ("def test_bad(): assert False\n", "body_failure"),
        "explicit_fail": ("import pytest\ndef test_bad(): pytest.fail('body')\n", "body_failure"),
        "runtime_exception": ("def test_bad(): raise ValueError('body')\n", "error"),
        "setup": ("import pytest\n@pytest.fixture\ndef value(): assert False\ndef test_bad(value): pass\n", "error"),
        "teardown": ("import pytest\n@pytest.fixture\ndef value():\n yield\n assert False\ndef test_bad(value): pass\n", "error"),
        "mixed": ("import pytest\n@pytest.fixture\ndef value():\n yield\n assert False\ndef test_bad(value): assert False\n", "error"),
        "finalizer": ("import pytest\n@pytest.fixture\ndef value(request):\n def cleanup(): raise RuntimeError('cleanup')\n request.addfinalizer(cleanup)\ndef test_bad(value): assert False\n", "error"),
        "collection": ("raise RuntimeError('collection')\n", "error"),
        "syntax": ("def broken(\n", "error"),
        "import": ("import nonexistent_mutie_module\n", "error"),
        "empty": ("value = 1\n", "error"),
        "skip": ("import pytest\n@pytest.mark.skip\ndef test_skip(): pass\n", "error"),
        "xfail": ("import pytest\n@pytest.mark.xfail\ndef test_bad(): assert False\n", "error"),
        "xpass": ("import pytest\n@pytest.mark.xfail\ndef test_ok(): pass\n", "error"),
        "strict_xpass": ("import pytest\n@pytest.mark.xfail(strict=True)\ndef test_ok(): pass\n", "error"),
        "parametrized": ("import pytest\n@pytest.mark.parametrize('value',[1,2,3])\ndef test_values(value): assert value > 0\n", "passed"),
        "parametrized_failure": ("import pytest\n@pytest.mark.parametrize('value',[1,0,3])\ndef test_values(value): assert value > 0\n", "body_failure"),
        "early_exit": ("import os\ndef test_exit(): os._exit(0)\n", "error"),
        "system_exit": ("import sys\ndef test_exit(): sys.exit(1)\n", "error"),
        "thread_error": ("import threading\ndef test_thread():\n def bad(): raise RuntimeError('thread')\n t=threading.Thread(target=bad); t.start(); t.join()\n", "error"),
        "unraisable": ("import gc\ndef test_gc():\n class Bad:\n  def __del__(self): raise RuntimeError('destructor')\n value=Bad(); del value; gc.collect()\n", "error"),
        "timeout": ("import time\ndef test_wait(): time.sleep(20)\n", "timeout"),
        "session_teardown": ("import pytest\n@pytest.fixture(scope='session',autouse=True)\ndef resource():\n yield\n assert False\ndef test_ok(): pass\n", "error"),
        "module_setup": ("def setup_module(): assert False\ndef test_ok(): pass\n", "error"),
        # Known native evidence gaps, deliberately retained as negative controls.
        "unittest_setup_blindspot": ("import unittest\nclass TestExample(unittest.TestCase):\n def setUp(self): assert False\n def test_ok(self): pass\n", "body_failure"),
        "atexit_blindspot": ("import atexit\ndef bad(): raise RuntimeError('late cleanup')\natexit.register(bad)\ndef test_ok(): pass\n", "passed"),
    }
    with tempfile.TemporaryDirectory(prefix="mutie-pytest-inputs-") as temp:
        root = Path(temp)
        for name, (code, expected) in cases.items():
            project = root / name
            project.mkdir()
            (project / "test_subject.py").write_text(code)
            record = run(python, project, output, name, ["test_subject.py"], timeout=2 if name=="timeout" else 15)
            assert record["classification"] == expected, (name, record)
            records.append(record)
            print(name, record["classification"], flush=True)

        # Positive/negative cache controls: same timestamp and byte count retain
        # stale bytecode even with -B. A fresh prefix must force source loading.
        cache = root / "stale"
        cache.mkdir()
        source = cache / "subject.py"
        source.write_text("FLAG = True \n")
        subprocess.run([str(python), "-I", "-c",
                        "import py_compile,sys; py_compile.compile(sys.argv[1],doraise=True)",
                        str(source)], check=True)
        stamp = source.stat().st_mtime_ns
        source.write_text("FLAG = False\n")
        os.utime(source, ns=(stamp, stamp))
        (cache / "test_subject.py").write_text("import subject\ndef test_flag(): assert subject.FLAG is False\n")
        for prefix, expected in [(False, "body_failure"), (True, "passed")]:
            record = run(python, cache, output, f"stale-prefix-{prefix}", ["test_subject.py"], prefix=prefix)
            assert record["classification"] == expected
            records.append(record)

        # Fixture survivor -> stronger test -> kill, with all inputs retained.
        fixture = root / "fixture"
        fixture.mkdir()
        (fixture / "subject.py").write_text("def enabled(): return True\n")
        (fixture / "test_subject.py").write_text("import subject\ndef test_smoke(): assert callable(subject.enabled)\n")
        for name, value, strong, expected in [
            ("fixture-baseline", "True", False, "passed"),
            ("fixture-survivor", "False", False, "passed"),
            ("fixture-strong-baseline", "True", True, "passed"),
            ("fixture-strong-kill", "False", True, "body_failure"),
        ]:
            (fixture / "subject.py").write_text(f"def enabled(): return {value}\n")
            if strong:
                (fixture / "test_stronger.py").write_text("import subject\ndef test_enabled(): assert subject.enabled() is True\n")
            record = run(python, fixture, output, name, ["."])
            assert record["classification"] == expected
            records.append(record)

        # Real project runtime scope: explicit tests + package doctests, no mypy,
        # packaging, or documentation imports. Never claim the full tox recipe.
        project = args.project.resolve()
        original_hashes = hashes(project)
        selection = ["test_inflection.py", "inflection", "--doctest-modules"]
        baseline = run(python, project, output, "inflection-baseline", selection)
        assert baseline["classification"] == "passed", baseline
        records.append(baseline)
        owned = root / "inflection"
        shutil.copytree(project, owned, ignore=shutil.ignore_patterns(".git"))
        source = owned / "inflection/__init__.py"
        text = source.read_text()
        booleans = [n for n in ast.walk(ast.parse(text))
                    if isinstance(n, ast.Constant) and type(n.value) is bool]
        assert len(booleans) == 1
        lines = text.splitlines(keepends=True)
        node = booleans[0]
        lines[node.lineno-1] = lines[node.lineno-1][:node.col_offset] + "False" + lines[node.lineno-1][node.end_col_offset:]
        source.write_text("".join(lines))
        record = run(python, owned, output, "inflection-boolean", selection)
        # Flipping camelize's default also makes its recursive call recurse
        # indefinitely. The assertion-only policy conservatively rejects it.
        assert record["classification"] == "error", record["classification"]
        records.append(record)
        # Manual regex probe is intentionally outside boolean discovery.
        needle = r'([a-z\d])([A-Z])'
        assert text.count(needle) == 1
        source.write_text(text.replace(needle, r'([a-z])([A-Z])'))
        record = run(python, owned, output, "inflection-ascii-digit-control", selection)
        assert record["classification"] == "body_failure", record["classification"]
        records.append(record)
        source.write_text(text.replace(needle, r'([a-z0-9])([A-Z])'))
        record = run(python, owned, output, "inflection-unicode-digit-survivor", selection)
        records.append(record)
        assert record["classification"] == "passed", record
        stronger = owned / "test_stronger.py"
        stronger.write_text((HERE / "inflection-stronger.py").read_text())
        source.write_text(text)
        record = run(python, owned, output, "inflection-strong-baseline", [*selection, "test_stronger.py"])
        assert record["classification"] == "passed", record
        records.append(record)
        source.write_text(text.replace(needle, r'([a-z0-9])([A-Z])'))
        record = run(python, owned, output, "inflection-strong-kill", [*selection, "test_stronger.py"])
        assert record["classification"] == "body_failure", record
        records.append(record)
        assert hashes(project) == original_hashes

    # Evidence rejection controls on a known complete failure.
    example = next(r for r in records if r["name"] == "assertion")
    events = example["events"]
    for corrupt in [events[:-1], events[:-2], events + [events[-1]],
                    [e for e in events if e["event"] != "collection"],
                    [e for e in events if e.get("when") != "teardown"]]:
        assert classify(corrupt, 1) == "error"
    metadata = subprocess.check_output([str(python), "-I", "-c",
        "import sys,importlib.metadata,json; print(json.dumps({'python':sys.version,'dependencies':{n:importlib.metadata.version(n) for n in ['pytest','pluggy','packaging','iniconfig','Pygments']}}))"], text=True)
    result = dict(environment=json.loads(metadata), records=records,
                  rejection_controls=5, original_project_hashes=original_hashes,
                  project_preserved=True, project_commit=subprocess.check_output(
                      ["git", "-C", str(args.project), "rev-parse", "HEAD"], text=True).strip())
    (output / "results.json").write_text(json.dumps(result, indent=2) + "\n")
    print(f"PASS: {len(records)} native probes, five evidence rejections", flush=True)


if __name__ == "__main__":
    main()
