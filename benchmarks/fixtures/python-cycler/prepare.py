"""Copy pinned Cycler production and its complete test suite into scoped layout.

Usage: python prepare.py PRISTINE_CHECKOUT NEW_DESTINATION
No upstream code executes and no production/test bytes change.
"""
import hashlib
import json
from pathlib import Path
import shutil
import subprocess
import sys

original, destination = map(Path, sys.argv[1:])
commit = subprocess.check_output(
    ["git", "-C", str(original), "rev-parse", "HEAD"], text=True
).strip()
assert commit == "d1737eb0a42bd451e8770cef245ebe60ebb36bfc", commit
assert not subprocess.check_output(
    ["git", "-C", str(original), "status", "--porcelain"], text=True
).strip(), "upstream checkout must be pristine"
assert not destination.exists(), "destination must be new"
destination.mkdir()
shutil.copytree(original / "cycler", destination / "cycler")
(destination / "tests").mkdir()
shutil.copyfile(original / "test_cycler.py", destination / "tests/test_cycler.py")
for name in ("LICENSE", "README.rst"):
    shutil.copyfile(original / name, destination / name)
(destination / "pyproject.toml").write_text(
    '[project]\nname = "cycler"\ndependencies = []\n'
    '[tool.pytest.ini_options]\ntestpaths = ["tests"]\n'
)
inventory = []
for path in sorted(destination.rglob("*")):
    if path.is_file():
        inventory.append({"path": path.relative_to(destination).as_posix(),
                          "sha256": hashlib.sha256(path.read_bytes()).hexdigest()})
(destination / "acceptance-inputs.json").write_text(json.dumps(
    {"repository": "https://github.com/matplotlib/cycler", "commit": commit,
     "files": inventory}, indent=2) + "\n")
print(destination)
