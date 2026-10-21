"""Validate unchanged Binary Tree Library and empty-traversal findings in a copy."""
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
args = parser.parse_args()
root = args.project.resolve()
output = args.output.resolve()
output.mkdir(parents=True, exist_ok=False)
commit = subprocess.check_output(['git', '-C', str(root), 'rev-parse', 'HEAD'], text=True).strip()
assert commit == '68d688fb4fe9c867c77a9d54f4fdb46a58dea34e'


def hashes(directory):
    return {p.relative_to(directory).as_posix(): hashlib.sha256(p.read_bytes()).hexdigest()
            for p in sorted(directory.rglob('*')) if p.is_file()
            and '.git' not in p.relative_to(directory).parts}


def clean():
    assert not subprocess.check_output(['git', '-C', str(root), 'status', '--porcelain'], text=True).strip()


clean()
original = hashes(root)
records = []
reports = {}


def command(label, argv, expected=0, **kwargs):
    start = time.perf_counter()
    result = subprocess.run(list(map(str, argv)), capture_output=True, text=True,
                            timeout=360, **kwargs)
    seconds = time.perf_counter() - start
    (output / (label + '.stdout')).write_text(result.stdout)
    (output / (label + '.stderr')).write_text(result.stderr)
    assert result.returncode == expected, result.stdout + result.stderr
    assert hashes(root) == original
    for report, before in reports.items():
        assert hashes(report) == before
    clean()
    record = dict(label=label, command=list(map(str, argv)), seconds=seconds,
                  exit_code=result.returncode)
    records.append(record)
    print(label, round(seconds, 3), flush=True)
    return result, record


mutie = args.mutie.absolute()
python = args.python.absolute()  # Preserve the venv launcher symlink.
result, preview = command('discovery', [mutie, 'python-list', root, '--python', python, '--json'])
preview['scope'] = json.loads(result.stdout)['scope']
assert preview['scope']['tests'] == ['tests/test_avl_tree.py', 'tests/test_binary_search_tree.py', 'tests/test_traversal.py']
assert len(preview['scope']['mutations']) == 11
assert preview['scope']['conftests'] == ['tests/conftest.py']

# Native pytest uses the original project configuration. It neither builds nor installs
# the package; the fresh cache and disabled cache provider preserve the checkout.
environment = dict(os.environ, PYTEST_DISABLE_PLUGIN_AUTOLOAD='1', PYTEST_ADDOPTS='', PYTEST_PLUGINS='')
code = ('import sys; sys.path.insert(0, sys.argv[1]); import pytest; '
        'raise SystemExit(pytest.main(["-q", "-p", "no:cacheprovider", sys.argv[1]]))')
result, _ = command('native-baseline', [python, '-I', '-B', '-X',
    'pycache_prefix=' + str(output / 'native-cache'), '-c', code, root], cwd=root, env=environment)
assert '7 passed' in result.stdout

result, baseline = command('isolated-baseline', [mutie, 'python-check', root,
    '--python', python, '--timeout', '30', '--json'])
data = json.loads(result.stdout)
assert len(data['tests']) == 7
baseline.update(tests=data['tests'], identity=data['identity'])


def scored(label, project, summary, expected=0, extra=()):
    result, record = command(label, [mutie, project, '--lang', 'python', '--python', python,
        '--timeout', '20', '--baseline-timeout', '30', '--ui', 'plain',
        '--output-dir', output / 'reports', *extra], expected=expected)
    report = Path(next(line.removeprefix('Saving report to ') for line in result.stdout.splitlines()
                       if line.startswith('Saving report to ')))
    run = json.loads((report / 'run.json').read_text())
    outcomes = json.loads((report / 'outcomes.json').read_text())
    inventory = json.loads((report / 'mutants.json').read_text())
    assert run['status'] == ('failed' if expected else 'completed')
    assert len(run['python_scope']['test_nodes']) == (16 if label.startswith('stronger') else 7)
    assert outcomes['summary'] == dict(killed=summary[0], missed=summary[1], errors=0,
                                      timeout=0, build_failed=0)
    reports[report] = hashes(report)
    record.update(report=str(report), scope=run['python_scope'], score=run['score'],
                  status=run['status'], summary=outcomes['summary'], inventory=inventory,
                  outcomes=outcomes['mutations'], report_sha256=reports[report])
    return report


full = scored('original-full', root, (5, 6))
scored('original-survivor-rerun', root, (0, 6), extra=['--rerun-missed', full])
stronger = output / 'stronger-project'
shutil.copytree(root, stronger, ignore=shutil.ignore_patterns('.git'))
assert hashes(stronger) == original
(stronger / 'tests/test_empty_traversals.py').write_bytes(
    Path(__file__).with_name('test_empty_traversals.py').read_bytes())
stronger_hashes = hashes(stronger)
assert all(stronger_hashes[name] == digest for name, digest in original.items())
result, _ = command('stronger-native-baseline', [python, '-I', '-B', '-X',
    'pycache_prefix=' + str(output / 'stronger-cache'), '-c', code, stronger], cwd=stronger, env=environment)
assert '16 passed' in result.stdout
scored('stronger-survivor-rerun', stronger, (6, 0), extra=['--rerun-missed', full])
scored('stronger-full', stronger, (11, 0))
assert hashes(stronger) == stronger_hashes
assert hashes(root) == original
clean()
(output / 'results.json').write_text(json.dumps(dict(
    repository='https://github.com/burpeesDaily/sphinx-sample-code', commit=commit,
    input_sha256=original, stronger_input_sha256=stronger_hashes, records=records,
    original_inputs_preserved=True, original_reports_preserved=True), indent=2) + '\n')
