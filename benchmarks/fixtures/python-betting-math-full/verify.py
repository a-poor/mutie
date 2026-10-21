"""Verify complete identity accounting and immutable original/previous evidence."""
import hashlib
import json
from pathlib import Path
import subprocess
import sys

output, project, prior_path = map(Path, sys.argv[1:4])
state = json.loads((output/'results.json').read_text())
prior = json.loads(prior_path.read_text())
def hashes(root):
    return {p.relative_to(root).as_posix(): hashlib.sha256(p.read_bytes()).hexdigest()
            for p in sorted(root.rglob('*')) if p.is_file() and '.git' not in p.relative_to(root).parts}
def identity(m):
    return tuple(m[k] for k in ('file','source_fingerprint','operator','byte_start','byte_end','original','replacement'))
assert hashes(project) == state['input_sha256'] == prior['input_sha256']
assert hashlib.sha256(Path(state['records'][0]['command'][0]).read_bytes()).hexdigest() == state['mutie_sha256']
assert not subprocess.check_output(['git','-C',project,'status','--porcelain'],text=True).strip()
assert subprocess.check_output(['git','-C',project,'rev-parse','HEAD'],text=True).strip() == state['commit']
assert hashes(prior_path.parent/'stronger-project') == prior['stronger_sha256']
strong = hashes(output/'review-project')
assert strong == state['review_sha256']
assert {p for p,d in state['input_sha256'].items() if strong.get(p)!=d} == {'tests/test_kelly.py'}
assert set(strong)-set(state['input_sha256']) == {'tests/test_metric_boundaries.py','tests/test_public_contracts.py'}
# Reconstruct the only edited original test file exactly, preserving its assertions.
expected = (project/'tests/test_kelly.py').read_text()
for value in ('0.0','1.0'):
    before=f'        assert full_kelly_fraction({value}, 5.0) == 0.0'
    after=(f'        try:\n            actual = full_kelly_fraction({value}, 5.0)\n'
           '        except InvalidProbabilityError as error:\n'
           "            pytest.fail(f'valid endpoint was rejected: {error}')\n"
           '        assert actual == 0.0')
    assert expected.count(before)==1
    expected=expected.replace(before,after)
assert (output/'review-project/tests/test_kelly.py').read_text()==expected
mutations = state['records'][0]['inventory']['mutations']
canonical = {identity(m):m['id'] for m in mutations}
assert len(canonical)==87 and set(canonical.values())==set(range(1,88))
records = {r['report']:r for s in (prior,state) for r in s['records'] if 'report' in r}
for r in records.values():
    report=Path(r['report'])
    assert hashes(report)==r['report_sha256']
    if r['run']['status']=='failed':
        assert r['run']['score']['percentage'] is None
    for key, name in [('run','run.json'),('inventory','mutants.json'),('outcomes','outcomes.json')]:
        assert json.loads((report/name).read_text())==r[key]
for field in ('accounting','review_accounting'):
    for key, item in state[field].items():
        r=records[item['report']]
        m=next(m for m in r['inventory']['mutations'] if m['id']==item['local_id'])
        o=next(o for o in r['outcomes']['mutations'] if o['id']==item['local_id'])
        assert canonical[identity(m)]==int(key)
        assert o['state']=='completed' and o['outcome']==item['outcome']
        if field=='review_accounting':
            assert item['outcome']=='killed'
            assert state['accounting'][key]['outcome'] in ('missed','error')
assert set(map(int,state['accounting']))==set(range(1,88))
for ledger in prior.get('prior_ledgers_preserved',[]):
    for r in json.loads(Path(ledger).read_text())['records']:
        if 'report' in r: assert hashes(Path(r['report']))==r['report_sha256']
state['prior_ledgers_preserved']=[str(prior_path),*prior.get('prior_ledgers_preserved',[])]
state['final_verification']=True
(output/'results.json').write_text(json.dumps(state,indent=2)+'\n')
print('Verified 87 original outcomes, stronger rerun identities, exact test-only changes, and all original/prior report hashes.')
