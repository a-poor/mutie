#!/usr/bin/env python3
"""Summarize logs from the ignored rust::adapter_probe::profile_cargo_adapter.

Usage: python3 benchmarks/profile-rust-adapter.py /absolute/probe/root
Reads retained probe captures only; does not execute Cargo or modify reports.
"""
import argparse
import json
import re
import statistics
from pathlib import Path


def cargo_seconds(text):
    parts = re.findall(r"([0-9.]+)(m|s)", text)
    if not parts:
        raise ValueError(f"Unrecognized Cargo duration: {text!r}")
    return sum(float(number) * (60 if unit == "m" else 1) for number, unit in parts)


def read_sample(path):
    result = json.loads(path.read_text())
    if not result["success"] or any(result[key] for key in ("timed_out", "interrupted", "capture_error")):
        raise ValueError(f"Failed execution cannot be a clean timing sample: {path}")
    match = re.fullmatch(r"jobs-(\d+)-cached-true-round-(\d+)-worker-(\d+)", result["label"])
    if match is None:
        raise ValueError(f"Unexpected profile label: {result['label']}")
    jobs, round_number, worker = map(int, match.groups())
    phases = result["phases"]
    if [event["phase"] for event in phases] != ["Building", "Testing"]:
        raise ValueError(f"Incomplete phases: {path}")
    stdout = path.with_suffix(".stdout.log").read_text()
    stderr = path.with_suffix(".stderr.log").read_text()
    targets = re.findall(r"^\s*Running (?:unittests )?(\S+\.rs) \(", stderr, re.M)
    summaries = re.findall(
        r"^test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored;.*finished in ([0-9.]+)s", stdout, re.M
    )
    if not targets or len(targets) != len(summaries) or len(set(targets)) != len(targets):
        raise ValueError(f"Cannot pair Cargo targets and harness summaries: {path}")
    if any(int(summary[1]) for summary in summaries):
        raise ValueError(f"Failed harness: {path}")
    finished = re.findall(r"Finished `test` profile .*? in ([^\n]+)", stderr)
    if len(finished) != 2:
        raise ValueError(f"Expected compilation and execution Cargo summaries: {path}")
    artifacts = []
    for line in stdout.splitlines():
        try:
            event = json.loads(line)
        except ValueError:
            continue
        if not isinstance(event, dict):
            continue
        if event.get("reason") == "build-finished":
            break
        if event.get("reason") == "compiler-artifact":
            artifacts.append(event)
    local = [event for event in artifacts if event["package_id"].startswith("path+")]
    dependencies = [event for event in artifacts if not event["package_id"].startswith("path+")]
    if not local or any(event["fresh"] for event in local):
        raise ValueError(f"Expected local package targets to rebuild: {path}")
    target_seconds = {target: float(summary[3]) for target, summary in zip(targets, summaries)}
    harness = sum(target_seconds.values())
    testing = result["observed_seconds"] - phases[1]["seconds"]
    return {
        "label": result["label"], "jobs": jobs, "round": round_number, "worker": worker,
        "cache": "cold" if round_number == 0 else "warm",
        "observed_seconds": result["observed_seconds"],
        "build_and_cleanup_seconds": phases[1]["seconds"] - phases[0]["seconds"],
        "cargo_compilation_seconds": cargo_seconds(finished[0]),
        "cargo_test_preparation_seconds": cargo_seconds(finished[1]),
        "testing_and_finalization_seconds": testing,
        "harness_seconds": harness,
        # This is NOT solely Cargo startup: it includes executable loading,
        # scheduling gaps, shutdown and capture/report preparation as well.
        "outside_harness_seconds": testing - harness,
        "passed": sum(int(summary[0]) for summary in summaries),
        "ignored": sum(int(summary[2]) for summary in summaries),
        "targets": target_seconds,
        "local_targets_rebuilt": len(local),
        "dependency_records": len(dependencies),
        "dependency_records_reused": sum(event["fresh"] for event in dependencies),
    }


def summarize(root):
    paths = sorted((root / "logs").glob("jobs-*-cached-true-*.json"))
    if not paths:
        raise ValueError("No profiling results found")
    samples = [read_sample(path) for path in paths]
    groups = []
    metrics = ("observed_seconds", "build_and_cleanup_seconds", "cargo_compilation_seconds",
               "testing_and_finalization_seconds", "harness_seconds", "outside_harness_seconds")
    for jobs, cache in sorted({(s["jobs"], s["cache"]) for s in samples}):
        rows = [s for s in samples if (s["jobs"], s["cache"]) == (jobs, cache)]
        groups.append({
            "jobs": jobs, "cache": cache, "samples": len(rows),
            "seconds": {key: {"min": min(s[key] for s in rows),
                               "median": statistics.median(s[key] for s in rows),
                               "max": max(s[key] for s in rows)} for key in metrics},
            "target_medians": dict(sorted(
                ((name, statistics.median(s["targets"][name] for s in rows)) for name in rows[0]["targets"]),
                key=lambda item: item[1], reverse=True)),
        })
    return {"samples": samples, "groups": groups}


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("root", type=Path)
    args = parser.parse_args()
    print(json.dumps(summarize(args.root), indent=2))
