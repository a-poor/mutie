#!/usr/bin/env python3
"""Paired Composer baseline/finding checks for the experimental observer."""
import argparse
import hashlib
import json
from pathlib import Path
import runpy
import shutil
import tempfile

HERE=Path(__file__).resolve().parent
run=runpy.run_path(str(HERE/'probe.py'))['run']
classify=runpy.run_path(str(HERE/'classify-file.py'))['classify']


def main():
    p=argparse.ArgumentParser();p.add_argument('--bun',required=True);p.add_argument('--project',required=True);p.add_argument('--out',required=True);a=p.parse_args()
    source=Path(a.project).resolve()
    expected=json.loads((HERE.parents[1]/'bun-composer-probe-results.json').read_text())['original_file_sha256']
    def check(root):
        for f,digest in expected.items(): assert hashlib.sha256((root/f).read_bytes()).hexdigest()==digest, f
    check(source)
    record={'original_file_sha256':expected,'observer_sha256':hashlib.sha256((HERE/'observer.mjs').read_bytes()).hexdigest(),'runs':[]}
    with tempfile.TemporaryDirectory(prefix='mutie-bun-observed-project-') as tmp:
        root=Path(tmp)/'project';shutil.copytree(source,root,ignore=shutil.ignore_patterns('.git'))
        path=root/'src/utils.ts';original=path.read_bytes()
        for mode in ['baseline','candidate','stronger_baseline','stronger_candidate']:
            path.write_bytes(original)
            if mode.startswith('stronger'):
                shutil.copyfile(HERE/'composer-stronger.test.ts',root/'tests/mutie-stronger.test.ts')
            if mode.endswith('candidate'):
                path.write_bytes(original.replace(b'configurable: true,',b'configurable: false,'))
            control=run(a.bun,root,15,timeout_ms=1000)
            observed=run(a.bun,root,15,HERE/'observer.mjs',timeout_ms=1000)
            events=[json.loads(line) for line in (root/'observer.jsonl').read_text().splitlines()]
            result=classify(observed,events)
            record['runs'].append({'mode':mode,'control':control,'run':observed,'observer':events,'classification':result})
            print(mode,control['exit_code'],observed['exit_code'],result,flush=True)
            assert control['exit_code']==observed['exit_code']
        path.write_bytes(original);check(root)
    check(source)
    Path(a.out).write_text(json.dumps(record,indent=2)+'\n')
    assert [r['classification']['outcome'] for r in record['runs']]==['passed','passed','passed','body_failure']

if __name__=='__main__':main()
