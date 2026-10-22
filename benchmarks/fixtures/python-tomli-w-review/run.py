"""Run a separate-copy quality review against preserved Tomli-W reports."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import time

parser = argparse.ArgumentParser()
parser.add_argument('--evidence', type=Path, required=True)
parser.add_argument('--project', type=Path, required=True)
parser.add_argument('--output', type=Path, required=True)
parser.add_argument('--python', type=Path, required=True)
args = parser.parse_args()


def hashes(root):
    return {p.relative_to(root).as_posix(): hashlib.sha256(p.read_bytes()).hexdigest()
            for p in sorted(root.rglob('*')) if p.is_file()
            and '.git' not in p.relative_to(root).parts}


def identity(m):
    return tuple(m[k] for k in ('file', 'source_fingerprint', 'operator', 'byte_start',
                               'byte_end', 'original', 'replacement'))


state = json.loads(args.evidence.read_text())
old_evidence_hash = hashlib.sha256(args.evidence.read_bytes()).hexdigest()
original = Path(state['original'])
project = args.project.resolve()
inputs = hashes(project)
changed = {p for p, h in inputs.items() if state['input_sha256'].get(p) != h}
assert changed == {'tests/test_valid.py', 'tests/test_mutie_quality.py'}
assert set(state['input_sha256']) <= set(inputs)
mutie = args.evidence.parent / 'mutie'


def preserve():
    assert hashes(original) == state['input_sha256']
    assert hashes(project) == inputs
    assert hashlib.sha256(args.evidence.read_bytes()).hexdigest() == old_evidence_hash
    assert hashlib.sha256(mutie.read_bytes()).hexdigest() == state['mutie_sha256']
    for record in state['records']:
        assert hashes(Path(record['report'])) == record['report_sha256']


preserve()
args.output.mkdir(parents=True, exist_ok=False)
selected = [1, 3, 4, 7, 10, 24, 25, 29]
command = [str(mutie), str(project), '--lang', 'python', '--python', str(args.python),
           '--rerun', state['records'][0]['report'], '--id', ','.join(map(str, selected)),
           '--continue-on-error', '64', '--timeout', '30', '--baseline-timeout', '60',
           '--ui', 'plain', '--output-dir', str(args.output / 'reports')]
start = time.monotonic()
result = subprocess.run(command, capture_output=True, text=True, timeout=1200)
(args.output / 'stdout.log').write_text(result.stdout)
(args.output / 'stderr.log').write_text(result.stderr)
report = Path(next(line.removeprefix('Saving report to ') for line in result.stdout.splitlines()
                   if line.startswith('Saving report to ')))
run = json.loads((report / 'run.json').read_text())
inventory = json.loads((report / 'mutants.json').read_text())
outcomes = json.loads((report / 'outcomes.json').read_text())
canonical = {identity(m): m['id'] for m in state['canonical']}
local = {m['id']: canonical[identity(m)] for m in inventory['mutations']}
accounting = {str(local[m['id']]): m for m in outcomes['mutations']}
preserve()
record = dict(command=command, seconds=round(time.monotonic()-start, 3),
              exit_code=result.returncode, report=str(report), report_sha256=hashes(report),
              source_evidence=str(args.evidence), source_evidence_sha256=old_evidence_hash,
              input_sha256=inputs, run=run, accounting=accounting, preserved=True)
(args.output / 'results.json').write_text(json.dumps(record, indent=2)+'\n')
assert outcomes['baseline']['outcome'] == 'passed'
assert len(run['python_scope']['test_nodes']) == 275
assert len(run['python_scope']['expected_failure_nodes']) == 2
assert set(accounting) == set(map(str, selected))
expected = {1:'missed', 3:'error', 4:'error', 7:'killed', 10:'killed', 24:'killed', 25:'error', 29:'killed'}
assert all(accounting[str(i)]['state']=='completed' and accounting[str(i)]['outcome']==o
           for i,o in expected.items())
assert result.returncode == 1 and run['status'] == 'failed' and run['score']['percentage'] is None
print(record['seconds'], {i:m['outcome'] for i,m in accounting.items()}, flush=True)
