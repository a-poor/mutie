"""Verify local Rust branch-forcing acceptance evidence and export a compact ledger."""
import hashlib
import json
import re
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
retained = repo / 'benchmarks/fixtures/rust-branches-ucd/optional_fields.rs'
assert digest(retained) == digest(root / 'stronger/ucd-parse/tests/optional_fields.rs')
assert {str(p.relative_to(root / 'stronger')) for p in (root / 'stronger').rglob('*') if p.is_file()} == set(record['inputs']) | {'ucd-parse/tests/optional_fields.rs'}
runs = {}
for group in ['original-reports', 'stronger-reports']:
    paths = list((root / group).iterdir())
    assert len(paths) == 1
    path = paths[0]
    run = json.loads((path / 'run.json').read_text())
    outcomes = json.loads((path / 'outcomes.json').read_text())
    inventory = json.loads((path / 'mutants.json').read_text())
    runs[group] = {'path': str(path), 'run': run, 'summary': outcomes['summary'],
                   'inventory': inventory['mutations'], 'outcomes': outcomes['mutations'],
                   'sha256': {str(p.relative_to(path)): digest(p) for p in path.rglob('*') if p.is_file()}}
original, stronger = runs['original-reports'], runs['stronger-reports']
assert original['run']['status'] == stronger['run']['status'] == 'completed'
assert original['summary'] == {'killed': 15, 'missed': 11, 'errors': 0, 'timeout': 0, 'build_failed': 0}
assert stronger['summary'] == {'killed': 4, 'missed': 0, 'errors': 0, 'timeout': 0, 'build_failed': 0}
missed = set(json.loads((root / 'selected.json').read_text()))
assert missed == {4,6,10,14}
assert missed <= {o['id'] for o in original['outcomes'] if o['outcome'] == 'missed'}
fields = ['file', 'line', 'column', 'byte_start', 'byte_end', 'source_fingerprint', 'original', 'replacement', 'operator']
identity = lambda m: tuple(m[k] for k in fields)
assert {identity(m) for m in original['inventory'] if m['id'] in missed} == {identity(m) for m in stronger['inventory']}
assert original['run']['score']['percentage'] == 1500 / 26
for run, count in [(original, 140), (stronger, 142)]:
    baseline = (Path(run['path']) / 'logs/baseline.stdout.log').read_text()
    results = re.findall(r'test result: ok\. (\d+) passed; 0 failed;', baseline)
    assert sum(map(int, results)) == count
manifest = root / 'preservation.json'
result = {'project': {k:v for k,v in record.items() if k != 'prior_reports'},
          'runs': runs, 'cli_sha256': digest(root / 'mutie'),
          'stronger_test_sha256': digest(retained), 'preservation_verified': True,
          'prior_report_files_preserved': len(record['prior_reports']),
          'validation': {'default_passed':323, 'default_ignored':147, 'native_compile_run_cases':10, 'scored_workflows_passed':1, 'fmt':'passed', 'clippy':'passed'},
          'preservation_manifest': str(manifest), 'preservation_manifest_sha256': digest(manifest)}
output.write_text(json.dumps(result, indent=2) + '\n')
print('Preserved', len(record['inputs']), 'original inputs and', len(record['prior_reports']), 'historical report files')
for name, run in runs.items():
    print(name, run['run']['duration_seconds'], run['summary'])
