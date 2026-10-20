#!/usr/bin/env python3
"""Serial CLI build-mode comparison; build binaries before running this script."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import time

parser = argparse.ArgumentParser()
parser.add_argument('--project', type=Path, required=True)
parser.add_argument('--debug', type=Path, required=True)
parser.add_argument('--release', type=Path, required=True)
parser.add_argument('--output', type=Path, required=True)
parser.add_argument('--focused-only', action='store_true')
parser.add_argument('--modes', nargs='+', choices=['debug', 'release'], default=['debug', 'release'])
args = parser.parse_args()
project = args.project.resolve()
output = args.output.resolve()
output.mkdir(parents=True, exist_ok=False)


def inputs():
    return {str(p.relative_to(project)): hashlib.sha256(p.read_bytes()).hexdigest()
            for p in project.rglob('*') if p.is_file() and '.git' not in p.parts}


before = inputs()
records = []
# Compare the whole inventory once, then repeat one useful candidate with a
# fresh baseline each time. Alternate focused order to reduce ordering bias.
workloads = [] if args.focused_only else [('full', 1, mode) for mode in args.modes]
for repeat in range(1, 4):
    modes = list(reversed(args.modes)) if repeat % 2 else args.modes
    workloads.extend(('focused', repeat, mode) for mode in modes)
for workload, repeat, mode in workloads:
    binary = getattr(args, mode).resolve()
    command = [str(binary), str(project), '--lang', 'bun', '--timeout', '5',
               '--baseline-timeout', '15', '--ui', 'plain', '--output-dir', str(output / 'reports')]
    if workload == 'focused':
        command.extend(['--include', 'src/main/isAlphanumeric.js'])
    start = time.perf_counter()
    result = subprocess.run(command, capture_output=True, text=True, timeout=900)
    elapsed = time.perf_counter() - start
    (output / f'{workload}-{repeat}-{mode}.log').write_text(result.stdout + result.stderr)
    if result.returncode != 0:
        raise RuntimeError(f'{mode} {workload} failed: {result.stderr}')
    report = Path(next(line.removeprefix('Saving report to ') for line in result.stdout.splitlines()
                       if line.startswith('Saving report to ')))
    run = json.loads((report / 'run.json').read_text())
    outcomes = json.loads((report / 'outcomes.json').read_text())
    inventory = json.loads((report / 'mutants.json').read_text())
    assert run['status'] == 'completed'
    assert outcomes['summary']['errors'] == outcomes['summary']['timeout'] == 0
    assert inputs() == before
    record = dict(workload=workload, repeat=repeat, mode=mode, wall_seconds=elapsed,
                  command=command, report=str(report), run=run, outcomes=outcomes, inventory=inventory)
    previous = next((r for r in records if r['workload'] == workload), None)
    if previous:
        assert previous['inventory']['mutations'] == inventory['mutations']
        assert previous['run']['bun_scope'] == run['bun_scope']
        assert [(m['id'], m['outcome']) for m in previous['outcomes']['mutations']] == [
            (m['id'], m['outcome']) for m in outcomes['mutations']]
    records.append(record)
    (output / 'results.json').write_text(json.dumps(dict(input_sha256=before, records=records), indent=2) + '\n')
    print(f'{workload} {repeat} {mode}: {elapsed:.3f}s; {outcomes["summary"]}', flush=True)
