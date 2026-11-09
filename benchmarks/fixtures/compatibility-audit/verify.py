"""Verify audit preservation and export a machine-path-free evidence ledger.

Usage: verify.py AUDIT_ROOT OUTPUT_JSON
"""
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys

root = Path(sys.argv[1]).resolve()
destination = Path(sys.argv[2])
repo = Path(__file__).resolve().parents[3]


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def sanitize(value):
    if isinstance(value, dict):
        return {k: sanitize(v) for k, v in value.items()}
    if isinstance(value, list):
        return [sanitize(v) for v in value]
    if not isinstance(value, str):
        return value
    value = value.replace(str(root), "<AUDIT_ROOT>")
    value = value.replace(str(Path.home()), "<HOME>")
    value = re.sub(r"/(?:private/)?var/folders/[^\s\"']+", "<TEMP_PATH>", value)
    value = re.sub(r"/private/tmp/mutie-python-calls-env[^\s\"']*", "<AUDITED_PYTHON>", value)
    value = re.sub(r"/opt/homebrew/[^\s\"']+", "<PYTHON_INSTALLATION>", value)
    return value


def check_inputs(directory, manifest):
    for name, record in manifest.items():
        path = directory / name
        if record["mode"] == "120000":
            assert path.is_symlink(), path
            actual = hashlib.sha256(os.readlink(path).encode()).hexdigest()
        else:
            assert path.is_file() and not path.is_symlink(), path
            actual = digest(path)
            assert bool(path.stat().st_mode & 0o111) == (record["mode"] == "100755"), path
        assert actual == record["sha256"], path


def result(directory, name):
    path = directory / "logs" / f"{name}.json"
    record = json.loads(path.read_text())
    assert not record["timed_out"], path
    return record


projects = {}
for name in ["sjson", "ripgrep", "click", "ufo", "eslint-node-test", "elysia"]:
    directory = root / name
    source = json.loads((directory / "inputs.json").read_text())
    original = directory / "original"
    assert subprocess.check_output(["git", "-C", str(original), "rev-parse", "HEAD"]).decode().strip() == source["commit"]
    assert not subprocess.check_output(["git", "-C", str(original), "status", "--porcelain"])
    check_inputs(original, source["inputs"])
    check_inputs(directory / "onboarding", source["inputs"])
    observed = {str(p.relative_to(directory / "onboarding")) for p in (directory / "onboarding").rglob("*") if p.is_file() or p.is_symlink()}
    expected = set(source["inputs"])
    if name != "ufo":
        expected.add("mutie.toml")
        assert digest(directory / "onboarding/mutie.toml") == digest(directory / "generated.toml")
    assert observed == expected, name
    stages = {}
    for path in sorted((directory / "logs").glob("*.json")):
        record = result(directory, path.stem)
        text = (directory / record["stderr"]).read_text()
        record["stderr_tail"] = "\n".join(text.splitlines()[-4:])
        record["sha256"] = {key: digest(directory / record[key]) for key in ["stdout", "stderr"]}
        stages[path.stem] = record
    projects[name] = {
        **source,
        "upstream_url": f'https://github.com/{source["project"]}/tree/{source["commit"]}',
        "input_count": len(source["inputs"]),
        "preserved_original_and_onboarding": True,
        "config": (directory / "generated.toml").read_text() if name != "ufo" else None,
        "stages": stages,
    }

assert sum(p["input_count"] for p in projects.values()) == 1239
for name in ["sjson", "ripgrep", "click"]:
    assert projects[name]["stages"]["init-auto"]["exit"] == 0
for name in ["ufo", "eslint-node-test", "elysia"]:
    assert projects[name]["stages"]["init-auto"]["exit"] == 1
for name in ["eslint-node-test", "elysia"]:
    assert projects[name]["stages"]["init-explicit"]["exit"] == 0
for name in ["ripgrep", "click", "eslint-node-test", "elysia"]:
    assert projects[name]["stages"]["check"]["exit"] == 1
assert projects["ufo"]["stages"]["check-explicit-config"]["exit"] == 1
assert projects["sjson"]["stages"]["check-host"]["exit"] == 0

for name in ["ripgrep", "click"]:
    check_inputs(root / name / "native", projects[name]["inputs"])
for stage in ["native-rust", "native-rust-git"]:
    assert projects["ripgrep"]["stages"][stage]["exit"] == 0
    text = (root / "ripgrep/logs" / f"{stage}.stdout").read_text()
    totals = re.findall(r"test result: (\w+)\. (\d+) passed; (\d+) failed; (\d+) ignored", text)
    assert len(totals) == 14 and all(r[0] == "ok" for r in totals)
    counts = [sum(int(r[i]) for r in totals) for i in [1, 2, 3]]
    assert counts == [1209, 0, 1]
    projects["ripgrep"]["stages"][stage]["tests"] = dict(zip(["passed", "failed", "ignored"], counts))
assert subprocess.check_output(["git", "-C", str(root / "ripgrep/native"), "rev-parse", "HEAD"]).decode().strip() == projects["ripgrep"]["commit"]
for target, expected in [("native-target", "output from `git rev-parse` is empty"),
                         ("native-git-target", "cargo:rustc-env=RIPGREP_BUILD_GIT_HASH=178cc027e2")]:
    outputs = list((root / "ripgrep" / target / "debug/build").glob("ripgrep-*/output"))
    assert any(expected in path.read_text() for path in outputs), target
