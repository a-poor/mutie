"""Verify arithmetic acceptance, retained inputs and historical report bytes."""
import hashlib
import json
from pathlib import Path
import subprocess
import sys

output, project = map(Path, sys.argv[1:3])
state=json.loads((output/'results.json').read_text())
def hashes(root):
    return {p.relative_to(root).as_posix():hashlib.sha256(p.read_bytes()).hexdigest()
            for p in sorted(root.rglob('*')) if p.is_file() and '.git' not in p.relative_to(root).parts}
def identity(m):
    return tuple(m[k] for k in ('file','source_fingerprint','operator','byte_start','byte_end','original','replacement'))
assert hashes(project)==state['input_sha256']
assert not subprocess.check_output(['git','-C',project,'status','--porcelain'],text=True).strip()
assert subprocess.check_output(['git','-C',project,'rev-parse','HEAD'],text=True).strip()==state['commit']
assert hashlib.sha256(Path(state['records'][0]['command'][0]).read_bytes()).hexdigest()==state['mutie_sha256']
strong=hashes(output/'stronger-project')
assert strong==state['stronger_sha256']
assert {k:v for k,v in strong.items() if k in state['input_sha256']}==state['input_sha256']
assert set(strong)-set(state['input_sha256'])=={'tests/test_arithmetic_contracts.py'}
original=next(r for r in state['records'] if r['label']=='original-kelly')
ids={identity(m):m['id'] for m in original['inventory']['mutations']}
assert len(ids)==30
assert all(m['operator']=='arithmetic_replace' for m in original['inventory']['mutations'])
accounted={}
for r in state['records']:
    if 'report' not in r:continue
    p=Path(r['report'])
    assert hashes(p)==r['report_sha256']
    for key,name in [('run','run.json'),('inventory','mutants.json'),('outcomes','outcomes.json')]:
        assert json.loads((p/name).read_text())==r[key]
    mapping={m['id']:ids[identity(m)] for m in r['inventory']['mutations']}
    r['original_id_mapping']=mapping
    if r['run']['status']=='failed':assert r['run']['score']['percentage'] is None
    for o in r['outcomes']['mutations']:
        if o['state']!='completed':continue
        old=mapping[o['id']]
        if r['label'].startswith('original-'):
            assert old not in accounted
            accounted[old]=o['outcome']
        elif r['label']=='stronger-focused':
            assert state['arithmetic_accounting'][str(old)]=='missed'
            assert o['outcome']=='killed'
assert accounted=={int(k):v for k,v in state['arithmetic_accounting'].items()}
assert len(accounted)==30
scope=state['records'][0]['scope']
pairs={('+','-'),('-','+'),('*','/'),('/','*')}
arithmetic=[m for m in scope['mutations'] if (m['original'],m['replacement']) in pairs]
assert len(arithmetic)==194 and len(scope['mutations'])==281
new_sites={(m['file'],m['start'],m['end'],m['original'],m['replacement']) for m in scope['mutations']}
for name,digest in state['prior_ledgers'].items():
    p=Path(name);assert hashlib.sha256(p.read_bytes()).hexdigest()==digest
    prior=json.loads(p.read_text())
    for key,folder in [('stronger_sha256','stronger-project'),('review_sha256','review-project')]:
        if key in prior:assert hashes(p.parent/folder)==prior[key]
    for r in prior['records']:
        if 'report' in r:
            assert hashes(Path(r['report']))==r['report_sha256']
            for m in r['inventory']['mutations']:
                assert (m['file'],m['byte_start'],m['byte_end'],m['original'],m['replacement']) in new_sites
state['final_verification']=True
(output/'results.json').write_text(json.dumps(state,indent=2)+'\n')
print('Verified all 30 selected identities, stronger kills, unchanged inputs/reports, and preserved prior mutation sites.')
