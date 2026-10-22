"""Verify preserved Tomli-W evidence without executing project code."""
import collections
import hashlib
import json
from pathlib import Path
import subprocess
import sys


def hashes(root):
    return {p.relative_to(root).as_posix(): hashlib.sha256(p.read_bytes()).hexdigest()
            for p in sorted(root.rglob('*')) if p.is_file()
            and '.git' not in p.relative_to(root).parts}


for directory in sys.argv[1:]:
    root = Path(directory)
    state = json.loads((root / 'results.json').read_text())
    original = Path(state['original'])
    assert hashes(original) == state['input_sha256']
    assert not subprocess.check_output(['git', '-C', original, 'status', '--porcelain'])
    assert hashlib.sha256((root / 'mutie').read_bytes()).hexdigest() == state['mutie_sha256']
    assert len(state['accounting']) == len(state['canonical']) == 30
    assert collections.Counter(v['outcome'] for v in state['accounting'].values()) == {
        'killed': 9, 'missed': 2, 'error': 19}
    assert collections.Counter(m['operator'] for m in state['canonical']) == {
        'boolean_flip': 12, 'comparison_boundary': 2, 'arithmetic_replace': 16}
    for record in state['records']:
        assert hashes(Path(record['report'])) == record['report_sha256']
        scope = record['run']['python_scope']
        assert len(scope['expected_failure_nodes']) == 2
        if record['label'] == 'stronger':
            assert record['exit_code'] == 0 and len(scope['test_nodes']) == 272
            assert [m['outcome'] for m in record['outcomes']['mutations']] == ['killed']
        else:
            assert record['run']['status'] == 'failed'
            assert record['run']['score']['percentage'] is None
            assert len(scope['test_nodes']) == 271
        assert all(state['input_sha256'][p] == value.removeprefix('sha256:')
                   for p, value in scope['data_fingerprints'].items())
        if 'data_directories' in scope:
            assert scope['data_directories'] == sorted(
                p.relative_to(original).as_posix() for p in original.rglob('*')
                if p.is_dir() and '.git' not in p.relative_to(original).parts)
    review = hashes(root / 'stronger-project')
    assert review == state['review_sha256']
    assert {k: v for k, v in review.items() if k != 'tests/test_mutie_stronger.py'} == state['input_sha256']
    # The preliminary copy has one additional trailing blank line.
    assert (root / 'stronger-project/tests/test_mutie_stronger.py').read_bytes().rstrip() == (
        Path(__file__).with_name('stronger_test.py').read_bytes().rstrip())
    print(directory, ': all inputs, reports, outcomes and stronger-test bytes verified')