projects["ripgrep"]["build_revision_control"] = {"without_git": "explicit missing-revision warning", "with_git": "178cc027e2"}
assert projects["click"]["stages"]["native-python"]["exit"] == 2
assert projects["click"]["stages"]["native-python-installed"]["exit"] == 0
text = (root / "click/logs/native-python-installed.stdout").read_text()
assert "2622 passed, 25 skipped, 31000 deselected, 1 xfailed" in text
projects["click"]["stages"]["native-python-installed"]["tests"] = {
    "passed": 2622, "skipped": 25, "deselected": 31000, "xfailed": 1,
}

directory = root / "sjson"
listing = json.loads((directory / "logs/list-all-host.stdout").read_text())
assert len(listing["mutations"]) == 646
check_inputs(directory / "stronger", projects["sjson"]["inputs"])
retained = repo / "benchmarks/fixtures/compatibility-audit/sjson_contracts_test.go"
assert digest(retained) == digest(directory / "stronger/compatibility_test.go")
assert {str(p.relative_to(directory / "stronger")) for p in (directory / "stronger").rglob("*") if p.is_file()} == set(projects["sjson"]["inputs"]) | {"compatibility_test.go"}
fields = ["file", "operator", "source_fingerprint", "byte_start", "byte_end", "original", "replacement"]


def identity(m):
    return tuple(m[f] for f in fields)


runs, raw = {}, {}
for name, folder in [("original", "reports"), ("stronger", "stronger-reports")]:
    report, = (directory / folder).iterdir()
    run = json.loads((report / "run.json").read_text())
    inventory = json.loads((report / "mutants.json").read_text())
    outcomes = json.loads((report / "outcomes.json").read_text())
    assert run["status"] == outcomes["status"] == "completed"
    assert outcomes["baseline"]["outcome"] == "passed"
    events = [json.loads(line) for line in (report / outcomes["baseline"]["logs"]["stdout"]).read_text().splitlines()]
    tests = sorted(e["Test"] for e in events if e.get("Action") == "pass" and "Test" in e)
    hashes = {str(p.relative_to(report)): digest(p) for p in report.rglob("*") if p.is_file()}
    if name == "original":
        assert hashes == json.loads((directory / "report-hashes.json").read_text())
    raw[name] = run, inventory, outcomes, tests
    runs[name] = {
        "path": str(report.relative_to(root)), "seconds": run["duration_seconds"],
        "tests": tests, "score": run["score"], "summary": outcomes["summary"],
        "inventory": [{"id": m["id"], **{f: m[f] for f in fields}} for m in inventory["mutations"]],
        "outcomes": [{f: m[f] for f in ["id", "outcome", "state", "error"]} for m in outcomes["mutations"]],
        "sha256": hashes,
    }
before, inventory, outcomes, before_tests = raw["original"]
after, selected, focused, after_tests = raw["stronger"]
assert len(before_tests) == 14 and len(after_tests) == 19 and set(before_tests) < set(after_tests)
assert outcomes["summary"] == dict(killed=23, missed=11, timeout=0, build_failed=2, errors=0)
assert focused["summary"] == dict(killed=5, missed=6, timeout=0, build_failed=0, errors=0)
by_id = {m["id"]: m for m in inventory["mutations"]}
selected_by_id = {m["id"]: m for m in selected["mutations"]}
focused_by_id = {m["id"]: m["outcome"] for m in focused["mutations"]}
missed = {m["id"] for m in outcomes["mutations"] if m["outcome"] == "missed"}
assert {m["source_id"] for m in after["rerun"]["mutations"]} == missed
killed = []
for mapping in after["rerun"]["mutations"]:
    assert identity(by_id[mapping["source_id"]]) == identity(selected_by_id[mapping["id"]])
    if focused_by_id[mapping["id"]] == "killed":
        killed.append(mapping["source_id"])
assert sorted(killed) == [16, 27, 28, 31, 36]
assert len(inventory["mutations"]) == 36
assert {identity(m) for m in inventory["mutations"]} == {
    identity(m) for m in listing["mutations"] if m["operator"] in {"string_replace", "negation_remove"}
}
projects["sjson"]["workflow"] = {
    "all_operator_candidates": 646, "selected_candidates": 36,
    "selected_operators": ["string_replace", "negation_remove"],
    "stronger_test_sha256": digest(retained),
    "killed_source_ids": sorted(killed), "remaining_source_ids": sorted(missed - set(killed)),
    "runs": runs,
}
record = sanitize({
    "base_commit": "53968c8", "date": "2026-11-09", "platform": "macOS arm64",
    "build_profile": "release", "cli_sha256": digest(root / "mutie"),
    "toolchains": {"go": "1.27.2", "rust": "1.99.1", "node": "24.22.0", "bun": "1.3.11+af24e281e", "python": "3.14.7", "pytest": "9.0.2"},
    "projects": projects,
    "upstream_files_preserved": 1239,
    "unchanged_projects_admitted": 1,
    "selection_limit": "Six purposively chosen projects, not a population estimate or full JS/TS syntax matrix",
    "recommended_next_goal": "Cargo build-script support and inactive custom release-profile admission, using unchanged ripgrep",
})
destination.write_text(json.dumps(record, indent=2) + "\n")
print("Verified 1239 original inputs, onboarding copies, native copies and SJSON stronger copy")
print("SJSON reports:", {name: len(r["sha256"]) for name, r in runs.items()})
print("SJSON source IDs killed:", killed, "remaining:", sorted(missed - set(killed)))
