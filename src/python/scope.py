"""Static scoped-project reader. Never imports or executes project code."""
import ast
import configparser
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
PYTEST_MEMBERS = {"fixture", "mark", "param", "raises", "fail", "approx", "xfail"}
UNIT_INTERNALS = {"run", "debug", "doCleanups", "doClassCleanups", "subTest", "skipTest",
                  "failureException", "_outcome", "_cleanups", "_class_cleanups",
                  "_callSetUp", "_callTestMethod", "_callTearDown", "_callCleanup",
                  "defaultTestResult", "_addUnexpectedSuccess"}
UNIT_REGISTRATION = {"addCleanup", "addClassCleanup", "enterContext", "enterClassContext"}
LOGGING_MEMBERS = {"getLogger", "StreamHandler", "Formatter", "DEBUG", "INFO",
                   "WARNING", "ERROR", "CRITICAL"}
FROM_MEMBERS = {
    "io": {"StringIO"},
    "abc": {"ABC", "abstractmethod"},
    "types": {"MappingProxyType"},
    "datetime": {"date", "datetime", "time", "timezone"},
    "time": {"time", "sleep"},
    "colorsys": {"hls_to_rgb"},
    "fnmatch": {"fnmatch", "fnmatchcase"},
    "urllib.parse": {"parse_qsl", "urlencode", "urlsplit", "urlunsplit"},
}


def require(condition, message):
    if not condition:
        raise ValueError(message)


def module_exists(root, parts):
    path = root.joinpath(*parts)
    return path.with_suffix(".py").is_file() or (path / "__init__.py").is_file()


def validate_conftest(tree):
    """Fixture declarations and literal data only; no general plugin module."""
    literal_data = {}
    fixture_names = {"fixture"} if any(isinstance(n, ast.ImportFrom) and n.module == "pytest"
        and any(a.name == "fixture" and a.asname is None for a in n.names) for n in tree.body) else set()
    for statement in tree.body:
        if isinstance(statement, (ast.Import, ast.ImportFrom)):
            require(all(not (a.asname or a.name).startswith(("pytest_", "collect_", "__"))
                        for a in statement.names), "Python conftest cannot import hook/plugin aliases")
            continue  # The common import audit still applies below.
        if isinstance(statement, ast.Expr) and isinstance(statement.value, ast.Constant) and isinstance(statement.value.value, str):
            continue
        if isinstance(statement, (ast.Assign, ast.AnnAssign)):
            targets = statement.targets if isinstance(statement, ast.Assign) else [statement.target]
            require(all(isinstance(t, ast.Name) and not t.id.startswith(("pytest", "collect_", "test", "Test", "__"))
                        for t in targets), "Python conftest data cannot configure collection/plugins")
            try:
                value = ast.literal_eval(statement.value)
                literal_data.update((target.id, value) for target in targets)
            except (ValueError, TypeError):
                raise ValueError("Python conftest requires literal module data")
            continue
        if isinstance(statement, ast.FunctionDef) and not statement.decorator_list:
            require(not statement.name.startswith(("pytest", "test", "Test", "__")),
                    "Python conftest helper cannot configure collection/plugins")
            for default in statement.args.defaults + [d for d in statement.args.kw_defaults if d is not None]:
                try:
                    ast.literal_eval(default)
                except (ValueError, TypeError):
                    raise ValueError("Python conftest helper defaults must be literal")
            continue  # Common body/import audit applies to ordinary helpers too.
        require(isinstance(statement, ast.FunctionDef) and len(statement.decorator_list) == 1
                and not statement.name.startswith(("pytest", "test", "Test", "__")),
                "Python conftest only admits synchronous fixture declarations")
        decorator = statement.decorator_list[0]
        function = decorator.func if isinstance(decorator, ast.Call) else decorator
        require((isinstance(function, ast.Name) and function.id in fixture_names) or
                (isinstance(function, ast.Attribute) and function.attr == "fixture" and
                 isinstance(function.value, ast.Name) and function.value.id == "pytest"),
                "Python conftest only admits pytest.fixture decorators")
        if isinstance(decorator, ast.Call):
            require(not decorator.args and all(k.arg in {"scope", "autouse", "params", "ids", "name"}
                                              for k in decorator.keywords), "unsupported Python fixture options")
            for keyword in decorator.keywords:
                if keyword.arg in {"params", "ids"}:
                    try:
                        value = (literal_data[keyword.value.id] if isinstance(keyword.value, ast.Name)
                                 else ast.literal_eval(keyword.value))
                    except (KeyError, ValueError, TypeError):
                        raise ValueError("Python fixture params/ids require literal data")
                    require(value is None or isinstance(value, (list, tuple)),
                            "Python fixture params/ids require a literal sequence")
                    if keyword.arg == "ids" and value is not None:
                        require(all(item is None or isinstance(item, str) for item in value),
                                "Python fixture ids require strings or None")
                if keyword.arg == "scope":
                    require(isinstance(keyword.value, ast.Constant) and keyword.value.value in
                            {"function", "class", "module", "session"}, "unsupported Python fixture scope")
                if keyword.arg == "autouse":
                    require(isinstance(keyword.value, ast.Constant) and type(keyword.value.value) is bool,
                            "Python fixture autouse must be literal")
                if keyword.arg == "name":
                    require(isinstance(keyword.value, ast.Constant) and isinstance(keyword.value.value, str)
                            and keyword.value.value.isidentifier() and not keyword.value.value.startswith(("pytest", "__")),
                            "unsupported Python fixture name")
        require(not statement.args.defaults and not statement.args.kw_defaults
                and statement.args.vararg is None and statement.args.kwarg is None,
                "Python conftest fixture parameters must name dependencies")
        require(all(a.arg not in {"monkeypatch", "pytestconfig", "pytester", "testdir", "doctest_namespace"}
                    for a in statement.args.args + statement.args.posonlyargs),
                "Python conftest fixture cannot alter pytest configuration")


