#!/usr/bin/env python3
"""Replay attribution evidence and reject corrupted/insufficient records."""
import copy
import hashlib
import json
from pathlib import Path
import runpy
import sys

HERE=Path(__file__).resolve().parent
classify=runpy.run_path(str(HERE/'classify-file.py'))['classify']
record=json.loads(Path(sys.argv[1]).read_text())
assert record['observer_sha256']==hashlib.sha256((HERE/'observer.mjs').read_bytes()).hexdigest()
for name,digest in record['recipe_files_sha256'].items():
    assert digest==hashlib.sha256((HERE/name).read_bytes()).hexdigest(), name
assert len(record['results'])==112
for row in record['results']:
    assert row['control']['exit_code']==row['run']['exit_code'], row['scenario']
    assert classify(row['run'],row['observer'])==row['classification']
    assert row['classification']['outcome']==row['expected'], row['scenario']

for syntax in ('js','ts'):
    rows={r['scenario']:r for r in record['results'] if r['syntax']==syntax}
    assert len(rows)==56
    body=rows['body_throw']
    def reject(run=None, observed=None):
        assert classify(run if run is not None else body['run'], observed if observed is not None else body['observer'])['outcome']=='error'
    reject(observed=[])
    reject(observed=body['observer'][:-1])
    altered=copy.deepcopy(body['observer']);altered[-1]['sequence']+=1;reject(observed=altered)
    altered=copy.deepcopy(body['observer']);altered[1]['testId']+=100;reject(observed=altered)
    altered=copy.deepcopy(body['observer']);altered[-1]['phase']='afterEach';reject(observed=altered)
    altered=copy.deepcopy(body['observer']);altered[0]['version']='other';reject(observed=altered)
    altered=[e for e in copy.deepcopy(body['observer']) if e['event']!='caught']
    for i,e in enumerate(altered,1):e['sequence']=i
    reject(observed=altered)
    for field,value in [('junit',None),('junit','<testsuites>'),('exit_code',0),('exit_code',-9),('outer_timeout',True)]:
        altered=copy.deepcopy(body['run']);altered[field]=value;reject(run=altered)
    altered=copy.deepcopy(body['run']);altered['junit']=altered['junit'].replace('tests="1"','tests="2"',1);reject(run=altered)
    altered=copy.deepcopy(body['run']);altered['stderr']+='\n# Unhandled error between tests\n';reject(run=altered)
    # Native failure flags alone cannot turn a clean observed body into a kill.
    reject(observed=rows['pass']['observer'])
print('Verified 112 paired JS/TS cases and 30 fail-closed evidence controls.')
