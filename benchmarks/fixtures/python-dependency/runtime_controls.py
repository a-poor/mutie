"""Reject drift in a disposable copy of the audited dependency environment."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile

parser = argparse.ArgumentParser()
parser.add_argument('--environment', type=Path, required=True)
parser.add_argument('--output', type=Path, required=True)
args = parser.parse_args()
args.output.mkdir(parents=True, exist_ok=False)
source = Path(__file__).resolve().parents[3] / 'src/python/runtime.py'
script = args.output / 'runtime.py'
shutil.copy2(source, script)
env = args.output / 'env'
shutil.copytree(args.environment, env, symlinks=True)
site = env / 'lib/python3.14/site-packages'
package = site / 'webencodings'
dist = site / 'webencodings-0.5.1.dist-info'
records = []


def check(label, passing):
    with tempfile.TemporaryDirectory(dir=args.output) as cache:
        result = subprocess.run([str(env/'bin/python'), '-I', '-S', '-B', '-X',
                                 'pycache_prefix='+cache, str(script)], capture_output=True,
                                text=True, timeout=60)
    (args.output/(label+'.stdout')).write_text(result.stdout)
    (args.output/(label+'.stderr')).write_text(result.stderr)
    assert (result.returncode == 0) == passing, (label, result.stderr)
    records.append(dict(label=label, code=result.returncode, accepted=passing))


check('original', True)
for label, path, transform in [
    ('backdated-bytes', package/'__init__.py', lambda b:b.replace(b'CACHE = {}', b'CACHE = []')),
    ('wrong-version', dist/'METADATA', lambda b:b.replace(b'Version: 0.5.1', b'Version: 0.5.2')),
    ('transitive', dist/'METADATA', lambda b:b.replace(b'Name: webencodings', b'Name: webencodings\nRequires-Dist: other')),
    ('native-wheel', dist/'WHEEL', lambda b:b.replace(b'Root-Is-Purelib: true', b'Root-Is-Purelib: false')),
]:
    before, stamp = path.read_bytes(), path.stat()
    after = transform(before)
    assert before != after
    if label == 'backdated-bytes': assert len(before) == len(after)
    path.write_bytes(after)
    os.utime(path, ns=(stamp.st_atime_ns, stamp.st_mtime_ns))
    check(label, False)
    path.write_bytes(before)
    os.utime(path, ns=(stamp.st_atime_ns, stamp.st_mtime_ns))
for label, path, data in [
    ('plugin', dist/'entry_points.txt', b'[pytest11]\nbad=webencodings.tests\n'),
    ('extra-module', package/'extra.py', b'raise RuntimeError("must never execute")\n'),
    ('native-extension', package/'native.so', b'not a native library'),
    ('startup-hook', site/'startup.pth', b'import must_never_execute\n'),
]:
    assert not path.exists()
    path.write_bytes(data)
    check(label, False)
    path.unlink()
package.rename(site/'held-package')
check('missing-package', False)
package.symlink_to(site/'held-package', target_is_directory=True)
check('symlink-package', False)
package.unlink()
(site/'held-package').rename(package)
check('restored', True)
(args.output/'results.json').write_text(json.dumps(dict(
    runtime_sha256=hashlib.sha256(script.read_bytes()).hexdigest(), records=records), indent=2)+'\n')
print(len(records), 'runtime controls passed')
