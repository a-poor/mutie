"""Verify preserved inputs, reports, complete accounting and focused identity."""
import hashlib
import json
from pathlib import Path
import sys


def hashes(root):
    return {p.relative_to(root).as_posix(): hashlib.sha256(p.read_bytes()).hexdigest()
            for p in sorted(root.rglob('*')) if p.is_file() and '.git' not in p.relative_to(root).parts}


def identity(m):
    return tuple(m[k] for k in ('file', 'source_fingerprint', 'operator', 'byte_start',
                               'byte_end', 'original', 'replacement'))


root = Path(sys.argv[1])
state = json.loads((root / 'results.json').read_text())
records = state['records']
original = next(r for r in records if r['label'] == 'original-all-boundaries')
original_project = Path(original['command'][1])
assert hashes(original_project) == state['original_inputs']
assert hashes(root / 'stronger-project') == state['stronger_input_sha256']
assert {k: v for k, v in state['stronger_input_sha256'].items() if k in state['original_inputs']} == state['original_inputs']
assert set(state['stronger_input_sha256']) - set(state['original_inputs']) == {'tests/test_retry_boundary.py'}
ids = {identity(m): m['id'] for m in original['inventory']['mutations']}
accounting = {}
for r in records:
    if 'report' not in r:
        continue
    assert hashes(Path(r['report'])) == r['report_sha256']
    mapping = {m['id']: ids[identity(m)] for m in r['inventory']['mutations']}
    r['original_id_mapping'] = mapping
    if r['label'].startswith('original-'):
        for outcome in r['outcomes']['mutations']:
            if outcome['state'] == 'completed':
                original_id = mapping[outcome['id']]
                assert original_id not in accounting
                accounting[original_id] = outcome['outcome']
assert accounting == {1:'error', 2:'killed', 3:'killed', 4:'missed', 5:'killed', 6:'killed', 7:'missed'}
focused = next(r for r in records if r['label'] == 'stronger-network-focused')
assert focused['original_id_mapping'] == {1:4}
assert focused['outcomes']['summary'] == dict(killed=1, missed=0, errors=0, timeout=0, build_failed=0)
state['original_accounting'] = accounting
for ledger in sys.argv[2:]:
    prior = json.loads(Path(ledger).read_text())
    for record in prior['records']:
        if 'report' in record:
            assert hashes(Path(record['report'])) == record['report_sha256']
state['additional_preserved_ledgers'] = sys.argv[2:]
(root/'results.json').write_text(json.dumps(state, indent=2)+'\n')
print('All original inputs, stronger-copy constraints, reports and seven candidate identities verified.')
