#!/usr/bin/env python3
"""Bounded direct-run controls, NOT automatic mutation classifications or scores."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import tempfile
import time


def hashes(root):
    return {str(p.relative_to(root)): hashlib.sha256(p.read_bytes()).hexdigest()
            for p in sorted(root.rglob('*')) if p.is_file() and '.git' not in p.relative_to(root).parts}


def run(bun, root):
    start = time.monotonic()
    p = subprocess.run([bun, 'test', '--max-concurrency=1', '--timeout=1000'], cwd=root,
                       capture_output=True, text=True, timeout=15)
    return {'exit_code': p.returncode, 'duration_seconds': time.monotonic()-start,
            'stdout': p.stdout, 'stderr': p.stderr}


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--bun', required=True)
    parser.add_argument('--project', required=True)
    parser.add_argument('--out', required=True)
    args = parser.parse_args()
    bun = str(Path(args.bun).resolve())
    assert subprocess.check_output([bun, '--revision'], text=True).strip() == '1.3.11+af24e281e'
    project = Path(args.project).resolve()
    commit = subprocess.check_output(['git', '-C', str(project), 'rev-parse', 'HEAD'], text=True).strip()
    assert commit == '3de15b846e3f5ef46cf8aebd5d136908baa43b54'
    original = hashes(project)
    record = {'project': 'https://github.com/gramiojs/composer', 'commit': commit,
              'original_file_sha256': original, 'runs': []}
    candidates = [('src/queue.ts', 'private active = true;', 'true', 'false'),
                  ('src/queue.ts', 'this.active = false;', 'false', 'true'),
                  ('src/utils.ts', 'configurable: true,', 'true', 'false')]
    with tempfile.TemporaryDirectory(prefix='mutie-bun-composer-') as tmp:
        root = Path(tmp)/'project'
        shutil.copytree(project, root, ignore=shutil.ignore_patterns('.git'))
        baseline = run(bun, root)
        assert baseline['exit_code'] == 0
        record['runs'].append({'label': 'baseline', **baseline})
        for file, marker, old, new in candidates:
            path = root/file
            pristine = path.read_bytes()
            needle = marker.encode()
            assert pristine.count(needle) == 1
            start = pristine.index(needle) + marker.index(old)
            mutated = pristine[:start] + new.encode() + pristine[start+len(old):]
            try:
                path.write_bytes(mutated)
                record['runs'].append({'label': 'manual_candidate', 'file': file,
                  'byte_start': start, 'original': old, 'replacement': new, **run(bun, root)})
            finally:
                path.write_bytes(pristine)
        assert hashes(root) == original
        stronger_path = Path(__file__).with_name('composer-stronger.test.ts')
        added = root/'tests/mutie-stronger.test.ts'
        added.write_bytes(stronger_path.read_bytes())
        baseline = run(bun, root)
        assert baseline['exit_code'] == 0
        path = root/'src/utils.ts'
        pristine = path.read_bytes()
        try:
            path.write_bytes(pristine.replace(b'configurable: true,', b'configurable: false,'))
            candidate = run(bun, root)
            assert candidate['exit_code'] == 1
            record['runs'].append({'label': 'stronger_manual_candidate', 'file': 'src/utils.ts',
                'test_sha256': hashlib.sha256(added.read_bytes()).hexdigest(),
                'baseline': baseline, 'candidate': candidate})
        finally:
            path.write_bytes(pristine)
            added.unlink()
        assert hashes(root) == original
    repo = Path(__file__).resolve().parents[3]
    for syntax, fixture in [('js','bun-js'),('ts','bun-typescript')]:
        source = repo/'tests/fixtures'/fixture
        before = hashes(source)
        with tempfile.TemporaryDirectory(prefix='mutie-bun-fixture-') as tmp:
            root=Path(tmp)/'project';shutil.copytree(source,root)
            baseline=run(bun,root);assert baseline['exit_code']==0
            path=root/f'src/flags.{syntax}';pristine=path.read_bytes()
            outcomes=[]
            for old,new in [(b'return true;',b'return false;'),(b'|| false;',b'|| true;')]:
                try:
                    path.write_bytes(pristine.replace(old,new))
                    outcomes.append(run(bun,root))
                finally:path.write_bytes(pristine)
            assert [x['exit_code'] for x in outcomes]==[1,0]
            stronger=f'import {{test,expect}} from "bun:test";\nimport {{fallback}} from "../src/flags.{syntax}";\ntest("empty fallback",()=>expect(fallback(" ")).toBe(false));\n'
            (root/f'test/stronger.test.{syntax}').write_text(stronger)
            strengthened=run(bun,root);assert strengthened['exit_code']==0
            try:
                path.write_bytes(pristine.replace(b'|| false;',b'|| true;'))
                rerun=run(bun,root);assert rerun['exit_code']==1
            finally:path.write_bytes(pristine)
            record['runs'].append({'label':'fixture','syntax':syntax,'baseline':baseline,
                'manual_candidates':outcomes,'stronger_test':stronger,'stronger_baseline':strengthened,'stronger_candidate':rerun})
        assert hashes(source)==before
    assert hashes(project)==original
    record['preserved_original_files']=len(original)
    Path(args.out).write_text(json.dumps(record,indent=2)+'\n')

if __name__=='__main__':main()
