"""Independently verify original and separate-copy review preservation."""
import difflib
import hashlib
import json
from pathlib import Path
import subprocess
import sys


def hashes(root):
    return {p.relative_to(root).as_posix(): hashlib.sha256(p.read_bytes()).hexdigest()
            for p in sorted(root.rglob('*')) if p.is_file()
            and '.git' not in p.relative_to(root).parts}


receipt = json.loads(Path(sys.argv[1]).read_text())
evidence = Path(receipt['source_evidence'])
assert hashlib.sha256(evidence.read_bytes()).hexdigest() == receipt['source_evidence_sha256']
state = json.loads(evidence.read_text())
original = Path(state['original'])
assert hashes(original) == state['input_sha256']
assert not subprocess.check_output(['git', '-C', original, 'status', '--porcelain'])
for record in state['records']:
    assert hashes(Path(record['report'])) == record['report_sha256']
assert hashes(Path(receipt['report'])) == receipt['report_sha256']
project = Path(receipt['command'][1])
assert hashes(project) == receipt['input_sha256']
changed = {p for p,h in receipt['input_sha256'].items() if state['input_sha256'].get(p)!=h}
assert changed == {'tests/test_valid.py', 'tests/test_mutie_quality.py'}
assert set(state['input_sha256']) <= set(receipt['input_sha256'])
assert (project/'tests/test_mutie_quality.py').read_bytes() == Path(__file__).with_name('test_serialization_contracts.py').read_bytes()
expected = {1:'missed', 3:'error', 4:'error', 7:'killed', 10:'killed', 24:'killed', 25:'error', 29:'killed'}
assert {int(i):m['outcome'] for i,m in receipt['accounting'].items()} == expected
assert all(m['state']=='completed' for m in receipt['accounting'].values())
assert receipt['run']['status']=='failed' and receipt['run']['score']['percentage'] is None
assert len(receipt['run']['python_scope']['test_nodes'])==275
assert receipt['run']['python_scope']['expected_failure_nodes']==state['records'][0]['run']['python_scope']['expected_failure_nodes']
print('Original inputs/reports, review inputs/report, source-mapped outcomes and score boundaries verified.')

patch = ''.join(difflib.unified_diff(
    (original/'tests/test_valid.py').read_text().splitlines(True),
    (project/'tests/test_valid.py').read_text().splitlines(True),
    fromfile='a/tests/test_valid.py', tofile='b/tests/test_valid.py'))
assert patch == Path(__file__).with_name('validity.patch').read_text()
review = json.loads(Path(__file__).with_name('review.json').read_text())
canonical = {m['id']:m for m in state['canonical']}
assert {r['canonical_id'] for r in review} == {1,2,3,4,7,10,14,15,16,17,19,20,21,23,24,25,26,27,28,29}
for row in review:
    i = row['canonical_id']
    assert row['source_identity'] == canonical[i]
    assert row['original'] == state['accounting'][str(i)]
    log = Path(row['original']['report'])/'logs'/f"{row['original']['local_id']:06d}.stdout.log"
    assert hashlib.sha256(log.read_bytes()).hexdigest() == row['log_sha256']
print('All 20 review entries match original source identities/logs; exact test patch verified.')
