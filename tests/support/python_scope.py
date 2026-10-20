"""Static scope regressions; run with the pinned interpreter and -I -S -B."""
import importlib.util
from pathlib import Path
import tempfile
import unittest

SOURCE = Path(__file__).resolve().parents[2] / "src/python/scope.py"
spec = importlib.util.spec_from_file_location("scope", SOURCE)
scope = importlib.util.module_from_spec(spec)
spec.loader.exec_module(scope)


class ScopeTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        self.write("pyproject.toml", '[project]\nname = "sample"\ndependencies = []\n')
        self.write("sample/__init__.py", "def enabled():\n    return True\n")
        self.write("tests/test_sample.py", "from sample import enabled\n"
                   "def test_enabled():\n    assert enabled() is True\n")

    def write(self, path, content):
        path = self.root / path
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(content)

    def test_unicode_offsets_and_annotation_exclusion(self):
        self.write("sample/__init__.py", 'from typing import Literal\n'
                   'label = "π"; active: Literal[True] = False\n')
        result = scope.inspect(self.root)
        self.assertEqual(len(result["mutations"]), 1)
        mutation = result["mutations"][0]
        data = (self.root / mutation["file"]).read_bytes()
        self.assertEqual(data[mutation["start"]:mutation["end"]], b"False")

    def test_imports_and_runtime_access_fail_closed(self):
        for source in (
            "import threading", "import os", "import unittest", "import subprocess",
            "from sample.missing import value", "from .missing import value",
            "exec('pass')", "value = object.__subclasses__()",
            "import math\nmath = None",
            "import math\ndef f(math): pass",
            "import math\ndef math(): pass",
            "import math\ntry: pass\nexcept Exception as math: pass",
            "async def f(): pass",
        ):
            with self.subTest(source=source):
                self.write("sample/__init__.py", source)
                with self.assertRaises(ValueError):
                    scope.inspect(self.root)

    def test_configuration_cannot_hide_tests(self):
        for name in ("conftest.py", "pytest.ini", "setup.cfg", "setup.py"):
            with self.subTest(name=name):
                self.write(name, "")
                with self.assertRaises(ValueError):
                    scope.inspect(self.root)
                (self.root / name).unlink()
        self.write("pyproject.toml", '[project]\nname="sample"\n'
                   '[tool.pytest.ini_options]\naddopts="--ignore=tests"\n')
        with self.assertRaises(ValueError):
            scope.inspect(self.root)

    def test_string_buffers_logging_and_super_initialization(self):
        self.write("sample/__init__.py", "import logging as log\nfrom io import StringIO\n"
                   "logger = log.getLogger(__name__)\nlogger.addHandler(log.StreamHandler())\n"
                   "class Buffer(StringIO):\n    def __init__(self):\n        super().__init__('text')\n")
        scope.inspect(self.root)
        for source in ("import io", "from io import open", "import logging.config",
                       "import logging as log\nlog.FileHandler('file')",
                       "import logging\nlogging.shutdown()",
                       "value = super().__class__", "value = object.__init__"):
            with self.subTest(source=source):
                self.write("sample/__init__.py", source)
                with self.assertRaises(ValueError):
                    scope.inspect(self.root)

    def test_input_parameter_is_not_interactive_builtin(self):
        self.write("sample/__init__.py", "def identity(input):\n    return input\n")
        scope.inspect(self.root)
        for source in ("value = input", "def f():\n    return input\n",
                       "def f(input=input):\n    return input\n",
                       "def f(input):\n    def nested():\n        return input\n",
                       "def f(input):\n    return input()\n"):
            self.write("sample/__init__.py", source)
            with self.assertRaises(ValueError):
                scope.inspect(self.root)

    def test_shallow_copy_and_index_protocol(self):
        self.write("sample/__init__.py", "import copy as cp\n"
                   "def clone(value):\n    return cp.copy(value)\n"
                   "def first(value):\n    return value.__getitem__(0)\n")
        scope.inspect(self.root)
        for source in ("import copy as cp\ncp.deepcopy([])", "import copy\ncopy.dispatch_table",
                       "from copy import copy", "import copy\ncopy = None",
                       "value = object.__reduce__", "value = object.__dict__"):
            self.write("sample/__init__.py", source)
            with self.assertRaises(ValueError):
                scope.inspect(self.root)

    def test_url_and_glob_string_helpers_without_network_imports(self):
        self.write("sample/__init__.py", "from fnmatch import fnmatch, fnmatchcase\n"
                   "from urllib.parse import parse_qsl, urlencode, urlsplit, urlunsplit\n")
        scope.inspect(self.root)
        for source in ("import urllib", "import urllib.parse", "import urllib.request",
                       "from urllib.request import urlopen", "from urllib.parse import *",
                       "from urllib.parse import clear_cache", "from fnmatch import os",
                       "from fnmatch import *", "import fnmatch"):
            self.write("sample/__init__.py", source)
            with self.assertRaises(ValueError):
                scope.inspect(self.root)

    def test_hidden_and_unselected_python_rejected(self):
        for name in ("scripts/example.py", "tests/helper.py", "sample/test_hidden.py",
                     "sample/nested/module.py", ".hidden/module.py"):
            with self.subTest(name=name):
                self.write(name, "")
                with self.assertRaises(ValueError):
                    scope.inspect(self.root)
                (self.root / name).unlink()

    def test_non_utf8_cookie_rejected(self):
        self.write("sample/__init__.py", "# coding: latin-1\nvalue = True\n")
        with self.assertRaises(ValueError):
            scope.inspect(self.root)

    def test_pytest_filtering_and_alias_shadowing_rejected(self):
        for source in ("import pytest\npytest.mark.filterwarnings('ignore')",
                       "from pytest import mark as m\nm.skip('reason')",
                       "from pytest import fixture\ndef f(fixture): pass"):
            with self.subTest(source=source):
                self.write("tests/test_sample.py", source)
                with self.assertRaises(ValueError):
                    scope.inspect(self.root)

    def test_symlinks_rejected(self):
        (self.root / "sample/link.py").symlink_to(self.root / "sample/__init__.py")
        with self.assertRaises(ValueError):
            scope.inspect(self.root)

    def test_shadowed_stdlib_and_sourceless_modules_rejected(self):
        self.write("sample/hidden.pyc", "")
        with self.assertRaises(ValueError):
            scope.inspect(self.root)
        (self.root / "sample/hidden.pyc").unlink()
        (self.root / "sample").rename(self.root / "json")
        with self.assertRaises(ValueError):
            scope.inspect(self.root)

    def test_fixture_has_two_source_flips(self):
        result = scope.inspect(SOURCE.parents[2] / "tests/fixtures/python")
        self.assertEqual(len(result["mutations"]), 2)
        self.assertEqual(result["tests"], ["tests/test_sample.py"])


if __name__ == "__main__":
    unittest.main()
