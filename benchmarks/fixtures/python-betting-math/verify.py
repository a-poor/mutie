"""Check original/stronger inputs and map every selected outcome by source identity."""
import hashlib
import json
from pathlib import Path
import subprocess
import sys

root=Path(sys.argv[1]);project=Path(sys.argv[2])
state=json.loads((root/'results.json').read_text())
def hashes(directory):
    return {p.relative_to(directory).as_posix():hashlib.sha256(p.read_bytes()).hexdigest()
            for p in sorted(directory.rglob('*')) if p.is_file() and '.git' not in p.relative_to(directory).parts}
def identity(m):
    return tuple(m[k] for k in ('file','source_fingerprint','operator','byte_start','byte_end','original','replacement'))
assert hashes(project)==state['input_sha256']
assert subprocess.check_output(['git','-C',project,'rev-parse','HEAD'],text=True).strip()==state['commit']
assert not subprocess.check_output(['git','-C',project,'status','--porcelain'],text=True).strip()
strong=hashes(root/'stronger-project')
assert strong==state['stronger_sha256']
assert {k:v for k,v in strong.items() if k in state['input_sha256']}==state['input_sha256']
assert set(strong)-set(state['input_sha256'])=={'tests/test_metric_boundaries.py'}
original=next(r for r in state['records'] if r['label']=='original-metrics')
ids={identity(m):m['id'] for m in original['inventory']['mutations']}
accounted={}
for r in state['records']:
    if 'report' not in r:continue
    assert hashes(Path(r['report']))==r['report_sha256']
    mapping={m['id']:ids[identity(m)] for m in r['inventory']['mutations']}
    r['original_id_mapping']=mapping
    for outcome in r['outcomes']['mutations']:
        if outcome['state']!='completed':continue
        old=mapping[outcome['id']]
        if r['label'].startswith('original-'):
            assert old not in accounted
            accounted[old]=outcome['outcome']
        elif r['label']=='stronger-focused':
            assert state['metrics_accounting'][str(old)]=='missed'
            assert outcome['outcome']=='killed'
assert accounted=={int(k):v for k,v in state['metrics_accounting'].items()}
assert len(accounted)==10
for prior in sys.argv[3:]:
    ledger=json.loads(Path(prior).read_text())
    for r in ledger['records']:
        if 'report' in r:assert hashes(Path(r['report']))==r['report_sha256']
state['prior_ledgers_preserved']=sys.argv[3:]
state['final_verification']=True
(root/'results.json').write_text(json.dumps(state,indent=2)+'\n')
print('Preserved original inputs, only one added test file, report hashes, and complete ten-candidate identity accounting.')
