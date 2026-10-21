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

    def test_unittest_scope_and_inert_main(self):
        for header, base in [("import unittest", "unittest.TestCase"),
                             ("from unittest import TestCase", "TestCase")]:
            self.write("tests/test_sample.py", f"{header}\nclass TestSample({base}):\n"
                       " def setUp(self): self.addCleanup(self.assertTrue, True)\n"
                       " def test_body(self): self.assertTrue(True)\n")
            self.assertEqual(len(scope.inspect(self.root)["tests"]), 1)
        self.write("tests/test_sample.py", 'import unittest\nclass TestSample(unittest.TestCase):\n'
                   ' def test_body(self): pass\nif __name__ == "__main__":\n _ = unittest.main()\n')
        scope.inspect(self.root)

    def test_unittest_reserved_apis_do_not_restrict_ordinary_application_helpers(self):
        self.write("sample/__init__.py", "class Worker:\n def run(self): return True\n")
        self.write("tests/test_sample.py", "from sample import Worker\n"
                   "def run(): return Worker().run()\ndef test_worker(): assert run()\n")
        self.assertEqual(len(scope.inspect(self.root)["mutations"]), 1)

    def test_unittest_rejects_unobserved_dispatch_and_framework_apis(self):
        for source in [
            "import unittest as unit", "from unittest.mock import patch",
            "from unittest import skip", "import unittest\nunittest.main()",
            "import unittest\nbase = unittest.TestCase", "import unittest\nunit = unittest",
            "import unittest\nclass TestSample(unittest.IsolatedAsyncioTestCase): pass",
            "import unittest\nclass TestSample(unittest.TestCase, object): pass",
            "import unittest\nclass TestSample(unittest.TestCase):\n def __init__(self): pass",
            "import unittest\nclass TestSample(unittest.TestCase):\n def run(self): pass",
            "import unittest\nclass TestSample(unittest.TestCase):\n def test_a(self): self.doCleanups()",
            "import unittest\nclass TestSample(unittest.TestCase):\n def test_a(self): self.subTest()",
            "import unittest\nunittest.addModuleCleanup(print)",
            "import unittest\nclass TestSample(unittest.TestCase):\n def test_a(self): super().__init__()",
            "import unittest\nclass TestSample(unittest.TestCase):\n def test_a(self): self.addCleanup = lambda: None",
            "import unittest\nclass TestSample(unittest.TestCase):\n @unittest.expectedFailure\n def test_a(self): pass",
        ]:
            with self.subTest(source=source):
                self.write("tests/test_sample.py", source)
                with self.assertRaises(ValueError):
                    scope.inspect(self.root)

    def test_inert_build_and_development_metadata_rejects_hooks(self):
        base = '[project]\nname="sample"\n[build-system]\nrequires=["hatchling"]\nbuild-backend="hatchling.build"\n'
        self.write("pyproject.toml", base + '[dependency-groups]\ndev=["ruff>=0.15.7"]\n'
                   '[tool.ruff]\nline-length=120\n[tool.uv]\noverride-dependencies=["urllib3>=2.6.3"]\n')
        scope.inspect(self.root)
        for extra in [
            '[tool.hatch.build.hooks.custom]\npath="build_hook.py"\n',
            '[tool.uv]\nindex-url="https://example.invalid"\n',
            '[dependency-groups]\ndev=["arbitrary-plugin>=1"]\n',
            '[tool.ruff]\nextend="other.toml"\n',
        ]:
            with self.subTest(extra=extra):
                self.write("pyproject.toml", base + extra)
                with self.assertRaises(ValueError):
                    scope.inspect(self.root)

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
        for name in ("pytest.ini", "setup.cfg", "setup.py"):
            with self.subTest(name=name):
                self.write(name, "")
                with self.assertRaises(ValueError):
                    scope.inspect(self.root)
                (self.root / name).unlink()
        self.write("pyproject.toml", '[project]\nname="sample"\n'
                   '[tool.pytest.ini_options]\naddopts="--ignore=tests"\n')
        with self.assertRaises(ValueError):
            scope.inspect(self.root)

    def test_plain_pytest_ini_and_empty_test_package(self):
        self.write("pytest.ini", "[pytest]\ntestpaths = tests\n")
        self.write("tests/__init__.py", "# No executable initialization.\n")
        result = scope.inspect(self.root)
        self.assertEqual(result["tests"], ["tests/test_sample.py"])
        self.assertIn("pytest.ini", result["files"])
        self.assertIn("tests/__init__.py", result["files"])
        for contents in ["flag=True", "import pytest", "def helper(): pass"]:
            self.write("tests/__init__.py", contents)
            with self.assertRaises(ValueError): scope.inspect(self.root)
        self.write("tests/__init__.py", "")
        for contents in ["[pytest]\naddopts=--ignore=tests", "[pytest]\ntestpaths=tests/subset",
                         "[pytest]\npythonpath=other", "[pytest]\n[other]", "[DEFAULT]\ntestpaths=tests\n[pytest]"]:
            self.write("pytest.ini", contents)
            with self.assertRaises(ValueError): scope.inspect(self.root)
        self.write("pytest.ini", "[pytest]\ntestpaths=tests")
        self.write("pyproject.toml", '[project]\nname="sample"\n[tool.pytest.ini_options]\ntestpaths=["tests"]\n')
        with self.assertRaises(ValueError): scope.inspect(self.root)

    def test_sample_library_stdlib_calls_are_bounded(self):
        self.write("sample/__init__.py", 'class Problem(Exception):\n def __init__(self, key):\n  Exception.__init__(self, f"Missing {key}")\n')
        self.write("tests/test_sample.py", "import random\ndef test_sample(): assert len(random.sample(range(10), 3)) == 3\n")
        scope.inspect(self.root)
        for source in ["import random\nvalue=random.seed(1)", "import random\nvalue=random",
                       "from random import sample", "import random\nrandom.sample = None"]:
            self.write("tests/test_sample.py", source)
            with self.assertRaises(ValueError): scope.inspect(self.root)
        self.write("tests/test_sample.py", "def test_sample(): pass")
        for source in ["import random", "value=Exception.__init__", "Exception.__init__(object())",
                       "class Other: \n def __init__(self): Exception.__init__(self)"]:
            self.write("sample/__init__.py", source)
            with self.assertRaises(ValueError): scope.inspect(self.root)

    def test_conftest_is_fingerprinted_but_not_a_test_or_mutation_source(self):
        fixture = "import pytest\nDATA=[True, False]\n@pytest.fixture(params=DATA)\ndef value(request):\n yield request.param\n"
        for name in ["conftest.py", "tests/conftest.py"]:
            self.write(name, fixture)
        result = scope.inspect(self.root)
        self.assertEqual(result["conftests"], ["conftest.py", "tests/conftest.py"])
        self.assertEqual(result["tests"], ["tests/test_sample.py"])
        self.assertEqual(len(result["mutations"]), 1)
        self.assertIn("conftest.py", result["files"])
        self.write("conftest.py", fixture + "\n# fixture change\n")
        self.assertNotEqual(result["files"], scope.inspect(self.root)["files"])

    def test_conftest_rejects_hooks_plugins_and_dynamic_configuration(self):
        for source in [
            "pytest_plugins = ['sample']", "collect_ignore = ['tests']",
            "from sample import enabled as pytest_collection_modifyitems",
            "def pytest_runtest_call(item): pass", "def test_hidden(): pass",
            "class TestHidden: pass", "def helper(): pass", "raise RuntimeError('imported')",
            "import pytest\n@pytest.fixture(scope=lambda **kw: 'session')\ndef value(): return 1",
            "import pytest\n@pytest.fixture(ids=lambda x: str(x))\ndef value(): return 1",
            "import pytest\n@pytest.fixture(params=range(3))\ndef value(): return 1",
            "import pytest\n@pytest.fixture\ndef value(request): return request.config",
            "import pytest\n@pytest.fixture\ndef value(monkeypatch): return 1",
            "import pytest\n@pytest.fixture\nasync def value(): return 1",
        ]:
            with self.subTest(source=source):
                self.write("conftest.py", source)
                with self.assertRaises(ValueError):
                    scope.inspect(self.root)
        (self.root / "conftest.py").unlink()
        for path in ["sample/conftest.py", "tests/nested/conftest.py", "elsewhere/conftest.py"]:
            self.write(path, "")
            with self.assertRaises(ValueError):
                scope.inspect(self.root)
            (self.root / path).unlink()

    def test_fixture_request_cannot_reconfigure_or_alias_pytest(self):
        for expression in ["request.config", "request._fixture_defs", "request",
                           "request.getfixturevalue('pytestconfig')",
                           "request.getfixturevalue('request')",
                           "request.getfixturevalue(name)", "request.getfixturevalue"]:
            for path in ["conftest.py", "tests/test_sample.py"]:
                with self.subTest(expression=expression, path=path):
                    self.write(path, "import pytest\n@pytest.fixture\ndef value(request):\n return " + expression)
                    with self.assertRaises(ValueError):
                        scope.inspect(self.root)
                    (self.root / path).unlink()
            self.write("tests/test_sample.py", "def test_value(): pass")

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

    def src_layout(self):
        (self.root / "src").mkdir()
        (self.root / "sample").rename(self.root / "src/sample")

    def test_flat_package_named_src_keeps_its_existing_import_root(self):
        (self.root / "sample").rename(self.root / "src")
        self.write("src/nested/__init__.py", "value = False\n")
        self.write("tests/test_sample.py", "from src import enabled\ndef test_enabled(): assert enabled()\n")
        result = scope.inspect(self.root)
        self.assertEqual(result["package"], "src")
        self.assertEqual(result["source_root"], "")
        self.assertEqual(len(result["mutations"]), 2)

    def test_src_layout_offsets_and_relative_imports(self):
        self.src_layout()
        self.write("src/sample/nested/__init__.py", "from .. import enabled\n")
        self.write("src/sample/nested/value.py", "from . import enabled\nfrom .. import enabled as outer\nactive = False\n")
        result = scope.inspect(self.root)
        self.assertEqual(result["source_root"], "src")
        self.assertEqual([m["file"] for m in result["mutations"]],
                         ["src/sample/__init__.py", "src/sample/nested/value.py"])
        for value in ("from .. import escaped", "from ...outside import escaped"):
            self.write("src/sample/__init__.py", value)
            with self.assertRaises(ValueError):
                scope.inspect(self.root)

    def test_src_static_manifest_without_execution(self):
        self.src_layout()
        manifest = ('[project]\nname="sample"\n'
                    '[project.optional-dependencies]\ntest=["pytest>=8.0"]\n'
                    '[tool.pytest.ini_options]\npythonpath=["src"]\naddopts="-q"\n'
                    '[tool.setuptools]\npackage-dir={""="src"}\n'
                    '[tool.setuptools.packages.find]\nwhere=["src"]\n')
        self.write("pyproject.toml", manifest)
        scope.inspect(self.root)
        for old, new in [('pytest>=8.0', 'pytest>=99.0'), ('pytest>=8.0', 'requests'),
                         ('pytest>=8.0', "pytest>=8.0; python_version >= '3.10'"),
                         ('addopts="-q"', 'addopts="-k hidden"'),
                         ('pythonpath=["src"]', 'pythonpath=["../outside"]'),
                         ('where=["src"]', 'where=["src", "other"]'),
                         ('where=["src"]', 'where=["src"]\nexclude=["sample"]'),
                         ('package-dir={""="src"}', 'package-dir={"sample"="src/sample"}')]:
            self.write("pyproject.toml", manifest.replace(old, new))
            with self.assertRaises((ValueError, scope.tomllib.TOMLDecodeError)):
                scope.inspect(self.root)

    def test_src_ambiguity_namespace_and_escaped_files_rejected(self):
        self.src_layout()
        for name in ('sample/__init__.py', 'src/second/__init__.py', 'src/namespace/module.py',
                     'src/__init__.py', 'src/outside.py', 'src/sample/nested/value.py'):
            self.write(name, '')
            with self.assertRaises(ValueError):
                scope.inspect(self.root)
            (self.root / name).unlink()
        self.write('src/sample/mod.py', '')
        self.write('src/sample/mod/__init__.py', '')
        with self.assertRaises(ValueError):
            scope.inspect(self.root)

    def test_src_symlink_and_namespace_root_rejected(self):
        self.src_layout()
        (self.root / 'src/sample/__init__.py').unlink()
        self.write('src/sample/value.py', '')
        with self.assertRaises(ValueError):
            scope.inspect(self.root)
        self.write('src/sample/__init__.py', '')
        (self.root / 'alias').symlink_to(self.root / 'src', target_is_directory=True)
        with self.assertRaises(ValueError):
            scope.inspect(self.root)

    def test_fixture_has_two_source_flips(self):
        result = scope.inspect(SOURCE.parents[2] / "tests/fixtures/python")
        self.assertEqual(len(result["mutations"]), 2)
        self.assertEqual(result["tests"], ["tests/test_sample.py"])

    def test_root_and_directory_tests_are_both_inventoried(self):
        self.write('test_root.py', 'def test_root(): assert True\n')
        result = scope.inspect(self.root)
        self.assertEqual(result['tests'], ['test_root.py', 'tests/test_sample.py'])
        self.assertEqual(len(result['mutations']), 1)
        self.write('pyproject.toml', '[project]\nname="sample"\n[tool.pytest.ini_options]\ntestpaths=["tests"]\n')
        with self.assertRaisesRegex(ValueError, 'excluded by configured testpaths'):
            scope.inspect(self.root)

    def test_static_version_and_optional_tools_do_not_execute(self):
        manifest = ('[project]\nname="sample"\ndynamic=["version"]\n'
                    '[project.optional-dependencies]\ntests=["pytest","pytest-cov","pytest-xdist"]\n'
                    'docs=["sphinx"]\n[tool.setuptools]\npackages=["sample"]\n'
                    '[tool.setuptools.dynamic]\nversion={attr="sample.__version__"}\n'
                    '[tool.setuptools.package-data]\nsample=["py.typed"]\n')
        self.write('pyproject.toml', manifest)
        self.write('sample/__init__.py', '__version__="1.0"\nraise RuntimeError("must not execute")\n')
        scope.inspect(self.root)
        for old, new in [('"pytest-cov"', '"unreviewed-plugin"'),
                         ('"sphinx"', '"requests"'), ('dynamic=["version"]', 'dynamic=["dependencies"]'),
                         ('attr="sample.__version__"', 'attr="outside.__version__"'),
                         ('packages=["sample"]', 'packages=["sample","other"]')]:
            self.write('pyproject.toml', manifest.replace(old, new))
            with self.assertRaises(ValueError):
                scope.inspect(self.root)
        self.write('pyproject.toml', manifest)
        for source in ['__version__=calculate()', '__version__="1"\n__version__="2"',
                       '__version__=1', 'from .version import __version__']:
            self.write('sample/__init__.py', source)
            with self.assertRaises(ValueError):
                scope.inspect(self.root)

    def test_ancillary_files_are_fingerprinted_and_cannot_hide_tests(self):
        for path in ['doc/source/conf.py', '.circleci/build_docs.py']:
            self.write(path, 'import os\nraise RuntimeError("not executed")\n')
            result = scope.inspect(self.root)
            self.assertIn(path, result['files'])
            self.assertNotIn(path, result['sources'])
            self.assertNotIn(path, result['tests'])
            for source in ['def test_hidden(): pass', 'class TestHidden: pass',
                           'from pytest import fixture', 'import unittest as unit',
                           'pytest_plugins = []', 'test_hidden = lambda: False']:
                self.write(path, source)
                with self.assertRaises(ValueError):
                    scope.inspect(self.root)
            (self.root / path).unlink()
        for path in ['.circleci/test_hidden.py', '.circleci/hidden_test.py', 'doc/source/test_hidden.py']:
            self.write(path, '')
            with self.assertRaises(ValueError):
                scope.inspect(self.root)
            (self.root / path).unlink()
        self.write('sample/__init__.py', 'from doc.source import conf')
        with self.assertRaises(ValueError):
            scope.inspect(self.root)

    def test_package_named_doc_cannot_bypass_source_validation(self):
        (self.root / 'sample').rename(self.root / 'doc')
        self.write('tests/test_sample.py', 'from doc import enabled\ndef test_enabled(): assert enabled()\n')
        self.write('doc/source/__init__.py', '')
        self.write('doc/source/conf.py', 'value = False\n')
        result = scope.inspect(self.root)
        self.assertIn('doc/source/conf.py', result['sources'])
        self.assertEqual(len(result['mutations']), 2)
        self.write('doc/source/conf.py', 'import os\n')
        with self.assertRaises(ValueError):
            scope.inspect(self.root)


if __name__ == "__main__":
    unittest.main()
