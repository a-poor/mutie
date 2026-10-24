"""Verify local preserved acceptance inputs and reports; emit compact evidence."""
import hashlib
import json
from pathlib import Path
import subprocess
import sys

root = Path(sys.argv[1])
repo = Path(__file__).resolve().parents[3]
record = json.loads((root / 'preservation.json').read_text())
project = Path(record['project'])


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


assert not subprocess.check_output(['git', '-C', str(project), 'status', '--porcelain'])
assert subprocess.check_output(['git', '-C', str(project), 'rev-parse', 'HEAD']).decode().strip() == record['commit']
for name, sha in record['inputs'].items():
    assert digest(project / name) == sha, name
    assert digest(root / 'stronger' / name) == sha, name
for path, sha in record['prior_reports'].items():
    assert digest(Path(path)) == sha, path
for path, sha in json.loads((root / 'original-report-hashes.json').read_text()).items():
    assert digest(Path(path)) == sha, path
extra = 'tests/test_delete_balance.py'
assert digest(root / 'stronger' / extra) == digest(Path(__file__).parent / 'test_delete_balance.py')
assert {str(p.relative_to(root / 'stronger')) for p in (root / 'stronger').rglob('*') if p.is_file()} == set(record['inputs']) | {extra}
recipe_paths = ['src/python.rs', 'src/python/identity.rs', 'src/python/scope.py',
                'src/python/runtime.py', 'src/python/driver.py', 'src/python/protocol.rs',
                'src/python/project.rs', 'src/process.rs', 'src/capture.rs',
                'src/workspace.rs', 'src/command.rs', 'Cargo.lock']
recipe = 'sha256:' + hashlib.sha256(b''.join((repo / p).read_bytes() for p in recipe_paths)).hexdigest()
runs = {}
for group in ['final-original', 'final-survivor', 'stronger-reports']:
    path, = (root / group).iterdir()
    run = json.loads((path / 'run.json').read_text())
    outcomes = json.loads((path / 'outcomes.json').read_text())
    inventory = json.loads((path / 'mutants.json').read_text())
    assert run['python_scope']['recipe_fingerprint'] == recipe
    runs[group] = {'path': str(path), 'status': run['status'], 'score': run['score'],
                   'test_count': len(run['python_scope']['test_nodes']),
                   'duration_seconds': run['duration_seconds'], 'summary': outcomes['summary'],
                   'inventory': inventory['mutations'], 'outcomes': outcomes['mutations'],
                   'sha256': {str(p.relative_to(path)): digest(p) for p in path.rglob('*') if p.is_file()}}
original, survivor, stronger = (runs[k] for k in ['final-original', 'final-survivor', 'stronger-reports'])
assert original['status'] == 'failed' and original['score']['percentage'] is None
assert original['test_count'] == survivor['test_count'] == 7
assert stronger['test_count'] == 8
assert survivor['summary'] == {'killed': 0, 'missed': 1, 'errors': 0, 'timeout': 0, 'build_failed': 0}
assert stronger['summary'] == {'killed': 1, 'missed': 0, 'errors': 0, 'timeout': 0, 'build_failed': 0}
site, = [m for m in original['inventory'] if m['id'] == 5]
assert site['operator'] == 'loop_skip' and site['line'] == 346
fields = ['file', 'line', 'column', 'byte_start', 'byte_end', 'source_fingerprint', 'original', 'replacement', 'operator']
for run in [survivor, stronger]:
    other, = run['inventory']
    assert all(site[f] == other[f] for f in fields)
preliminary = []
for group in ['original-reports', 'original-focused']:
    for path in sorted((root / group).iterdir()):
        run = json.loads((path / 'run.json').read_text())
        outcomes = json.loads((path / 'outcomes.json').read_text())
        inventory = json.loads((path / 'mutants.json').read_text())
        preliminary.append({'path': str(path), 'recipe': run['python_scope']['recipe_fingerprint'],
                            'summary': outcomes['summary'], 'inventory': inventory['mutations'],
                            'outcomes': outcomes['mutations']})
Path(sys.argv[2]).write_text(json.dumps({'project': record, 'runs': runs,
    'preliminary_inventory_review': preliminary,
    'stronger_test_sha256': digest(root / 'stronger' / extra)}, indent=2) + '\n')
print('Verified', len(record['inputs']), 'inputs,', len(record['prior_reports']), 'historical outcome files; original survivor and stronger kill.')
