"""Prepare pinned Scrubify with unchanged production and complete test bytes.

Usage: python prepare.py PRISTINE_CHECKOUT NEW_DESTINATION
Only the src layout and packaging/test configuration are adapted.
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
assert commit == "558265805d942c03d2e6d8522baa0ca2992ec067", commit
assert not subprocess.check_output(
    ["git", "-C", str(original), "status", "--porcelain"], text=True
).strip(), "upstream checkout must be pristine"
assert not destination.exists(), "destination must be new"
destination.mkdir()
shutil.copytree(original / "src/scrubify", destination / "scrubify")
shutil.copytree(original / "tests", destination / "tests")
for name in ("LICENSE", "README.md"):
    shutil.copyfile(original / name, destination / name)
(destination / "pyproject.toml").write_text(
    '[project]\nname = "scrubify"\ndependencies = []\n'
    '[tool.pytest.ini_options]\ntestpaths = ["tests"]\n'
)
inventory = []
for path in sorted(destination.rglob("*")):
    if path.is_file():
        inventory.append({"path": path.relative_to(destination).as_posix(),
                          "sha256": hashlib.sha256(path.read_bytes()).hexdigest()})
(destination / "acceptance-inputs.json").write_text(json.dumps(
    {"repository": "https://github.com/MartiAble/scrubify-py", "commit": commit,
     "files": inventory}, indent=2) + "\n")
print(destination)
