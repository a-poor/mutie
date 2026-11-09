"""Verify preserved Route Mini inputs/reports and focused call-removal findings."""
import hashlib
import json
from pathlib import Path
import subprocess
import sys

root = Path(sys.argv[1]).resolve()
repo = Path(__file__).resolve().parents[3]
record = json.loads((root / "preservation.json").read_text())
commit = "35a3db157731ba0073d66f432b69b997ce2e9f01"


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def paths(directory):
    return {str(p.relative_to(directory)) for p in directory.rglob("*")
            if p.is_file() and ".git" not in p.relative_to(directory).parts}


assert record["commit"] == commit
assert subprocess.check_output(["git", "-C", str(root / "original"), "rev-parse", "HEAD"]).decode().strip() == commit
assert not subprocess.check_output(["git", "-C", str(root / "original"), "status", "--porcelain"])
for name, sha in record["inputs"].items():
    for copy in ["original", "stronger"]:
        assert digest(root / copy / name) == sha, (copy, name)
extra = "tests/test_call_effects.py"
assert paths(root / "original") == set(record["inputs"])
assert paths(root / "stronger") == set(record["inputs"]) | {extra}
retained = Path(__file__).parent / "test_call_effects.py"
assert digest(root / "stronger" / extra) == digest(retained)
saved = json.loads((root / "original-report-hashes.json").read_text())
assert paths(root / "original-reports") == set(saved)
for name, sha in saved.items():
    assert digest(root / "original-reports" / name) == sha, name

recipe_paths = ["src/python.rs", "src/python/identity.rs", "src/python/scope.py",
                "src/python/runtime.py", "src/python/driver.py", "src/python/protocol.rs",
                "src/python/project.rs", "src/process.rs", "src/capture.rs",
                "src/workspace.rs", "src/command.rs", "Cargo.lock"]
recipe = "sha256:" + hashlib.sha256(b"".join((repo / p).read_bytes() for p in recipe_paths)).hexdigest()
runs = {}
for group, count in [("original-reports", 68), ("stronger-reports", 71)]:
    directory, = (root / group).iterdir()
    run = json.loads((directory / "run.json").read_text())
    outcomes = json.loads((directory / "outcomes.json").read_text())
    inventory = json.loads((directory / "mutants.json").read_text())
    assert run["schema_version"] == run["operator_catalog_version"] == 1
    assert run["python_scope"]["recipe_fingerprint"] == recipe
    assert len(run["python_scope"]["test_nodes"]) == count
    assert all(m["operator"] == "call_remove" and m["replacement"] == "pass"
               and m["file"] == "src/route_mini/router.py" for m in inventory["mutations"])
    runs[group] = {
        "directory": str(directory.relative_to(root)), "status": run["status"],
        "duration_seconds": run["duration_seconds"], "baseline_tests": count,
        "summary": outcomes["summary"], "score": run["score"],
        "recipe_fingerprint": recipe, "inventory": inventory["mutations"],
        "outcomes": [{k: m[k] for k in ["id", "state", "outcome", "duration_seconds", "exit_code"]}
                     for m in outcomes["mutations"]],
        "rerun_mapping": run.get("rerun", {}).get("mutations", []),
        "report_sha256": {str(p.relative_to(directory)): digest(p)
                          for p in sorted(directory.rglob("*")) if p.is_file()},
    }
first, second = runs["original-reports"], runs["stronger-reports"]
assert first["status"] == "failed" and first["score"]["percentage"] is None
assert first["summary"] == {"killed": 1, "missed": 4, "errors": 2, "timeout": 0, "build_failed": 0}
assert {m["id"] for m in first["outcomes"] if m["outcome"] == "missed"} == {1, 2, 3, 5}
assert second["status"] == "completed" and second["score"]["scope"] == "rerun"
assert second["summary"] == {"killed": 3, "missed": 0, "errors": 0, "timeout": 0, "build_failed": 0}
assert second["rerun_mapping"] == [{"id": 1, "source_id": 1}, {"id": 2, "source_id": 2}, {"id": 3, "source_id": 5}]
fields = ["file", "line", "column", "byte_start", "byte_end", "source_fingerprint", "original", "replacement", "operator"]
identity = lambda m: tuple(m[k] for k in fields)
assert {identity(m) for m in first["inventory"] if m["id"] in [1, 2, 5]} == {identity(m) for m in second["inventory"]}
result = {
    "project": {"repository": "https://github.com/JustinJLeopard/route-mini", **record},
    "scope": {"source": "src/route_mini/router.py", "operators": ["call_remove"],
              "test_scope": "full", "jobs": 1, "timeout_seconds": 15,
              "baseline_timeout_seconds": 120, "continue_on_error": 10,
              "python": "3.14.7", "pytest": "9.0.2", "platform": "macOS"},
    "runs": runs, "stronger_test_sha256": digest(retained),
    "original_report_files_preserved": len(saved), "preservation_verified": True,
    "open_findings": {"survivor_source_ids": [3], "error_source_ids": [4, 7]},
}
Path(sys.argv[2]).write_text(json.dumps(result, indent=2) + "\n")
print(f"Verified {len(record['inputs'])} original files in both copies, {len(saved)} report files and three stronger-test kills")
