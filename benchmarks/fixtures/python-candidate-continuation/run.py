"""Compare bounded continuation with preserved, identity-matched original outcomes."""
import argparse
import collections
import hashlib
import json
import shutil
import subprocess
import time
from pathlib import Path

parser = argparse.ArgumentParser()
for name in ('project', 'python', 'mutie', 'output', 'previous', 'initial'):
    parser.add_argument('--'+name, type=Path, required=True)
args = parser.parse_args()
root = args.project.resolve()
out = args.output.resolve()
out.mkdir(parents=True, exist_ok=False)

def hashes(path):
    return {p.relative_to(path).as_posix(): hashlib.sha256(p.read_bytes()).hexdigest()
            for p in sorted(path.rglob('*')) if p.is_file() and '.git' not in p.relative_to(path).parts}

def identity(m):
    return tuple(m[k] for k in ('file', 'source_fingerprint', 'operator', 'byte_start', 'byte_end', 'original', 'replacement'))

previous = json.loads(args.previous.read_text())
initial = json.loads(args.initial.read_text())
ledger_paths = [args.previous, args.initial, *map(Path, initial['prior_ledgers'])]
ledgers = {str(p): hashlib.sha256(p.read_bytes()).hexdigest() for p in ledger_paths}
old = [json.loads(p.read_text()) for p in ledger_paths]
cli = out/'mutie'
shutil.copy2(args.mutie, cli)
state = dict(input_sha256=hashes(root), prior_ledgers=ledgers,
             mutie_sha256=hashlib.sha256(cli.read_bytes()).hexdigest(), records=[])
assert state['input_sha256'] == previous['input_sha256'] == initial['input_sha256']

def preserve():
    assert hashes(root) == state['input_sha256']
    assert not subprocess.check_output(['git', '-C', root, 'status', '--porcelain'], text=True).strip()
    assert hashlib.sha256(cli.read_bytes()).hexdigest() == state['mutie_sha256']
    for path, ledger in zip(ledger_paths, old):
        assert hashlib.sha256(path.read_bytes()).hexdigest() == ledgers[str(path)]
        for field, folder in [('stronger_sha256', 'stronger-project'), ('review_sha256', 'review-project')]:
            if field in ledger:
                assert hashes(path.parent/folder) == ledger[field]
        for record in ledger['records']:
            if 'report' in record:
                assert hashes(Path(record['report'])) == record['report_sha256']
    for r in state['records']:
        if 'report' in r:
            assert hashes(Path(r['report'])) == r['report_sha256']

def execute(label, extra):
    preserve()
    command = [str(cli), str(root), '--lang', 'python', '--python', str(args.python),
               '--timeout', '30', '--baseline-timeout', '60', '--ui', 'plain',
               '--output-dir', str(out/'reports'), *extra]
    start = time.monotonic()
    result = subprocess.run(command, capture_output=True, text=True, timeout=3600)
    (out/(label+'.stdout')).write_text(result.stdout)
    (out/(label+'.stderr')).write_text(result.stderr)
    report = Path(next(line.removeprefix('Saving report to ') for line in result.stdout.splitlines()
                       if line.startswith('Saving report to ')))
    r = dict(label=label, command=command, seconds=round(time.monotonic()-start, 3),
             exit_code=result.returncode, report=str(report), report_sha256=hashes(report))
    for key, filename in [('run', 'run.json'), ('inventory', 'mutants.json'), ('outcomes', 'outcomes.json')]:
        if (report/filename).exists():
            r[key] = json.loads((report/filename).read_text())
    state['records'].append(r)
    (out/'results.json').write_text(json.dumps(state, indent=2)+'\n')
    preserve()
    print(label, r['seconds'], r['exit_code'], flush=True)
    return r

r = execute('all-arithmetic', ['--operator', 'arithmetic_replace', '--continue-on-error', '64',
                               '--min-score', '100', '--fail-on-missed'])
assert r['exit_code'] == 1 and r['run']['status'] == 'failed'
assert r['run']['score']['percentage'] is None and r['run']['score']['unfinished'] == 0
assert len(r['run']['python_scope']['test_nodes']) == 198
assert r['outcomes']['baseline']['outcome'] == 'passed'
canonical = {identity(m): m['id'] for m in previous['canonical']}
actual = {m['id']: canonical[identity(m)] for m in r['inventory']['mutations']}
assert len(actual) == len(set(actual.values())) == 194
state['canonical_id_mapping'] = actual
for outcome in r['outcomes']['mutations']:
    assert outcome['state'] == 'completed'
    assert outcome['outcome'] == previous['accounting'][str(actual[outcome['id']])]['outcome']
state['counts'] = dict(collections.Counter(o['outcome'] for o in r['outcomes']['mutations']))
assert state['counts'] == dict(killed=61, missed=97, error=36)
# A recipe change must reject historical focused execution rather than guess compatibility.
prior_report = next(x['report'] for x in previous['records'] if x['label']=='original-remaining')
r = execute('old-recipe-rejected', ['--rerun', prior_report, '--id', '1', '--continue-on-error', '64'])
assert r['exit_code'] == 1 and r['run']['status'] == 'failed'
assert 'Python execution identity changed; start a fresh run' in (out/'old-recipe-rejected.stderr').read_text()
preserve()
state['preserved'] = True
(out/'results.json').write_text(json.dumps(state, indent=2)+'\n')
print('All 194 outcomes match preserved originals; failed score, historical recipe rejection and preservation verified.', flush=True)
