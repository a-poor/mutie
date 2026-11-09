"""Verify unchanged inputs, original reports, and separate stronger-test evidence."""
import hashlib
import json
from pathlib import Path
import subprocess
import sys

root = Path(sys.argv[1]).resolve()
output = Path(sys.argv[2])
repo = Path(__file__).resolve().parents[3]
record = json.loads((root / "preservation.json").read_text())
original = root / "original"
stronger = root / "stronger"


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


assert not subprocess.check_output(["git", "-C", str(original), "status", "--porcelain"])
assert subprocess.check_output(["git", "-C", str(original), "rev-parse", "HEAD"]).decode().strip() == record["commit"]
for name, sha in record["inputs"].items():
    assert digest(original / name) == sha, name
    assert digest(stronger / name) == sha, name
extra = "test/session-keys.test.js"
assert {str(p.relative_to(stronger)) for p in stronger.rglob("*") if p.is_file()} == set(record["inputs"]) | {extra}
retained = repo / "benchmarks/fixtures/javascript-strings/session-keys.test.js"
assert digest(retained) == digest(stronger / extra)

runs = {}
records = {}
fields = ["file", "operator", "source_fingerprint", "byte_start", "byte_end", "original", "replacement", "line", "column", "end_line", "end_column"]


def identity(mutation):
    return tuple(mutation[key] for key in fields)


for group, tests in [("original-reports", 62), ("stronger-reports", 67)]:
    reports = list((root / group).iterdir())
    assert len(reports) == 1
    report = reports[0]
    run = json.loads((report / "run.json").read_text())
    outcomes = json.loads((report / "outcomes.json").read_text())
    inventory = json.loads((report / "mutants.json").read_text())
    assert run["status"] == outcomes["status"] == "completed"
    assert outcomes["baseline"]["outcome"] == "passed"
    baseline = (report / "logs/baseline.stdout.log").read_text()
    assert f"tests {tests}\n" in baseline and f"pass {tests}\n" in baseline and "fail 0\n" in baseline
    hashes = {str(p.relative_to(report)): digest(p) for p in report.rglob("*") if p.is_file()}
    if group == "original-reports":
        assert hashes == json.loads((root / "original-report-hashes.json").read_text())
    records[group] = (run, inventory, outcomes)
    runs[group] = {
        "path": str(report.relative_to(root)),
        "status": run["status"],
        "tests": tests,
        "seconds": run["duration_seconds"],
        "summary": outcomes["summary"],
        "score": run["score"],
        "inventory": [{"id": m["id"], **{key: m[key] for key in fields}} for m in inventory["mutations"]],
        "outcomes": [{key: m[key] for key in ["id", "state", "outcome", "error"]} for m in outcomes["mutations"]],
        "sha256": hashes,
    }

before, before_inventory, before_outcomes = records["original-reports"]
after, after_inventory, after_outcomes = records["stronger-reports"]
assert before_outcomes["summary"] == dict(killed=2, missed=11, errors=0, timeout=0, build_failed=0)
assert after_outcomes["summary"] == dict(killed=11, missed=0, errors=0, timeout=0, build_failed=0)
assert before["node_scope"] == after["node_scope"]
missed = {m["id"] for m in before_outcomes["mutations"] if m["outcome"] == "missed"}
assert missed == {1, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13}
assert {identity(m) for m in before_inventory["mutations"] if m["id"] in missed} == {identity(m) for m in after_inventory["mutations"]}
assert before["score"]["killed"] == 2 and before["score"]["missed"] == 11
original_by_id = {m["id"]: m for m in before_inventory["mutations"]}
focused_by_id = {m["id"]: m for m in after_inventory["mutations"]}
assert {m["source_id"] for m in after["rerun"]["mutations"]} == missed
for mapping in after["rerun"]["mutations"]:
    assert identity(original_by_id[mapping["source_id"]]) == identity(focused_by_id[mapping["id"]])
result = {
    "project": "https://github.com/yukiharukonishi/openclaw-cc",
    "commit": record["commit"],
    "tracked_inputs": record["inputs"],
    "stronger_test": {"path": extra, "sha256": digest(retained)},
    "cli_sha256": digest(root / "mutie"),
    "preservation_verified": True,
    "base_commit": "f7e3968",
    "implementation": "working tree with scoped JS/TS string_replace",
    "runtime": "Node 24.22.0",
    "runs": runs,
}
output.write_text(json.dumps(result, indent=2) + "\n")
print(f"Preserved {len(record['inputs'])} tracked files and {len(runs['original-reports']['sha256'])} original report files")
for group, run in runs.items():
    print(group, run["seconds"], run["summary"])
