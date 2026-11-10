"""Static admission for an installed, dependency-free Flit CLI package.

Concatenated before scope.py in the isolated validator. Discovery never builds
or imports application code. This scope uses the observed process/fixture/thread
ledger and owned filesystem boundary; it does not grant arbitrary pytest plugins.
"""


def installed_scope(root, files, directories, config):
    import shlex

    project = config.get("project", {})
    tool = config.get("tool", {})
    build = config.get("build-system", {})
    require(build == {"requires": ["flit_core>=3.11,<4"],
                      "build-backend": "flit_core.buildapi"},
            "installed Python requires the audited static Flit backend")
    require(set(project) <= {"name", "version", "description", "readme", "license", "license-files",
                            "maintainers", "authors", "classifiers", "requires-python", "urls"},
            "installed Python dependencies, plugins and dynamic metadata unsupported")
    require(all(isinstance(project.get(key), str) and project[key]
                for key in ("name", "version", "description")),
            "installed Python requires static name, version and description")
    require(re.fullmatch(r"[A-Za-z][A-Za-z0-9_-]*", project["name"]),
            "invalid installed distribution name")
    require("readme" not in project or isinstance(project["readme"], str)
            and project["readme"] in files and not Path(project["readme"]).is_absolute()
            and ".." not in Path(project["readme"]).parts,
            "installed Python readme must be an audited file")
    require("license" not in project or isinstance(project["license"], str),
            "installed Python requires a literal SPDX license")
    license_files = project.get("license-files", [])
    require(isinstance(license_files, list) and all(isinstance(p, str) and p
            and not Path(p).is_absolute() and ".." not in Path(p).parts
            and "\\" not in p for p in license_files), "installed Python license paths must remain owned")
    flit = tool.get("flit", {})
    require(set(flit) <= {"module", "sdist"} and set(flit.get("module", {})) == {"name"},
            "installed Python requires one explicit Flit module")
    package = flit["module"]["name"]
    require(isinstance(package, str) and package.isidentifier(), "invalid installed package")
    reserved = {"pytest", "_pytest", "pluggy", "packaging", "iniconfig", "pygments", "flit_core", "pip"}
    require(package not in sys.stdlib_module_names | reserved
            and project["name"].lower().replace('-', '_') not in reserved,
            "installed Python package/distribution collides with the pinned runtime")
    require(set(flit.get("sdist", {})) <= {"include", "exclude"}
            and all(isinstance(v, list) and all(isinstance(p, str) for p in v)
                    for v in flit.get("sdist", {}).values()), "invalid inert Flit sdist metadata")
    roots = [prefix for prefix in ("", "src")
             if ((root / prefix / package) / "__init__.py").is_file()]
    require(len(roots) == 1, "installed Python requires one flat or src package")
    source_root = roots[0]
    package_dir = (source_root + "/" if source_root else "") + package
    require(set(tool) <= {"flit", "pytest", "uv", "coverage", "mypy", "pyright", "ruff", "tox"},
            "unsupported installed Python tool metadata")
    # These tables never select, install or execute their commands. Retain every
    # byte in execution identity, including test/development dependency groups.
    require(all(isinstance(table, dict) for table in tool.values()), "invalid Python tool table")
    groups = config.get("dependency-groups", {})
    require(isinstance(groups, dict) and all(isinstance(k, str) and isinstance(v, list)
            and all(isinstance(item, str) for item in v) for k, v in groups.items()),
            "installed Python groups must be inert dependency strings")
    pytest_config = tool.get("pytest", {})
    require(set(pytest_config) <= {"ini_options"}, "unsupported pytest configuration")
    options = pytest_config.get("ini_options", {})
    require(set(options) <= {"testpaths", "filterwarnings", "markers", "addopts"}
            and options.get("testpaths", ["tests"]) == ["tests"]
            and options.get("filterwarnings", []) in ([], ["error"]),
            "installed Python requires the complete tests directory and strict warnings")
    marks = options.get("markers", [])
    require(isinstance(marks, list) and all(isinstance(m, str) and
            re.fullmatch(r"[a-z][a-z_]*: [^\n]+", m) for m in marks), "invalid pytest marker declarations")
    declared = {m.split(":", 1)[0] for m in marks}
    args = shlex.split(options.get("addopts", ""))
    require(not args or (len(args) == 2 and args[0] == "-m"
            and re.fullmatch(r"not [a-z][a-z_]*", args[1])
            and args[1][4:] in declared), "unsupported pytest selection options")

    sources, tests, conftests, ancillary = [], [], [], []
    trees = {}
    for relative in files:
        path = Path(relative)
        require(path.name not in {"setup.py", "setup.cfg", "pytest.ini", "pytest.toml", "tox.ini",
                                 "sitecustomize.py", "usercustomize.py", "pyvenv.cfg"}
                and path.suffix not in {".pth", ".so", ".dylib", ".pyd", ".pyo"},
                f"unsupported installed Python executable configuration: {relative}")
        if path.suffix != ".py":
            require(path.suffix != ".pyc" or "__pycache__" in path.parts,
                    "Python sourceless bytecode unsupported")
            continue
        require(not any(p.startswith(".") or p == "__pycache__" for p in path.parts),
                f"hidden Python executable: {relative}")
        data = (root / relative).read_bytes()
        require(tokenize.detect_encoding(io.BytesIO(data).readline)[0] == "utf-8"
                and not data.startswith(b"\xef\xbb\xbf") and b"\0" not in data,
                "Python source requires plain UTF-8")
        tree = ast.parse(data, filename=relative)
        trees[relative] = tree
        if path.is_relative_to(package_dir):
            require(not path.name.startswith("test_") and not path.name.endswith("_test.py"),
                    "Python tests outside tests directory")
            for directory in path.parents:
                if directory == Path(package_dir).parent:
                    break
                require((directory / "__init__.py").as_posix() in files,
                        "Python namespace packages unsupported")
            sources.append(relative)
        elif relative in {"conftest.py", "tests/conftest.py"}:
            validate_conftest(tree)
            conftests.append(relative)
        elif path.parts[0] == "tests" and path.name == "__init__.py":
            require(all(isinstance(n, ast.Expr) and isinstance(n.value, ast.Constant)
                        and isinstance(n.value.value, str) for n in tree.body),
                    "Python test initializers must be empty/docstrings only")
            ancillary.append(relative)
        elif path.parts[0] == "tests" and path.name.startswith("test_"):
            tests.append(relative)
        elif path.parts[0] in {"docs", "examples"} or path.is_relative_to("tests/typing"):
            typing_only = (path.is_relative_to("tests/typing")
                           and "tests/typing" in tool.get("mypy", {}).get("files", [])
                           and path.name.startswith("typing_"))
            require(path.name != "conftest.py" and not path.name.startswith("test_")
                    and not path.name.endswith("_test.py")
                    and not any(isinstance(n, (ast.FunctionDef, ast.AsyncFunctionDef, ast.ClassDef))
                        and (n.name.startswith("pytest_") or
                             (not typing_only and n.name.startswith(("test", "Test"))))
                        for n in ast.walk(tree))
                    and not any((isinstance(n, ast.Import) and any(a.name.split('.')[0] in {"pytest", "unittest"} for a in n.names))
                        or (isinstance(n, ast.ImportFrom) and (n.module or '').split('.')[0] in {"pytest", "unittest"})
                        for n in ast.walk(tree)), "ancillary Python cannot hide tests/hooks")
            ancillary.append(relative)
        else:
            require(False, f"Python executable outside installed package/tests: {relative}")
    require(tests and sources, "installed Python requires source and tests")
    local_tests = {Path(f).stem for f in tests if len(Path(f).parts) == 2}
    allowed = STDLIB | {"__future__", "abc", "codecs", "contextlib", "copy", "datetime", "difflib",
        "errno", "faulthandler", "gc", "getpass", "gettext", "glob", "importlib.metadata", "inspect",
        "io", "json", "keyword", "locale", "logging", "optparse", "os", "pathlib", "pdb", "pickle",
        "platform", "random", "shlex", "shutil", "stat", "subprocess", "sys", "tempfile", "termios",
        "textwrap", "threading", "time", "tty", "types", "urllib.parse", "uuid", "warnings", "weakref",
        "webbrowser", "concurrent.futures", "collections.abc"}
    mutations = []
    for relative in sources + tests + conftests:
        tree = trees[relative]
        is_test = relative not in sources
        parents = {child: parent for node in ast.walk(tree) for child in ast.iter_child_nodes(node)
                   for parent in [node]}
        annotations = set()
        for node in ast.walk(tree):
            annotation = node.annotation if isinstance(node, (ast.arg, ast.AnnAssign)) else (
                node.returns if isinstance(node, ast.FunctionDef) else None)
            if annotation is not None:
                annotations.update(ast.walk(annotation))
            require(not isinstance(node, (ast.AsyncFunctionDef, ast.Await, ast.AsyncFor, ast.AsyncWith)),
                    "installed Python async execution unsupported")
            if isinstance(node, (ast.FunctionDef, ast.ClassDef)):
                require(not node.name.startswith("pytest_"), "Python pytest hooks unsupported")
            if isinstance(node, ast.Import):
                modules = [alias.name for alias in node.names]
            elif isinstance(node, ast.ImportFrom) and not node.level:
                modules = [node.module or ""]
            else:
                modules = []
            for module in modules:
                owned = module == package or module.startswith(package + ".")
                # Windows compatibility and typing-only imports remain statically
                # retained. The observer independently rejects loading them on Unix.
                inactive = module in {"ctypes", "ctypes.wintypes", "msvcrt", "typing_extensions", "_typeshed"}
                require(owned or module in allowed or module in local_tests or inactive
                        or (is_test and module in {"pytest", "unittest.mock", "unittest"}),
                        f"unsupported installed Python import {module} in {relative}")
            if isinstance(node, ast.Call) and isinstance(node.func, ast.Name):
                require(node.func.id not in {"eval", "exec", "compile", "__import__", "locals", "breakpoint"},
                        "Python dynamic code execution unsupported")
                if node.func.id == "globals":
                    # Conventional deprecated module exports, no access to the
                    # interpreter's module registry or observer globals.
                    parent = parents.get(node)
                    require(not node.args and not node.keywords and isinstance(parent, ast.Subscript)
                            and isinstance(parent.slice, ast.JoinedStr)
                            and len(parent.slice.values) == 2
                            and isinstance(parent.slice.values[0], ast.Constant)
                            and parent.slice.values[0].value == "_"
                            and isinstance(parent.slice.values[1], ast.FormattedValue)
                            and isinstance(parent.slice.values[1].value, ast.Name)
                            and parent.slice.values[1].value.id == "name",
                            "unsupported Python globals lookup")
            if isinstance(node, ast.Attribute):
                if node.attr == "modules":
                    parent = parents.get(node)
                    require(isinstance(node.value, ast.Name) and node.value.id == "sys"
                            and isinstance(parent, ast.Subscript) and isinstance(parent.ctx, ast.Load)
                            and isinstance(parent.slice, ast.Constant) and parent.slice.value == "__main__",
                            "Python module lookup only admits CLI main-module identity")
                if node.attr == "f_globals":
                    parent = parents.get(node)
                    direct = isinstance(parent, ast.Attribute) and parent.attr == "get"
                    saved = (isinstance(parent, ast.IfExp) and isinstance(parents.get(parent), ast.Assign)
                             and len(parents[parent].targets) == 1
                             and isinstance(parents[parent].targets[0], ast.Name)
                             and parents[parent].targets[0].id == "f_globals")
                    require(direct or saved, "Python frame globals only expose module identity")
                if node.attr == "get" and (isinstance(node.value, ast.Name) and node.value.id == "f_globals"
                        or isinstance(node.value, ast.Attribute) and node.value.attr == "f_globals"):
                    call = parents.get(node)
                    require(isinstance(call, ast.Call) and call.args
                            and isinstance(call.args[0], ast.Constant)
                            and call.args[0].value in {"__name__", "__package__"},
                            "Python frame globals only expose module identity")
                require(node.attr not in {"__globals__", "__builtins__", "__subclasses__", "__code__",
                    "f_locals", "gi_frame", "cr_frame", "_bootstrap", "_bootstrap_inner",
                    "excepthook", "unraisablehook", "addaudithook", "settrace", "setprofile",
                    "_start_joinable_thread", "_start_new_thread", "start_new_thread"},
                    f"unsupported Python observer/reflection access: {node.attr}")
                if isinstance(node.value, ast.Name) and node.value.id == "pytest":
                    require(node.attr in PYTEST_MEMBERS | {"skip", "warns", "FixtureRequest"},
                            "unsupported installed pytest API")
                if isinstance(node.value, ast.Attribute) and node.value.attr == "mark":
                    require(node.attr in declared | {"parametrize", "skip", "skipif", "xfail", "filterwarnings", "usefixtures"},
                            "undeclared pytest mark")
            if isinstance(node, ast.Name):
                require(node.id not in {"pytest_plugins", "__test__"}, "Python test hiding/plugins unsupported")
        if not is_test:
            data = (root / relative).read_bytes()
            text = data.decode("utf-8")
            for node in ast.walk(tree):
                if node not in annotations and isinstance(node, ast.Constant) and type(node.value) is bool:
                    start = sum(map(len, data.splitlines(keepends=True)[:node.lineno - 1])) + node.col_offset
                    end = start + (4 if node.value else 5)
                    mutations.append(dict(file=relative, start=start, end=end, line=node.lineno,
                        column=node.col_offset + 1, end_line=node.end_lineno, end_column=node.end_col_offset + 1,
                        original=str(node.value), replacement=str(not node.value)))
            mutations.extend(operator_sites(tree, text, data, annotations, relative))
    return dict(package=package, source_root=source_root, files=files, directories=sorted(directories),
                sources=sorted(sources), tests=sorted(tests), conftests=sorted(conftests),
                data_files=sorted(f for f in files if Path(f).suffix not in {".py", ".pyc"}),
                application_dependencies={}, expected_failure_calls=True,
                installation=dict(backend="flit-core", version="3.12.0", ancillary=sorted(ancillary)),
                mutations=sorted(mutations, key=lambda m: (m["file"], m["start"], -m["end"], m["replacement"])))
