"""Build and verify one owned pure-Python wheel with the pinned Flit backend.

No resolver, network, editable install, project hook or dependency installation.
The caller validates the unchanged inputs and the backend before and afterwards.
"""
import base64
import csv
import hashlib
import io
import json
import logging
import os
from pathlib import Path, PurePosixPath
import resource
import shutil
import sys
import sysconfig
import tomllib
import zipfile
import venv


def require(condition, message):
    if not condition:
        raise ValueError(message)


root, destination, package, source_root = sys.argv[1:]
root, destination = Path(root).resolve(strict=True), Path(destination).resolve()
require(not destination.exists() and not destination.is_relative_to(root),
        "installation must be a fresh owned directory outside project inputs")
config = tomllib.loads((root / "pyproject.toml").read_text())
project = config["project"]
require(not set(project) & {"dynamic", "dependencies", "optional-dependencies", "entry-points", "scripts", "gui-scripts"}
        and all(isinstance(project.get(k), str) and project[k] for k in ("name", "version", "description")),
        "installation requires static dependency-free metadata")
require(config["build-system"] == {"requires": ["flit_core>=3.11,<4"], "build-backend": "flit_core.buildapi"}
        and config["tool"]["flit"]["module"] == {"name": package}
        and package.isidentifier() and source_root in ("", "src"), "unsupported installation recipe")
resource.setrlimit(resource.RLIMIT_FSIZE, (16 << 20, 16 << 20))
destination.mkdir()
wheel_directory = destination / "wheel"
wheel_directory.mkdir()
sys.path.append(sysconfig.get_path("purelib"))
import flit_core
from flit_core import buildapi
from packaging.version import Version
require(flit_core.__version__ == "3.12.0", "installation requires flit-core 3.12.0")
os.chdir(root)
build_messages = []


class BuildLog(logging.Handler):
    def emit(self, record):
        if record.levelno >= logging.WARNING:
            build_messages.append((record.name, record.getMessage()))


logging.getLogger().addHandler(BuildLog())


def no_application_import(event, args):
    if event == "import" and (args[0] == package or args[0].startswith(package + ".")):
        raise RuntimeError("wheel build attempted to import application code")


sys.addaudithook(no_application_import)
wheel = wheel_directory / buildapi.build_wheel(str(wheel_directory))
normalized = str(Version(project["version"]))
expected_messages = [] if normalized == project["version"] else [(
    "flit_core.versionno", f"Version number normalised: {project['version']!r} -> {normalized!r} (see PEP 440)")]
require(build_messages == expected_messages, "unexpected Flit build diagnostics")
require(wheel.parent == wheel_directory and wheel.name.endswith(("-py3-none-any.whl", "-py2.py3-none-any.whl")),
        "installation requires a pure-Python wheel")
environment = destination / "venv"
venv.EnvBuilder(with_pip=False, symlinks=True).create(environment)
site = environment / "lib" / f"python{sys.version_info.major}.{sys.version_info.minor}" / "site-packages"
require(site.is_dir(), "owned virtual environment omitted site-packages")
# Child CLI probes use sys.executable. Give them the same verified installation,
# without PYTHONPATH, startup hooks, editable installs or host-site inheritance.
shutil.copytree(sysconfig.get_path("purelib"), site, dirs_exist_ok=True,
                ignore=shutil.ignore_patterns("__pycache__", "*.pyc", "*.pyo"))
hashes = {}
with zipfile.ZipFile(wheel) as archive:
    entries = archive.infolist()
    require(len(entries) < 10000 and sum(i.file_size for i in entries) <= 64 << 20,
            "wheel exceeds input budget")
    names = [i.filename for i in entries]
    require(len(set(names)) == len(names), "duplicate wheel entry")
    metadata = {PurePosixPath(n).parts[0] for n in names if PurePosixPath(n).parts[0].endswith(".dist-info")}
    require(len(metadata) == 1, "wheel requires one distribution metadata directory")
    dist = metadata.pop()
    records = list(csv.reader(io.StringIO(archive.read(dist + "/RECORD").decode())))
    require(len(records) == len(names) and {r[0] for r in records} == set(names), "wheel RECORD incomplete")
    record = {r[0]: r[1:] for r in records}
    require("Root-Is-Purelib: true" in archive.read(dist + "/WHEEL").decode(), "wheel is not pure Python")
    for entry in entries:
        name = PurePosixPath(entry.filename)
        require(not name.is_absolute() and all(p not in (".", "..", "") for p in name.parts)
                and "\\" not in entry.filename and not entry.is_dir()
                and (entry.external_attr >> 16) & 0o170000 in (0, 0o100000)
                and name.parts[0] in {package, dist}, "unsafe wheel member")
        require(name.suffix not in {".pth", ".so", ".dylib", ".pyd", ".pyc"}
                and name.name not in {"entry_points.txt", "sitecustomize.py", "usercustomize.py"},
                "wheel hooks/extensions unsupported")
        data = archive.read(entry)
        require(len(data) == entry.file_size and len(data) <= 8 << 20, "wheel entry exceeds budget")
        if entry.filename != dist + "/RECORD":
            digest = base64.urlsafe_b64encode(hashlib.sha256(data).digest()).rstrip(b"=").decode()
            require(record[entry.filename] == ["sha256=" + digest, str(len(data))], "wheel RECORD hash mismatch")
        if name.parts[0] == package:
            require((root / source_root / name).read_bytes() == data, "wheel changed source/package bytes")
        output = site / name
        require(not output.exists(), "wheel member collides with the pinned runtime")
        output.parent.mkdir(parents=True, exist_ok=True)
        output.write_bytes(data)
        hashes[entry.filename] = "sha256:" + hashlib.sha256(data).hexdigest()
    source_files = {p.relative_to(root / source_root).as_posix()
                    for p in (root / source_root / package).rglob("*") if p.is_file()
                    and "__pycache__" not in p.parts and p.suffix not in {".pyc", ".pyo"}}
    require(source_files == {n for n in names if n.startswith(package + "/")},
            "installed package inventory differs from source")
hashes = {p.relative_to(site).as_posix(): "sha256:" + hashlib.sha256(p.read_bytes()).hexdigest()
          for p in site.rglob("*") if p.is_file()}
receipt = dict(site=str(site), executable=str(environment / "bin/python"), package=package,
               environment_fingerprint="sha256:" + hashlib.sha256((environment / "pyvenv.cfg").read_bytes()).hexdigest(),
               files=hashes, build_diagnostics=build_messages)
(destination / "receipt.json").write_text(json.dumps(receipt, sort_keys=True))
print(json.dumps(receipt))
