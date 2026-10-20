"""Process-lifetime native controls and protocol rejection tests."""
import argparse
import copy
import hashlib
import json
import os
from pathlib import Path
import resource
import shutil
import signal
import subprocess
import tempfile
import time
import uuid

from audit import hashes
from lifetime_protocol import MAX_EVIDENCE, classify

HERE = Path(__file__).resolve().parent


def limits():
    resource.setrlimit(resource.RLIMIT_FSIZE, (MAX_EVIDENCE, MAX_EVIDENCE))
    resource.setrlimit(resource.RLIMIT_CORE, (0, 0))


def run(python, source, output, name, selection, timeout=15):
    work = output / name
    work.mkdir()
    root = work / "project"
    shutil.copytree(source, root, ignore=shutil.ignore_patterns(".git", "__pycache__"))
    before = hashes(source)
    config = work / "pytest.ini"
    config.write_text("[pytest]\n")
    evidence = work / "evidence.jsonl"
    nonce = uuid.uuid4().hex
    command = [str(python), "-I", "-B", "-X", f"pycache_prefix={work / 'cache'}",
               str(HERE / "lifetime_driver.py"), str(root), str(evidence),
               str(config), nonce, *selection]
    env = {k: v for k, v in os.environ.items()
           if not k.startswith(("PYTHON", "PYTEST"))}
    env["PYTEST_DISABLE_PLUGIN_AUTOLOAD"] = "1"
    started = time.monotonic()
    with (work / "stdout.log").open("wb") as stdout, (work / "stderr.log").open("wb") as stderr:
        process = subprocess.Popen(command, cwd=work, env=env, stdout=stdout,
                                   stderr=stderr, start_new_session=True, preexec_fn=limits)
        timed_out = False
        try:
            code = process.wait(timeout=timeout)
        except subprocess.TimeoutExpired:
            timed_out = True
            os.killpg(process.pid, signal.SIGKILL)
            code = process.wait()
    # Treat surviving members as an error even when the direct child succeeded.
    descendants = False
    try:
        os.killpg(process.pid, 0)
        descendants = True
        os.killpg(process.pid, signal.SIGKILL)
    except ProcessLookupError:
        pass
    raw = evidence.read_bytes() if evidence.exists() else b""
    err = (work / "stderr.log").read_bytes()
    exceeded = any(p.stat().st_size >= MAX_EVIDENCE for p in
                   (work / "stdout.log", work / "stderr.log", evidence) if p.exists())
    outcome = classify(raw, code, pid=process.pid, nonce=nonce, stderr=err,
                       descendants=descendants or exceeded)
    if timed_out:
        outcome = "timeout"
    assert hashes(source) == before
    try:
        events = [json.loads(line) for line in raw.decode().splitlines()]
    except (ValueError, UnicodeError):
        events = []
    returned = next((e for e in events if e.get("event") == "returned"), {})
    for filename in returned.get("imports", {}).values():
        assert Path(filename).is_relative_to(root)
    record = dict(name=name, classification=outcome, exit_code=code,
                  wall_seconds=time.monotonic()-started, command=command, pid=process.pid,
                  nonce=nonce, stderr_bytes=len(err), descendants=descendants,
                  budget_exceeded=exceeded, events=events, input_sha256=before)
    (work / "record.json").write_text(json.dumps(record, indent=2) + "\n")
    print(name, outcome, flush=True)
    return record


