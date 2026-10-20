"""Static scoped-project reader. Never imports or executes project code."""
import ast
import hashlib
import json
import io
import re
from pathlib import Path
import sys
import tomllib
import tokenize

STDLIB = {
    "collections", "dataclasses", "decimal", "enum", "fractions", "functools",
    "itertools", "math", "operator", "re", "statistics", "string", "typing",
    "unicodedata",
}
FORBIDDEN_NAMES = {
    "__import__", "eval", "exec", "compile", "globals", "locals", "vars",
    "getattr", "setattr", "delattr", "open", "breakpoint", "input",
    "exit", "quit", "__builtins__", "__loader__", "__spec__", "__path__", "__test__",
}
PYTEST_MEMBERS = {"fixture", "mark", "param", "raises", "fail", "approx"}
LOGGING_MEMBERS = {"getLogger", "StreamHandler", "Formatter", "DEBUG", "INFO",
                   "WARNING", "ERROR", "CRITICAL"}
FROM_MEMBERS = {
    "io": {"StringIO"},
    "fnmatch": {"fnmatch", "fnmatchcase"},
    "urllib.parse": {"parse_qsl", "urlencode", "urlsplit", "urlunsplit"},
}


def require(condition, message):
    if not condition:
        raise ValueError(message)


def module_exists(root, parts):
    path = root.joinpath(*parts)
    return path.with_suffix(".py").is_file() or (path / "__init__.py").is_file()


