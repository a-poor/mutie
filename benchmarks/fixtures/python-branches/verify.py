"""Verify the preserved local acceptance ledger and export a compact result record."""
import hashlib
import json
from pathlib import Path
import subprocess
import sys

root = Path(sys.argv[1])
output = Path(sys.argv[2])
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
retained = repo / 'benchmarks/fixtures/python-branches/test_trial_branches.py'
assert digest(retained) == digest(root / 'stronger/tests/test_trial_branches.py')
assert {str(p.relative_to(root / 'stronger')) for p in (root / 'stronger').rglob('*') if p.is_file()} == set(record['inputs']) | {'tests/test_trial_branches.py'}
recipe_paths = ['src/python.rs', 'src/python/identity.rs', 'src/python/scope.py',
                'src/python/runtime.py', 'src/python/driver.py', 'src/python/protocol.rs',
                'src/python/project.rs', 'src/process.rs', 'src/capture.rs',
                'src/workspace.rs', 'src/command.rs', 'Cargo.lock']
recipe = 'sha256:' + hashlib.sha256(b''.join((repo / p).read_bytes() for p in recipe_paths)).hexdigest()
runs = {}
for group in ['original-reports', 'stronger-reports']:
    paths = list((root / group).iterdir())
    assert len(paths) == 1
    path = paths[0]
    run = json.loads((path / 'run.json').read_text())
    outcomes = json.loads((path / 'outcomes.json').read_text())
    inventory = json.loads((path / 'mutants.json').read_text())
    assert run['python_scope']['recipe_fingerprint'] == recipe
    runs[group] = {'path': str(path), 'status': run['status'], 'score': run['score'],
                   'test_count': len(run['python_scope']['test_nodes']),
                   'duration_seconds': run['duration_seconds'], 'summary': outcomes['summary'],
                   'selection': run['effective_config'], 'inventory': inventory['mutations'],
                   'outcomes': outcomes['mutations'],
                   'sha256': {str(p.relative_to(path)): digest(p) for p in path.rglob('*') if p.is_file()}}
original, stronger = runs['original-reports'], runs['stronger-reports']
assert original['test_count'] == 198
assert original['status'] == 'failed'
assert stronger['test_count'] == 200
assert stronger['summary'] == {'killed': 3, 'missed': 0, 'errors': 0, 'timeout': 0, 'build_failed': 0}
sites = [m for m in original['inventory'] if m['id'] in (15,16,22)]
assert len(sites) == 3
assert {m['operator'] for m in sites} == {'branch_force_true','branch_force_false'}
assert {m['line'] for m in sites} == {111,197}
assert len(original['inventory']) == 38
for site in sites:
    assert next(o for o in original['outcomes'] if o['id'] == site['id'])['outcome'] == 'missed'
assert original['score']['percentage'] is None
fields = ['file', 'line', 'column', 'byte_start', 'byte_end', 'source_fingerprint', 'original', 'replacement', 'operator']
assert len(stronger['inventory']) == 3
for site in sites:
    assert any(all(site[k] == new[k] for k in fields) for new in stronger['inventory'])
prior_manifest = root / 'preservation.json'
result = {'project': {k:v for k,v in record.items() if k != 'prior_reports'},
          'runs': runs, 'recipe_fingerprint': recipe, 'cli_sha256': digest(root / 'mutie'),
          'stronger_test_sha256': digest(retained), 'preservation_verified': True,
          'prior_report_files_preserved': len(record['prior_reports']),
          'preservation_manifest': str(prior_manifest), 'preservation_manifest_sha256': digest(prior_manifest)}
output.write_text(json.dumps(result, indent=2) + '\n')
print('Preserved', len(record['inputs']), 'original inputs and', len(record['prior_reports']), 'historical report files')
for name, run in runs.items():
    print(name, run['test_count'], run['duration_seconds'], run['summary'])
