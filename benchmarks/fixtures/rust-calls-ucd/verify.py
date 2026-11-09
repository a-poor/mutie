"""Verify original inputs, saved reports and stronger-test Rust call findings."""
import hashlib
import json
from pathlib import Path
import re
import subprocess
import sys

root = Path(sys.argv[1]).resolve()
output = Path(sys.argv[2])
repo = Path(__file__).resolve().parents[3]
commit = "e8fa937f0cac643669dcaf5edac2785b15cab917"
original = root / "original"
stronger = root / "stronger"


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def paths(directory):
    return {str(p.relative_to(directory)) for p in directory.rglob("*")
            if (p.is_file() or p.is_symlink()) and ".git" not in p.relative_to(directory).parts}


assert subprocess.check_output(["git", "-C", str(original), "rev-parse", "HEAD"]).decode().strip() == commit
assert not subprocess.check_output(["git", "-C", str(original), "status", "--porcelain"])
inputs = json.loads((root / "original-inputs.json").read_text())
for name, record in inputs.items():
    for directory in [original, stronger]:
        path = directory / name
        if record["kind"] == "symlink":
            assert path.is_symlink() and str(path.readlink()) == record["target"], name
        else:
            assert not path.is_symlink() and digest(path) == record["sha256"], name
extra = "ucd-parse/tests/file_collections.rs"
assert paths(original) == set(inputs)
assert paths(stronger) == set(inputs) | {extra}
retained = repo / "benchmarks/fixtures/rust-calls-ucd/file_collections.rs"
assert digest(retained) == digest(stronger / extra)
saved = json.loads((root / "original-report-hashes.json").read_text())
assert paths(root / "original-reports") == set(saved)
for name, sha in saved.items():
    assert digest(root / "original-reports" / name) == sha, name

runs = {}
for group, baseline_tests in [("original-reports", 140), ("stronger-reports", 142)]:
    directories = sorted(p for p in (root / group).iterdir() if p.is_dir())
    assert len(directories) == 1
    directory = directories[0]
    run = json.loads((directory / "run.json").read_text())
    outcomes = json.loads((directory / "outcomes.json").read_text())
    inventory = json.loads((directory / "mutants.json").read_text())
    assert run["status"] == "completed"
    assert run["schema_version"] == run["operator_catalog_version"] == 1
    assert "--workspace" in run["baseline_command"]
    assert run["rust_scope"]["workspace"]["selected"] == ["ucd-parse"]
    baseline = (directory / "logs/baseline.stdout.log").read_text()
    counts = re.findall(r"test result: ok\. (\d+) passed; 0 failed;", baseline)
    assert sum(map(int, counts)) == baseline_tests
    runs[group] = {
        "directory": str(directory.relative_to(root)),
        "duration_seconds": run["duration_seconds"],
        "baseline_tests": baseline_tests,
        "summary": outcomes["summary"],
        "score": run["score"],
        "inventory": inventory["mutations"],
        "outcomes": [{k: m[k] for k in ["id", "state", "outcome", "duration_seconds", "exit_code"]}
                     for m in outcomes["mutations"]],
        "rerun_mapping": run.get("rerun", {}).get("mutations", []),
        "manifest_fingerprint": run["rust_scope"]["manifest_fingerprint"],
        "lockfile_fingerprint": run["rust_scope"]["lockfile_fingerprint"],
        "report_sha256": {str(p.relative_to(directory)): digest(p)
                          for p in sorted(directory.rglob("*")) if p.is_file()},
    }
first, second = runs["original-reports"], runs["stronger-reports"]
assert first["summary"] == {"killed": 3, "missed": 2, "build_failed": 0, "timeout": 0, "errors": 0}
assert second["summary"] == {"killed": 2, "missed": 0, "build_failed": 0, "timeout": 0, "errors": 0}
assert first["score"]["percentage"] == 60.0
assert second["score"]["scope"] == "rerun"
assert second["rerun_mapping"] == [{"id": 1, "source_id": 1}, {"id": 2, "source_id": 2}]
fields = ["file", "line", "column", "byte_start", "byte_end", "source_fingerprint", "original", "replacement", "operator"]
identity = lambda m: tuple(m[k] for k in fields)
assert {m["id"] for m in first["outcomes"] if m["outcome"] == "missed"} == {1, 2}
assert {identity(m) for m in first["inventory"] if m["id"] in [1, 2]} == {identity(m) for m in second["inventory"]}
result = {
    "project": {"repository": "https://github.com/BurntSushi/ucd-generate", "commit": commit, "inputs": inputs},
    "scope": {"package": "ucd-parse", "source": "ucd-parse/src/common.rs", "operators": ["call_remove"],
              "test_scope": "full workspace", "jobs": 2, "build_cache": "worker", "timeout_seconds": 60,
              "baseline_timeout_seconds": 180, "rustc": "1.99.1", "cargo": "1.99.1", "platform": "macOS"},
    "runs": runs,
    "stronger_test_sha256": digest(retained),
    "original_report_files_preserved": len(saved),
    "preservation_verified": True,
}
output.write_text(json.dumps(result, indent=2) + "\n")
print(f"Verified {len(inputs)} original paths in both copies, {len(saved)} saved report files, and two stronger-test kills")