def validate_fixture_request(tree):
    parents = {child: parent for parent in ast.walk(tree) for child in ast.iter_child_nodes(parent)}
    for node in ast.walk(tree):
        if isinstance(node, ast.Name) and node.id == "request":
            require(isinstance(node.ctx, ast.Load) and isinstance(parents.get(node), ast.Attribute)
                    and parents[node].value is node, "Python fixture request aliasing unsupported")
        if isinstance(node, ast.Attribute) and isinstance(node.value, ast.Name) and node.value.id == "request":
            require(node.attr in {"param", "addfinalizer", "getfixturevalue"} and isinstance(node.ctx, ast.Load),
                    "unsupported Python fixture request API")
            if node.attr == "getfixturevalue":
                call = parents.get(node)
                require(isinstance(call, ast.Call) and call.func is node and len(call.args) == 1
                        and not call.keywords and isinstance(call.args[0], ast.Constant)
                        and isinstance(call.args[0].value, str) and call.args[0].value.isidentifier()
                        and not call.args[0].value.startswith("_")
                        and call.args[0].value not in {"request", "monkeypatch", "pytestconfig", "pytester", "testdir", "doctest_namespace"},
                        "Python dynamic fixture lookup requires a literal ordinary fixture name")


def operator_sites(tree, text, data, annotation_nodes, relative):
    """Discover native token edits and scoped expression/statement mutations."""
    # AST rows use physical newlines, not Unicode string line separators.
    lines = [line.decode("utf-8") for line in data.splitlines(keepends=True)]
    offsets = [0]
    for line in lines:
        offsets.append(offsets[-1] + len(line.encode("utf-8")))
    def bounds(node):
        return (offsets[node.lineno - 1] + node.col_offset,
                offsets[node.end_lineno - 1] + node.end_col_offset)

    def numeric(node):
        if isinstance(node, ast.Constant) and type(node.value) in (int, float):
            return node
        if (isinstance(node, ast.UnaryOp) and isinstance(node.op, (ast.UAdd, ast.USub))
                and isinstance(node.operand, ast.Constant)
                and type(node.operand.value) in (int, float)):
            start, end = bounds(node)
            inner_start, inner_end = bounds(node.operand)
            # Only a direct sign owns the literal. Parenthesized operands retain
            # their outer sign and expose the inner literal as an independent site.
            if end == inner_end and re.fullmatch(
                    rb"[+-](?:[ \t\f\r\n]|\\\r?\n|\#[^\r\n]*(?:\r?\n))*",
                    data[start:inner_start]):
                return node.operand
        return None

    signed_children = {node.operand for node in ast.walk(tree)
                       if isinstance(node, ast.UnaryOp) and numeric(node) is not None}
    # Match patterns have their own literal grammar, not ordinary expressions.
    patterns = {part for node in ast.walk(tree)
                if isinstance(node, (ast.pattern, ast.TypeAlias, ast.type_param))
                for part in ast.walk(node)}
    # Do not change documentation or formatted/template strings. Excluding all
    # standalone string expressions also covers attribute/additional docstrings.
    string_exclusions = {part for node in ast.walk(tree)
                         if isinstance(node, (ast.JoinedStr, ast.TemplateStr))
                         for part in ast.walk(node)}
    string_exclusions.update(node.value for node in ast.walk(tree)
                             if isinstance(node, ast.Expr)
                             and isinstance(node.value, ast.Constant)
                             and type(node.value.value) is str)
    tokens = []
    for token in tokenize.generate_tokens(io.StringIO(text, newline=None).readline):
        if ((token.type == tokenize.OP and token.string in {"<", "<=", ">", ">=", "==", "!=", "+", "-", "*", "/", "%"})
                or (token.type == tokenize.NAME and token.string in {"and", "or"})):
            row, column = token.start
            start = offsets[row - 1] + len(lines[row - 1][:column].encode("utf-8"))
            tokens.append((start, token.string, row))
    replacements = {"<": "<=", "<=": "<", ">": ">=", ">=": ">",
                    "+": "-", "-": "+", "*": "/", "/": "*", "%": "*", "and": "or", "or": "and"}
    inversions = {"==": "!=", "!=": "==", "<": ">=", "<=": ">", ">": "<=", ">=": "<"}
    operators = {ast.And: "and", ast.Or: "or", ast.Eq: "==", ast.NotEq: "!=", ast.Lt: "<", ast.LtE: "<=", ast.Gt: ">", ast.GtE: ">=",
                 ast.Add: "+", ast.Sub: "-", ast.Mult: "*", ast.Div: "/", ast.Mod: "%"}
    # Constructor-store deletion was entirely error noise in the unchanged
    # Route Mini audit. Keep this syntactic exclusion explicit, including nested
    # closures; it is not a proof that other methods cannot initialize state.
    constructor_nodes = {part for node in ast.walk(tree)
                         if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef))
                         and node.name in {"__init__", "__new__"}
                         for part in ast.walk(node)}
    for node in ast.walk(tree):
        if node in annotation_nodes:
            continue
        if (isinstance(node, ast.Constant) and type(node.value) is str
                and node not in patterns and node not in string_exclusions):
            start, end = bounds(node)
            # The AST combines adjacent literals and resolves their escapes.
            # Emit one exact source span; never evaluate/import project code.
            yield dict(file=relative, start=start, end=end, line=node.lineno,
                       column=node.col_offset + 1, end_line=node.end_lineno,
                       end_column=node.end_col_offset + 1, original=data[start:end].decode(),
                       replacement='("")' if node.value else '("x")')
            continue
        literal = numeric(node)
        if literal is not None and node not in signed_children and node not in patterns:
            start, end = bounds(node)
            replacement = "1" if literal.value == 0 else "0"
            if type(literal.value) is float:
                replacement += ".0"
            yield dict(file=relative, start=start, end=end, line=node.lineno,
                       column=node.col_offset + 1, end_line=node.end_lineno,
                       end_column=node.end_col_offset + 1, original=data[start:end].decode(),
                       replacement="(" + replacement + ")")
            continue
        if (node not in constructor_nodes and isinstance(node, ast.Assign) and len(node.targets) == 1
                and isinstance(node.targets[0], (ast.Attribute, ast.Subscript))):
            # Do not remove local bindings, unpacking/chained stores, annotations
            # or suspension. Ordinary attribute/item stores skip RHS and target
            # effects together; missing initialization remains a runtime error.
            if any(isinstance(part, (ast.NamedExpr, ast.Yield, ast.YieldFrom, ast.Await))
                   for part in ast.walk(node)):
                continue
            start = offsets[node.lineno - 1] + node.col_offset
            end = offsets[node.end_lineno - 1] + node.end_col_offset
            yield dict(file=relative, start=start, end=end, line=node.lineno,
                       column=node.col_offset + 1, end_line=node.end_lineno,
                       end_column=node.end_col_offset + 1,
                       original=data[start:end].decode(), replacement="pass",
                       assignment_remove=True)
            continue
        if isinstance(node, ast.Expr) and isinstance(node.value, ast.Call):
            # Removing bindings can alter symbol-table scope; removing yield can
            # turn an enclosing generator into an ordinary function. Do not infer
            # nested closure/generator boundaries or broaden async admission.
            if any(isinstance(part, (ast.NamedExpr, ast.Yield, ast.YieldFrom, ast.Await))
                   for part in ast.walk(node.value)):
                continue
            start = offsets[node.lineno - 1] + node.col_offset
            end = offsets[node.end_lineno - 1] + node.end_col_offset
            yield dict(file=relative, start=start, end=end, line=node.lineno,
                       column=node.col_offset + 1, end_line=node.end_lineno,
                       end_column=node.end_col_offset + 1,
                       original=data[start:end].decode(), replacement="pass")
            continue
        if isinstance(node, (ast.If, ast.While)):
            condition = node.test
            # Parentheses do not survive in AST spans. Literal Boolean conditions
            # belong to boolean_flip. Binding expressions are excluded rather than
            # leaving names expected in the body or following code uninitialized.
            if ((isinstance(condition, ast.Constant) and type(condition.value) is bool)
                    or any(isinstance(part, ast.NamedExpr) for part in ast.walk(condition))):
                continue
            start = offsets[condition.lineno - 1] + condition.col_offset
            end = offsets[condition.end_lineno - 1] + condition.end_col_offset
            original = data[start:end].decode()
            for guard in (("False and",) if isinstance(node, ast.While) else ("True or", "False and")):
                yield dict(file=relative, start=start, end=end, line=condition.lineno,
                           column=condition.col_offset + 1, end_line=condition.end_lineno,
                           end_column=condition.end_col_offset + 1, original=original,
                           replacement="(" + guard + " (" + original + "))",
                           **({"loop_skip": True} if isinstance(node, ast.While) else {}))
            continue
        if isinstance(node, ast.UnaryOp) and isinstance(node.op, ast.Not):
            start = offsets[node.lineno - 1] + node.col_offset
            end = offsets[node.end_lineno - 1] + node.end_col_offset
            original = data[start:end].decode()
            require(original.startswith("not"), "Python negation span mismatch")
            # Keep operand parentheses/trivia, and isolate the original low-precedence
            # operand from its parent. No bool conversion or duplicate evaluation.
            yield dict(file=relative, start=start, end=end, line=node.lineno,
                       column=node.col_offset + 1, end_line=node.end_lineno,
                       end_column=node.end_col_offset + 1, original=original,
                       replacement="(" + original[3:] + ")")
            continue
        if isinstance(node, ast.Compare) and len(node.ops) == 1:
            left, right, op = node.left, node.comparators[0], node.ops[0]
        elif isinstance(node, ast.BinOp):
            left, right, op = node.left, node.right, node.op
        elif isinstance(node, ast.BoolOp) and len(node.values) == 2:
            left, right, op = *node.values, node.op
        else:
            continue
        if type(op) not in operators:
            continue
        lower = offsets[left.end_lineno - 1] + left.end_col_offset
        upper = offsets[right.lineno - 1] + right.col_offset
        between = [(start, op, row) for start, op, row in tokens if lower <= start < upper]
        require(len(between) == 1 and between[0][1] == operators[type(op)],
                "Python operator token mismatch")
        start, original, row = between[0]
        end = start + len(original)
        require(data[start:end].decode() == original, "Python operator span mismatch")
        column = start - offsets[row - 1] + 1
        for replacement in ([replacements[original]] if original in replacements else []) + ([inversions[original]] if original in inversions else []):
            yield dict(file=relative, start=start, end=end, line=row, column=column,
                       end_line=row, end_column=column + len(original), original=original,
                       replacement=replacement)


