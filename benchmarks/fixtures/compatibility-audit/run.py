"""Bounded, unchanged-project onboarding probes; all raw artifacts stay outside the repo.

Usage: run.py AUDIT_ROOT prepare|admission|retry-check [PROJECT ...]
Set MUTIE_AUDIT_NODE, MUTIE_AUDIT_BUN and MUTIE_AUDIT_PYTHON to audited executables.
Clone the six repositories into AUDIT_ROOT/NAME/original before preparing.
"""
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

ROOT = Path(sys.argv[1]).resolve()
ACTION = sys.argv[2]
PROJECTS = {
    "sjson": ("tidwall/sjson", "go", None),
    "ripgrep": ("BurntSushi/ripgrep", "rust", None),
    "click": ("pallets/click", "python", None),
    "ufo": ("unjs/ufo", "js", "vitest"),
    "eslint-node-test": ("sindresorhus/eslint-node-test", "js", "node"),
    "elysia": ("elysiajs/elysia", "js", "bun"),
}
NAMES = sys.argv[3:] or list(PROJECTS)


def sha(data):
    return hashlib.sha256(data).hexdigest()


def inputs(root):
    entries = subprocess.check_output(["git", "-C", str(root), "ls-files", "--stage", "-z"])
    result = {}
    for entry in entries.decode().strip("\0").split("\0"):
        metadata, name = entry.split("\t", 1)
        mode = metadata.split()[0]
        path = root / name
        assert mode in {"100644", "100755", "120000"}, (name, mode)
        data = os.readlink(path).encode() if mode == "120000" else path.read_bytes()
        result[name] = {"mode": mode, "sha256": sha(data)}
    return result


def copy_inputs(source, target, manifest):
    target.mkdir()
    for name, record in manifest.items():
        destination = target / name
        destination.parent.mkdir(parents=True, exist_ok=True)
        if record["mode"] == "120000":
            destination.symlink_to(os.readlink(source / name))
        else:
            shutil.copy2(source / name, destination)


def limits():
    resource.setrlimit(resource.RLIMIT_FSIZE, (16 << 20, 16 << 20))


