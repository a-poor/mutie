"""Compare prebuilt adapters on fresh Scrubify full/stronger-test rerun pairs."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import time

parser = argparse.ArgumentParser()
parser.add_argument('--project', type=Path, required=True)
parser.add_argument('--before', type=Path, required=True)
parser.add_argument('--after', type=Path, required=True)
parser.add_argument('--python', type=Path, required=True)
parser.add_argument('--output', type=Path, required=True)
args = parser.parse_args()
output = args.output.resolve()
output.mkdir(parents=True, exist_ok=False)
stronger = Path(__file__).resolve().parents[1] / 'python-scrubify/test_blank_queries.py'


def hashes(root):
    return {str(p.relative_to(root)): hashlib.sha256(p.read_bytes()).hexdigest()
            for p in sorted(root.rglob('*')) if p.is_file()}


source = hashes(args.project)
assert 'tests/test_blank_queries.py' not in source
records = []
for repeat in range(1, 4):
    for mode in (['before', 'after'] if repeat % 2 else ['after', 'before']):
        root = output / f'{mode}-{repeat}'
        project = root / 'project'
        shutil.copytree(args.project, project)
        original = hashes(project)
        saved = None
        for workload in ['full', 'rerun']:
            if workload == 'rerun':
                shutil.copyfile(stronger, project / 'tests/test_blank_queries.py')
            expected = hashes(project)
            assert all(expected[k] == v for k, v in original.items())
            command = [str(getattr(args, mode).absolute()), str(project), '--lang', 'python',
                       '--python', str(args.python.absolute()), '--timeout', '10',
                       '--baseline-timeout', '20', '--max-capture-bytes', '4194304',
                       '--ui', 'plain', '--output-dir', str(root / 'reports')]
            if saved:
                command.extend(['--rerun-missed', str(saved)])
            start = time.perf_counter()
            result = subprocess.run(command, capture_output=True, text=True, timeout=300)
            seconds = time.perf_counter() - start
            (root / f'{workload}.log').write_text(result.stdout + result.stderr)
            assert result.returncode == 0, result.stdout + result.stderr
            report = Path(next(line.removeprefix('Saving report to ')
                               for line in result.stdout.splitlines() if line.startswith('Saving report to ')))
            run = json.loads((report / 'run.json').read_text())
            outcomes = json.loads((report / 'outcomes.json').read_text())
            inventory = json.loads((report / 'mutants.json').read_text())
            assert run['status'] == 'completed'
            assert hashes(project) == expected
            if saved:
                assert hashes(saved) == saved_hashes
            else:
                saved, saved_hashes = report, hashes(report)
            summary = outcomes['summary']
            assert summary == dict(killed=1, missed=2 if workload == 'full' else 1,
                                   timeout=0, errors=0, build_failed=0), summary
            scope = run['python_scope']
            record = dict(mode=mode, repeat=repeat, workload=workload, wall_seconds=seconds,
                          command=command, report=str(report), summary=summary,
                          scope=scope, mutations=inventory['mutations'],
                          outcomes=[(m['id'], m['outcome']) for m in outcomes['mutations']],
                          input_sha256=expected, report_sha256=hashes(report))
            prior = next((r for r in records if r['workload'] == workload), None)
            if prior:
                assert prior['mutations'] == record['mutations']
                assert prior['outcomes'] == record['outcomes']
                assert prior['scope']['runtime'] == scope['runtime']
                assert prior['scope']['test_nodes'] == scope['test_nodes']
            records.append(record)
            assert hashes(args.project) == source
            (output / 'results.json').write_text(json.dumps(dict(source_sha256=source, records=records), indent=2) + '\n')
            print(f'{mode} {repeat} {workload}: {seconds:.3f}s; {summary}', flush=True)