def replay_controls(record):
    events = record["events"]
    encode = lambda e: (json.dumps(e) + "\n").encode()
    serialize = lambda es: b"".join(encode(e) for e in es)
    good = serialize(events)
    kwargs = dict(pid=record["pid"], nonce=record["nonce"])
    assert classify(good, record["exit_code"], **kwargs) == record["classification"]
    controls = {}
    for kind in ("lifetime_start", "start", "collection", "finish", "returned", "checkpoint"):
        controls["missing_" + kind] = serialize([e for e in events if e["event"] != kind])
    controls["missing_teardown"] = serialize([e for e in events if e.get("when") != "teardown"])
    controls["duplicate_exit"] = good + encode(events[-1])
    controls["truncated"] = good[:-1]
    controls["invalid_json"] = b"not json\n"
    controls["invalid_utf8"] = b"\xff\n"
    controls["oversized"] = b" " * (MAX_EVIDENCE + 1)
    controls["duplicate_json_key"] = good.replace(b'"seq": 0', b'"seq": 0, "seq": 0', 1)
    controls["nan"] = good.replace(b'"seq": 0', b'"seq": NaN', 1)
    controls["extra_field"] = good.replace(b'"seq": 0', b'"extra": 1, "seq": 0', 1)
    for name, index, key, value in [
        ("wrong_sequence", 1, "seq", 99),
        ("bool_sequence", 1, "seq", True),
        ("wrong_pid", 1, "pid", -1),
        ("wrong_nonce", 1, "nonce", "wrong"),
        ("unknown_event", 1, "event", "unknown"),
        ("wrong_exit", -3, "status", 99),
        ("wrong_checkpoint", -1, "stage", "returned"),
        ("changed_hook", -1, "hooks_intact", False),
        ("live_thread", -1, "alive_threads", ["worker"]),
        ("duplicate_inventory", 2, "nodes", [events[2]["nodes"][0]] * 2),
        ("invalid_inventory", 2, "nodes", [12]),
        ("wrong_pytest", 1, "pytest", "0.0"),
        ("invalid_imports", -3, "imports", []),
        ("missing_field_value", -1, "hooks_intact", None),
    ]:
        changed = copy.deepcopy(events)
        changed[index][key] = value
        controls[name] = serialize(changed)
    for name, raw in controls.items():
        assert classify(raw, record["exit_code"], **kwargs) == "error", name
    for name, extra in [("late_stderr", {"stderr": b"ignored cleanup"}),
                        ("descendants", {"descendants": True})]:
        assert classify(good, record["exit_code"], **kwargs, **extra) == "error", name
    assert classify(good, 7, **kwargs) == "error"
    assert classify(good, True, **kwargs) == "error"
    return len(controls) + 4


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--python", type=Path, required=True)
    parser.add_argument("--project", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    python = args.python.absolute()
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    records = []
    # Each process-lifetime variant runs with a passing and failing body.
    variants = {
        "clean": ("", False),
        "clean_atexit": ("import atexit\natexit.register(lambda: None)\n", False),
        "atexit_raise": ("import atexit\ndef cleanup(): raise RuntimeError('late')\natexit.register(cleanup)\n", True),
        "atexit_assert": ("import atexit\ndef cleanup(): assert False\natexit.register(cleanup)\n", True),
        "atexit_system_exit": ("import atexit,sys\natexit.register(sys.exit, 1)\n", True),
        "atexit_exit_zero": ("import atexit,os\natexit.register(os._exit, 0)\n", True),
        "atexit_clear": ("import atexit\natexit._clear()\n", True),
        "atexit_stderr": ("import atexit,os\natexit.register(os.write,2,b'late diagnostic\\n')\n", True),
        "late_gc": ("import atexit,gc\nclass Bad:\n def __del__(self): raise RuntimeError('late destructor')\nvalue=Bad()\ndef cleanup():\n global value\n del value\n gc.collect()\natexit.register(cleanup)\n", True),
        "module_gc": ("class Bad:\n def __del__(self): raise RuntimeError('module destructor')\nvalue=Bad()\n", True),
        "weakref_cleanup": ("import weakref\nclass Resource: pass\nvalue=Resource()\ndef cleanup(): raise RuntimeError('weakref cleanup')\nweakref.finalize(value,cleanup)\n", True),
        "hook_replaced": ("import atexit,sys\natexit.register(setattr,sys,'unraisablehook',lambda args:None)\n", True),
    }
    cases = {}
    for name, (setup, error) in variants.items():
        for failing in (False, True):
            cases[f"{name}-{'fail' if failing else 'pass'}"] = (
                setup + f"def test_body(): assert {not failing}\n",
                "error" if error else ("body_failure" if failing else "passed"))
    cases.update({
        "unittest_setup": ("import unittest\nclass TestExample(unittest.TestCase):\n def setUp(self): assert False\n def test_body(self): pass\n", "error"),
        "unittest_body": ("import unittest\nclass TestExample(unittest.TestCase):\n def test_body(self): assert False\n", "error"),
        "setup_failure": ("import pytest\n@pytest.fixture\ndef value(): assert False\ndef test_body(value): pass\n", "error"),
        "teardown_failure": ("import pytest\n@pytest.fixture\ndef value():\n yield\n assert False\ndef test_body(value): pass\n", "error"),
        "body_teardown_failure": ("import pytest\n@pytest.fixture\ndef value():\n yield\n assert False\ndef test_body(value): assert False\n", "error"),
        "collection_error": ("raise RuntimeError('collection')\n", "error"),
        "no_tests": ("value=1\n", "error"),
        "early_exit": ("import os\ndef test_body(): os._exit(0)\n", "error"),
        "same_named_exception": ("class AssertionError(Exception): pass\ndef test_body(): raise AssertionError()\n", "error"),
        "explicit_body_fail": ("import pytest\ndef test_body(): pytest.fail('body')\n", "body_failure"),
        "parametrized": ("import pytest\n@pytest.mark.parametrize('n',[1,2])\ndef test_body(n): assert n>0\n", "passed"),
        "parametrized_failure": ("import pytest\n@pytest.mark.parametrize('n',[0,2])\ndef test_body(n): assert n>0\n", "body_failure"),
        "skip": ("import pytest\n@pytest.mark.skip\ndef test_body(): pass\n", "error"),
        "xfail": ("import pytest\n@pytest.mark.xfail\ndef test_body(): assert False\n", "error"),
        "joined_thread": ("import threading\ndef test_body():\n t=threading.Thread(target=lambda:None); t.start(); t.join()\n", "error"),
        "captured_stderr": ("import sys\ndef test_body(): sys.stderr.write('captured body diagnostic\\n')\n", "passed"),
        "atexit_stdout": ("import atexit\natexit.register(print,'clean callback')\ndef test_body(): pass\n", "passed"),
        "thread_failure": ("import threading\ndef test_body():\n def bad(): raise RuntimeError('thread')\n t=threading.Thread(target=bad); t.start(); t.join()\n", "error"),
        "daemon_pending": ("import threading,time\ndef test_body():\n threading.Thread(target=lambda:time.sleep(20),daemon=True).start()\n", "error"),
        "late_thread_failure": ("import threading,time\ndef test_body():\n def bad():\n  time.sleep(.2)\n  raise RuntimeError('late thread')\n threading.Thread(target=bad).start()\n", "error"),
        "body_and_thread_failure": ("import threading\ndef test_body():\n def bad(): raise RuntimeError('thread')\n t=threading.Thread(target=bad); t.start(); t.join()\n assert False\n", "error"),
        "multiple_atexit_failures": ("import atexit\ndef bad(): raise RuntimeError('late')\natexit.register(bad); atexit.register(bad)\ndef test_body(): assert False\n", "error"),
        "timeout": ("import time\ndef test_body(): time.sleep(20)\n", "timeout"),
        "atexit_timeout": ("import atexit,time\natexit.register(time.sleep,20)\ndef test_body(): pass\n", "timeout"),
        "capture_overflow": ("import atexit,os\ndef flood():\n for i in range(1024): os.write(1,b'x'*8192)\natexit.register(flood)\ndef test_body(): pass\n", "error"),
        "leftover_child": ("import subprocess,sys\ndef test_body():\n subprocess.Popen([sys.executable,'-I','-c','import time; time.sleep(20)'])\n", "error"),
    })
    with tempfile.TemporaryDirectory(prefix="mutie-pytest-lifetime-inputs-") as temporary:
        root = Path(temporary)
        for name, (code, expected) in cases.items():
            project = root / name
            project.mkdir()
            (project / "test_subject.py").write_text(code)
            record = run(python, project, output, name, ["test_subject.py"],
                         timeout=3 if "timeout" in name else 15)
            records.append(record)
            assert record["classification"] == expected, (name, record["classification"], expected)

        project = args.project.resolve()
        original = hashes(project)
        selected = ["test_inflection.py", "inflection", "--doctest-modules"]
        record = run(python, project, output, "inflection-baseline", selected)
        assert record["classification"] == "passed"
        records.append(record)
        owned = root / "inflection"
        shutil.copytree(project, owned, ignore=shutil.ignore_patterns(".git", "__pycache__"))
        source = owned / "inflection/__init__.py"
        original_text = source.read_text()
        needle, replacement = r"([a-z\d])([A-Z])", r"([a-z0-9])([A-Z])"
        assert original_text.count(needle) == 1
        source.write_text(original_text.replace(needle, replacement))
        record = run(python, owned, output, "inflection-survivor", selected)
        assert record["classification"] == "passed"
        records.append(record)
        (owned / "test_stronger.py").write_text((HERE / "inflection-stronger.py").read_text())
        source.write_text(original_text)
        record = run(python, owned, output, "inflection-strong-baseline", [*selected, "test_stronger.py"])
        assert record["classification"] == "passed"
        records.append(record)
        source.write_text(original_text.replace(needle, replacement))
        record = run(python, owned, output, "inflection-strong-kill", [*selected, "test_stronger.py"])
        assert record["classification"] == "body_failure"
        records.append(record)
        assert hashes(project) == original
    rejections = sum(replay_controls(r) for r in records if r["name"] in ("clean-pass", "clean-fail"))
    result = dict(records=records, rejection_controls=rejections,
                  original_project_hashes=original, project_preserved=True,
                  probe_sha256={p.name: hashlib.sha256(p.read_bytes()).hexdigest()
                                for p in [HERE / n for n in (
                                    "audit.py", "lifetime_driver.py", "lifetime_protocol.py",
                                    "lifetime_audit.py", "inflection-stronger.py")]})
    (output / "results.json").write_text(json.dumps(result, indent=2) + "\n")
    print(f"PASS: {len(records)} native controls, {rejections} replay controls", flush=True)


if __name__ == "__main__":
    main()