def execute(directory, label, command, cwd, timeout=90, env=None, compile=False):
    logs = directory / "logs"
    logs.mkdir(exist_ok=True)
    result_path = logs / f"{label}.json"
    assert not result_path.exists(), f"retain prior attempt: {result_path}"
    started = time.monotonic()
    expired = False
    with (logs / f"{label}.stdout").open("wb") as out, (logs / f"{label}.stderr").open("wb") as err:
        child = subprocess.Popen(command, cwd=cwd, env=env, stdout=out, stderr=err,
                                 stdin=subprocess.DEVNULL, start_new_session=True,
                                 preexec_fn=None if compile else limits)
        try:
            child.wait(timeout=timeout)
        except subprocess.TimeoutExpired:
            expired = True
            os.killpg(child.pid, signal.SIGTERM)
            try:
                child.wait(timeout=2)
            except subprocess.TimeoutExpired:
                pass
            try:
                os.killpg(child.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            child.wait(timeout=5)
    result = {"command": [str(s) for s in command], "cwd": str(cwd),
              "seconds": round(time.monotonic() - started, 3), "exit": child.returncode,
              "timed_out": expired, "deadline_seconds": timeout,
              "stdout": f"logs/{label}.stdout", "stderr": f"logs/{label}.stderr"}
    result_path.write_text(json.dumps(result, indent=2) + "\n")
    print(directory.name, label, result["exit"], result["seconds"], flush=True)
    return result


def prepare():
    assert not (ROOT / "mutie").exists(), "keep the frozen CLI"
    shutil.copy2(Path(__file__).resolve().parents[3] / "target/release/mutie", ROOT / "mutie")
    for name in NAMES:
        directory = ROOT / name
        original = directory / "original"
        assert not subprocess.check_output(["git", "-C", str(original), "status", "--porcelain"])
        manifest = inputs(original)
        commit = subprocess.check_output(["git", "-C", str(original), "rev-parse", "HEAD"]).decode().strip()
        record = {"project": PROJECTS[name][0], "commit": commit, "inputs": manifest}
        (directory / "inputs.json").write_text(json.dumps(record, indent=2) + "\n")
        copy_inputs(original, directory / "onboarding", manifest)
        print(name, commit, len(manifest), flush=True)


def admission():
    env = os.environ.copy()
    env["PATH"] = os.pathsep.join([str(Path(env["MUTIE_AUDIT_NODE"]).parent),
                                 str(Path(env["MUTIE_AUDIT_BUN"]).parent), env["PATH"]])
    for name in NAMES:
        directory = ROOT / name
        root = directory / "onboarding"
        _, language, runner = PROJECTS[name]
        init = [str(ROOT / "mutie"), "init", "--non-interactive"]
        if language == "python":
            init += ["--python", env["MUTIE_AUDIT_PYTHON"]]
        result = execute(directory, "init-auto", init, root, env=env)
        if result["exit"] != 0 and runner:
            result = execute(directory, "init-explicit", init + ["--lang", "js", "--runner", runner], root, env=env)
        if result["exit"] == 0:
            generated = root / "mutie.toml"
            shutil.copy2(generated, directory / "generated.toml")
            execute(directory, "check", [str(ROOT / "mutie"), "check", "--json"], root, timeout=120, env=env)
        else:
            # Explicit external config allows authoritative adapter diagnostics
            # even when init rejects tooling. It does not alter upstream metadata.
            text = f'baseline_timeout = 60\n[{language}]\npath = "original"\n'
            if runner:
                text += f'runner = "{runner}"\n'
            config = directory / "probe.toml"
            config.write_text(text)
            execute(directory, "check-explicit-config", [str(ROOT / "mutie"), "check", "--json", "--config", str(config)], directory, timeout=120, env=env)
        expected = json.loads((directory / "inputs.json").read_text())["inputs"]
        assert inputs(directory / "original") == expected
        for relative, record in expected.items():
            path = root / relative
            data = os.readlink(path).encode() if record["mode"] == "120000" else path.read_bytes()
            assert sha(data) == record["sha256"], (name, relative)


if ACTION == "prepare":
    prepare()
elif ACTION == "admission":
    admission()
elif ACTION == "retry-check":
    for name in NAMES:
        directory = ROOT / name
        execute(directory, "check-host", [str(ROOT / "mutie"), "check", "--json"],
                directory / "onboarding", timeout=120)
elif ACTION in {"native-rust", "native-python"}:
    name = "ripgrep" if ACTION == "native-rust" else "click"
    directory = ROOT / name
    manifest = json.loads((directory / "inputs.json").read_text())["inputs"]
    native = directory / "native"
    copy_inputs(directory / "original", native, manifest)
    env = os.environ.copy()
    if ACTION == "native-rust":
        env["CARGO_TARGET_DIR"] = str(directory / "native-target")
        command = ["cargo", "test", "--locked", "--workspace", "--tests", "--no-fail-fast"]
    else:
        env["PYTHONPATH"] = str(native / "src")
        env["PYTHONDONTWRITEBYTECODE"] = "1"
        command = [env["MUTIE_AUDIT_PYTHON"], "-m", "pytest", "-q", "-p", "no:cacheprovider"]
    execute(directory, ACTION, command, native, timeout=240, env=env, compile=True)
elif ACTION in {"sjson-list", "sjson-list-host"}:
    directory = ROOT / "sjson"
    config = directory / "run.toml"
    config.write_text('jobs = 2\ntimeout = 10\nbaseline_timeout = 60\noutput_dir = "reports"\n[go]\npath = "original"\n')
    label = "list-all" if ACTION == "sjson-list" else "list-all-host"
    execute(directory, label, [str(ROOT / "mutie"), "list", "--json", "--config", str(config)], directory)
elif ACTION == "sjson-run":
    directory = ROOT / "sjson"
    config = directory / "selected.toml"
    config.write_text('jobs = 2\ntimeout = 10\nbaseline_timeout = 60\noutput_dir = "reports"\n[go]\npath = "original"\noperators = ["string_replace", "negation_remove"]\n')
    execute(directory, "run", [str(ROOT / "mutie"), "run", "--plain", "--config", str(config)],
            directory, timeout=180)
elif ACTION == "native-python-installed":
    directory = ROOT / "click"
    env = os.environ.copy()
    env["PYTHONDONTWRITEBYTECODE"] = "1"
    execute(directory, ACTION, [str(directory / "native-env/bin/python"), "-m", "pytest", "-q", "-p", "no:cacheprovider"],
            directory / "native", timeout=240, env=env, compile=True)
elif ACTION == "sjson-rerun":
    directory = ROOT / "sjson"
    report, = (directory / "reports").iterdir()
    hashes = {str(p.relative_to(report)): sha(p.read_bytes()) for p in report.rglob("*") if p.is_file()}
    (directory / "report-hashes.json").write_text(json.dumps(hashes, indent=2) + "\n")
    manifest = json.loads((directory / "inputs.json").read_text())["inputs"]
    stronger = directory / "stronger"
    copy_inputs(directory / "original", stronger, manifest)
    shutil.copy2(Path(__file__).with_name("sjson_contracts_test.go"), stronger / "compatibility_test.go")
    config = directory / "stronger.toml"
    config.write_text('jobs = 2\ntimeout = 10\nbaseline_timeout = 60\noutput_dir = "stronger-reports"\n[go]\npath = "stronger"\noperators = ["string_replace", "negation_remove"]\n')
    execute(directory, "rerun", [str(ROOT / "mutie"), "rerun", str(report), "--missed", "--plain", "--config", str(config)],
            directory, timeout=180)
elif ACTION == "native-rust-git":
    directory = ROOT / "ripgrep"
    native = directory / "native"
    shutil.copytree(directory / "original/.git", native / ".git")
    env = os.environ.copy()
    env["CARGO_TARGET_DIR"] = str(directory / "native-git-target")
    execute(directory, ACTION, ["cargo", "test", "--locked", "--workspace", "--tests", "--no-fail-fast"],
            native, timeout=240, env=env, compile=True)
else:
    raise SystemExit(f"unknown action {ACTION}")
