"""Verify UCD string campaign preservation, native scopes and rerun identities."""
import hashlib
import json
from pathlib import Path
import re
import subprocess
import sys

root = Path(sys.argv[1]).resolve()
output = Path(sys.argv[2])
repo = Path(__file__).resolve().parents[3]
commit = 'e8fa937f0cac643669dcaf5edac2785b15cab917'
original, stronger = root/'original', root/'stronger'
selected = [1, 2, 3, 4, 5, 6, 7, 20, *range(94, 110)]


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def paths(directory):
    return {str(p.relative_to(directory)) for p in directory.rglob('*')
            if (p.is_file() or p.is_symlink()) and '.git' not in p.relative_to(directory).parts}


assert subprocess.check_output(['git', '-C', original, 'rev-parse', 'HEAD']).decode().strip() == commit
assert not subprocess.check_output(['git', '-C', original, 'status', '--porcelain'])
inputs = json.loads((root/'original-inputs.json').read_text())
for name, record in inputs.items():
    for directory in [original, stronger]:
        path = directory/name
        if record['kind'] == 'symlink':
            assert path.is_symlink() and str(path.readlink()) == record['target'], name
        else:
            assert not path.is_symlink() and digest(path) == record['sha256'], name
extra = 'ucd-parse/tests/string_contracts.rs'
assert paths(original) == set(inputs)
assert paths(stronger) == set(inputs) | {extra}
retained = repo/'benchmarks/fixtures/rust-strings-ucd/string_contracts.rs'
assert digest(retained) == digest(stronger/extra)
saved = json.loads((root/'original-report-hashes.json').read_text())
assert paths(root/'original-reports') == set(saved)
for name, sha in saved.items():
    assert digest(root/'original-reports'/name) == sha, name

runs = {}
scopes = []
for group, baseline_tests in [('original-reports', 140), ('stronger-reports', 144)]:
    directory, = sorted(p for p in (root/group).iterdir() if p.is_dir())
    run = json.loads((directory/'run.json').read_text())
    outcomes = json.loads((directory/'outcomes.json').read_text())
    inventory = json.loads((directory/'mutants.json').read_text())
    assert run['status'] == outcomes['status'] == 'completed'
    assert outcomes['baseline']['outcome'] == 'passed'
    assert '--workspace' in run['baseline_command']
    assert run['rust_scope']['workspace']['selected'] == ['ucd-parse']
    scopes.append(run['rust_scope'])
    baseline = (directory/'logs/baseline.stdout.log').read_text()
    counts = re.findall(r'test result: ok\. (\d+) passed; 0 failed;', baseline)
    assert sum(map(int, counts)) == baseline_tests
    assert all(m['operator'] == 'string_replace' for m in inventory['mutations'])
    runs[group] = dict(
        directory=str(directory.relative_to(root)), duration_seconds=run['duration_seconds'],
        baseline_tests=baseline_tests, summary=outcomes['summary'], score=run['score'],
        inventory=inventory['mutations'],
        outcomes=[{key: m[key] for key in ['id', 'state', 'outcome', 'duration_seconds', 'exit_code']}
                  for m in outcomes['mutations']],
        rerun_mapping=run.get('rerun', {}).get('mutations', []),
        manifest_fingerprint=run['rust_scope']['manifest_fingerprint'],
        lockfile_fingerprint=run['rust_scope']['lockfile_fingerprint'],
        report_sha256={str(p.relative_to(directory)): digest(p)
                       for p in sorted(directory.rglob('*')) if p.is_file()})
assert scopes[0] == scopes[1]
first, second = runs['original-reports'], runs['stronger-reports']
assert len(first['inventory']) == 112
assert first['summary'] == dict(killed=48, missed=64, build_failed=0, timeout=0, errors=0)
assert second['summary'] == dict(killed=24, missed=0, build_failed=0, timeout=0, errors=0)
assert second['score']['scope'] == 'rerun'
assert second['rerun_mapping'] == [{'id': i, 'source_id': sid} for i, sid in enumerate(selected, 1)]
fields = ['file', 'line', 'column', 'end_line', 'end_column', 'byte_start', 'byte_end',
          'source_fingerprint', 'original', 'replacement', 'operator']
identity = lambda m: tuple(m[key] for key in fields)
assert {identity(m) for m in first['inventory'] if m['id'] in selected} == {identity(m) for m in second['inventory']}
outcomes = {m['id']: m['outcome'] for m in first['outcomes']}
assert all(outcomes[i] == 'missed' for i in selected)
final = json.loads((root/'final-list.json').read_text())['mutations']
listing_fields = [key for key in fields if key not in ('end_line', 'end_column')]
assert [{key: m[key] for key in listing_fields} for m in final] == [
    {key: m[key] for key in listing_fields} for m in first['inventory']]

result = dict(
    project=dict(repository='https://github.com/BurntSushi/ucd-generate', commit=commit, inputs=inputs),
    scope=dict(package='ucd-parse', source='all eligible ucd-parse source files',
               operators=['string_replace'], test_scope='full workspace', jobs=2,
               build_cache='worker', timeout_seconds=30, baseline_timeout_seconds=180,
               rustc='1.99.1', cargo='1.99.1', platform='macOS'),
    runs=runs, stronger_test_sha256=digest(retained),
    original_report_files_preserved=len(saved), preservation_verified=True,
    final_inventory_verified=True, cli_sha256=digest(root/'mutie'), base_commit='9829bdf',
    remaining_survivors=[i for i, value in outcomes.items() if value == 'missed' and i not in selected])
output.write_text(json.dumps(result, indent=2)+'\n')
print('Verified', len(inputs), 'original paths,', len(saved), 'original report files, and 24 stronger-test kills')
print(len(result['remaining_survivors']), 'survivors remain open; historical score unchanged')
