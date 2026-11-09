"""Verify unchanged Scrubify inputs, reports and focused string identities."""
import collections
import hashlib
import json
from pathlib import Path
import subprocess
import sys

root = Path(sys.argv[1]).resolve()
output = Path(sys.argv[2])
repo = Path(__file__).resolve().parents[3]
preserved = json.loads((root/'preservation.json').read_text())
original, stronger = root/'original', root/'stronger'


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def hashes(directory):
    return {str(p.relative_to(directory)): sha(p)
            for p in sorted(directory.rglob('*')) if p.is_file()}


def read(directory, name):
    return json.loads((directory/name).read_text())


assert not subprocess.check_output(['git', '-C', original, 'status', '--porcelain'])
assert subprocess.check_output(['git', '-C', original, 'rev-parse', 'HEAD']).decode().strip() == preserved['commit']
for name, digest in preserved['inputs'].items():
    assert sha(original/name) == sha(stronger/name) == digest, name
extra = 'tests/test_redaction_defaults.py'
assert set(hashes(stronger)) == set(preserved['inputs']) | {extra}
assert sha(stronger/extra) == sha(repo/'benchmarks/fixtures/python-strings/test_redaction_defaults.py')

fields = ['file', 'source_fingerprint', 'operator', 'byte_start', 'byte_end',
          'original', 'replacement', 'line', 'column', 'end_line', 'end_column']


def identity(m):
    return tuple(m[key] for key in fields)


records = read(root, 'original-records.json')
first = root/records['records'][0]['path']
first_run = read(first, 'run.json')
inventory = read(first, 'mutants.json')['mutations']
assert len(inventory) == 34 and all(m['operator'] == 'string_replace' for m in inventory)
ids = {identity(m): m['id'] for m in inventory}
assert len(ids) == 34
listing_fields = [key for key in fields if key not in ('end_line', 'end_column')]
assert [{key: m[key] for key in listing_fields} for m in read(root, 'final-list.json')['mutations']] == [
    {key: m[key] for key in listing_fields} for m in inventory]


def summarize(directory):
    run, outcomes = read(directory, 'run.json'), read(directory, 'outcomes.json')
    assert run['status'] == outcomes['status'] and run['checkpoint'] == outcomes['checkpoint']
    mapping = {m['id']: ids[identity(m)] for m in read(directory, 'mutants.json')['mutations']}
    if run['status'] == 'failed':
        assert run['score']['percentage'] is None
    return run, outcomes, mapping, dict(
        path=str(directory.relative_to(root)), status=run['status'],
        seconds=run['duration_seconds'], summary=outcomes['summary'], score=run['score'],
        original_id_mapping=mapping,
        outcomes=[{key: m[key] for key in ['id', 'state', 'outcome', 'error']}
                  for m in outcomes['mutations']], sha256=hashes(directory))


accounted, runs = {}, []
for record in records['records']:
    directory = root/record['path']
    run, outcomes, mapping, summary = summarize(directory)
    assert summary['sha256'] == record['sha256']
    assert run['python_scope'] == first_run['python_scope']
    assert outcomes['baseline']['outcome'] == 'passed'
    assert '6 passed' in (directory/'logs/baseline.stdout.log').read_text()
    for m in outcomes['mutations']:
        if m['state'] == 'completed':
            old = mapping[m['id']]
            assert old not in accounted
            accounted[old] = m['outcome']
    runs.append(summary)
assert accounted == {int(key): value for key, value in records['accounted'].items()}
assert set(accounted) == set(ids.values())
assert collections.Counter(accounted.values()) == dict(killed=8, missed=25, error=1)
assert accounted[32] == 'error'
assert 'IndexError: string index out of range' in (first/'logs/000032.stdout.log').read_text()

focused_ids = set(records['focused_ids'])
assert len(focused_ids) == 19
assert focused_ids == {m['id'] for m in inventory
                       if m['file'] == 'src/scrubify/core.py' and accounted[m['id']] == 'missed'}
strong_runs = []
for directory in sorted((root/'stronger-reports').iterdir()):
    run, outcomes, mapping, summary = summarize(directory)
    assert run['status'] == 'completed'
    assert outcomes['baseline']['outcome'] == 'passed'
    assert '32 passed' in (directory/'logs/baseline.stdout.log').read_text()
    assert set(mapping.values()) == focused_ids
    assert outcomes['summary'] == dict(killed=19, missed=0, errors=0, timeout=0, build_failed=0)
    for field in ['runtime', 'recipe_fingerprint', 'manifest_fingerprint', 'source_root']:
        assert run['python_scope'].get(field) == first_run['python_scope'].get(field)
    strong_runs.append(summary)
assert len(strong_runs) == 1

result = dict(project='https://github.com/MartiAble/scrubify-py', commit=preserved['commit'],
    tracked_inputs=preserved['inputs'], cli_sha256=sha(root/'mutie'),
    stronger_test=dict(path=extra, sha256=sha(stronger/extra)),
    inventory=[{'id': m['id'], **{key: m[key] for key in fields}} for m in inventory],
    combined_accounting=accounted, combined_counts=dict(collections.Counter(accounted.values())),
    original_runs=runs, stronger_runs=strong_runs,
    open_survivors=sorted(set(key for key, value in accounted.items() if value == 'missed') - focused_ids),
    final_inventory_verified=True, preservation_verified=True)
output.write_text(json.dumps(result, indent=2)+'\n')
print('Verified', len(preserved['inputs']), 'inputs and', sum(len(r['sha256']) for r in runs), 'original report files')
print(result['combined_counts'], 'combined accounting, not a full score')
print('Stronger tests killed', len(focused_ids), 'survivors;', len(result['open_survivors']), 'remain open')
