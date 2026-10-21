"""Review existing Route Mini reports without modifying original evidence."""
import argparse
import ast
import difflib
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import time

parser = argparse.ArgumentParser()
parser.add_argument('--previous', type=Path, required=True)
parser.add_argument('--output', type=Path, required=True)
parser.add_argument('--mutie', type=Path, required=True)
args = parser.parse_args()
previous = json.loads(args.previous.read_text())
original = Path(previous['records'][0]['command'][2])
python = Path(previous['records'][0]['command'][4])
full = next(r for r in previous['records'] if r['label'] == 'original-full')
output = args.output.resolve()
output.mkdir(parents=True, exist_ok=False)
project = output / 'stronger-project'

def hashes(root):
    return {p.relative_to(root).as_posix(): hashlib.sha256(p.read_bytes()).hexdigest()
            for p in sorted(root.rglob('*')) if p.is_file() and '.git' not in p.relative_to(root).parts}

def preserve():
    assert hashes(original) == previous['input_sha256']
    assert not subprocess.check_output(['git', '-C', original, 'status', '--porcelain'], text=True).strip()
    for r in previous['records']:
        if 'report' in r:
            assert hashes(Path(r['report'])) == r['report_sha256']
    old_stronger = args.previous.parent / 'stronger-project'
    assert hashes(old_stronger) == previous['stronger_input_sha256']
    if 'stronger_input_sha256' in state:
        assert hashes(project) == state['stronger_input_sha256']
    for r in state['records']:
        if 'report' in r:
            assert hashes(Path(r['report'])) == r['report_sha256']

state = dict(previous=str(args.previous), records=[])
preserve()
shutil.copytree(original, project, ignore=shutil.ignore_patterns('.git'))
assert hashes(project) == previous['input_sha256']
fixtures = Path(__file__).parent
shutil.copyfile(fixtures / 'test_review_contracts.py', project / 'tests/test_review_contracts.py')
shutil.copyfile(fixtures.parent / 'python-route-mini/test_stronger_boundaries.py', project / 'tests/test_stronger_boundaries.py')
# Retain every upstream test and assertion. Guard impossible retries before route,
# and express unexpected exhaustion of expected-success routes as assertions.
p = project / 'tests/test_router.py'
before = p.read_text()
lines = before.splitlines(keepends=True)
offsets = [0]
for line in lines:
    offsets.append(offsets[-1] + len(line))
edits = []
for node in ast.walk(ast.parse(before)):
    if isinstance(node, ast.FunctionDef) and node.name.startswith('test_'):
        pos = offsets[node.body[0].lineno - 1]
        edits.append((pos, pos, '    assert FallbackChain().decide(ProviderError(status_code=503), 0).retry_same is False\n'))
    if isinstance(node, ast.Assign) and isinstance(node.value, ast.Call):
        call = node.value
        if isinstance(call.func, ast.Attribute) and call.func.attr == 'route':
            a = offsets[call.lineno - 1] + call.col_offset
            b = offsets[call.end_lineno - 1] + call.end_col_offset
            receiver = ast.get_source_segment(before, call.func.value)
            argument = ast.get_source_segment(before, call.args[0])
            edits.append((a, b, f'expected_success({receiver}, {argument})'))
after = before
for a, b, replacement in sorted(edits, reverse=True):
    after = after[:a] + replacement + after[b:]
after = after.replace('    router.route(route_request)\n    assert len(telemetry.decisions())', '    expected_success(router, route_request)\n    assert len(telemetry.decisions())')
after += '''\n\ndef expected_success(router, route_request):
    try:
        return router.route(route_request)
    except FallbackExhaustedError as error:
        pytest.fail(f"expected successful route, exhausted: {error}")
'''
p.write_text(after)
(output / 'router-tests.patch').write_text(''.join(difflib.unified_diff(before.splitlines(True), after.splitlines(True), fromfile='original/tests/test_router.py', tofile='stronger/tests/test_router.py')))
state['stronger_input_sha256'] = hashes(project)
state['original_input_sha256'] = previous['input_sha256']

def save():
    preserve()
    state['original_inputs_and_reports_preserved'] = True
    (output / 'results.json').write_text(json.dumps(state, indent=2) + '\n')

def command(label, argv, expected=(0,), **kwargs):
    preserve()
    start = time.perf_counter()
    result = subprocess.run(list(map(str, argv)), capture_output=True, text=True, timeout=900, **kwargs)
    (output / (label + '.stdout')).write_text(result.stdout)
    (output / (label + '.stderr')).write_text(result.stderr)
    record = dict(label=label, command=list(map(str, argv)), seconds=time.perf_counter()-start, exit_code=result.returncode)
    state['records'].append(record)
    save()
    assert result.returncode in expected, result.stdout + result.stderr
    print(label, round(record['seconds'], 2), flush=True)
    return result, record

code = 'import sys; from pathlib import Path; sys.path.insert(0, str(Path(sys.argv[1])/"src")); import pytest; raise SystemExit(pytest.main(["-p", "no:cacheprovider", sys.argv[1]]))'
env = dict(os.environ, PYTEST_DISABLE_PLUGIN_AUTOLOAD='1', PYTEST_ADDOPTS='', PYTEST_PLUGINS='')
result, _ = command('native-baseline', [python, '-I', '-B', '-c', code, project], cwd=project, env=env)
assert '85 passed' in result.stdout
result, _ = command('isolated-baseline', [args.mutie.resolve(), 'python-check', project, '--python', python, '--timeout', '30', '--json'])
assert len(json.loads(result.stdout)['tests']) == 85

# IDs are selected against the original full inventory; record local mapping.
for label, ids in [('retry-fallback', [9, 10, 11, 14, 20]), ('dataclass', [1,2,4,5,7,8,16,17,18,19,25,26,27,28,29,30,31,32,33,34,35,36,37,38])]:
    result, record = command(label, [args.mutie.resolve(), project, '--lang', 'python', '--python', python,
        '--timeout', '20', '--baseline-timeout', '30', '--ui', 'plain', '--output-dir', output / 'reports',
        '--rerun', full['report'], '--id', ','.join(map(str, ids))], expected=(0,1))
    report = Path(next(s.removeprefix('Saving report to ') for s in result.stdout.splitlines() if s.startswith('Saving report to ')))
    run = json.loads((report / 'run.json').read_text())
    outcomes = json.loads((report / 'outcomes.json').read_text())
    inventory = json.loads((report / 'mutants.json').read_text())
    def key(m):
        return tuple(m[k] for k in ('file','source_fingerprint','operator','byte_start','byte_end','original','replacement'))
    original_ids = {key(m): m['id'] for m in full['inventory']['mutations']}
    record.update(report=str(report), report_sha256=hashes(report), status=run['status'], score=run['score'],
        summary=outcomes['summary'], outcomes=outcomes['mutations'], inventory=inventory,
        original_ids={m['id']: original_ids[key(m)] for m in inventory['mutations']})
    save()
    assert run['status'] == 'completed', outcomes
    expected_kills = set(ids) if label == 'retry-fallback' else {1,4,7,16,18,25,27,29,31,33,35,37}
    mapping = record['original_ids']
    assert len(record['outcomes']) == len(ids)
    for row in record['outcomes']:
        assert row['state'] == 'completed'
        assert row['outcome'] == ('killed' if mapping[row['id']] in expected_kills else 'missed'), row
save()
