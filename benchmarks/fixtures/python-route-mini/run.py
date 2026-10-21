"""Validate unchanged Route Mini, then review findings in a separate copy.

Run --stage original first, inspect its reports, then --stage review with the
same output directory. Both stages verify every earlier input/report hash.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import time

parser = argparse.ArgumentParser()
for name in ('project', 'mutie', 'python', 'output'):
    parser.add_argument('--' + name, type=Path, required=True)
parser.add_argument('--stage', choices=('original', 'account', 'review'), required=True)
args = parser.parse_args()
root = args.project.resolve()
output = args.output.resolve()
mutie, python = args.mutie.absolute(), args.python.absolute()
commit = subprocess.check_output(['git', '-C', root, 'rev-parse', 'HEAD'], text=True).strip()
assert commit == '35a3db157731ba0073d66f432b69b997ce2e9f01'


def hashes(directory):
    return {p.relative_to(directory).as_posix(): hashlib.sha256(p.read_bytes()).hexdigest()
            for p in sorted(directory.rglob('*')) if p.is_file()
            and '.git' not in p.relative_to(directory).parts}


if args.stage == 'original':
    output.mkdir(parents=True, exist_ok=False)
    state = dict(repository='https://github.com/JustinJLeopard/route-mini', commit=commit,
                 input_sha256=hashes(root), records=[])
else:
    state = json.loads((output / 'results.json').read_text())


def preserve():
    assert hashes(root) == state['input_sha256']
    assert not subprocess.check_output(['git', '-C', root, 'status', '--porcelain'], text=True).strip()
    for record in state['records']:
        if 'report' in record:
            assert hashes(Path(record['report'])) == record['report_sha256']
    if 'stronger_input_sha256' in state:
        assert hashes(output / 'stronger-project') == state['stronger_input_sha256']


def save():
    preserve()
    state.update(original_inputs_preserved=True, original_reports_preserved=True)
    (output / 'results.json').write_text(json.dumps(state, indent=2) + '\n')


def command(label, argv, expected=(0,), **kwargs):
    preserve()
    start = time.perf_counter()
    result = subprocess.run(list(map(str, argv)), capture_output=True, text=True,
                            timeout=900, **kwargs)
    record = dict(label=label, command=list(map(str, argv)),
                  seconds=time.perf_counter() - start, exit_code=result.returncode)
    (output / (label + '.stdout')).write_text(result.stdout)
    (output / (label + '.stderr')).write_text(result.stderr)
    assert result.returncode in expected, result.stdout + result.stderr
    state['records'].append(record)
    save()
    print(label, round(record['seconds'], 3), flush=True)
    return result, record


environment = dict(os.environ, PYTEST_DISABLE_PLUGIN_AUTOLOAD='1', PYTEST_ADDOPTS='', PYTEST_PLUGINS='')
code = ('import sys; from pathlib import Path; sys.path.insert(0, str(Path(sys.argv[1])/"src")); '
        'import pytest; raise SystemExit(pytest.main(['
        '"-p", "no:cacheprovider", sys.argv[1]]))')
# Native pytest reads the original quiet-output and testpaths configuration.

def native(label, project):
    result, _ = command(label, [python, '-I', '-B', '-c', code, project], cwd=project, env=environment)
    return result


def scored(label, project, extra=()):
    result, record = command(label, [mutie, project, '--lang', 'python', '--python', python,
        '--timeout', '20', '--baseline-timeout', '30', '--ui', 'plain',
        '--output-dir', output / 'reports', *extra], expected=(0, 1))
    report = Path(next(line.removeprefix('Saving report to ') for line in result.stdout.splitlines()
                       if line.startswith('Saving report to ')))
    run = json.loads((report / 'run.json').read_text())
    outcomes = json.loads((report / 'outcomes.json').read_text())
    inventory = json.loads((report / 'mutants.json').read_text())
    assert run['status'] in ('completed', 'failed')
    record.update(report=str(report), scope=run['python_scope'], score=run['score'],
                  status=run['status'], summary=outcomes['summary'], inventory=inventory,
                  outcomes=outcomes['mutations'], report_sha256=hashes(report))
    save()
    return record


if args.stage == 'original':
    result, preview = command('discovery', [mutie, 'python-list', root, '--python', python, '--json'])
    preview['scope'] = json.loads(result.stdout)['scope']
    assert len(preview['scope']['tests']) == 4
    assert preview['scope']['conftests'] == ['tests/conftest.py']
    assert len(preview['scope']['mutations']) == 38
    assert '68 passed' in native('native-baseline', root).stdout
    result, baseline = command('isolated-baseline', [mutie, 'python-check', root,
        '--python', python, '--timeout', '30', '--json'])
    data = json.loads(result.stdout)
    assert len(data['tests']) == 68
    baseline.update(tests=data['tests'], identity=data['identity'])
    full = scored('original-full', root)
    save()

if args.stage in ('original', 'account'):
    full = next(r for r in state['records'] if r['label'] == 'original-full')
    def key(mutation):
        return tuple(mutation[k] for k in ('file', 'source_fingerprint', 'operator',
                                           'byte_start', 'byte_end', 'original', 'replacement'))
    original_ids = {key(m): m['id'] for m in full['inventory']['mutations']}
    seen = {}
    def absorb(record):
        mapping = {m['id']: original_ids[key(m)] for m in record['inventory']['mutations']}
        for row in record['outcomes']:
            if row['state'] == 'completed':
                original_id = mapping[row['id']]
                assert original_id not in seen
                seen[original_id] = dict(row, original_id=original_id, report=record['report'])
    for record in state['records']:
        if record['label'] == 'original-full' or record['label'].startswith('original-pending-'):
            absorb(record)
    while len(seen) < 38:
        remaining = sorted(set(range(1, 39)) - seen.keys())
        record = scored('original-pending-' + str(len(seen)), root,
                        ['--rerun', full['report'], '--id', ','.join(map(str, remaining))])
        before = len(seen)
        absorb(record)
        assert len(seen) > before
    state['original_accounting'] = [seen[i] for i in sorted(seen)]
    save()
else:
    assert '68 passed' in native('original-native-configured', root).stdout
    full = next(r for r in state['records'] if r['label'] == 'original-full')
    review_ids = [3, 21, 24]
    assert all(next(r for r in state['original_accounting'] if r['original_id'] == i)['outcome'] == 'missed' for i in review_ids)
    assert review_ids
    stronger = output / 'stronger-project'
    shutil.copytree(root, stronger, ignore=shutil.ignore_patterns('.git'))
    assert hashes(stronger) == state['input_sha256']
    (stronger / 'tests/test_stronger_boundaries.py').write_bytes(
        Path(__file__).with_name('test_stronger_boundaries.py').read_bytes())
    state['stronger_input_sha256'] = hashes(stronger)
    assert '72 passed' in native('stronger-native-baseline', stronger).stdout
    control = scored('original-selected-control', root, ['--rerun', full['report'], '--id', ','.join(map(str, review_ids))])
    assert control['summary'] == dict(killed=0, missed=3, errors=0, timeout=0, build_failed=0)
    result = scored('stronger-focused', stronger, ['--rerun', full['report'], '--id', ','.join(map(str, review_ids))])
    assert result['summary'] == dict(killed=3, missed=0, errors=0, timeout=0, build_failed=0)
    assert len(result['scope']['test_nodes']) == 72
    save()
