"""Opt-in native matrix; requires prepared, pinned installations (never installs).

Each invocation creates a new output directory. Failed attempts remain evidence.
Run with the pinned Node/Bun executables first on PATH.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess


def hashes(root):
    result = {}
    for path in sorted(root.rglob("*")):
        if path.is_symlink():
            result[str(path.relative_to(root))] = "link:" + os.readlink(path)
        elif path.is_file():
            result[str(path.relative_to(root))] = hashlib.sha256(path.read_bytes()).hexdigest()
    return result


def command(args, log, rejection=None):
    with log.open("wb") as output:
        child = subprocess.Popen(args, stdout=output, stderr=subprocess.STDOUT, start_new_session=True)
        try:
            code = child.wait(timeout=120)
        except subprocess.TimeoutExpired:
            os.killpg(child.pid, signal.SIGKILL)
            child.wait()
            raise AssertionError(f"outer startup/workflow bound exceeded; see {log}")
    reports = [line.removeprefix("Saving report to ") for line in log.read_text().splitlines()
               if line.startswith("Saving report to ")]
    assert len(reports) == 1, log
    report = Path(reports[0])
    run = json.loads((report / "run.json").read_text())
    if rejection is not None:
        assert code == 1 and rejection in run["error"], f"expected identity rejection; see {log}"
        assert run["status"] == "failed" and run["phase"] == "snapshot", run
        assert run["planned_mutations"] is None and run["score"]["percentage"] is None, run
        assert run["workspace_metrics"]["baseline_setup"]["operations"] == 0, run
        return None
    assert code == 0, f"exit {code}; see {log}"
    assert run["status"] == "completed", run
    return report, run


def case(options, manager, runner, syntax):
    name = f"{manager}-{runner}-{syntax}"
    destination = options.output / name
    destination.mkdir()
    fixture = {("node", "js"): "node", ("node", "ts"): "node-typescript",
               ("bun", "js"): "bun-js", ("bun", "ts"): "bun-typescript"}[runner, syntax]
    root = destination / "original"
    shutil.copytree(options.repo / "tests/fixtures" / fixture, root,
                    ignore=shutil.ignore_patterns("node_modules", ".mutie"))
    installation = options.installations / manager
    manifest = json.loads((installation / "package.json").read_text())
    manifest["scripts"] = json.loads((root / "package.json").read_text())["scripts"]
    (root / "package.json").write_text(json.dumps(manifest) + "\n")
    for filename in ["package-lock.json", "pnpm-lock.yaml", "pnpm-workspace.yaml",
                     "yarn.lock", ".yarnrc.yml", "bun.lock"]:
        if (installation / filename).exists():
            shutil.copy2(installation / filename, root / filename)
    shutil.copytree(installation / "node_modules", root / "node_modules", symlinks=True)
    # Reading installed bytes establishes that snapshots retain the installation.
    test = root / f"test/installation.test.{syntax}"
    assertion = "assert.equal(installed.version, '4.1.9')" if runner == "node" else "expect(installed.version).toBe('4.1.9')"
    imports = "import { test } from 'node:test';\nimport assert from 'node:assert/strict';" if runner == "node" else "import { test, expect } from 'bun:test';"
    test.write_text(imports + "\nimport { readFileSync } from 'node:fs';\n"
                    "test('development installation remains visible', () => {\n"
                    "const installed = JSON.parse(readFileSync('node_modules/vitest/package.json', 'utf8'));\n"
                    + assertion + ";\n});\n")
    original = hashes(root)
    base = [str(options.mutie), str(root), "--lang", runner, "--ui", "plain",
            "--timeout", "20", "--baseline-timeout", "30", "--output-dir", str(destination / "reports")]
    report, run = command(base, destination / "original.log")
    assert run["score"]["killed"] == (2 if runner == "node" else 1), run
    assert run["score"]["missed"] == 1, run
    saved = hashes(report)
    stronger = destination / "stronger"
    shutil.copytree(root, stronger, symlinks=True)
    with (stronger / f"test/flags.test.{syntax}").open("a") as output:
        output.write("\ntest('fallback boundary', () => " + (
            "assert.equal(fallback(false), true)" if runner == "node" else
            "expect(fallback('   ')).toBe(false)") + ");\n")
    rerun = base.copy()
    rerun[1] = str(stronger)
    _, rerun_record = command(rerun + ["--rerun-missed", str(report)], destination / "stronger.log")
    assert rerun_record["score"]["killed"] == 1, rerun_record
    assert rerun_record["score"]["missed"] == 0, rerun_record
    assert hashes(root) == original, "original project changed"
    assert hashes(report) == saved, "original report changed"
    return {"case": name, "original_score": run["score"], "stronger_score": rerun_record["score"]}


def drift(options, result):
    name = result["case"]
    runner = name.split("-")[1]
    destination = options.output / name
    root = destination / "stronger"
    report_line = next(line for line in (destination / "original.log").read_text().splitlines()
                       if line.startswith("Saving report to "))
    report = Path(report_line.removeprefix("Saving report to "))
    saved = hashes(report)
    # Content-only drift: package name/version and lock metadata remain unchanged.
    file = root / "node_modules/vitest/README.md"
    before = file.read_bytes()
    log = destination / "drift.log"
    attempt = 1
    while log.exists():
        attempt += 1
        log = destination / f"drift-attempt{attempt}.log"
    try:
        file.write_bytes(before + b"\nMutie installation drift control.\n")
        command([str(options.mutie), str(root), "--lang", runner, "--ui", "plain",
                 "--rerun-missed", str(report), "--output-dir", str(destination / "drift-reports")],
                log, "identity changed")
    finally:
        file.write_bytes(before)
    assert hashes(report) == saved, "original report changed"
    return {"case": name, "installation_drift_rejected": True}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", type=Path, required=True)
    parser.add_argument("--mutie", type=Path, required=True)
    parser.add_argument("--installations", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--managers", nargs="+", default=["npm", "pnpm", "yarn", "bun"])
    parser.add_argument("--runners", nargs="+", default=["node", "bun"])
    parser.add_argument("--syntaxes", nargs="+", default=["js", "ts"])
    parser.add_argument("--verify-drift", action="store_true",
                        help="verify existing successful matrix cases without rerunning mutations")
    options = parser.parse_args()
    if options.verify_drift:
        results = json.loads((options.output / "results.json").read_text())
        checked = [drift(options, result) for result in results if "error" not in result]
        (options.output / "drift-results.json").write_text(json.dumps(checked, indent=2) + "\n")
        print(json.dumps(checked), flush=True)
        return
    options.output.mkdir()
    results = []
    for manager in options.managers:
        for runner in options.runners:
            for syntax in options.syntaxes:
                try:
                    result = case(options, manager, runner, syntax)
                except Exception as error:
                    result = {"case": f"{manager}-{runner}-{syntax}", "error": str(error)}
                results.append(result)
                (options.output / "results.json").write_text(json.dumps(results, indent=2) + "\n")
                print(json.dumps(result), flush=True)
    raise SystemExit(any("error" in result for result in results))


if __name__ == "__main__":
    main()