def inspect(root):
    root = root.resolve(strict=True)
    files = {}
    directories = []
    total = 0

    def walk(directory):
        nonlocal total
        for path in sorted(directory.iterdir()):
            relative = path.relative_to(root).as_posix()
            if path.parent == root and path.name in (".git", ".mutie"):
                continue
            if path.parent == root and path.name in ("mutie.dismissals.toml", ".mutie.dismissals.lock"):
                require(path.is_file() and not path.is_symlink(),
                        "Python review metadata must be a regular file")
                # Mutie owns these files and omits them from executed copies.
                # Review decisions must not become application data identities.
                continue
            if path.name == ".git" and path.parent != root:
                require(path.is_file() and not path.is_symlink()
                        and re.fullmatch(r"gitdir: [^\r\n]+\n?", path.read_text()),
                        "Python nested Git directories unsupported")
                continue
            require(not path.is_symlink(), f"Python symlink unsupported: {relative}")
            require(len(path.relative_to(root).parts) < 32, "Python tree is too deep")
            if path.is_dir():
                require(path.name not in (".venv", "venv", "node_modules"),
                        f"Python installed project environments unsupported: {relative}")
                directories.append(relative)
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
    require(set(config) <= {"project", "build-system", "tool", "dependency-groups"}, "unsupported Python project table")
    groups = config.get("dependency-groups", {})
    dev_tools = {"commitizen", "coverage", "editorconfig-checker", "pyright", "ruff",
                 "setuptools", "twine", "wheel"}
    require(isinstance(groups, dict) and set(groups) <= {"dev"} and all(
        isinstance(items, list) and all(isinstance(item, str) and
            re.fullmatch(r"[a-z-]+>=[0-9]+(?:\.[0-9]+)*", item) and
            item.split(">=")[0] in dev_tools for item in items) for items in groups.values()),
        "Python only accepts audited inert development dependencies")
    project = config.get("project", {})
    require(isinstance(project.get("name"), str) and project["name"],
            "Python requires a named [project]")
    dependencies = project.get("dependencies", [])
    require(dependencies in ([], ["webencodings >=0.4"], ["webencodings>=0.4"],
                             ["webencodings==0.5.1"]),
            "Python only admits the audited webencodings dependency")
    application_dependencies = {"webencodings": "0.5.1"} if dependencies else {}
    # The test extra is declarative only: the pinned environment is supplied by
    # the caller, never resolved or installed from this manifest.
    extras = project.get("optional-dependencies", {})
    # These optional tools are inert metadata, never installed or auto-loaded.
    optional_tools = {"docs": {"ipython", "matplotlib", "numpydoc", "sphinx"},
                      "tests": {"pytest", "pytest-cov", "pytest-xdist"},
                      "dev": {"mypy", "pytest", "pytest-cov", "ruff"}}
    audited_extras = (isinstance(extras, dict) and set(extras) <= optional_tools.keys()
                      and all(isinstance(items, list) and items
                              and all(isinstance(item, str) and re.fullmatch(r"[a-z-]+(?:>=[0-9]+(?:\.[0-9]+){0,2})?", item)
                                      and item.split(">=")[0] in optional_tools[group]
                                      and (not item.startswith("pytest>=") or
                                           tuple(map(int, item.split(">=")[1].split("."))) <= (9, 0, 2))
                                      for item in items) for group, items in extras.items()))
    audited_extras = audited_extras or extras == {
        "doc": ["sphinx", "sphinx_rtd_theme"], "test": ["pytest", "isort", "flake8"]}
    require(audited_extras or (isinstance(extras, dict) and set(extras) <= {"test"}
            and all(isinstance(items, list) and items and all(
                isinstance(item, str) and re.fullmatch(r"pytest(?:>=[0-9]+(?:\.[0-9]+){0,2})?", item)
                and (">=" not in item or tuple(map(int, item.split(">=")[1].split("."))) <= (9, 0, 2))
                for item in items) for items in extras.values())),
            "Python only accepts audited declarative test/documentation extras")
    require(project.get("dynamic", []) in ([], ["version"]) and not project.get("entry-points"),
            "Python dynamic metadata and entry-point plugins unsupported")
    tool = config.get("tool", {})
    require(set(tool) <= {"pytest", "setuptools", "hatch", "ruff", "uv", "mypy",
                          "isort", "tox", "coverage", "flit"}, "unsupported Python tool configuration")
    require(tool.get("flit", {}) in ({}, {"sdist": {"exclude": [".*"]}}),
            "Python only admits inert Flit sdist metadata")
    # These tools are never invoked by the runner. Their declarative settings
    # remain fingerprinted inputs; admitting them does not install their plugins
    # or execute their commands (including tox's build/test environments).
    for name in ("isort", "tox", "coverage"):
        require(isinstance(tool.get(name, {}), dict), f"invalid Python {name} metadata")
    uv = tool.get("uv", {})
    require(set(uv) <= {"override-dependencies"} and isinstance(uv.get("override-dependencies", []), list)
            and all(isinstance(item, str) and re.fullmatch(r"(?:cryptography|urllib3)>=[0-9]+(?:\.[0-9]+)*", item)
                    for item in uv.get("override-dependencies", [])), "unsupported Python resolver metadata")
    mypy = tool.get("mypy", {})
    mypy_flags = {"strict", "warn_return_any", "warn_unused_configs", "disallow_untyped_defs",
                  "show_error_codes", "warn_unreachable", "warn_unused_ignores",
                  "warn_redundant_casts", "incremental", "check_untyped_defs",
                  "strict_equality", "implicit_reexport", "no_implicit_optional"}
    overrides = mypy.get("overrides", [])
    require(set(mypy) <= {"python_version", "files", "overrides"} | mypy_flags
            and mypy.get("python_version", "3.11") in ("3.10", "3.11")
            and all(type(mypy.get(key, False)) is bool for key in mypy_flags)
            and isinstance(overrides, list) and all(isinstance(item, dict)
                and set(item) <= {"module", "disallow_untyped_defs", "ignore_errors"}
                and isinstance(item.get("module"), str)
                and all(type(item.get(key, False)) is bool
                        for key in ("disallow_untyped_defs", "ignore_errors")) for item in overrides)
            and mypy.get("files", ["src", "tests", "examples"]) == ["src", "tests", "examples"],
            "unsupported Python type-checker metadata")
    ruff = tool.get("ruff", {})
    lint = ruff.get("lint", {})
    require(set(ruff) <= {"line-length", "lint", "target-version"}
            and ruff.get("target-version", "py311") in ("py310", "py311") and type(ruff.get("line-length", 88)) is int
            and set(lint) <= {"select", "fixable", "unfixable", "mccabe"}
            and all(isinstance(lint.get(key, []), list) and all(isinstance(x, str) for x in lint.get(key, []))
                    for key in ("select", "fixable", "unfixable"))
            and set(lint.get("mccabe", {})) <= {"max-complexity"}
            and type(lint.get("mccabe", {}).get("max-complexity", 10)) is int,
            "unsupported Python lint metadata")
    pytest_config = tool.get("pytest", {})
    require(set(pytest_config) <= {"ini_options"}, "unsupported pytest configuration")
    options = pytest_config.get("ini_options", {})
    if "pytest.ini" in files:
        require(not pytest_config, "Python cannot combine pytest.ini and pyproject pytest settings")
        ini = configparser.ConfigParser(interpolation=None, strict=True)
        ini.read_string((root / "pytest.ini").read_text(encoding="utf-8"))
        require(not ini.defaults() and ini.sections() == ["pytest"]
                and set(ini["pytest"]) <= {"testpaths"},
                "Python pytest.ini only admits testpaths")
        options = {"testpaths": ini["pytest"].get("testpaths", "tests").split()}

    require(set(options) <= {"testpaths", "pythonpath", "addopts", "xfail_strict", "norecursedirs"}
            and options.get("testpaths", ["tests"]) in (["tests"], ["."])
            and options.get("addopts", "") in ("", "-q", "--strict-markers --strict-config")
            and type(options.get("xfail_strict", False)) is bool,
            "Python only accepts full tests with optional quiet output")
    require(options.get("norecursedirs", []) in ([], ["tests/css-parsing-tests"]),
            "Python only admits the audited data-only test directory exclusion")
    # Build metadata is never executed. Admit only a conventional static backend.
    build = config.get("build-system")
    if build is not None:
        require(set(build) <= {"requires", "build-backend"}
                and ((build.get("build-backend") == "hatchling.build" and build.get("requires") == ["hatchling"])
                     or (build.get("build-backend") == "flit_core.buildapi"
                         and build.get("requires") in (["flit_core>=3.11.0,<4"], ["flit_core >=3.2,<4"]))
                     or (build.get("build-backend") == "setuptools.build_meta"
                and isinstance(build.get("requires"), list)
                and all(isinstance(x, str) and
                        (x == "wheel" or re.fullmatch(r"setuptools(?:>=[0-9]+(?:\.[0-9]+)*)?", x))
                        for x in build["requires"]))),
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
    hatch = tool.get("hatch")
    if hatch is not None:
        require(hatch == {"build": {"targets": {"wheel": {"packages": [(source_root + "/" if source_root else "") + package]}}}}
                or hatch == {"build": {"targets": {
            "wheel": {"packages": [package]},
            "sdist": {"include": [package, "tests"], "exclude": ["*.json", "pkg/_compat.py"]}}}},
            "Python only accepts audited static Hatch package metadata")
    require(package.isidentifier(), "Python package must be an identifier")
    require(options.get("pythonpath", [source_root] if source_root else [])
            == ([source_root] if source_root else []),
            "Python pythonpath must match the single audited source root")
    setuptools = tool.get("setuptools")
    static_version = None
    if project.get("dynamic"):
        flit_version = build is not None and build.get("build-backend") == "flit_core.buildapi"
        require(flit_version or isinstance(setuptools, dict), "Python version requires static backend metadata")
        require(flit_version or setuptools.get("dynamic") == {"version": {"attr": package + ".__version__"}},
                "Python only accepts a literal package version")
        version_tree = ast.parse((import_root / package / "__init__.py").read_bytes())
        versions = [node for node in version_tree.body if isinstance(node, ast.Assign)
                    and any(isinstance(target, ast.Name) and target.id == "__version__"
                            for target in node.targets)]
        require(len(versions) == 1 and all(isinstance(t, ast.Name) and t.id in {"VERSION", "__version__"}
                                         for t in versions[0].targets)
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
        "pytest", "_pytest", "tests", "pluggy", "packaging", "iniconfig", "pygments", "pip", "webencodings"},
            "Python package shadows an admitted dependency")
    tests = []
    conftests = []
    mutations = []
    sources = []
    expected_failure_calls = False
    for relative in files:
        path = Path(relative)
        is_conftest = path.name == "conftest.py"
        require(not is_conftest or relative in {"conftest.py", "tests/conftest.py"},
                "Python conftest must be at the root or directly under tests/")
        require(path.name != "pytest.ini" or relative == "pytest.ini",
                "Python pytest.ini must be at the project root")
        require(path.name not in {".pytest.ini",
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
        if not in_package and (relative in {"doc/source/conf.py", "docs/source/conf.py"}
                               or (len(parts) == 2 and parts[0] in {".circleci", "examples"})
                               or relative == "benchmark/run.py"
                               or relative in {"docs/conf.py", "docs/css_diagram_role.py",
                                               "tests/css-parsing-tests/make_color3_hsl.py",
                                               "tests/css-parsing-tests/make_color3_keywords.py"}):
            # Documentation configuration and CI scripts are retained, never
            # imported or executed. They cannot conceal tests or pytest hooks.
            require(not path.name.startswith("test_") and not path.name.endswith("_test.py"),
                    "Python ancillary files cannot be tests")
            doc_tree = ast.parse((root / path).read_bytes())
            require(not any(
                (isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef, ast.ClassDef))
                 and (node.name.startswith(("test", "Test", "pytest_"))))
                or (isinstance(node, ast.Name) and node.id.startswith(("pytest", "test_", "Test"))
                    and relative != "benchmark/run.py")
                or (isinstance(node, (ast.Import, ast.ImportFrom)) and any(
                    alias.name.split(".")[0] in {"pytest", "unittest"} for alias in node.names))
                or (isinstance(node, ast.ImportFrom) and (node.module or "").split(".")[0]
                    in {"pytest", "unittest"}) for node in ast.walk(doc_tree)),
                    "Python ancillary configuration cannot contain tests/hooks")
            continue
        require(not any(path.is_relative_to(excluded) for excluded in options.get("norecursedirs", [])),
                "Python excluded data directory cannot contain executable tests/helpers")
        is_test = is_conftest or parts[0] == "tests" or (len(parts) == 1 and path.name.startswith("test_"))
        require(not (is_test and not is_conftest and len(parts) == 1 and options.get("testpaths") == ["tests"]),
                "Python root test is excluded by configured testpaths")
        require(in_package or is_test,
                f"Python executable file outside package/tests: {relative}")
        module_parts = parts[1:] if source_root and not is_test else parts
        require("__pycache__" not in parts and all(not p.startswith(".") for p in parts),
                f"hidden Python executable file: {relative}")
        if relative == "tests/__init__.py":
            require(all(isinstance(n, ast.Expr) and isinstance(n.value, ast.Constant)
                        and isinstance(n.value.value, str)
                        for n in ast.parse((root / path).read_text(encoding="utf-8")).body),
                    "Python test package initializer must be empty or docstrings only")
            continue
        require(not is_test or is_conftest or path.name.startswith("test_"),
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
        elif is_conftest:
            conftests.append(relative)
        else:
            tests.append(relative)
        data = (root / path).read_bytes()
        encoding, _ = tokenize.detect_encoding(io.BytesIO(data).readline)
        require(encoding == "utf-8", "Python source requires UTF-8 encoding")
        text = data.decode("utf-8")
        tree = ast.parse(text, filename=relative)
        if is_conftest:
            validate_conftest(tree)
        if is_test:
            validate_fixture_request(tree)
        # A conventional script entry point is inert under pytest's import mode.
        # Admit only this exact shape, never arbitrary unittest runner calls.
        main_calls = set()
        case_bases = set()
        parents = {child: parent for parent in ast.walk(tree) for child in ast.iter_child_nodes(parent)}
        # Only this conventional stdlib compatibility branch is resolved using
        # the pinned interpreter. Do not evaluate arbitrary project conditions
        # or silently admit imports in other unreachable code.
        inactive_imports = set()
        version_accesses = set()
        if is_test:
            expected_guard = ast.dump(ast.parse('sys.version_info >= (3, 11)', mode='eval').body)
            for statement in tree.body:
                if isinstance(statement, ast.If) and ast.dump(statement.test) == expected_guard:
                    require(len(statement.body) == len(statement.orelse) == 1
                            and isinstance(statement.body[0], ast.Import)
                            and isinstance(statement.orelse[0], ast.Import)
                            and len(statement.body[0].names) == len(statement.orelse[0].names) == 1
                            and statement.body[0].names[0].name == "tomllib"
                            and statement.body[0].names[0].asname == "tomli"
                            and statement.orelse[0].names[0].name == "tomli"
                            and statement.orelse[0].names[0].asname is None,
                            "Python version branch only admits the pinned tomllib fallback")
                    inactive_imports.add(statement.orelse[0])
                    version_accesses.add(statement.test.left)
        for cls in (node for node in ast.walk(tree) if isinstance(node, ast.ClassDef)):
            bases = [base for base in cls.bases if
                     (isinstance(base, ast.Name) and base.id == "TestCase") or
                     (isinstance(base, ast.Attribute) and base.attr == "TestCase"
                      and isinstance(base.value, ast.Name) and base.value.id == "unittest")]
            if bases:
                require(is_test and len(cls.bases) == 1 and not cls.keywords and not cls.decorator_list,
                        "Python requires a direct undecorated unittest.TestCase subclass")
                case_bases.update(bases)
                for member in cls.body:
                    if isinstance(member, ast.FunctionDef):
                        require(member.name not in UNIT_INTERNALS | UNIT_REGISTRATION and not member.name.startswith("__"),
                                "Python unittest dispatch overrides unsupported")
                        require(not member.decorator_list or
                                (member.name in {"setUpClass", "tearDownClass"} and
                                 len(member.decorator_list) == 1 and
                                 isinstance(member.decorator_list[0], ast.Name) and
                                 member.decorator_list[0].id == "classmethod"),
                                "Python unittest method decorators unsupported")
        for statement in tree.body:
            if (is_test and isinstance(statement, ast.If) and not statement.orelse
                    and ast.dump(statement.test) == ast.dump(ast.parse('__name__ == "__main__"', mode='eval').body)
                    and len(statement.body) == 1):
                body = statement.body[0]
                value = body.value if isinstance(body, (ast.Expr, ast.Assign)) else None
                if (isinstance(value, ast.Call) and not value.args and not value.keywords
                        and ast.dump(value.func) == ast.dump(ast.parse('unittest.main', mode='eval').body)):
                    main_calls.add(value.func)
        # A BOM/coding cookie must not change how byte spans map to source bytes.
        require(not text.startswith("\ufeff") and "\x00" not in text,
                "Python source requires plain UTF-8 without BOM/NUL")
        aliases = set()
        logging_aliases = set()
        copy_aliases = set()
        random_aliases = set()
        json_aliases = set()
        toml_aliases = set()
        pytest_marks = {"mark"}
        for node in ast.walk(tree):
            if node in inactive_imports:
                continue
            if isinstance(node, ast.Import):
                for alias in node.names:
                    top = alias.name.split(".")[0]
                    require(top in STDLIB | {package, "logging", "copy", "random", "sys"} | ({"pytest", "unittest", "random", "json", "tomllib", "os", "pprint"} if is_test else set()),
                            f"unsupported Python import {alias.name} in {relative}")
                    if top == "json":
                        require(alias.name == "json", "Python json submodules unsupported")
                        json_aliases.add(alias.asname or "json")
                    if top == "tomllib":
                        require(alias.name == "tomllib", "Python tomllib submodules unsupported")
                        toml_aliases.add(alias.asname or "tomllib")
                    if top == "sys":
                        require(alias.name == "sys" and alias.asname is None,
                                "Python only admits unaliased sys for the pinned version branch")
                    if top == "os":
                        require(alias.name == "os" and alias.asname is None,
                                "Python only admits unaliased os for the profiling iteration control")
                    if top == "pprint":
                        require(alias.name == "pprint" and alias.asname is None,
                                "Python only admits unaliased test pprint")
                    if top == "random":
                        require(alias.name == "random", "Python random submodules unsupported")
                        random_aliases.add(alias.asname or "random")
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
                    if top == "unittest":
                        require(alias.name == "unittest" and alias.asname is None,
                                "Python only admits unaliased unittest")
                    aliases.add(alias.asname or top)
            elif isinstance(node, ast.ImportFrom):
                if (node.level == 1 and node.module == "conftest" and is_test
                        and not is_conftest and len(parts) == 2 and parts[0] == "tests"):
                    require("tests/__init__.py" in files and "tests/conftest.py" in files,
                            "Python conftest helper imports require a regular tests package")
                    fixture_tree = ast.parse((root / "tests/conftest.py").read_bytes())
                    helpers = {n.name for n in fixture_tree.body if isinstance(n, ast.FunctionDef)
                               and not n.decorator_list and not n.name.startswith(("pytest", "test", "Test", "__"))}
                    require(all(a.name in helpers and a.asname is None for a in node.names),
                            "Python only imports ordinary conftest helpers without aliases")
                    top = "tests"
                elif node.level:
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
                    require(top in STDLIB | {package} | ({"pytest", "unittest"} if is_test else set())
                            or node.module in FROM_MEMBERS or node.module == "__future__"
                            or (is_test and node.module == "pathlib")
                            or (node.module == "webencodings" and application_dependencies),
                            f"unsupported Python import {node.module} in {relative}")
                    if node.module in FROM_MEMBERS:
                        require(all(a.name in FROM_MEMBERS[node.module] for a in node.names),
                                f"unsupported Python API from {node.module}")
                    if node.module == "pathlib":
                        require(all(a.name == "Path" and a.asname is None for a in node.names),
                                "Python only admits unaliased test pathlib.Path")
                    if node.module == "webencodings":
                        require(all(a.name in {"UTF8", "Encoding", "decode", "lookup", "ascii_lower"}
                                    for a in node.names), "Python webencodings API unsupported")
                    if top == package:
                        require(module_exists(import_root, node.module.split(".")),
                                "unresolved local Python import")
                require(all(a.name != "*" for a in node.names), "Python wildcard imports unsupported")
                if top == "pytest":
                    require(all(a.name in PYTEST_MEMBERS - {"xfail"} for a in node.names),
                            "unsupported pytest import")
                    pytest_marks.update(a.asname or a.name for a in node.names if a.name == "mark")
                if top == "unittest":
                    require(node.module == "unittest" and all(a.name == "TestCase" and a.asname is None for a in node.names),
                            "Python only admits unittest.TestCase")
                aliases.update(a.asname or a.name for a in node.names)
        unit_module = is_test and bool(aliases & {"unittest", "TestCase"})
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
            # A class field named input does not reference the interactive builtin.
            if (isinstance(node, ast.AnnAssign) and isinstance(parents.get(node), ast.ClassDef)
                    and isinstance(node.target, ast.Name) and node.target.id == "input"):
                input_parameters.add(node.target)
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
            if isinstance(node, ast.type_param):
                annotation_nodes.update(ast.walk(node))
        for node in ast.walk(tree):
            require(not isinstance(node, (ast.AsyncFunctionDef, ast.Await, ast.AsyncFor,
                                          ast.AsyncWith, ast.TypeAlias)),
                    f"Python async/type-alias execution unsupported: {relative}")
            if isinstance(node, ast.Name):
                if node.id == "Path" and "Path" in aliases:
                    call = parents.get(node)
                    require(isinstance(call, ast.Call) and call.func is node
                            and len(call.args) == 1 and not call.keywords
                            and isinstance(call.args[0], ast.Name) and call.args[0].id == "__file__",
                            "Python test Path must be constructed from __file__")
                if node.id == "os" and "os" in aliases:
                    attribute = parents.get(node)
                    method = parents.get(attribute)
                    call = parents.get(method)
                    require(isinstance(attribute, ast.Attribute) and attribute.attr == "environ"
                            and isinstance(method, ast.Attribute) and method.attr == "get"
                            and isinstance(call, ast.Call) and call.func is method and not call.keywords
                            and len(call.args) == 2
                            and all(isinstance(arg, ast.Constant) for arg in call.args)
                            and call.args[0].value == "PROFILER_ITERATIONS"
                            and type(call.args[1].value) is int and call.args[1].value == 1,
                            "Python environment access only admits the fixed profiling iteration control")
                if node.id == "sys" and "sys" in aliases:
                    require(parents.get(node) in version_accesses or (
                        isinstance(parents.get(node), ast.Attribute)
                        and parents[node].attr == "maxunicode" and isinstance(parents[node].ctx, ast.Load)),
                            "Python sys access only admits the pinned version branch")
                if node.id == "pprint" and "pprint" in aliases:
                    require(isinstance(parents.get(node), ast.Attribute)
                            and parents[node].attr == "pprint"
                            and isinstance(parents[node].ctx, ast.Load), "Python only admits test pprint.pprint")
                if node.id in toml_aliases:
                    require(isinstance(parents.get(node), ast.Attribute) and parents[node].value is node,
                            "Python tomllib aliasing unsupported")
                if node.id in json_aliases:
                    require(isinstance(parents.get(node), ast.Attribute) and parents[node].value is node,
                            "Python json aliasing unsupported")
                if node.id in random_aliases:
                    require(isinstance(parents.get(node), ast.Attribute) and parents[node].value is node,
                            "Python random aliasing unsupported")
                if unit_module and node.id == "TestCase":
                    require(node in case_bases, "Python TestCase is only supported as a direct base")
                if unit_module and node.id == "unittest":
                    require(isinstance(parents.get(node), ast.Attribute), "Python unittest aliasing unsupported")
                temporary_open = False
                if node.id == "open" and is_test:
                    call = parents.get(node)
                    temporary_open = (isinstance(call, ast.Call) and call.func is node
                        and len(call.args) == 2 and not call.keywords
                        and isinstance(call.args[0], ast.Name)
                        and isinstance(call.args[1], ast.Constant) and call.args[1].value == "wb")
                require((node.id not in FORBIDDEN_NAMES or node in input_parameters or temporary_open)
                        and not node.id.startswith("pytest_"),
                        f"unsupported Python runtime access: {node.id}")
                require(not (isinstance(node.ctx, (ast.Store, ast.Del)) and node.id in aliases),
                        f"Python import rebinding unsupported: {node.id}")
            if isinstance(node, ast.Call) and isinstance(node.func, ast.Name):
                require(node.func.id != "input", "Python interactive input unsupported")
            if isinstance(node, ast.Attribute):
                if isinstance(node.value, ast.Name) and node.value.id in toml_aliases:
                    require(node.attr == "loads" and isinstance(node.ctx, ast.Load),
                            "Python only admits in-memory TOML in tests")
                require(not unit_module or node.attr not in UNIT_INTERNALS,
                        "Python unittest dispatch/internal APIs unsupported")
                require(not unit_module or node.attr not in UNIT_REGISTRATION
                        or isinstance(node.ctx, ast.Load), "Python unittest registration rebinding unsupported")
                if isinstance(node.value, ast.Name) and node.value.id == "unittest":
                    require(node in case_bases or node in main_calls,
                            "Python only admits unittest.TestCase and an inert main guard")
                super_init = (not unit_module and node.attr == "__init__" and isinstance(node.value, ast.Call)
                              and isinstance(node.value.func, ast.Name)
                              and node.value.func.id == "super")
                # Direct Exception initialization is conventional in small custom
                # exceptions. It does not expose arbitrary dunder dispatch.
                call = parents.get(node)
                method = call
                while method is not None and not isinstance(method, (ast.FunctionDef, ast.ClassDef)):
                    method = parents.get(method)
                cls = parents.get(method) if method is not None else None
                exception_init = (not unit_module and node.attr == "__init__"
                    and isinstance(node.value, ast.Name) and node.value.id == "Exception"
                    and isinstance(call, ast.Call) and call.func is node and call.args
                    and isinstance(call.args[0], ast.Name) and call.args[0].id == "self"
                    and isinstance(method, ast.FunctionDef) and method.name == "__init__"
                    and isinstance(cls, ast.ClassDef) and len(cls.bases) == 1
                    and isinstance(cls.bases[0], ast.Name) and cls.bases[0].id == "Exception")
                # Formatting a type's display name does not expose an object's
                # dictionary, MRO or arbitrary reflective lookup.
                type_name = (node.attr == "__qualname__" and isinstance(node.ctx, ast.Load)
                    and isinstance(node.value, ast.Call)
                    and isinstance(node.value.func, ast.Name) and node.value.func.id == "type"
                    and len(node.value.args) == 1 and not node.value.keywords)
                local_base_init = (not unit_module and node.attr == "__init__"
                    and isinstance(node.value, ast.Name)
                    and isinstance(call, ast.Call) and call.func is node and call.args
                    and isinstance(call.args[0], ast.Name) and call.args[0].id == "self"
                    and isinstance(method, ast.FunctionDef) and method.name == "__init__"
                    and isinstance(cls, ast.ClassDef) and len(cls.bases) == 1
                    and isinstance(cls.bases[0], ast.Name) and cls.bases[0].id == node.value.id
                    and any(isinstance(c, ast.ClassDef) and c.name == node.value.id for c in tree.body))
                test_function_name = (is_test and node.attr == "__name__"
                    and isinstance(node.ctx, ast.Load) and isinstance(node.value, ast.Name)
                    and isinstance(method, ast.FunctionDef)
                    and node.value.id in {a.arg for a in method.args.args})
                require(super_init or exception_init or local_base_init or test_function_name
                        or type_name or node.attr == "__getitem__"
                        or not (node.attr.startswith("__") and node.attr.endswith("__")),
                        f"Python reflective attribute unsupported: {node.attr}")
                if isinstance(node.value, ast.Name) and node.value.id in json_aliases:
                    require(node.attr in {"dumps", "loads"} and isinstance(node.ctx, ast.Load),
                            "Python only admits in-memory JSON in tests")
                if isinstance(node.value, ast.Name) and node.value.id in random_aliases:
                    require(node.attr in ({"Random", "sample"} if is_test else {"Random"})
                            and isinstance(node.ctx, ast.Load),
                            "Python only admits random.Random and test-only random.sample")
                    if node.attr == "Random":
                        require(isinstance(parents.get(node), ast.Call) and parents[node].func is node,
                                "Python random.Random is only supported as a direct constructor")
                if isinstance(node.value, ast.Name) and node.value.id in logging_aliases:
                    require(node.attr in LOGGING_MEMBERS, "unsupported Python logging API")
                if isinstance(node.value, ast.Name) and node.value.id in copy_aliases:
                    require(node.attr == "copy", "Python only admits copy.copy")
                if isinstance(node.value, ast.Name) and node.value.id == "pytest":
                    require(node.attr in PYTEST_MEMBERS, f"unsupported pytest API: {node.attr}")
                    if node.attr == "xfail":
                        call = parents.get(node)
                        require(is_test and not is_conftest and isinstance(call, ast.Call)
                                and call.func is node and len(call.args) == 1 and not call.keywords
                                and isinstance(call.args[0], ast.Constant)
                                and isinstance(call.args[0].value, str) and call.args[0].value,
                                "Python xfail requires a direct test-only call with a literal reason")
                        expected_failure_calls = True
                mark = (isinstance(node.value, ast.Attribute) and node.value.attr == "mark"
                        and isinstance(node.value.value, ast.Name) and node.value.value.id == "pytest")
                mark = mark or (isinstance(node.value, ast.Name) and node.value.id in pytest_marks)
                require(not mark or node.attr == "parametrize", "unsupported pytest mark")
            if isinstance(node, (ast.FunctionDef, ast.ClassDef)):
                require(not node.name.startswith("pytest_"), "Python pytest hooks unsupported")
                require(node.name not in aliases or (isinstance(node, ast.FunctionDef)
                        and isinstance(parents.get(node), ast.ClassDef) and not unit_module),
                        "Python import rebinding unsupported")
                if unit_module:
                    require(node.name not in UNIT_INTERNALS,
                            "Python unittest dispatch overrides unsupported")
            if isinstance(node, ast.arg):
                require(node.arg not in aliases, "Python import shadowing unsupported")
            if isinstance(node, ast.ExceptHandler):
                require(node.name not in aliases or (isinstance(node, ast.FunctionDef)
                        and isinstance(parents.get(node), ast.ClassDef) and not unit_module),
                        "Python import rebinding unsupported")
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
        if not is_test:
            mutations.extend(operator_sites(tree, text, data, annotation_nodes, relative))
    require(tests, "Python requires test_*.py files at the root or under tests/")
    return dict(package=package, source_root=source_root, files=files, tests=sorted(tests), conftests=sorted(conftests), sources=sorted(sources),
                expected_failure_calls=expected_failure_calls, application_dependencies=application_dependencies,
                data_files=sorted(f for f in files if Path(f).suffix not in {".py", ".pyc"}),
                directories=sorted(directories),
                mutations=sorted(mutations, key=lambda m: (m["file"], m["start"], -m["end"], m["replacement"])))


if __name__ == "__main__":
    require(sys.version_info[:3] == (3, 14, 7), "Python requires CPython 3.14.7")
    print(json.dumps(inspect(Path(sys.argv[1]))))
