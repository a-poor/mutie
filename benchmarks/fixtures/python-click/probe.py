"""Native observer exploration, never mutation scores.

Usage: probe.py ROOT LABEL DRIVER PYTHON
ROOT/observer-probe is a complete unchanged Click copy. Each label owns its
HOME/TMP/evidence/bytecode directories and retains bounded command diagnostics.
"""
import collections
import hashlib
import json
import os
from pathlib import Path
import resource
import shutil
import signal
import subprocess
import sys
import time

root = Path(sys.argv[1]).resolve()
label, driver, python = sys.argv[2:]
owned = root / label
owned.mkdir()
for name in ["home", "tmp"]:
    (owned / name).mkdir()
shutil.copy2(driver, owned / "driver.py")
project = root / "observer-probe"
evidence = owned / "evidence.jsonl"
command = [python, "-I", "-B", "-X", f"pycache_prefix={owned / 'cache'}",
           str(owned / "driver.py"), str(project), str(evidence),
           str(project / "pyproject.toml"), label, "src", "click", "tests"]
environment = dict(os.environ, HOME=str(owned / "home"), TMPDIR=str(owned / "tmp"),
                   TMP=str(owned / "tmp"), TEMP=str(owned / "tmp"),
                   PYTEST_DISABLE_PLUGIN_AUTOLOAD="1", PYTEST_ADDOPTS="", PYTEST_PLUGINS="")


def limits():
    resource.setrlimit(resource.RLIMIT_FSIZE, (16 << 20, 16 << 20))


started = time.monotonic()
expired = False
with (owned / "stdout").open("xb") as out, (owned / "stderr").open("xb") as err:
    child = subprocess.Popen(command, cwd=project, env=environment, stdout=out,
                             stderr=err, stdin=subprocess.DEVNULL,
                             start_new_session=True, preexec_fn=limits)
    try:
        child.wait(timeout=60)
    except subprocess.TimeoutExpired:
        expired = True
        os.killpg(child.pid, signal.SIGTERM)
        try:
            child.wait(timeout=2)
        except subprocess.TimeoutExpired:
            os.killpg(child.pid, signal.SIGKILL)
            child.wait(timeout=5)
events = []
if evidence.exists():
    for line in evidence.read_bytes().splitlines():
        try:
            events.append(json.loads(line))
        except ValueError:
            events.append({"event": "invalid_record"})
bad = [e for e in events if e["event"] in
       {"unsupported", "lifetime_error", "fixture_error", "warning", "internal_error"}]
summary = dict(exit=child.returncode, seconds=round(time.monotonic() - started, 3),
               timed_out=expired, events=dict(collections.Counter(e["event"] for e in events)),
               bad=dict(collections.Counter(str((e["event"], e.get("kind"), e.get("category"))) for e in bad)),
               driver_sha256=hashlib.sha256(Path(driver).read_bytes()).hexdigest(),
               evidence_bytes=evidence.stat().st_size if evidence.exists() else 0)
(owned / "result.json").write_text(json.dumps(summary, indent=2) + "\n")
print(json.dumps(summary, indent=2))
