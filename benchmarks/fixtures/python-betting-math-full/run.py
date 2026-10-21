"""Unchanged numerical project acceptance; never install or rewrite upstream."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import time

parser = argparse.ArgumentParser()
for key in ('project', 'python', 'mutie', 'output'):
    parser.add_argument('--'+key, type=Path, required=True)
parser.add_argument('--stage', choices=['original', 'review'], required=True)
parser.add_argument('--previous', type=Path, required=True)
args = parser.parse_args()
previous = json.loads(args.previous.read_text())
root = args.project.resolve()
output = args.output.resolve()
mutie = args.mutie.resolve()
python = args.python.absolute()

def hashes(path):
    return {p.relative_to(path).as_posix(): hashlib.sha256(p.read_bytes()).hexdigest()
            for p in sorted(path.rglob('*')) if p.is_file() and '.git' not in p.relative_to(path).parts}

if args.stage == 'original':
    output.mkdir(parents=True, exist_ok=False)
    state = dict(commit=subprocess.check_output(['git','-C',root,'rev-parse','HEAD'],text=True).strip(),
                 input_sha256=hashes(root), mutie_sha256=hashlib.sha256(mutie.read_bytes()).hexdigest(), records=[])
else:
    state = json.loads((output/'results.json').read_text())

def identity(m):
    return tuple(m[k] for k in ('file','source_fingerprint','operator','byte_start','byte_end','original','replacement'))

def preserve():
    assert hashes(root) == previous['input_sha256']
    assert hashes(args.previous.parent/'stronger-project') == previous['stronger_sha256']
    for record in previous['records']:
        if 'report' in record: assert hashes(Path(record['report'])) == record['report_sha256']
    assert hashes(root) == state['input_sha256']
    assert not subprocess.check_output(['git','-C',root,'status','--porcelain'],text=True).strip()
    assert hashlib.sha256(mutie.read_bytes()).hexdigest() == state['mutie_sha256']
    for r in state['records']:
        if 'report' in r: assert hashes(Path(r['report'])) == r['report_sha256']
    if 'review_sha256' in state: assert hashes(output/'review-project') == state['review_sha256']

def save():
    preserve()
    state['preserved'] = True
    (output/'results.json').write_text(json.dumps(state,indent=2)+'\n')

def command(label, argv, **kwargs):
    preserve()
    start=time.monotonic()
    result=subprocess.run(list(map(str,argv)),capture_output=True,text=True,timeout=3600,**kwargs)
    (output/(label+'.stdout')).write_text(result.stdout)
    (output/(label+'.stderr')).write_text(result.stderr)
    r=dict(label=label,command=list(map(str,argv)),seconds=round(time.monotonic()-start,3),exit_code=result.returncode)
    state['records'].append(r);save();print(label,r['seconds'],r['exit_code'],flush=True)
    return result,r

code='import sys; sys.path.insert(0, sys.argv[1]+"/src"); import pytest; raise SystemExit(pytest.main(["-p","no:cacheprovider",sys.argv[1]]))'
env=dict(os.environ,PYTEST_DISABLE_PLUGIN_AUTOLOAD='1',PYTEST_ADDOPTS='',PYTEST_PLUGINS='')

def scored(label,project,extra):
    result,r=command(label,[mutie,project,'--lang','python','--python',python,'--timeout','30','--baseline-timeout','60','--ui','plain','--output-dir',output/'reports',*extra])
    assert result.returncode in (0,1),result.stdout+result.stderr
    report=Path(next(x.removeprefix('Saving report to ') for x in result.stdout.splitlines() if x.startswith('Saving report to ')))
    run=json.loads((report/'run.json').read_text())
    r.update(report=str(report),report_sha256=hashes(report),run=run,
             inventory=json.loads((report/'mutants.json').read_text()),outcomes=json.loads((report/'outcomes.json').read_text()))
    save();return r


if args.stage == 'original':
    first=scored('original-all',root,[])
    mutations=first['inventory']['mutations']
    assert len(mutations)==87
    ids={identity(m):m['id'] for m in mutations}
    accounting={}
    for r in previous['records']:
        if not r['label'].startswith('original-'):continue
        local={m['id']:ids[identity(m)] for m in r['inventory']['mutations']}
        for o in r['outcomes']['mutations']:
            if o['state']=='completed':
                key=local[o['id']]
                assert key not in accounting
                accounting[key]=dict(outcome=o['outcome'],report=r['report'],local_id=o['id'],prior=True)
    assert len(accounting)==10
    current=first
    round_number=0
    while True:
        local={m['id']:ids[identity(m)] for m in current['inventory']['mutations']}
        progress=0
        for o in current['outcomes']['mutations']:
            if o['state']=='completed':
                key=local[o['id']]
                if key in accounting:
                    assert accounting[key]['outcome']==o['outcome']
                else:
                    accounting[key]=dict(outcome=o['outcome'],report=current['report'],local_id=o['id'],prior=False)
                    progress+=1
        remaining=sorted(set(ids.values())-set(accounting))
        state['accounting']=accounting;save()
        print('Accounted',len(accounting),'of',len(ids),flush=True)
        if not remaining:break
        assert progress or current is first
        round_number+=1
        current=scored('original-pending-'+str(round_number),root,
                      ['--rerun',first['report'],'--id',','.join(map(str,remaining))])

if args.stage == 'review':
    assert len(state['accounting']) == 87
    strong = output/'review-project'
    state['review_sha256'] = hashes(strong)
    # Only tests change: all application and configuration bytes stay original.
    changed = {p for p, digest in state['input_sha256'].items()
               if state['review_sha256'].get(p) != digest}
    assert changed == {'tests/test_kelly.py'}
    assert set(state['review_sha256']) - set(state['input_sha256']) == {
        'tests/test_public_contracts.py', 'tests/test_metric_boundaries.py'}
    result, _ = command('review-native', [python, '-I', '-B', '-X',
        'pycache_prefix='+str(output/'native-review-cache'), '-c', code, strong], env=env)
    assert result.returncode == 0 and '222 passed' in result.stdout
    original = state['records'][0]['report']
    canonical = {identity(m):m['id'] for m in state['records'][0]['inventory']['mutations']}
    state['review_accounting'] = {}
    for label, choices in (
        ('review-survivors', [20,25,26,27,28,29,31,32,34,37,39,46,47,48,49,50,51,60,62,63,66,75,83,85,87]),
        ('review-endpoint-errors', [16,17]),
    ):
        wanted = 'error' if label.endswith('errors') else 'missed'
        selected = [i for i in choices if state['accounting'][str(i)]['outcome'] == wanted]
        assert selected
        r = scored(label, strong, ['--rerun', original, '--id', ','.join(map(str,selected))])
        local = {m['id']:canonical[identity(m)] for m in r['inventory']['mutations']}
        for o in r['outcomes']['mutations']:
            assert o['state'] == 'completed', o
            state['review_accounting'][local[o['id']]] = dict(outcome=o['outcome'], report=r['report'], local_id=o['id'])
        assert all(o['outcome']=='killed' for o in r['outcomes']['mutations']), r['outcomes']
        assert hashes(strong) == state['review_sha256']
        save()
