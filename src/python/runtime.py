"""Identify a pinned, non-editable environment with site initialization disabled."""
import hashlib
import importlib.metadata
import json
import os
from pathlib import Path
import sys
import sysconfig

PINNED = {
    "pytest": "9.0.2", "pluggy": "1.7.0", "packaging": "26.3",
    "iniconfig": "2.3.1", "pygments": "2.21.0",
}

# Audited pure-Python wheel webencodings-0.5.1-py2.py3-none-any.whl.
# Application imports are separately constrained by scope.py; this is not a
# resolver or permission to load arbitrary installed dependencies.
WEBENCODINGS = "sha256:c1e7f05855fc3004d21de68f4a04d7e9d88ba13544f02f705f9af26a1cb48026"


def require(condition, message):
    if not condition:
        raise ValueError(message)


def fingerprint_tree(root, *, site=False):
    # Keep the original component-wise Path ordering and byte framing. Scandir
    # reuses directory-entry metadata and prunes excluded cache/site subtrees;
    # every included file is still opened and hashed on every identity check.
    files = []

    def visit(directory, parents=()):
        with os.scandir(directory) as entries:
            for entry in entries:
                parts = parents + (entry.name,)
                if entry.name == "__pycache__" or (not site and entry.name == "site-packages"):
                    continue
                ignored = Path(entry.name).suffix in {".pyc", ".pyo"}
                if not ignored:
                    require(not entry.is_symlink() or (not site and entry.is_file()),
                            f"Python runtime directory/package symlink unsupported: {os.path.join(*parts)}")
                if entry.is_dir(follow_symlinks=False):
                    visit(entry.path, parts)
                elif not ignored:
                    require(entry.is_file(), "Python runtime requires regular files")
                    if site:
                        require(Path(entry.name).suffix not in {".pth", ".egg-link"}
                                and entry.name not in {"sitecustomize.py", "usercustomize.py"},
                                "Python startup hooks and editable installs unsupported")
                    files.append((parts, entry.path))

    visit(root)
    digest = hashlib.sha256()
    for parts, filename in sorted(files):
        name = os.path.join(*parts).encode()
        digest.update(len(name).to_bytes(8, "big"))
        digest.update(name)
        with open(filename, "rb") as stream:
            content = stream.read()
        digest.update(len(content).to_bytes(8, "big"))
        digest.update(content)
    return "sha256:" + digest.hexdigest()


def validate_application_dependency(site, distribution):
    require((site / "webencodings").is_dir() and not (site / "webencodings").is_symlink(),
            "Python dependency must be an owned package directory")
    require(distribution.version == "0.5.1", "Python requires webencodings 0.5.1")
    require(not distribution.requires and not distribution.entry_points,
            "Python dependency transitives and entry points unsupported")
    require("Root-Is-Purelib: true" in (distribution.read_text("WHEEL") or ""),
            "Python dependency must be a pure-Python wheel")
    require(fingerprint_tree(site / "webencodings", site=True) == WEBENCODINGS,
            "Python webencodings bytes differ from the audited dependency")


def identify():
    require(sys.implementation.name == "cpython" and sys.version_info[:3] == (3, 14, 7),
            "Python requires CPython 3.14.7")
    require(sys.flags.no_site and sys.flags.isolated, "Python identity requires -I -S")
    prefix = Path(sys.prefix)
    require(sys.prefix != sys.base_prefix and (prefix / "pyvenv.cfg").is_file(),
            "Python requires a dedicated virtual environment")
    config = (prefix / "pyvenv.cfg").read_text()
    require("include-system-site-packages = false" in config,
            "Python system-site packages unsupported")
    site = Path(sysconfig.get_path("purelib"))
    require(site.is_relative_to(prefix), "Python site-packages escaped environment")
    versions = {}
    for distribution in importlib.metadata.distributions(path=[str(site)]):
        name = distribution.metadata["Name"].lower().replace("_", "-")
        require(name not in versions, "Python duplicate installed distribution")
        versions[name] = distribution.version
        if name == "webencodings":
            validate_application_dependency(site, distribution)
    require(all(versions.get(k) == v for k, v in PINNED.items()),
            "Python requires the pinned pytest environment from the fixture requirements")
    require(set(versions) <= set(PINNED) | {"pip", "webencodings"}, "Python extra installed packages unsupported")
    stdlib = Path(sysconfig.get_path("stdlib"))
    executable = Path(sys.executable)
    library_root = (sysconfig.get_config_var("PYTHONFRAMEWORKPREFIX")
                    if sysconfig.get_config_var("PYTHONFRAMEWORK")
                    else sysconfig.get_config_var("LIBDIR"))
    library = Path(library_root) / sysconfig.get_config_var("LDLIBRARY")
    require(library.is_file(), "Python shared runtime library not found")
    return dict(
        executable=str(executable), python=sys.version, prefix=str(prefix),
        versions=versions, environment="sha256:" + hashlib.sha256(config.encode()).hexdigest(),
        executable_fingerprint="sha256:" + hashlib.sha256(executable.read_bytes()).hexdigest(),
        library_fingerprint="sha256:" + hashlib.sha256(library.read_bytes()).hexdigest(),
        stdlib=fingerprint_tree(stdlib), packages=fingerprint_tree(site, site=True),
    )


if __name__ == "__main__":
    print(json.dumps(identify()))
