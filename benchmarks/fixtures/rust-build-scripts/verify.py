"""Verify complete ripgrep copies/reports and export portable acceptance evidence.

Usage: verify.py ROOT OUTPUT_JSON
"""
import hashlib
import json
import os
from pathlib import Path
import re
import sys

root = Path(sys.argv[1]).resolve()
destination = Path(sys.argv[2])


def digest(path):
    data = os.readlink(path).encode() if path.is_symlink() else path.read_bytes()
    return hashlib.sha256(data).hexdigest()


def tree(path):
    return {str(p.relative_to(path)): digest(p) for p in sorted(path.rglob("*"))
            if p.is_symlink() or p.is_file()}


def portable(value):
    if isinstance(value, str):
        return value.replace(str(root), "$ROOT")
    if isinstance(value, dict):
        return {key: portable(item) for key, item in value.items()}
    if isinstance(value, list):
        return [portable(item) for item in value]
    return value


inputs = json.loads((root / "inputs.json").read_text())
assert tree(root / "original") == inputs
stronger = tree(root / "stronger")
assert {name: stronger[name] for name in inputs} == inputs
extra = "crates/cli/tests/command_reader_contracts.rs"
assert set(stronger) - set(inputs) == {extra}
assert stronger[extra] == digest(Path(__file__).with_name("command_reader_contracts.rs"))
reports = {}
original = None
for path in sorted((root / "reports").iterdir()):
    record = json.loads((path / "run.json").read_text())
    assert record["status"] == "completed", path
    outcomes = json.loads((path / "outcomes.json").read_text())
    mutations = json.loads((path / "mutants.json").read_text())["mutations"]
    by_id = {m["id"]: m for m in mutations}
    stdout = (path / outcomes["baseline"]["logs"]["stdout"]).read_text()
    harness = [list(map(int, match)) for match in re.findall(
        r"test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored", stdout)]
    assert harness and sum(row[1] for row in harness) == 0
    data = {"status": record["status"], "seconds": record["duration_seconds"],
            "baseline_seconds": outcomes["baseline"]["duration_seconds"],
            "baseline_harness_totals": list(map(sum, zip(*harness))),
            "baseline_targets": len(harness), "score": record["score"],
            "scope": record["rust_scope"], "config": record["effective_config"],
            "rerun": record.get("rerun"), "report_hashes": tree(path),
            "outcomes": [{"mutation": by_id[o["id"]], "outcome": o["outcome"],
                          "seconds": o["duration_seconds"]} for o in outcomes["mutations"]]}
    reports[path.name] = data
    if not record.get("rerun"):
        assert original is None
        original = data
        assert tree(path) == json.loads((root / "original-report-hashes.json").read_text())
assert original is not None
for record in reports.values():
    assert record["scope"] == original["scope"]
    if record["rerun"] and record["config"]["module_root"].endswith("/original"):
        old = {entry["mutation"]["id"]: entry["outcome"] for entry in original["outcomes"]}
        source_ids = {entry["id"]: entry["source_id"] for entry in record["rerun"]["mutations"]}
        for entry in record["outcomes"]:
            assert entry["outcome"] == old[source_ids[entry["mutation"]["id"]]]
result = {"upstream": "BurntSushi/ripgrep", "revision": "178cc027e283fd965cb72d0a7395b219eada170c",
          "unchanged_inputs": inputs, "stronger_test": {extra: stronger[extra]},
          "commands": {p.stem: json.loads(p.read_text()) for p in sorted((root / "logs").glob("*.json"))},
          "reports": reports}
destination.write_text(json.dumps(portable(result), indent=2) + "\n")
print(f"Verified {len(inputs)} original inputs, one separate test file and {len(reports)} reports")
