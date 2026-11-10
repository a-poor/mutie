#!/usr/bin/env python3
"""Run the UFO acceptance loop using an already installed, unchanged checkout.

Requires Node 24.22.0, pnpm 10.33.2 and a release Mutie executable. Install UFO's
frozen lockfile with scripts disabled before invoking this script. No installer
or shell package script is supplied to Mutie itself.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess
import time

REVISION = "f06c800d0c59f2a4a1b9ba65eb6cb61a84419be6"


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--project", type=Path, required=True)
    parser.add_argument("--mutie", type=Path, required=True)
    parser.add_argument("--node", type=Path, required=True)
    parser.add_argument("--pnpm", type=Path, required=True,
                        help="path to the pnpm launcher (whose directory also contains pnpm)")
    parser.add_argument("--output", type=Path, required=True, help="new output directory")
    args = parser.parse_args()
    project = args.project.resolve()
    mutie, node = args.mutie.resolve(), args.node.resolve()
    pnpm = args.pnpm.absolute()  # Retain a .bin launcher directory for pnpm's nested script.
    output = args.output.absolute()
    assert subprocess.check_output(["git", "-C", str(project), "rev-parse", "HEAD"], text=True).strip() == REVISION
    assert not subprocess.check_output(["git", "-C", str(project), "status", "--porcelain"])
    assert (project / "node_modules").is_dir(), "install the frozen dependencies first"
    output.mkdir()  # Never overwrite an earlier audit.
    (output / "logs").mkdir()
    env = os.environ.copy()
    env["PATH"] = os.pathsep.join([str(node.parent), str(pnpm.parent), env.get("PATH", "")])
    env.update(CI="true", NO_COLOR="1")
    env.pop("NODE_OPTIONS", None)
    env.pop("NODE_PATH", None)
    assert subprocess.check_output([str(node), "--version"], text=True, env=env).strip() == "v24.22.0"
    assert subprocess.check_output([str(node), str(pnpm), "--version"], text=True, env=env).strip() == "10.33.2"
    names = subprocess.check_output(["git", "-C", str(project), "ls-files", "-z"]).decode().split("\0")[:-1]
    inputs = {name: digest(project / name) for name in names}
    for label in ["original", "stronger"]:
        root = output / label
        root.mkdir()
        for name in names:
            source, target = project / name, root / name
            assert not source.is_symlink(), name
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(source, target)
        shutil.copytree(project / "node_modules", root / "node_modules", symlinks=True)

    def run(label, root, command, seconds, expected=0):
        start = time.monotonic()
        log = output / "logs" / (label + ".log")
        with log.open("wb") as stream:
            child = subprocess.Popen(list(map(str, command)), cwd=root, env=env,
                                     stdout=stream, stderr=subprocess.STDOUT, start_new_session=True)
            try:
                code = child.wait(timeout=seconds)
            finally:
                try:
                    os.killpg(child.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
                child.wait(timeout=5)
        record = {"exit": code, "seconds": round(time.monotonic() - start, 3)}
        (output / "logs" / (label + ".json")).write_text(json.dumps(record, indent=2) + "\n")
        assert code == expected, (label, code, log)
        return log

    def config(label, source, operators, include=None):
        text = f'jobs = 1\ntimeout = 60\nbaseline_timeout = 120\n[js]\npath = "{source}"\nrunner = "vitest"\noperators = {json.dumps(operators)}\n'
        if include:
            text += f'include = {json.dumps(include)}\n'
        path = output / (label + ".toml")
        path.write_text(text)
        return path

    def scored(label, config_path, expected=0):
        log = run(label, output, [mutie, "run", "--config", config_path, "--json"], 900, expected)
        events = [json.loads(line) for line in log.read_text().splitlines() if line.startswith("{")]
        summary = next(event for event in reversed(events) if event.get("event") == "summary")
        return Path(summary["report"])

    original, stronger = output / "original", output / "stronger"
    run("native", original, [node, pnpm, "test"], 180)
    boolean = config("boolean", "original", ["boolean_flip"])
    run("check", original, [mutie, "check", "--config", boolean, "--json"], 180)
    scored("boolean", boolean)
    boundary = config("boundary", "original", ["comparison_boundary"], ["src/punycode.ts"])
    report = scored("boundary", boundary)
    query = config("query", "original", ["comparison_boundary", "comparison_invert"], ["src/query.ts"])
    scored("query", query, expected=1)  # Preserve the source type error and failed score.
    saved = {str(path.relative_to(original)): digest(path)
             for path in (original / ".mutie").rglob("*") if path.is_file()}
    inventory = json.loads((report / "mutants.json").read_text())["mutations"]
    selected = [m["id"] for m in inventory if m["file"] == "src/punycode.ts"
                and m["line"] == 51 and (m["original"], m["replacement"]) in [(">=", ">"), ("<=", "<")]]
    assert len(selected) == 2
    outcomes = json.loads((report / "outcomes.json").read_text())["mutations"]
    assert all(next(item for item in outcomes if item["id"] == number)["outcome"] == "missed"
               for number in selected)
    shutil.copy2(Path(__file__).with_name("stronger-host.test.ts"), stronger / "test/mutie-host-boundaries.test.ts")
    run("stronger-native", stronger, [node, pnpm, "test"], 180)
    focused = config("stronger", "stronger", ["comparison_boundary"], ["src/punycode.ts"])
    focused_log = run("stronger-focused", stronger, [mutie, "rerun", report, "--config", focused,
                                                     "--id", ",".join(map(str, selected)), "--json"], 300)
    summary = json.loads(focused_log.read_text().splitlines()[-1])
    assert summary["event"] == "summary" and summary["status"] == "completed"
    assert summary["total"] == summary["outcomes"]["killed"] == 2
    for root in [project, original, stronger]:
        assert all(digest(root / name) == checksum for name, checksum in inputs.items())
    assert all(digest(original / name) == checksum for name, checksum in saved.items())
    (output / "preservation.json").write_text(json.dumps({"revision": REVISION, "inputs": inputs,
                                                           "original_reports": saved}, indent=2) + "\n")
    print(f"Preserved {len(inputs)} upstream inputs and {len(saved)} original report files.")


if __name__ == "__main__":
    main()
