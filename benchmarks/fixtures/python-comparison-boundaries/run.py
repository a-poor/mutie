"""Fresh comparison-only Route Mini run and separate-copy stronger rerun."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import time

parser = argparse.ArgumentParser()
parser.add_argument('--previous', type=Path, required=True)
parser.add_argument('--review', type=Path, required=True)
parser.add_argument('--output', type=Path, required=True)
parser.add_argument('--mutie', type=Path, required=True)
args = parser.parse_args()
previous = json.loads(args.previous.read_text())
review = json.loads(args.review.read_text())
root = Path(previous['records'][0]['command'][2])
python = previous['records'][0]['command'][4]
mutie = args.mutie.resolve()
output = args.output.resolve()
output.mkdir(parents=True, exist_ok=False)
state = dict(commit=previous['commit'], original_inputs=previous['input_sha256'], records=[])

def hashes(directory):
    return {p.relative_to(directory).as_posix(): hashlib.sha256(p.read_bytes()).hexdigest()
            for p in sorted(directory.rglob('*')) if p.is_file() and '.git' not in p.relative_to(directory).parts}

binary_hash = hashlib.sha256(mutie.read_bytes()).hexdigest()
state["mutie_sha256"] = binary_hash
def preserve():
    assert hashlib.sha256(mutie.read_bytes()).hexdigest() == binary_hash
    assert hashes(root) == previous['input_sha256']
    assert subprocess.check_output(['git', '-C', root, 'rev-parse', 'HEAD'], text=True).strip() == previous['commit']
    assert not subprocess.check_output(['git', '-C', root, 'status', '--porcelain'], text=True).strip()
    for record in previous['records'] + review['records'] + state['records']:
        if 'report' in record:
            assert hashes(Path(record['report'])) == record['report_sha256']
    assert hashes(args.previous.parent / 'stronger-project') == previous['stronger_input_sha256']
    # The review ledger may be the retained repository copy; its commands carry the original directory.
    review_project = Path(review['records'][0]['command'][-1])
    assert hashes(review_project) == review['stronger_input_sha256']
    if 'stronger_input_sha256' in state:
        assert hashes(output / 'stronger-project') == state['stronger_input_sha256']

def save():
    preserve()
    state['preserved'] = True
    (output / 'results.json').write_text(json.dumps(state, indent=2) + '\n')

def command(label, command, expected=(0,), **kwargs):
    preserve()
    start = time.perf_counter()
    result = subprocess.run(list(map(str, command)), capture_output=True, text=True, timeout=900, **kwargs)
    (output / (label + '.stdout')).write_text(result.stdout)
    (output / (label + '.stderr')).write_text(result.stderr)
    record = dict(label=label, command=list(map(str, command)), seconds=time.perf_counter()-start, exit_code=result.returncode)
    state['records'].append(record)
    save()
    assert result.returncode in expected, result.stdout + result.stderr
    print(label, round(record['seconds'], 2), flush=True)
    return result, record

result, record = command('discovery', [mutie, 'python-list', root, '--python', python, '--json'])
record['scope'] = json.loads(result.stdout)['scope']
code = 'import sys; from pathlib import Path; sys.path.insert(0, str(Path(sys.argv[1])/"src")); import pytest; raise SystemExit(pytest.main(["-p", "no:cacheprovider", sys.argv[1]]))'
env = dict(os.environ, PYTEST_DISABLE_PLUGIN_AUTOLOAD='1', PYTEST_ADDOPTS='', PYTEST_PLUGINS='')
result, _ = command('native-original', [python, '-I', '-B', '-c', code, root], cwd=root, env=env)
assert '68 passed' in result.stdout

def scored(label, project, extra):
    result, record = command(label, [mutie, project, '--lang', 'python', '--python', python,
        '--timeout', '20', '--baseline-timeout', '30', '--ui', 'plain', '--output-dir', output / 'reports', *extra])
    report = Path(next(s.removeprefix('Saving report to ') for s in result.stdout.splitlines() if s.startswith('Saving report to ')))
    run = json.loads((report / 'run.json').read_text())
    outcomes = json.loads((report / 'outcomes.json').read_text())
    record.update(report=str(report), report_sha256=hashes(report), status=run['status'], score=run['score'],
                  scope=run['python_scope'], outcomes=outcomes,
                  inventory=json.loads((report / 'mutants.json').read_text()))
    save()
    assert run['status'] == 'completed'
    return record

original = scored('original-router-boundary', root, ['--operator', 'comparison_boundary', '--include', 'src/route_mini/router.py'])
assert original['outcomes']['summary'] == dict(killed=0, missed=1, errors=0, timeout=0, build_failed=0)
assert original['inventory']['mutations'][0]['original'] == '>='
assert original['inventory']['mutations'][0]['replacement'] == '>'
stronger = output / 'stronger-project'
shutil.copytree(root, stronger, ignore=shutil.ignore_patterns('.git'))
assert hashes(stronger) == previous['input_sha256']
(stronger / 'tests/test_exact_capacity.py').write_bytes(Path(__file__).with_name('test_exact_capacity.py').read_bytes())
state['stronger_input_sha256'] = hashes(stronger)
result, _ = command('native-stronger', [python, '-I', '-B', '-c', code, stronger], cwd=stronger, env=env)
assert '69 passed' in result.stdout
focused = scored('stronger-focused', stronger, ['--rerun', original['report'], '--id', '1'])
assert focused['outcomes']['summary'] == dict(killed=1, missed=0, errors=0, timeout=0, build_failed=0)
assert len(focused['scope']['test_nodes']) == 69
save()
