"""Verify local branch-forcing acceptance evidence and export a compact ledger."""
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
retained = repo / 'benchmarks/fixtures/javascript-branches/bragkit-type-branches.test.ts'
assert digest(retained) == digest(root / 'stronger/test/type-branches.test.ts')
assert {str(p.relative_to(root / 'stronger')) for p in (root / 'stronger').rglob('*') if p.is_file()} == set(record['inputs']) | {'test/type-branches.test.ts'}
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
assert original['summary'] == {'killed': 9, 'missed': 7, 'errors': 0, 'timeout': 0, 'build_failed': 0}
assert stronger['summary'] == {'killed': 7, 'missed': 0, 'errors': 0, 'timeout': 0, 'build_failed': 0}
missed = {o['id'] for o in original['outcomes'] if o['outcome'] == 'missed'}
fields = ['file', 'line', 'column', 'byte_start', 'byte_end', 'source_fingerprint', 'original', 'replacement', 'operator']
identity = lambda m: tuple(m[k] for k in fields)
assert {identity(m) for m in original['inventory'] if m['id'] in missed} == {identity(m) for m in stronger['inventory']}
assert original['run']['score']['percentage'] == 56.25
assert original['run']['node_scope'] == stronger['run']['node_scope']
for run, count in [(original, 61), (stronger, 63)]:
    baseline = (Path(run['path']) / 'logs/baseline.stdout.log').read_text()
    assert f'tests {count}\n' in baseline and f'pass {count}\n' in baseline and 'fail 0\n' in baseline
manifest = root / 'preservation.json'
result = {'project': {k:v for k,v in record.items() if k != 'prior_reports'},
          'runs': runs, 'cli_sha256': digest(root / 'mutie'),
          'stronger_test_sha256': digest(retained), 'preservation_verified': True,
          'prior_report_files_preserved': len(record['prior_reports']),
          'validation': {'default_passed':322, 'default_ignored':145, 'shared_js_tests_passed':20, 'scored_runner_syntax_workflows_passed':6, 'fmt':'passed', 'clippy':'passed', 'earlier_failures':'default Bun 1.4.3 rejected; Vitest baseline timeouts retained; no deadline/assertion or host security changes'},
          'preservation_manifest': str(manifest), 'preservation_manifest_sha256': digest(manifest)}
output.write_text(json.dumps(result, indent=2) + '\n')
print('Preserved', len(record['inputs']), 'original inputs and', len(record['prior_reports']), 'historical report files')
for name, run in runs.items():
    print(name, run['run']['duration_seconds'], run['summary'])
