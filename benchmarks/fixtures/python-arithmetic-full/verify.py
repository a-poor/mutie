"""Verify 194-site accounting and independent stronger evidence without rewriting history."""
import hashlib
import json
from pathlib import Path
import subprocess
import sys

output, project, previous_path = map(Path, sys.argv[1:4])
state=json.loads((output/'results.json').read_text())
previous=json.loads(previous_path.read_text())
def hashes(root):
    return {p.relative_to(root).as_posix():hashlib.sha256(p.read_bytes()).hexdigest()
            for p in sorted(root.rglob('*')) if p.is_file() and '.git' not in p.relative_to(root).parts}
def identity(m):
    return tuple(m[k] for k in ('file','source_fingerprint','operator','byte_start','byte_end','original','replacement'))
assert hashes(project)==state['input_sha256']==previous['input_sha256']
assert subprocess.check_output(['git','-C',project,'rev-parse','HEAD'],text=True).strip()==state['commit']
assert not subprocess.check_output(['git','-C',project,'status','--porcelain'],text=True).strip()
assert hashlib.sha256(Path(state['records'][0]['command'][0]).read_bytes()).hexdigest()==state['mutie_sha256']==previous['mutie_sha256']
assert hashes(previous_path.parent/'stronger-project')==previous['stronger_sha256']
strong=hashes(output/'review-project');assert strong==state['review_sha256']
assert {p:d for p,d in strong.items() if p in state['input_sha256']}==state['input_sha256']
assert set(strong)-set(state['input_sha256'])=={'tests/test_numeric_contracts.py'}
assert len(state['canonical'])==194
canonical={identity(m):m['id'] for m in state['canonical']}
assert len(canonical)==194 and set(canonical.values())==set(range(1,195))
assert all(m['operator']=='arithmetic_replace' for m in state['canonical'])
records={r['report']:r for ledger in (previous,state) for r in ledger['records'] if 'report' in r}
for r in records.values():
    p=Path(r['report']);assert hashes(p)==r['report_sha256']
    for key,name in [('run','run.json'),('inventory','mutants.json'),('outcomes','outcomes.json')]:
        assert json.loads((p/name).read_text())==r[key]
    if r['run']['status']=='failed':assert r['run']['score']['percentage'] is None
    r['canonical_id_mapping']={m['id']:canonical[identity(m)] for m in r['inventory']['mutations']}
original_nodes=next(r['run']['python_scope']['test_nodes'] for r in previous['records'] if r['label'].startswith('original-'))
assert len(original_nodes)==198
for r in state['records']:
    if r['label'].startswith('original-'):
        assert r['run']['python_scope']['test_nodes']==original_nodes
    elif r['label']=='review-focused':
        nodes=r['run']['python_scope']['test_nodes']
        assert len(nodes)==len(set(nodes))==219 and set(original_nodes)<set(nodes)
# Reconstruct every accounting entry from the actual completed report outcome.
for field in ('accounting','review_accounting'):
    for key,item in state[field].items():
        r=records[item['report']]
        assert r['canonical_id_mapping'][item['local_id']]==int(key)
        o=next(o for o in r['outcomes']['mutations'] if o['id']==item['local_id'])
        assert o['state']=='completed' and o['outcome']==item['outcome']
        if field=='review_accounting':
            assert item['outcome'] in ('killed', 'missed', 'error', 'timeout') and state['accounting'][key]['outcome']=='missed'
assert set(map(int,state['accounting']))==set(range(1,195))
assert sum(a['prior'] for a in state['accounting'].values())==30
# Original execution covers exactly 164 other source identities, once each.
fresh=[]
for r in state['records']:
    if not r['label'].startswith('original-'):continue
    for o in r['outcomes']['mutations']:
        if o['state']=='completed':fresh.append(r['canonical_id_mapping'][o['id']])
assert len(fresh)==len(set(fresh))==164
assert set(fresh)=={int(k) for k,a in state['accounting'].items() if not a['prior']}
for name,digest in previous['prior_ledgers'].items():
    p=Path(name);assert hashlib.sha256(p.read_bytes()).hexdigest()==digest
    prior=json.loads(p.read_text())
    for key,folder in [('stronger_sha256','stronger-project'),('review_sha256','review-project')]:
        if key in prior:assert hashes(p.parent/folder)==prior[key]
    for r in prior['records']:
        if 'report' in r:assert hashes(Path(r['report']))==r['report_sha256']
state['previous_ledger_sha256']=hashlib.sha256(previous_path.read_bytes()).hexdigest()
state['final_verification']=True
(output/'results.json').write_text(json.dumps(state,indent=2)+'\n')
print('Verified 194 outcomes (30 prior, 164 new), all stronger identities, and original/historical inputs and report bytes.')
