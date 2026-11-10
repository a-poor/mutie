#!/usr/bin/env python3
"""Capture native/observed JS+TS controls for an exact Bun runtime.

The Rust replay tests classify this evidence. Expected outcomes are fixed before
execution; exception names or native exit status alone never establish a kill.
"""
import argparse
import hashlib
import json
from pathlib import Path
import runpy
import socket
import subprocess
import tempfile
import re

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]
historical = runpy.run_path(str(HERE.parent / "bun-audit/observe.py"))
probe = historical["probe"]
CASES = dict(historical["CASES"])
CASES.update({
    "type_error": 'test("body", () => { throw new TypeError("body"); });',
    "aggregate_error": 'test("body", () => { throw new AggregateError([new Error("nested")], "body"); });',
    "custom_error": 'test("body", () => { const error = new Error("body"); error.name = "CustomFailure"; throw error; });',
    "named_timeout_error": 'test("body", () => { const error = new Error("body"); error.name = "TimeoutError"; throw error; });',
    "primitive_rejection": 'test("body", async () => { throw 42; });',
    "misleading_hook": 'afterEach(() => { const error = new Error("cleanup"); error.name = "AssertionError"; throw error; }); test("body", () => { throw new Error("body"); });',
})
POSITIVE = historical["POSITIVE"] | {
    "type_error", "aggregate_error", "custom_error", "named_timeout_error", "primitive_rejection"
}
REVISIONS = {
    "1.3.11+af24e281e": "af24e281ebacd6ac77c0f14b4206599cf4ae1c9f",
    "1.4.4+38afee6c8": "38afee6c84fcf48c0edb2466aa4b27d287a59c30",
}


def digest(data):
    return hashlib.sha256(data).hexdigest()


def sanitize(value, replacements):
    if isinstance(value, dict):
        return {key: sanitize(item, replacements) for key, item in value.items()}
    if isinstance(value, list):
        return [sanitize(item, replacements) for item in value]
    if isinstance(value, str):
        for source, target in replacements:
            value = value.replace(source, target)
        value = value.replace("/private/tmp/", "/tmp/")
        value = re.sub(r'hostname="[^"]*"', 'hostname="redacted-host"', value)
    return value


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--bun", required=True, type=Path)
    parser.add_argument("--out", required=True, type=Path)
    args = parser.parse_args()
    bun = args.bun.resolve()
    revision = subprocess.check_output([str(bun), "--revision"], text=True).strip()
    assert revision in REVISIONS, revision
    observer = REPO / "src/bun/observer.mjs"
    results = {"runtime": revision, "revision": REVISIONS[revision],
               "executable_sha256": digest(bun.read_bytes()),
               "observer_sha256": digest(observer.read_bytes()),
               "audit_sha256": digest(Path(__file__).read_bytes()), "results": []}
    for syntax in ("js", "ts"):
        for name, body in CASES.items():
            with tempfile.TemporaryDirectory(prefix="mutie-bun-runtime-") as temporary:
                root = Path(temporary)
                (root / "package.json").write_text('{"type":"module"}')
                source = probe["HEADER"] + ('const typed: boolean = true;\n' if syntax == "ts" else '') + body + '\n'
                (root / f"case.test.{syntax}").write_text(source)
                if name == "second_file_import":
                    (root / f"other.test.{syntax}").write_text('throw new Error("other import");\n')
                control = probe["run"](str(bun), root, 5)
                run = probe["run"](str(bun), root, 5, observer)
                path = root / "observer.jsonl"
                events = [json.loads(line) for line in path.read_text().splitlines()] if path.exists() else []
                expected = ("body_failure" if name in POSITIVE else "passed" if name in historical["PASS"]
                            else "timeout" if name == "pending" else "error")
                record = {"syntax": syntax, "scenario": name, "source": source,
                          "control": control, "run": run, "observer": events, "expected": expected}
                raw_hash = digest(json.dumps(record, sort_keys=True, separators=(",", ":")).encode())
                record = sanitize(record, [(str(root), "/tmp/case"), (str(REPO), "/path/to/mutie"),
                                            (str(Path.home()), "/path/to/home"), (socket.gethostname(), "redacted-host")])
                record["raw_record_sha256"] = raw_hash
                results["results"].append(record)
                print(syntax, name, expected, "exit", control["exit_code"], run["exit_code"], flush=True)
    args.out.parent.mkdir(parents=True, exist_ok=True)
    with args.out.open("x") as stream:
        json.dump(results, stream, indent=2)
        stream.write("\n")
    assert all(row["control"]["exit_code"] == row["run"]["exit_code"] for row in results["results"])


if __name__ == "__main__":
    main()
