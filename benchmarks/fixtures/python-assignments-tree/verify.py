"""Verify the assignment audit, final recipes and preserved stronger-test findings."""
import collections
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tomllib

tree, route, output = map(Path, sys.argv[1:4])
repo = Path(__file__).resolve().parents[3]


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def files(root):
    return {str(p.relative_to(root)): digest(p) for p in root.rglob('*')
            if p.is_file() and '.git' not in p.relative_to(root).parts}


recipe_paths = ['src/python.rs', 'src/python/identity.rs', 'src/python/scope.py',
                'src/python/runtime.py', 'src/python/driver.py', 'src/python/protocol.rs',
                'src/python/project.rs', 'src/process.rs', 'src/capture.rs',
                'src/workspace.rs', 'src/command.rs', 'Cargo.lock']
scope = (repo / 'src/python/scope.py').read_text()
start = scope.index('    # Constructor-store deletion')
end = scope.index('    for node in ast.walk(tree):', start)
draft_scope = (scope[:start] + scope[end:]).replace(
    'if (node not in constructor_nodes and isinstance(node, ast.Assign)',
    'if (isinstance(node, ast.Assign)')
assert draft_scope.encode() == (tree / 'draft-scope.py').read_bytes()


def recipe(draft=False):
    return 'sha256:' + hashlib.sha256(b''.join(
        draft_scope.encode() if draft and p == 'src/python/scope.py'
        else (repo / p).read_bytes() for p in recipe_paths)).hexdigest()


def read_run(path, expected_recipe):
    run = json.loads((path / 'run.json').read_text())
    outcomes = json.loads((path / 'outcomes.json').read_text())
    inventory = json.loads((path / 'mutants.json').read_text())['mutations']
    assert run['python_scope']['recipe_fingerprint'] == expected_recipe
    assert run['schema_version'] == run['operator_catalog_version'] == 1
    assert all(m['operator'] == 'assignment_remove' and m['replacement'] == 'pass' for m in inventory)
    report_files = files(path)
    return {'directory': path.name, 'status': run['status'], 'score': run['score'],
            'baseline_tests': len(run['python_scope']['test_nodes']),
            'duration_seconds': run['duration_seconds'], 'summary': outcomes['summary'],
            'inventory': inventory,
            'outcomes': [{k: m[k] for k in ['id', 'state', 'outcome', 'duration_seconds']}
                         for m in outcomes['mutations']],
            'mapping': run.get('rerun', {}).get('mutations', []),
            'file_count': len(report_files),
            'report_digest': hashlib.sha256(json.dumps(report_files, sort_keys=True).encode()).hexdigest()}


records = {}
for label, root, commit, expected, test_count in [
    ('tree', tree, '68d688fb4fe9c867c77a9d54f4fdb46a58dea34e',
     {'killed': 16, 'missed': 7, 'timeout': 6, 'error': 3}, 7),
    ('route', route, '35a3db157731ba0073d66f432b69b997ce2e9f01', {'error': 16}, 68),
]:
    original = Path(tomllib.loads((root / 'original.toml').read_text())['python']['path'])
    preservation = json.loads((root / 'preservation.json').read_text())
    assert preservation['commit'] == commit
    assert subprocess.check_output(['git', '-C', str(original), 'rev-parse', 'HEAD']).decode().strip() == commit
    assert not subprocess.check_output(['git', '-C', str(original), 'status', '--porcelain'])
    assert files(original) == preservation['inputs']
    preserved = {}
    for ledger in ['draft-report-hashes.json', 'final-original-report-hashes.json']:
        hashes = json.loads((root / ledger).read_text())
        for name, sha in hashes.items():
            assert digest(root / name) == sha, name
        preserved[ledger] = len(hashes)
    original_run, = (root / 'original-reports').iterdir()
    draft = read_run(original_run, recipe(True))
    assert draft['status'] == 'failed' and draft['score']['percentage'] is None
    trials = [draft] + [read_run(p, recipe(True)) for p in sorted((root / 'focused-reports').iterdir())]
    results = {}
    for trial in trials:
        assert trial['baseline_tests'] == test_count
        mapping = {m['id']: m['source_id'] for m in trial['mapping']}
        for outcome in trial['outcomes']:
            if outcome['outcome']:
                source_id = mapping.get(outcome['id'], outcome['id'])
                assert source_id not in results
                results[source_id] = outcome['outcome']
    assert set(results) == {m['id'] for m in draft['inventory']}
    assert dict(collections.Counter(results.values())) == expected
    final, = (root / 'final-reports').iterdir()
    final = read_run(final, recipe())
    assert final['baseline_tests'] == test_count
    if label == 'route':
        assert final['status'] == 'completed' and final['inventory'] == []
    else:
        assert final['status'] == 'failed' and final['score']['percentage'] is None
        assert final['inventory'] == draft['inventory']
    records[label] = {'commit': commit, 'inputs': preservation['inputs'],
                      'draft_inventory': draft['inventory'], 'draft_combined_counts': expected,
                      'draft_results': results,
                      'draft_reports': [{k: v for k, v in t.items() if k not in ['inventory', 'score']} for t in trials],
                      'final_original': final, 'preserved_report_files': preserved}

extra = 'tests/test_rotation_assignments.py'
stronger_files = files(tree / 'stronger')
assert set(stronger_files) == set(records['tree']['inputs']) | {extra}
assert all(stronger_files[p] == h for p, h in records['tree']['inputs'].items())
assert stronger_files[extra] == digest(Path(__file__).parent / 'test_rotation_assignments.py')
for group, expected, count in [('final-survivors', 'missed', 7), ('final-stronger', 'killed', 9)]:
    path, = (tree / group).iterdir()
    run = read_run(path, recipe())
    assert run['status'] == 'completed' and run['baseline_tests'] == count
    assert run['summary'] == {'killed': 2 if expected == 'killed' else 0,
                              'missed': 2 if expected == 'missed' else 0,
                              'errors': 0, 'timeout': 0, 'build_failed': 0}
    assert run['mapping'] == [{'id': 1, 'source_id': 8}, {'id': 2, 'source_id': 10}]
    fields = ['file', 'line', 'column', 'byte_start', 'byte_end', 'source_fingerprint', 'original', 'replacement', 'operator']
    identity = lambda m: tuple(m[k] for k in fields)
    assert {identity(m) for m in run['inventory']} == {
        identity(m) for m in records['tree']['draft_inventory'] if m['id'] in [8, 10]}
    records['tree'][group] = run

result = {'scope': 'Python single-target attribute/item Assign; exclude constructor subtrees',
          'recipe': recipe(), 'draft_recipe': recipe(True), 'draft_scope_sha256': digest(tree / 'draft-scope.py'),
          'python': '3.14.7', 'pytest': '9.0.2', 'platform': 'macOS',
          'timeout_seconds': 10, 'baseline_timeout_seconds': 120,
          'jobs_per_run': 1, 'max_concurrent_audit_runs': 2, 'records': records,
          'stronger_test_sha256': stronger_files[extra], 'preservation_verified': True}
output.write_text(json.dumps(result, indent=2) + '\n')
print('Verified 48 draft outcomes, final constructor exclusion, two final stronger-test kills and all recorded input/report hashes')
