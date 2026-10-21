#!/usr/bin/env python3
"""Compare fresh shebang loading with an explicit shell; no Mutie involved."""
import argparse
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import time

parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--output", type=Path, required=True)
args = parser.parse_args()
args.output.mkdir(parents=True, exist_ok=False)
results = []
with tempfile.TemporaryDirectory(prefix="scripts-", dir=args.output) as directory:
    for index, kind in enumerate(["direct", "explicit", "direct", "explicit"]):
        script = Path(directory).resolve() / f"probe-{index}"
        script.write_text("#!/bin/sh\nprintf ready\n")
        script.chmod(0o755)
        command = [str(script)] if kind == "direct" else ["/bin/sh", str(script)]
        started = time.monotonic()
        child = subprocess.Popen(command, stdout=subprocess.PIPE,
                                 stderr=subprocess.PIPE, start_new_session=True)
        timed_out = False
        try:
            stdout, stderr = child.communicate(timeout=3)
        except subprocess.TimeoutExpired:
            timed_out = True
            try:
                if sys.platform == "darwin":
                    subprocess.run(["sample", str(child.pid), "1", "-file",
                                    str(args.output.resolve() / f"sample-{index}.txt")],
                                   stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
                                   timeout=5, check=False)
            finally:
                child.kill()
                stdout, stderr = child.communicate()
        result = {"kind": kind, "seconds": time.monotonic() - started,
                  "code": child.returncode, "timed_out": timed_out,
                  "stdout_bytes": len(stdout), "stderr": stderr.decode(errors="replace")}
        results.append(result)
        print(json.dumps(result), flush=True)
        (args.output / "results.json").write_text(json.dumps(results, indent=2) + "\n")