def inspect(root):
    root = root.resolve(strict=True)
    files = {}
    total = 0

    def walk(directory):
        nonlocal total
        for path in sorted(directory.iterdir()):
            relative = path.relative_to(root).as_posix()
            if path.parent == root and path.name in (".git", ".mutie"):
                continue
            require(not path.is_symlink(), f"Python symlink unsupported: {relative}")
            require(len(path.relative_to(root).parts) < 32, "Python tree is too deep")
            if path.is_dir():
                require(path.name not in (".venv", "venv", "node_modules"),
                        f"Python installed project environments unsupported: {relative}")
                walk(path)
            else:
                require(path.is_file(), f"Python input must be a regular file: {relative}")
                require(path.stat().st_size <= 8 * 1024 * 1024, f"Python input too large: {relative}")
                data = path.read_bytes()
                total += len(data)
                require(total <= 64 * 1024 * 1024 and len(files) < 10000,
                        "Python project exceeds scope limits")
                files[relative] = "sha256:" + hashlib.sha256(data).hexdigest()
    walk(root)
    require("pyproject.toml" in files, "Python requires pyproject.toml")
    config = tomllib.loads((root / "pyproject.toml").read_text(encoding="utf-8"))
    require(set(config) <= {"project", "build-system", "tool"}, "unsupported Python project table")
    project = config.get("project", {})
    require(isinstance(project.get("name"), str) and project["name"],
            "Python requires a named [project]")
    require(not project.get("dependencies"), "Python application dependencies unsupported")
    # The test extra is declarative only: the pinned environment is supplied by
    # the caller, never resolved or installed from this manifest.
    extras = project.get("optional-dependencies", {})
    # These optional tools are inert metadata, never installed or auto-loaded.
    optional_tools = {"docs": {"ipython", "matplotlib", "numpydoc", "sphinx"},
                      "tests": {"pytest", "pytest-cov", "pytest-xdist"}}
    audited_extras = (isinstance(extras, dict) and set(extras) <= optional_tools.keys()
                      and all(isinstance(items, list) and items
                              and all(isinstance(item, str) and item in optional_tools[group]
                                      for item in items) for group, items in extras.items()))
    require(audited_extras or (isinstance(extras, dict) and set(extras) <= {"test"}
            and all(isinstance(items, list) and items and all(
                isinstance(item, str) and re.fullmatch(r"pytest(?:>=[0-9]+(?:\.[0-9]+){0,2})?", item)
                and (">=" not in item or tuple(map(int, item.split(">=")[1].split("."))) <= (9, 0, 2))
                for item in items) for items in extras.values())),
            "Python only accepts audited declarative test/documentation extras")
    require(project.get("dynamic", []) in ([], ["version"]) and not project.get("entry-points"),
            "Python dynamic metadata and entry-point plugins unsupported")
    tool = config.get("tool", {})
    require(set(tool) <= {"pytest", "setuptools"}, "unsupported Python tool configuration")
    pytest_config = tool.get("pytest", {})
    require(set(pytest_config) <= {"ini_options"}, "unsupported pytest configuration")
    options = pytest_config.get("ini_options", {})
    require(set(options) <= {"testpaths", "pythonpath", "addopts"}
            and options.get("testpaths", ["tests"]) in (["tests"], ["."])
            and options.get("addopts", "") in ("", "-q"),
            "Python only accepts full tests with optional quiet output")
    # Build metadata is never executed. Admit only a conventional static backend.
    build = config.get("build-system")
    if build is not None:
        require(set(build) <= {"requires", "build-backend"}
                and build.get("build-backend") == "setuptools.build_meta"
                and isinstance(build.get("requires"), list)
                and all(isinstance(x, str) and
                        (x == "wheel" or re.fullmatch(r"setuptools(?:>=[0-9]+(?:\.[0-9]+)*)?", x))
                        for x in build["requires"]),
                "unsupported Python build metadata")
    flat = [p.name for p in root.iterdir()
            if p.is_dir() and (p / "__init__.py").is_file() and p.name != "tests"]
    src = root / "src"
    nested = []
    # A regular flat package may itself be named src. Its __init__.py keeps
    # the old root semantics; only a non-package src directory is an import root.
    if src.is_dir() and not (src / "__init__.py").is_file():
        nested = [p.name for p in src.iterdir()
                  if p.is_dir() and (p / "__init__.py").is_file()]
    require(len(flat) + len(nested) == 1,
            "Python requires exactly one flat or src importable package")
    source_root = "src" if nested else ""
    import_root = root / source_root
    package = (nested or flat)[0]
    require(package.isidentifier(), "Python package must be an identifier")
    require(options.get("pythonpath", [source_root] if source_root else [])
            == ([source_root] if source_root else []),
            "Python pythonpath must match the single audited source root")
    setuptools = tool.get("setuptools")
    static_version = None
    if project.get("dynamic"):
        require(isinstance(setuptools, dict), "Python version requires static setuptools metadata")
        require(setuptools.get("dynamic") == {"version": {"attr": package + ".__version__"}},
                "Python only accepts a literal package version")
        version_tree = ast.parse((import_root / package / "__init__.py").read_bytes())
        versions = [node for node in version_tree.body if isinstance(node, ast.Assign)
                    and any(isinstance(target, ast.Name) and target.id == "__version__"
                            for target in node.targets)]
        require(len(versions) == 1 and len(versions[0].targets) == 1
                and isinstance(versions[0].value, ast.Constant)
                and isinstance(versions[0].value.value, str),
                "Python package version must be a literal string")
        static_version = {"version": {"attr": package + ".__version__"}}
    if setuptools is not None:
        src_mapping = {"package-dir": {"": "src"}, "packages": {"find": {"where": ["src"]}}}
        flat_mapping = {"packages": [package], "package-data": {package: ["py.typed"]}}
        if static_version is not None:
            flat_mapping["dynamic"] = static_version
        require((source_root == "src" and setuptools == src_mapping)
                or (not source_root and setuptools == flat_mapping),
                "Python only accepts static setuptools package discovery")
    require(package not in sys.stdlib_module_names | {
        "pytest", "_pytest", "tests", "pluggy", "packaging", "iniconfig", "pygments", "pip"},
            "Python package shadows an admitted dependency")
    tests = []
    mutations = []
    sources = []
    for relative in files:
        path = Path(relative)
        require(path.name not in {"conftest.py", "pytest.ini", ".pytest.ini",
                                 "tox.ini", "setup.cfg", "setup.py", "sitecustomize.py",
                                 "usercustomize.py", "python._pth", "pyvenv.cfg"},
                f"unsupported Python configuration: {relative}")
        require(path.suffix not in {".pth", ".so", ".dylib", ".pyd"},
                f"Python plugins/extensions unsupported: {relative}")
        require(path.suffix != ".pyo" and
                (path.suffix != ".pyc" or "__pycache__" in path.parts),
                f"Python sourceless bytecode unsupported: {relative}")
        if path.suffix != ".py":
            continue
        parts = path.parts
        package_parts = (source_root, package) if source_root else (package,)
        in_package = parts[:len(package_parts)] == package_parts
        if not in_package and (relative == "doc/source/conf.py"
                               or (len(parts) == 2 and parts[0] == ".circleci")):
            # Documentation configuration and CI scripts are retained, never
            # imported or executed. They cannot conceal tests or pytest hooks.
            require(not path.name.startswith("test_") and not path.name.endswith("_test.py"),
                    "Python ancillary files cannot be tests")
            doc_tree = ast.parse((root / path).read_bytes())
            require(not any(
                (isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef, ast.ClassDef))
                 and (node.name.startswith(("test", "Test", "pytest_"))))
                or (isinstance(node, ast.Name) and node.id.startswith(("pytest", "test_", "Test")))
                or (isinstance(node, (ast.Import, ast.ImportFrom)) and any(
                    alias.name.split(".")[0] in {"pytest", "unittest"} for alias in node.names))
                or (isinstance(node, ast.ImportFrom) and (node.module or "").split(".")[0]
                    in {"pytest", "unittest"}) for node in ast.walk(doc_tree)),
                    "Python ancillary configuration cannot contain tests/hooks")
            continue
        is_test = parts[0] == "tests" or (len(parts) == 1 and path.name.startswith("test_"))
        require(not (is_test and len(parts) == 1 and options.get("testpaths") == ["tests"]),
                "Python root test is excluded by configured testpaths")
        require(in_package or is_test,
                f"Python executable file outside package/tests: {relative}")
        module_parts = parts[1:] if source_root and not is_test else parts
        require("__pycache__" not in parts and all(not p.startswith(".") for p in parts),
                f"hidden Python executable file: {relative}")
        require(not is_test or path.name.startswith("test_"),
                f"Python test files must use test_*.py: {relative}")
        if not is_test:
            require(not path.name.startswith("test_") and not path.name.endswith("_test.py"),
                    f"Python test outside tests/: {relative}")
            for count in range(len(package_parts), len(parts)):
                require((root.joinpath(*parts[:count]) / "__init__.py").is_file(),
                        "Python namespace packages unsupported")
            require(not path.with_suffix("").joinpath("__init__.py").as_posix() in files,
                    "Python ambiguous module/package paths unsupported")
            sources.append(relative)
        else:
            tests.append(relative)
        data = (root / path).read_bytes()
        encoding, _ = tokenize.detect_encoding(io.BytesIO(data).readline)
        require(encoding == "utf-8", "Python source requires UTF-8 encoding")
        text = data.decode("utf-8")
        tree = ast.parse(text, filename=relative)
        # A BOM/coding cookie must not change how byte spans map to source bytes.
        require(not text.startswith("\ufeff") and "\x00" not in text,
                "Python source requires plain UTF-8 without BOM/NUL")
        aliases = set()
        logging_aliases = set()
        copy_aliases = set()
        pytest_marks = {"mark"}
        for node in ast.walk(tree):
            if isinstance(node, ast.Import):
                for alias in node.names:
                    top = alias.name.split(".")[0]
                    require(top in STDLIB | {package, "logging", "copy"} | ({"pytest"} if is_test else set()),
                            f"unsupported Python import {alias.name} in {relative}")
                    if top == "logging":
                        require(alias.name == "logging", "Python logging submodules unsupported")
                        logging_aliases.add(alias.asname or "logging")
                    if top == "copy":
                        require(alias.name == "copy", "Python copy submodules unsupported")
                        copy_aliases.add(alias.asname or "copy")
                    require(top != package or module_exists(import_root, alias.name.split(".")),
                            f"unresolved local Python import: {alias.name}")
                    if top == "pytest":
                        require(alias.asname in (None, "pytest"), "pytest aliasing unsupported")
                    aliases.add(alias.asname or top)
            elif isinstance(node, ast.ImportFrom):
                if node.level:
                    require(not is_test and node.level < len(module_parts),
                            "Python relative import escapes package")
                    target = list(module_parts[:-node.level])
                    if node.module:
                        target += node.module.split(".")
                    require(target[0] == package and module_exists(import_root, target),
                            "unresolved relative Python import")
                    top = package
                else:
                    top = (node.module or "").split(".")[0]
                    require(top in STDLIB | {package} | ({"pytest"} if is_test else set())
                            or node.module in FROM_MEMBERS or node.module == "__future__",
                            f"unsupported Python import {node.module} in {relative}")
                    if node.module in FROM_MEMBERS:
                        require(all(a.name in FROM_MEMBERS[node.module] for a in node.names),
                                f"unsupported Python API from {node.module}")
                    if top == package:
                        require(module_exists(import_root, node.module.split(".")),
                                "unresolved local Python import")
                require(all(a.name != "*" for a in node.names), "Python wildcard imports unsupported")
                if top == "pytest":
                    require(all(a.name in PYTEST_MEMBERS for a in node.names),
                            "unsupported pytest import")
                    pytest_marks.update(a.asname or a.name for a in node.names if a.name == "mark")
                aliases.update(a.asname or a.name for a in node.names)
        annotation_nodes = set()
        input_parameters = set()
        def parameter_values(node):
            if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef, ast.ClassDef, ast.Lambda)):
                return
            if isinstance(node, ast.Name) and node.id == "input":
                input_parameters.add(node)
            for child in ast.iter_child_nodes(node):
                parameter_values(child)
        for node in ast.walk(tree):
            # `input` is a common parametrized-test argument, not necessarily
            # the interactive builtin. Admit its local value but never a call.
            if isinstance(node, ast.FunctionDef) and any(
                    arg.arg == "input" for arg in node.args.posonlyargs + node.args.args
                    + node.args.kwonlyargs):
                for statement in node.body:
                    parameter_values(statement)
            annotation = (node.annotation if isinstance(node, (ast.arg, ast.AnnAssign))
                          else node.returns if isinstance(node, ast.FunctionDef) else None)
            if annotation is not None:
                annotation_nodes.update(ast.walk(annotation))
        for node in ast.walk(tree):
            require(not isinstance(node, (ast.AsyncFunctionDef, ast.Await, ast.AsyncFor,
                                          ast.AsyncWith, ast.TypeAlias)),
                    f"Python async/type-alias execution unsupported: {relative}")
            if isinstance(node, ast.Name):
                require((node.id not in FORBIDDEN_NAMES or node in input_parameters)
                        and not node.id.startswith("pytest_"),
                        f"unsupported Python runtime access: {node.id}")
                require(not (isinstance(node.ctx, (ast.Store, ast.Del)) and node.id in aliases),
                        f"Python import rebinding unsupported: {node.id}")
            if isinstance(node, ast.Call) and isinstance(node.func, ast.Name):
                require(node.func.id != "input", "Python interactive input unsupported")
            if isinstance(node, ast.Attribute):
                super_init = (node.attr == "__init__" and isinstance(node.value, ast.Call)
                              and isinstance(node.value.func, ast.Name)
                              and node.value.func.id == "super")
                require(super_init or node.attr == "__getitem__"
                        or not (node.attr.startswith("__") and node.attr.endswith("__")),
                        f"Python reflective attribute unsupported: {node.attr}")
                if isinstance(node.value, ast.Name) and node.value.id in logging_aliases:
                    require(node.attr in LOGGING_MEMBERS, "unsupported Python logging API")
                if isinstance(node.value, ast.Name) and node.value.id in copy_aliases:
                    require(node.attr == "copy", "Python only admits copy.copy")
                if isinstance(node.value, ast.Name) and node.value.id == "pytest":
                    require(node.attr in PYTEST_MEMBERS, f"unsupported pytest API: {node.attr}")
                mark = (isinstance(node.value, ast.Attribute) and node.value.attr == "mark"
                        and isinstance(node.value.value, ast.Name) and node.value.value.id == "pytest")
                mark = mark or (isinstance(node.value, ast.Name) and node.value.id in pytest_marks)
                require(not mark or node.attr == "parametrize", "unsupported pytest mark")
            if isinstance(node, (ast.FunctionDef, ast.ClassDef)):
                require(not node.name.startswith("pytest_"), "Python pytest hooks unsupported")
                require(node.name not in aliases, "Python import rebinding unsupported")
            if isinstance(node, ast.arg):
                require(node.arg not in aliases, "Python import shadowing unsupported")
            if isinstance(node, ast.ExceptHandler):
                require(node.name not in aliases, "Python import rebinding unsupported")
            if (not is_test and node not in annotation_nodes
                    and isinstance(node, ast.Constant) and type(node.value) is bool):
                lines = data.splitlines(keepends=True)
                start = sum(map(len, lines[:node.lineno - 1])) + node.col_offset
                end = sum(map(len, lines[:node.end_lineno - 1])) + node.end_col_offset
                original = "True" if node.value else "False"
                require(data[start:end].decode() == original, "Python boolean span mismatch")
                mutations.append(dict(file=relative, start=start, end=end, line=node.lineno,
                                      column=node.col_offset + 1, end_line=node.end_lineno,
                                      end_column=node.end_col_offset + 1, original=original,
                                      replacement="False" if node.value else "True"))
    require(tests, "Python requires test_*.py files at the root or under tests/")
    return dict(package=package, source_root=source_root, files=files, tests=sorted(tests), sources=sorted(sources),
                mutations=sorted(mutations, key=lambda m: (m["file"], m["start"])))


if __name__ == "__main__":
    require(sys.version_info[:3] == (3, 14, 7), "Python requires CPython 3.14.7")
    print(json.dumps(inspect(Path(sys.argv[1]))))
