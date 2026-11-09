"""Bounded Cargo acceptance commands; retain each log, result and input hash.

Usage: run.py ROOT LABEL PROJECT CONFIG COMMAND [ARG ...]
ROOT/mutie is a frozen release build. PROJECT/CONFIG are relative to ROOT.
Labels are never overwritten. The caller prepares complete tracked-source copies.
"""
import hashlib
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import time

root = Path(sys.argv[1]).resolve()
label, project, config, *arguments = sys.argv[2:]
logs = root / "logs"
logs.mkdir(exist_ok=True)
result_path = logs / f"{label}.json"
assert not result_path.exists(), result_path
command = [str(root / "mutie"), "--path", str(root / project), *arguments,
           "--config", str(root / config)]
if arguments[0] in {"run", "rerun"}:
    command.append("--plain")
started = time.monotonic()
expired = False
env = dict(os.environ, CARGO_NET_OFFLINE="true")
with (logs / f"{label}.stdout").open("xb") as out, (logs / f"{label}.stderr").open("xb") as err:
    child = subprocess.Popen(command, stdout=out, stderr=err, stdin=subprocess.DEVNULL,
                             start_new_session=True, env=env)
    try:
        child.wait(timeout=900)
    except subprocess.TimeoutExpired:
        expired = True
        os.killpg(child.pid, signal.SIGTERM)
        try:
            child.wait(timeout=5)
        except subprocess.TimeoutExpired:
            os.killpg(child.pid, signal.SIGKILL)
            child.wait(timeout=5)
result = {"command": command, "seconds": round(time.monotonic() - started, 3),
          "exit": child.returncode, "timed_out": expired, "deadline_seconds": 900,
          "binary_sha256": hashlib.sha256((root / "mutie").read_bytes()).hexdigest()}
result_path.write_text(json.dumps(result, indent=2) + "\n")
print(json.dumps(result), flush=True)
sys.exit(child.returncode if not expired else 124)
