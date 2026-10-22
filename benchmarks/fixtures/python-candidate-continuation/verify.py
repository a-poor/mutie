"""Independently verify completed accounting, effective policy and preserved bytes."""
import collections
import hashlib
import json
import subprocess
import sys
from pathlib import Path

output, project, previous_path = map(Path, sys.argv[1:4])
s = json.loads((output/'results.json').read_text())
previous = json.loads(previous_path.read_text())
def hashes(root):
    return {p.relative_to(root).as_posix(): hashlib.sha256(p.read_bytes()).hexdigest()
            for p in sorted(root.rglob('*')) if p.is_file() and '.git' not in p.relative_to(root).parts}
def identity(m):
    return tuple(m[k] for k in ('file', 'source_fingerprint', 'operator', 'byte_start', 'byte_end', 'original', 'replacement'))
assert hashes(project) == s['input_sha256'] == previous['input_sha256']
assert not subprocess.check_output(['git', '-C', project, 'status', '--porcelain'], text=True).strip()
assert hashlib.sha256((output/'mutie').read_bytes()).hexdigest() == s['mutie_sha256']
for path, digest in s['prior_ledgers'].items():
    path = Path(path)
    assert hashlib.sha256(path.read_bytes()).hexdigest() == digest
    ledger = json.loads(path.read_text())
    for key, folder in [('stronger_sha256', 'stronger-project'), ('review_sha256', 'review-project')]:
        if key in ledger:
            assert hashes(path.parent/folder) == ledger[key]
    for record in ledger['records']:
        if 'report' in record:
            assert hashes(Path(record['report'])) == record['report_sha256']
for record in s['records']:
    report = Path(record['report'])
    assert hashes(report) == record['report_sha256']
    for key, name in [('run', 'run.json'), ('inventory', 'mutants.json'), ('outcomes', 'outcomes.json')]:
        if key in record:
            assert json.loads((report/name).read_text()) == record[key]
r = s['records'][0]
assert r['exit_code'] == 1 and r['run']['status'] == 'failed'
assert r['run']['score']['percentage'] is None and r['run']['score']['unfinished'] == 0
assert r['run']['effective_config']['continue_on_error'] == 64
old_scope = next(x['run']['python_scope'] for x in previous['records'] if x['label']=='original-remaining').copy()
new_scope = r['run']['python_scope'].copy()
assert len(new_scope['test_nodes']) == 198
assert old_scope.pop('recipe_fingerprint') != new_scope.pop('recipe_fingerprint')
assert old_scope == new_scope, 'runtime, scope, inventory or resource limits changed'
canonical = {identity(m): m['id'] for m in previous['canonical']}
actual = {m['id']: canonical[identity(m)] for m in r['inventory']['mutations']}
assert len(actual) == len(set(actual.values())) == 194
assert {str(k): v for k,v in actual.items()} == s['canonical_id_mapping']
completed = r['outcomes']['mutations']
assert len(completed) == 194
for outcome in completed:
    assert outcome['state'] == 'completed'
    assert outcome['outcome'] == previous['accounting'][str(actual[outcome['id']])]['outcome']
assert collections.Counter(o['outcome'] for o in completed) == dict(killed=61, missed=97, error=36)
rejected = s['records'][1]
assert rejected['label']=='old-recipe-rejected' and rejected['exit_code']==1
assert rejected['run']['status']=='failed'
assert not any(o['state']=='completed' for o in rejected.get('outcomes', {}).get('mutations', []))
assert 'Python execution identity changed; start a fresh run' in (output/'old-recipe-rejected.stderr').read_text()
s['preserved'] = True
s['independently_verified'] = True
(output/'results.json').write_text(json.dumps(s, indent=2)+'\n')
print('Verified all 194 exact outcomes, unchanged scope/runtime/limits, failed unscored status, recipe rejection and original/historical artifact preservation.')
