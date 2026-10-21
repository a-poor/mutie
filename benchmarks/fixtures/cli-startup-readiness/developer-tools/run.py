"""Run unchanged matching workloads after the human confirms a permission phase."""
import argparse
import concurrent.futures
import hashlib
import json
from pathlib import Path
import re
import subprocess
import time

parser = argparse.ArgumentParser()
parser.add_argument('phase', choices=['enabled', 'restored'])
args = parser.parse_args()
root = Path('/tmp/mutie-developer-tools-experiment') / args.phase
root.mkdir(exist_ok=False)
inputs = ['target/debug/mutie', 'tests/cancellation.rs', 'tests/log_limits.rs',
          'tests/support/startup_diagnostics.rs']
def hashes():
    return {p: hashlib.sha256(Path(p).read_bytes()).hexdigest() for p in inputs}
before = hashes()
(root/'before.json').write_text(json.dumps(before, indent=2)+'\n')

def run_series(kind, count, command):
    rows = []
    for index in range(1, count+1):
        start = time.monotonic()
        path = root/f'{kind}-{index}.log'
        with path.open('w') as log:
            result = subprocess.run(command, stdout=log, stderr=subprocess.STDOUT)
        results = re.findall(r'test result: (\w+)\. (\d+) passed; (\d+) failed; (\d+) ignored', path.read_text())
        row = dict(round=index, seconds=round(time.monotonic()-start,3),
                   exit_code=result.returncode, targets=len(results),
                   passed=sum(int(r[1]) for r in results),
                   failed=sum(int(r[2]) for r in results),
                   ignored=sum(int(r[3]) for r in results))
        rows.append(row)
        (root/f'{kind}-results.json').write_text(json.dumps(rows,indent=2)+'\n')
        print(kind, row, flush=True)
        if result.returncode:
            break
    return rows

with concurrent.futures.ThreadPoolExecutor(max_workers=2) as pool:
    full = pool.submit(run_series, 'full', 3, ['cargo','test','--offline'])
    focused = pool.submit(run_series, 'focused', 10,
                          ['cargo','test','--offline','--test','cancellation','--test','log_limits'])
    result = dict(phase=args.phase, full=full.result(), focused=focused.result())
result['unchanged_inputs'] = before == hashes()
result['after'] = hashes()
(root/'summary.json').write_text(json.dumps(result,indent=2)+'\n')
print('Finished phase:',args.phase,'unchanged inputs:',result['unchanged_inputs'],flush=True)
