"""Prepare a scoped copy without dropping upstream production code or tests.

Usage: python prepare.py UPSTREAM_CHECKOUT NEW_DESTINATION
The upstream checkout is never modified. No upstream code is executed here.
"""
import ast
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
assert commit == "4b625748db63f7f20353b5b0815e42122d739314", commit
assert not subprocess.check_output(
    ["git", "-C", str(original), "status", "--porcelain"], text=True
).strip(), "upstream checkout must be pristine"
assert not destination.exists(), "destination must be new"
destination.mkdir()
(destination / "caseconverter").mkdir()
(destination / "tests").mkdir()
exports = []
for node in ast.parse((original / "caseconverter/__init__.py").read_text()).body:
    assert isinstance(node, ast.ImportFrom)
    exports.extend(alias.asname or alias.name for alias in node.names)
inventory = []
for source in sorted((original / "caseconverter").glob("*.py")):
    data = source.read_bytes()
    adapted = data
    if source.name.endswith("_test.py"):
        target = destination / "tests" / ("test_" + source.name.removesuffix("_test.py") + ".py")
        adapted = data.replace(b"from . import *", ("from caseconverter import " + ", ".join(exports)).encode())
        adapted = adapted.replace(b"from . import ", b"from caseconverter import ")
    else:
        target = destination / "caseconverter" / source.name
    target.write_bytes(adapted)
    inventory.append({"original": source.relative_to(original).as_posix(),
                      "adapted": target.relative_to(destination).as_posix(),
                      "original_sha256": hashlib.sha256(data).hexdigest(),
                      "adapted_sha256": hashlib.sha256(adapted).hexdigest()})
for name in ("LICENSE", "README.md"):
    shutil.copyfile(original / name, destination / name)
(destination / "pyproject.toml").write_text(
    '[project]\nname = "case-converter"\ndependencies = []\n'
    '[tool.pytest.ini_options]\ntestpaths = ["tests"]\n'
)
(destination / "acceptance-inputs.json").write_text(json.dumps(
    {"repository": "https://github.com/chrisdoherty4/python-case-converter",
     "commit": commit, "files": inventory}, indent=2) + "\n")
print(json.dumps({"destination": str(destination), "files": len(inventory)}))
