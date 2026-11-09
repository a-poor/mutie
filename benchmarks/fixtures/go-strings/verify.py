"""Verify preserved Go string inputs, full/affected parity and stronger reruns."""
import hashlib
import json
from pathlib import Path
import subprocess
import sys

root = Path(sys.argv[1]).resolve()
output = Path(sys.argv[2])
repo = Path(__file__).resolve().parents[3]
inputs = json.loads((root / "original-inputs.json").read_text())
original, stronger = root / "original", root / "stronger"
commit = "3fc4292b58a67b78e1dbb6e47b4879a6cc602ec4"


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


assert not subprocess.check_output(["git", "-C", str(original), "status", "--porcelain"])
assert subprocess.check_output(["git", "-C", str(original), "rev-parse", "HEAD"]).decode().strip() == commit
assert len(inputs) == 20
for name, sha in inputs.items():
    assert digest(original / name) == digest(stronger / name) == sha, name
extra = "string_contracts_test.go"
assert {str(p.relative_to(stronger)) for p in stronger.rglob("*") if p.is_file()} == set(inputs) | {extra}
retained = repo / "benchmarks/fixtures/go-strings/contracts_test.go"
assert digest(retained) == digest(stronger / extra)
fields = ["file", "operator", "source_fingerprint", "byte_start", "byte_end", "original", "replacement", "line", "column", "end_line", "end_column"]


def identity(mutation):
    return tuple(mutation[key] for key in fields)


runs, records = {}, {}
for group in ["original", "affected", "stronger", "stronger-affected"]:
    report, = (root / f"{group}-reports").iterdir()
    run = json.loads((report / "run.json").read_text())
    outcomes = json.loads((report / "outcomes.json").read_text())
    inventory = json.loads((report / "mutants.json").read_text())
    assert run["status"] == outcomes["status"] == "completed"
    assert outcomes["baseline"]["outcome"] == "passed"
    events = [json.loads(line) for line in (report / outcomes["baseline"]["logs"]["stdout"]).read_text().splitlines()]
    tests = sorted((e["Package"], e["Test"]) for e in events if e.get("Action") == "pass" and e.get("Test"))
    assert tests and not any(e.get("Action") == "fail" for e in events)
    hashes = {str(p.relative_to(report)): digest(p) for p in report.rglob("*") if p.is_file()}
    if group in {"original", "affected"}:
        assert hashes == json.loads((root / f"{group}-report-hashes.json").read_text()), group
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
original_by_id = {m["id"]: m for m in inventory["mutations"]}
assert len(tests) == 43
assert outcomes["summary"] == dict(killed=16, missed=19, timeout=0, build_failed=4, errors=0)
missed = {m["id"] for m in outcomes["mutations"] if m["outcome"] == "missed"}
assert missed == set(range(1, 12)) | {21, 22, 24, 25, 26, 28, 38, 39}
assert abs(before["score"]["percentage"] - 100 * 16 / 35) < 1e-12
for left, right in [("original", "affected"), ("stronger", "stronger-affected")]:
    a, ai, ao, at = records[left]
    b, bi, bo, bt = records[right]
    assert ao["summary"] == bo["summary"]
    assert [(m["id"], identity(m)) for m in ai["mutations"]] == [(m["id"], identity(m)) for m in bi["mutations"]]
    assert {m["id"]: m["outcome"] for m in ao["mutations"]} == {m["id"]: m["outcome"] for m in bo["mutations"]}
    assert a["score"] == b["score"]
    assert at == bt
    assert a["effective_config"]["test_scope"] == "full"
    assert b["effective_config"]["test_scope"] == "affected"
for group in ["stronger", "stronger-affected"]:
    run, selected, results, stronger_tests = records[group]
    assert set(tests) < set(stronger_tests) and len(stronger_tests) == 47
    assert results["summary"] == dict(killed=4, missed=15, timeout=0, build_failed=0, errors=0)
    assert {identity(m) for m in inventory["mutations"] if m["id"] in missed} == {identity(m) for m in selected["mutations"]}
    assert {m["source_id"] for m in run["rerun"]["mutations"]} == missed
    by_id = {m["id"]: m for m in selected["mutations"]}
    outcomes_by_id = {m["id"]: m["outcome"] for m in results["mutations"]}
    killed = []
    for mapping in run["rerun"]["mutations"]:
        assert identity(original_by_id[mapping["source_id"]]) == identity(by_id[mapping["id"]])
        if outcomes_by_id[mapping["id"]] == "killed":
            killed.append(mapping["source_id"])
    assert sorted(killed) == [11, 21, 26, 39]
    runs[group]["killed_source_ids"] = sorted(killed)

sandbox_report, = (root / "sandbox-attempt-reports").iterdir()
sandbox_run = json.loads((sandbox_report / "run.json").read_text())
sandbox_outcomes = json.loads((sandbox_report / "outcomes.json").read_text())
assert sandbox_run["status"] == "failed"
assert sandbox_outcomes["baseline"]["outcome"] == "failed"
assert sandbox_outcomes["mutations"] == []
assert sandbox_run["score"]["percentage"] is None
result = {
    "project": "https://github.com/joho/godotenv",
    "tag": "v1.5.1", "commit": commit,
    "toolchain": "go1.27.2 darwin/arm64",
    "base_commit": "5b15b16",
    "implementation": "working tree with scoped Go string_replace",
    "cli_sha256": digest(root / "mutie"),
    "tracked_inputs": inputs,
    "stronger_test": {"path": extra, "sha256": digest(retained)},
    "preservation_verified": True,
    "stronger_killed_original_ids": [11, 21, 26, 39],
    "remaining_original_ids": sorted(missed - {11, 21, 26, 39}),
    "sandbox_attempt": {
        "status": sandbox_run["status"], "attempted": 0,
        "score": sandbox_run["score"],
        "reason": "Host Go build-cache access denied; retried with approved access",
        "sha256": {str(p.relative_to(sandbox_report)): digest(p) for p in sandbox_report.rglob("*") if p.is_file()},
    },
    "runs": runs,
}
output.write_text(json.dumps(result, indent=2) + "\n")
print(f"Preserved {len(inputs)} tracked inputs and {len(runs['original']['sha256'])} original report files")
for group, run in runs.items():
    print(group, run["tests"], run["seconds"], run["summary"], len(run["sha256"]))
