"""Unchanged numerical project acceptance; never install or rewrite upstream."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import time

parser = argparse.ArgumentParser()
for key in ('project', 'python', 'mutie', 'output'):
    parser.add_argument('--'+key, type=Path, required=True)
parser.add_argument('--stage', choices=['baseline', 'score', 'stronger'], required=True)
args = parser.parse_args()
root = args.project.resolve()
output = args.output.resolve()
mutie = args.mutie.resolve()
python = args.python.absolute()

def hashes(path):
    return {p.relative_to(path).as_posix(): hashlib.sha256(p.read_bytes()).hexdigest()
            for p in sorted(path.rglob('*')) if p.is_file() and '.git' not in p.relative_to(path).parts}

if args.stage == 'baseline':
    output.mkdir(parents=True, exist_ok=False)
    state = dict(commit=subprocess.check_output(['git','-C',root,'rev-parse','HEAD'],text=True).strip(),
                 input_sha256=hashes(root), mutie_sha256=hashlib.sha256(mutie.read_bytes()).hexdigest(), records=[])
else:
    state = json.loads((output/'results.json').read_text())

def preserve():
    assert hashes(root) == state['input_sha256']
    assert not subprocess.check_output(['git','-C',root,'status','--porcelain'],text=True).strip()
    assert hashlib.sha256(mutie.read_bytes()).hexdigest() == state['mutie_sha256']
    for r in state['records']:
        if 'report' in r: assert hashes(Path(r['report'])) == r['report_sha256']
    if 'stronger_sha256' in state: assert hashes(output/'stronger-project') == state['stronger_sha256']

def save():
    preserve()
    state['preserved'] = True
    (output/'results.json').write_text(json.dumps(state,indent=2)+'\n')

def command(label, argv, **kwargs):
    preserve()
    start=time.monotonic()
    result=subprocess.run(list(map(str,argv)),capture_output=True,text=True,timeout=900,**kwargs)
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

if args.stage == 'baseline':
    result,r=command('discovery',[mutie,'python-list',root,'--python',python,'--json'])
    assert result.returncode==0,result.stderr
    r['scope']=json.loads(result.stdout)['scope'];save()
    result,r=command('native-baseline',[python,'-I','-B','-X','pycache_prefix='+str(output/'native-cache'),'-c',code,root],cwd=root,env=env)
    assert result.returncode==0,result.stdout+result.stderr
    assert '198 passed' in result.stdout
    result,r=command('isolated-baseline',[mutie,'python-check',root,'--python',python,'--timeout','60','--json'])
    assert result.returncode==0,result.stdout+result.stderr
    r['baseline']=json.loads(result.stdout)
    assert len(r['baseline']['tests']) == 198
    save()
elif args.stage == 'score':
    original=scored('original-metrics',root,['--operator','comparison_boundary','--include','src/betting_math_kit/metrics.py'])
    def identity(m):
        return tuple(m[k] for k in ('file','source_fingerprint','operator','byte_start','byte_end','original','replacement'))
    ids={identity(m):m['id'] for m in original['inventory']['mutations']}
    accounted={}
    current=original
    round_number=0
    while True:
        local={m['id']:ids[identity(m)] for m in current['inventory']['mutations']}
        for outcome in current['outcomes']['mutations']:
            if outcome['state']=='completed':
                key=local[outcome['id']]
                assert key not in accounted
                accounted[key]=outcome['outcome']
        remaining=sorted(set(ids.values())-set(accounted))
        state['metrics_accounting']=accounted;save()
        if not remaining:break
        assert any(o['state']=='completed' for o in current['outcomes']['mutations'])
        round_number+=1
        current=scored('original-pending-'+str(round_number),root,
                       ['--rerun',original['report'],'--id',','.join(map(str,remaining))])
    print('ACCOUNTING', accounted,flush=True)

elif args.stage == 'stronger':
    original=next(r for r in state['records'] if r['label']=='original-metrics')
    selected=[int(k) for k,v in state['metrics_accounting'].items() if v=='missed']
    assert selected
    stronger=output/'stronger-project'
    shutil.copytree(root,stronger,ignore=shutil.ignore_patterns('.git'))
    assert hashes(stronger)==state['input_sha256']
    (stronger/'tests/test_metric_boundaries.py').write_bytes(Path(__file__).with_name('test_metric_boundaries.py').read_bytes())
    state['stronger_sha256']=hashes(stronger);save()
    result,r=command('native-stronger',[python,'-I','-B','-X','pycache_prefix='+str(output/'stronger-cache'),'-c',code,stronger],cwd=stronger,env=env)
    assert result.returncode==0,result.stdout+result.stderr
    assert '205 passed' in result.stdout
    focused=scored('stronger-focused',stronger,['--rerun',original['report'],'--id',','.join(map(str,selected))])
    assert focused['run']['status']=='completed'
    assert len(focused['run']['python_scope']['test_nodes'])==205
    assert focused['outcomes']['summary']==dict(killed=len(selected),missed=0,errors=0,timeout=0,build_failed=0)
    save()
