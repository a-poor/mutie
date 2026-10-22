"""Unchanged tinycss2 1.2.1 scoped dependency acceptance and stronger-test reruns."""
import argparse
import collections
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import time

parser = argparse.ArgumentParser()
for name in ('project', 'python', 'mutie', 'output'):
    parser.add_argument('--' + name, type=Path, required=True)
parser.add_argument('--stage', choices=['original', 'review'], default='original')
parser.add_argument('--ids')
args = parser.parse_args()
root, out = args.project.resolve(), args.output.resolve()


def hashes(path):
    return {p.relative_to(path).as_posix(): hashlib.sha256(p.read_bytes()).hexdigest()
            for p in sorted(path.rglob('*')) if p.is_file() and '.git' not in p.relative_to(path).parts}


def identity(m):
    return tuple(m[k] for k in ('file', 'source_fingerprint', 'operator', 'byte_start',
                               'byte_end', 'original', 'replacement'))


if args.stage == 'original':
    out.mkdir(parents=True, exist_ok=False)
    shutil.copy2(args.mutie, out / 'mutie')
    state = dict(original=str(root), input_sha256=hashes(root), records=[], accounting={},
                 commit=subprocess.check_output(['git', '-C', root, 'rev-parse', 'HEAD'], text=True).strip(),
                 mutie_sha256=hashlib.sha256((out / 'mutie').read_bytes()).hexdigest())
else:
    state = json.loads((out / 'results.json').read_text())
    review_hashes = hashes(root)
    assert all(review_hashes.get(k) == v for k, v in state['input_sha256'].items())
    assert set(review_hashes) - state['input_sha256'].keys() == {'tests/test_mutie_dependency.py'}
    state['review_sha256'] = review_hashes


def preserve():
    assert hashes(Path(state['original'])) == state['input_sha256']
    assert not subprocess.check_output(['git', '-C', state['original'], 'status', '--porcelain'], text=True).strip()
    assert hashlib.sha256((out / 'mutie').read_bytes()).hexdigest() == state['mutie_sha256']
    for record in state['records']:
        assert hashes(Path(record['report'])) == record['report_sha256']
    if args.stage == 'review':
        assert hashes(root) == state['review_sha256']


def save():
    preserve()
    state['preserved'] = True
    (out / 'results.json').write_text(json.dumps(state, indent=2) + '\n')


def execute(label, extra):
    preserve()
    command = [str(out / 'mutie'), str(root), '--lang', 'python', '--python', str(args.python),
               '--timeout', '30', '--baseline-timeout', '60', '--ui', 'plain',
               '--output-dir', str(out / 'reports'), '--continue-on-error', '64', *extra]
    start = time.monotonic()
    result = subprocess.run(command, capture_output=True, text=True, timeout=3600)
    (out / (label + '.stdout')).write_text(result.stdout)
    (out / (label + '.stderr')).write_text(result.stderr)
    report = Path(next(line.removeprefix('Saving report to ') for line in result.stdout.splitlines()
                       if line.startswith('Saving report to ')))
    record = dict(label=label, command=command, seconds=round(time.monotonic() - start, 3),
                  exit_code=result.returncode, report=str(report), report_sha256=hashes(report))
    for key, filename in [('run', 'run.json'), ('inventory', 'mutants.json'), ('outcomes', 'outcomes.json')]:
        record[key] = json.loads((report / filename).read_text())
    assert record['outcomes']['baseline']['outcome'] == 'passed'
    state['records'].append(record)
    save()
    print(label, record['seconds'], record['exit_code'], flush=True)
    return record


if args.stage == 'original':
    first = current = execute('original-all', ['--operator', 'boolean_flip,comparison_boundary,arithmetic_replace', '--include', 'tinycss2/bytes.py', '--include', 'tinycss2/serializer.py'])
    canonical = state['canonical'] = first['inventory']['mutations']
    assert len(canonical) == 14
    ids = {identity(m): m['id'] for m in canonical}
    while True:
        local = {m['id']: ids[identity(m)] for m in current['inventory']['mutations']}
        progress = 0
        for outcome in current['outcomes']['mutations']:
            if outcome['state'] == 'completed':
                key = str(local[outcome['id']])
                assert key not in state['accounting']
                state['accounting'][key] = dict(outcome=outcome['outcome'], report=current['report'],
                                                local_id=outcome['id'])
                progress += 1
        save()
        remaining = sorted(set(ids.values()) - {int(k) for k in state['accounting']})
        print('Accounted', len(state['accounting']), 'of', len(ids), flush=True)
        if not remaining:
            break
        assert progress
        current = execute('original-pending-' + str(len(state['records'])),
                          ['--rerun', first['report'], '--id', ','.join(map(str, remaining))])
    state['counts'] = dict(collections.Counter(v['outcome'] for v in state['accounting'].values()))
    save()
    print(state['counts'], flush=True)
else:
    assert args.ids
    chosen = [int(s) for s in args.ids.split(',')]
    assert all(state['accounting'][str(i)]['outcome'] == 'missed' for i in chosen)
    record = execute('stronger', ['--rerun', state['records'][0]['report'], '--id', args.ids])
    assert all(o['state'] == 'completed' and o['outcome'] == 'killed'
               for o in record['outcomes']['mutations'])
    assert len(record['outcomes']['mutations']) == len(chosen)
    save()
