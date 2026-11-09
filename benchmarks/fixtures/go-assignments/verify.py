"""Verify the unchanged Go campaign and its separate-copy stronger-test rerun."""
import hashlib
import json
from pathlib import Path
import subprocess
import sys

root = Path(sys.argv[1]).resolve()
output = Path(sys.argv[2])
repo = Path(__file__).resolve().parents[3]
inputs = json.loads((root / "preservation.json").read_text())
original, stronger = root / "original", root / "stronger"
commit = "3fc4292b58a67b78e1dbb6e47b4879a6cc602ec4"


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


assert not subprocess.check_output(["git", "-C", str(original), "status", "--porcelain"])
assert subprocess.check_output(["git", "-C", str(original), "rev-parse", "HEAD"]).decode().strip() == commit
for name, sha in inputs.items():
    assert digest(original / name) == digest(stronger / name) == sha, name
extra = "assignment_streams_test.go"
assert {str(p.relative_to(stronger)) for p in stronger.rglob("*") if p.is_file()} == set(inputs) | {extra}
retained = repo / "benchmarks/fixtures/go-assignments/streams_test.go"
assert digest(retained) == digest(stronger / extra)

fields = ["file", "operator", "source_fingerprint", "byte_start", "byte_end", "original", "replacement", "line", "column", "end_line", "end_column"]


def identity(mutation):
    return tuple(mutation[key] for key in fields)


runs, records = {}, {}
for group in ["original", "affected", "stronger"]:
    reports = list((root / f"{group}-reports").iterdir())
    assert len(reports) == 1
    report = reports[0]
    run = json.loads((report / "run.json").read_text())
    outcomes = json.loads((report / "outcomes.json").read_text())
    inventory = json.loads((report / "mutants.json").read_text())
    assert run["status"] == outcomes["status"] == "completed"
    assert outcomes["baseline"]["outcome"] == "passed"
    events = [json.loads(line) for line in (report / outcomes["baseline"]["logs"]["stdout"]).read_text().splitlines()]
    tests = sorted((e["Package"], e["Test"]) for e in events if e.get("Action") == "pass" and e.get("Test"))
    assert tests and not any(e.get("Action") == "fail" for e in events)
    hashes = {str(p.relative_to(report)): digest(p) for p in report.rglob("*") if p.is_file()}
    if group == "original":
        assert hashes == json.loads((root / "original-report-hashes.json").read_text())
    records[group] = (run, inventory, outcomes, tests)
    runs[group] = {
        "path": str(report.relative_to(root)),
        "status": run["status"],
        "tests": len(tests),
        "seconds": run["duration_seconds"],
        "test_scope": run["effective_config"]["test_scope"],
        "summary": outcomes["summary"],
        "score": run["score"],
        "inventory": [{"id": m["id"], **{key: m[key] for key in fields}} for m in inventory["mutations"]],
        "outcomes": [{key: m[key] for key in ["id", "state", "outcome", "error"]} for m in outcomes["mutations"]],
        "sha256": hashes,
    }

before, inventory, outcomes, tests = records["original"]
affected, affected_inventory, affected_outcomes, affected_tests = records["affected"]
after, selected, focused_outcomes, stronger_tests = records["stronger"]
assert outcomes["summary"] == affected_outcomes["summary"] == dict(killed=0, missed=3, timeout=0, build_failed=3, errors=0)
assert [(m["id"], identity(m)) for m in inventory["mutations"]] == [(m["id"], identity(m)) for m in affected_inventory["mutations"]]
assert {m["id"]: m["outcome"] for m in outcomes["mutations"]} == {m["id"]: m["outcome"] for m in affected_outcomes["mutations"]}
assert tests == affected_tests
assert set(tests) < set(stronger_tests) and len(stronger_tests) == len(tests) + 1
assert before["score"]["percentage"] == affected["score"]["percentage"] == 0
assert focused_outcomes["summary"] == dict(killed=3, missed=0, timeout=0, build_failed=0, errors=0)
missed = {m["id"] for m in outcomes["mutations"] if m["outcome"] == "missed"}
assert missed == {2, 3, 4}
assert {identity(m) for m in inventory["mutations"] if m["id"] in missed} == {identity(m) for m in selected["mutations"]}
assert {m["source_id"] for m in after["rerun"]["mutations"]} == missed
result = {
    "project": "https://github.com/joho/godotenv",
    "tag": "v1.5.1", "commit": commit,
    "toolchain": "go1.27.2 darwin/arm64",
    "base_commit": "1316dfc039113b7ab2a4dffe17924063f91f8675",
    "implementation": "working tree with scoped Go assignment_remove",
    "cli_sha256": digest(root / "mutie"),
    "tracked_inputs": inputs,
    "stronger_test": {"path": extra, "sha256": digest(retained)},
    "preservation_verified": True,
    "runs": runs,
}
output.write_text(json.dumps(result, indent=2) + "\n")
print(f"Preserved {len(inputs)} tracked inputs and {len(runs['original']['sha256'])} original report files")
for group, run in runs.items():
    print(group, run["tests"], run["seconds"], run["summary"])
