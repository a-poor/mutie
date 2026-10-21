"""Validate unchanged Cycler comparison boundaries and preserve prior evidence."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import time

parser = argparse.ArgumentParser()
for name in ('project', 'mutie', 'python', 'output'):
    parser.add_argument('--' + name, type=Path, required=True)
args = parser.parse_args()
root = args.project.resolve()
output = args.output.resolve()
output.mkdir(parents=True, exist_ok=False)
commit = subprocess.check_output(['git', '-C', str(root), 'rev-parse', 'HEAD'], text=True).strip()
assert commit == 'd1737eb0a42bd451e8770cef245ebe60ebb36bfc'


def hashes(directory):
    return {p.relative_to(directory).as_posix(): hashlib.sha256(p.read_bytes()).hexdigest()
            for p in sorted(directory.rglob('*')) if p.is_file()
            and '.git' not in p.relative_to(directory).parts}


def clean():
    assert not subprocess.check_output(['git', '-C', str(root), 'status', '--porcelain'], text=True).strip()


clean()
original = hashes(root)
previous = json.loads(Path(__file__).with_name('..').joinpath('python-cycler-unchanged/results.json').read_text())
assert original == previous['input_sha256']
prior_reports = {Path(r['report']): r['report_sha256'] for r in previous['records'] if 'report' in r}
binary_hash = hashlib.sha256(args.mutie.read_bytes()).hexdigest()
records = []
reports = {}


def command(label, argv, expected=0, **kwargs):
    start = time.perf_counter()
    result = subprocess.run(list(map(str, argv)), capture_output=True, text=True,
                            timeout=360, **kwargs)
    seconds = time.perf_counter() - start
    (output / (label + '.stdout')).write_text(result.stdout)
    (output / (label + '.stderr')).write_text(result.stderr)
    assert result.returncode == expected, result.stdout + result.stderr
    assert hashes(root) == original
    for report, before in (prior_reports | reports).items():
        assert hashes(report) == before
    assert hashlib.sha256(args.mutie.read_bytes()).hexdigest() == binary_hash
    clean()
    record = dict(label=label, command=list(map(str, argv)), seconds=seconds,
                  exit_code=result.returncode)
    records.append(record)
    print(label, round(seconds, 3), flush=True)
    return result, record


mutie = args.mutie.absolute()
python = args.python.absolute()  # Preserve the venv launcher symlink.
result, record = command('discovery', [mutie, 'python-list', root, '--python', python, '--json'])
record['scope'] = json.loads(result.stdout)['scope']
result, record = command('comparison-full', [mutie, root, '--lang', 'python', '--python', python,
    '--operator', 'comparison_boundary', '--timeout', '20', '--baseline-timeout', '30',
    '--ui', 'plain', '--output-dir', output / 'reports'])
report = Path(next(line.removeprefix('Saving report to ') for line in result.stdout.splitlines()
                   if line.startswith('Saving report to ')))
run = json.loads((report / 'run.json').read_text())
outcomes = json.loads((report / 'outcomes.json').read_text())
inventory = json.loads((report / 'mutants.json').read_text())
record.update(report=str(report), report_sha256=hashes(report), run=run, outcomes=outcomes, inventory=inventory)
state = dict(commit=commit, input_sha256=original, mutie_sha256=binary_hash, records=records,
             original_inputs_preserved=True, prior_reports_preserved=True)
(output / 'results.json').write_text(json.dumps(state, indent=2) + '\n')
assert run['status'] == 'completed'
assert len(run['python_scope']['test_nodes']) == 28
assert outcomes['summary'] == dict(killed=0, missed=1, errors=0, timeout=0, build_failed=0)
assert len(inventory['mutations']) == 1
assert inventory['mutations'][0]['original'] == '>'
assert inventory['mutations'][0]['replacement'] == '>='
