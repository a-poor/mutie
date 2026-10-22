"""Verify unchanged tinycss2 inputs, frozen reports and stronger-test evidence."""
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
    state = json.loads((root/'results.json').read_text())
    original = Path(state['original'])
    assert hashes(original) == state['input_sha256']
    assert not subprocess.check_output(['git','-C',original,'status','--porcelain'])
    assert state['commit'] == '853bc5a5661a59c7a9795a4e2758d0081808ce06'
    assert hashlib.sha256((root/'mutie').read_bytes()).hexdigest() == state['mutie_sha256']
    assert len(state['canonical']) == len(state['accounting']) == 14
    assert collections.Counter(x['outcome'] for x in state['accounting'].values()) == {'missed':4,'error':10}
    assert {int(i) for i,x in state['accounting'].items() if x['outcome']=='missed'} == {1,2,4,6}
    for record in state['records']:
        assert hashes(Path(record['report'])) == record['report_sha256']
        scope = record['run']['python_scope']
        assert scope['application_dependencies'] == {'webencodings':'0.5.1'}
        assert scope['runtime']['versions']['webencodings']=='0.5.1'
        assert record['outcomes']['baseline']['outcome']=='passed'
        if record['label']=='stronger':
            assert record['exit_code']==0 and record['run']['status']=='completed'
            assert len(scope['test_nodes'])==484
            assert len(record['outcomes']['mutations'])==4
            assert all(m['outcome']=='killed' and m['state']=='completed' for m in record['outcomes']['mutations'])
        else:
            assert record['exit_code']==1 and record['run']['status']=='failed'
            assert record['run']['score']['percentage'] is None
            assert len(scope['test_nodes'])==480
    review = hashes(root/'stronger-project')
    assert review == state['review_sha256']
    assert {p:h for p,h in review.items() if p!='tests/test_mutie_dependency.py'} == state['input_sha256']
    assert (root/'stronger-project/tests/test_mutie_dependency.py').read_bytes() == Path(__file__).with_name('test_dependency_contracts.py').read_bytes()
    print(directory, ': original inputs, dependency identity, reports and four stronger-test kills verified')
