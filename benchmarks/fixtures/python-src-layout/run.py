"""Validate pinned Scrubify unchanged, then improve tests in a separate copy."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import time

parser = argparse.ArgumentParser()
parser.add_argument('--project', type=Path, required=True)
parser.add_argument('--mutie', type=Path, required=True)
parser.add_argument('--python', type=Path, required=True)
parser.add_argument('--output', type=Path, required=True)
args = parser.parse_args()
root = args.project.resolve()
output = args.output.resolve()
output.mkdir(parents=True, exist_ok=False)
commit = subprocess.check_output(['git', '-C', str(root), 'rev-parse', 'HEAD'], text=True).strip()
assert commit == '558265805d942c03d2e6d8522baa0ca2992ec067'
assert not subprocess.check_output(['git', '-C', str(root), 'status', '--porcelain'], text=True).strip()


def hashes(directory):
    return {str(p.relative_to(directory)): hashlib.sha256(p.read_bytes()).hexdigest()
            for p in sorted(directory.rglob('*')) if p.is_file()
            and '.git' not in p.relative_to(directory).parts}


original = hashes(root)
records = []


def command(label, argv):
    start = time.perf_counter()
    result = subprocess.run([str(args.mutie.absolute()), *map(str, argv)],
                            capture_output=True, text=True, timeout=300)
    seconds = time.perf_counter() - start
    (output / (label + '.stdout')).write_text(result.stdout)
    (output / (label + '.stderr')).write_text(result.stderr)
    assert result.returncode == 0, result.stdout + result.stderr
    assert hashes(root) == original
    record = dict(label=label, command=[str(args.mutie.absolute()), *map(str, argv)],
                  seconds=seconds)
    records.append(record)
    print(label, round(seconds, 3), flush=True)
    return result, record


result, preview = command('discovery', ['python-list', root, '--python', args.python.absolute(), '--json'])
preview['scope'] = json.loads(result.stdout)['scope']
assert preview['scope']['source_root'] == 'src' and len(preview['scope']['mutations']) == 3
result, baseline_record = command('baseline', ['python-check', root, '--python', args.python.absolute(), '--timeout', '20', '--json'])
baseline = json.loads(result.stdout)
assert len(baseline['tests']) == 6
assert baseline['identity']['source_root'] == 'src'
baseline_record.update(identity=baseline['identity'], tests=baseline['tests'])


def scored(label, project, extra=()):
    result, record = command(label, [project, '--lang', 'python', '--python', args.python.absolute(),
        '--timeout', '10', '--baseline-timeout', '20', '--max-capture-bytes', '4194304',
        '--ui', 'plain', '--output-dir', output / 'reports', *extra])
    report = Path(next(line.removeprefix('Saving report to ') for line in result.stdout.splitlines()
                       if line.startswith('Saving report to ')))
    run = json.loads((report / 'run.json').read_text())
    outcomes = json.loads((report / 'outcomes.json').read_text())
    inventory = json.loads((report / 'mutants.json').read_text())
    assert run['status'] == 'completed'
    assert run['python_scope']['source_root'] == 'src'
    assert all(m['file'].startswith('src/scrubify/') for m in inventory['mutations'])
    assert outcomes['summary'] == dict(killed=1, missed=2 if label == 'full' else 1,
                                      timeout=0, errors=0, build_failed=0)
    record.update(report=str(report), scope=run['python_scope'], score=run['score'],
                  summary=outcomes['summary'], inventory=inventory,
                  outcomes=[(m['id'], m['outcome']) for m in outcomes['mutations']],
                  report_sha256=hashes(report))
    return report


full = scored('full', root)
full_hashes = hashes(full)
stronger = output / 'stronger-project'
shutil.copytree(root, stronger, ignore=shutil.ignore_patterns('.git'))
assert hashes(stronger) == original
retained_test = Path(__file__).resolve().parents[1] / 'python-scrubify/test_blank_queries.py'
shutil.copyfile(retained_test, stronger / 'tests/test_blank_queries.py')
stronger_hashes = hashes(stronger)
scored('stronger-rerun', stronger, ['--rerun-missed', full])
assert len(records[-1]['scope']['test_nodes']) == 9
assert hashes(stronger) == stronger_hashes
assert hashes(full) == full_hashes
assert hashes(root) == original
assert not subprocess.check_output(['git', '-C', str(root), 'status', '--porcelain'], text=True).strip()
(output / 'results.json').write_text(json.dumps(dict(commit=commit, input_sha256=original,
    stronger_input_sha256=stronger_hashes, records=records,
    original_inputs_preserved=True, original_report_preserved=True), indent=2) + '\n')
