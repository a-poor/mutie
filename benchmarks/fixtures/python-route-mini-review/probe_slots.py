"""Native diagnostic only: inspect slots effects outside Mutie's scored scope."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import tempfile

parser = argparse.ArgumentParser()
parser.add_argument('--previous', type=Path, required=True)
parser.add_argument('--output', type=Path, required=True)
args = parser.parse_args()
state = json.loads(args.previous.read_text())
root = Path(state['records'][0]['command'][2])
python = state['records'][0]['command'][4]
full = next(r for r in state['records'] if r['label'] == 'original-full')
ids = [2,5,8,17,19,26,28,30,32,34,36,38]
code = '''import json, sys
sys.path.insert(0, sys.argv[1] + '/src')
from route_mini.decision import AuditDecision
from route_mini.fallback import ErrorKind, FallbackDecision, ProviderError
from route_mini.providers import ModelCapabilities, ScriptedSuccess
from route_mini.telemetry import ProviderRollup
from route_mini.types import Cost, Decision, Model, Provider, Request, Response
objects = [AuditDecision('id', Model('p','m'), (), 'pending'), ProviderError(),
FallbackDecision(ErrorKind.UNKNOWN, False, True, 0), ModelCapabilities(Model('p','m'),10,.1,.1,10),
ScriptedSuccess(), ProviderRollup('p',1,1,1.,10,10,.1), Provider('p'), Cost(), Model('p','m'),
Request('prompt',10), Response('ok',Model('p','m'),1,1,10,Cost()), Decision('id',Model('p','m'),(),'pending')]
print(json.dumps([dict(name=type(o).__name__, slots=type(o).__dict__.get('__slots__'), instance_dict=hasattr(o,'__dict__')) for o in objects]))
'''
def hashes(directory):
    return {p.relative_to(directory).as_posix(): hashlib.sha256(p.read_bytes()).hexdigest()
            for p in sorted(directory.rglob('*')) if p.is_file() and '.git' not in p.relative_to(directory).parts}
assert hashes(root) == state['input_sha256']
results = []
with tempfile.TemporaryDirectory(prefix='mutie-route-slots-') as tmp:
    project = Path(tmp) / 'project'
    shutil.copytree(root, project, ignore=shutil.ignore_patterns('.git'))
    def inspect():
        return json.loads(subprocess.check_output([python, '-I', '-B', '-c', code, str(project)], text=True, timeout=30))
    baseline = inspect()
    for index, identifier in enumerate(ids):
        mutation = next(m for m in full['inventory']['mutations'] if m['id'] == identifier)
        path = project / mutation['file']
        before = path.read_bytes()
        a, b = mutation['byte_start'], mutation['byte_end']
        assert before[a:b].decode() == mutation['original']
        try:
            path.write_bytes(before[:a] + mutation['replacement'].encode() + before[b:])
            changed = inspect()
            assert all(row == baseline[i] for i, row in enumerate(changed) if i != index)
            assert baseline[index]['slots'] and changed[index]['slots'] is None
            results.append(dict(original_id=identifier, baseline=baseline[index], mutation=changed[index]))
        finally:
            path.write_bytes(before)
    assert hashes(project) == state['input_sha256']
assert hashes(root) == state['input_sha256']
args.output.write_text(json.dumps(dict(native_only=True, original_preserved=True, results=results), indent=2) + '\n')
