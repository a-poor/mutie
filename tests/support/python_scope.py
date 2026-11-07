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

    def loop_sites(self, source):
        self.write('sample/__init__.py', source)
        return [m for m in scope.inspect(self.root)['mutations'] if m.get('loop_skip')]

    def test_loop_exclusions_and_nested_inventory(self):
        source = ('def f(a):\n'
                  ' while True: break\n while (False): pass\n'
                  ' while (x := a()): pass\n'
                  ' while (lambda: (x := a()))(): pass\n'
                  ' while [x := v for v in a]: pass\n'
                  ' for v in a: pass\n'
                  ' while a:\n  while a.other: break\n  break\n')
        sites = self.loop_sites(source)
        self.assertEqual([m['original'] for m in sites], ['a', 'a.other'])
        self.write('tests/test_sample.py', 'def test_loop():\n while 1: break\n')
        self.assertEqual(len(self.loop_sites(source)), 2)

    def test_loop_coordinates_and_native_ast(self):
        import ast
        source = 'label = "π"\ndef f(a,b):\n while (a # comment\n        or b): break\n else: return 7\n'
        for newline in ('\n', '\r\n'):
            raw = source.replace('\n', newline).encode()
            site, = self.loop_sites(raw.decode())
            self.assertEqual(raw[site['start']:site['end']].decode(), site['original'])
            changed = raw[:site['start']] + site['replacement'].encode() + raw[site['end']:]
            expected = ast.parse(raw)
            node, = [n for n in ast.walk(expected) if isinstance(n, ast.While)]
            node.test = ast.BoolOp(op=ast.And(), values=[ast.Constant(False), node.test])
            self.assertEqual(ast.dump(ast.parse(changed)), ast.dump(expected))

    def test_loop_skips_truthiness_and_effects_runs_else(self):
        class BoolBomb:
            def __bool__(self): raise RuntimeError('truthiness')
        class LenBomb:
            def __len__(self): raise RuntimeError('length')
        for expression in ('a', 'call()', 'a and call()', 'a or call()',
                           'a.missing', '1 / 0', 'a < call()'):
            source = ('def f(a,call,trace,value):\n trace.append("before")\n while '
                      + expression + ':\n  trace.append("body")\n  break\n'
                      ' else: trace.append("else")\n trace.append("after")\n return value\n')
            site, = self.loop_sites(source)
            raw = source.encode()
            changed = raw[:site['start']] + site['replacement'].encode() + raw[site['end']:]
            namespace = {}
            exec(compile(changed, '<control>', 'exec'), namespace)
            for value in (BoolBomb(), LenBomb(), [], [1], None):
                trace = []
                def call(): raise AssertionError('condition called')
                self.assertIs(namespace['f'](value, call, trace, value), value)
                self.assertEqual(trace, ['before', 'else', 'after'])

    def test_loop_retains_generator_and_else_exception(self):
        import inspect
        source = 'def f():\n while (yield 1):\n  yield 2\n else: raise ValueError("else")\n'
        site, = self.loop_sites(source)
        raw = source.encode()
        changed = raw[:site['start']] + site['replacement'].encode() + raw[site['end']:]
        namespace = {}
        exec(compile(changed, '<control>', 'exec'), namespace)
        self.assertTrue(inspect.isgeneratorfunction(namespace['f']))
        with self.assertRaisesRegex(ValueError, 'else'):
            next(namespace['f']())

    def branch_sites(self, source):
        self.write('sample/__init__.py', source)
        return [m for m in scope.inspect(self.root)['mutations']
                if not m.get('loop_skip') and m['replacement'].startswith(('(True or (', '(False and ('))]

    def test_branch_coordinates_grouping_and_elif(self):
        import ast
        source = ('label = "π"\n'
                  'def choose(a,b,c):\n'
                  ' if (a # retained comment\n'
                  '     or b and c): return a\n'
                  ' elif b: return b\n'
                  ' else: return c\n')
        for newline in ('\n', '\r\n'):
            raw = source.replace('\n', newline).encode()
            sites = self.branch_sites(raw.decode())
            self.assertEqual(len(sites), 4)
            for site in sites:
                self.assertEqual(raw[site['start']:site['end']].decode(), site['original'])
                changed = raw[:site['start']] + site['replacement'].encode() + raw[site['end']:]
                expected = ast.parse(raw)
                node, = [n for n in ast.walk(expected) if isinstance(n, ast.If)
                         and n.test.lineno == site['line']]
                force = site['replacement'].startswith('(True or')
                node.test = ast.BoolOp(op=ast.Or() if force else ast.And(),
                                      values=[ast.Constant(force), node.test])
                self.assertEqual(ast.dump(ast.parse(changed)), ast.dump(expected))

    def test_branch_assignment_and_context_exclusions(self):
        source = ('def choose(a):\n'
                  ' if (True): pass\n'
                  ' if False: pass\n'
                  ' if (x := a()): pass\n'
                  ' if a and (x := a()): pass\n'
                  ' if (lambda: (x := a()))(): pass\n'
                  ' if [x := v for v in a]: pass\n'
                  ' while a: break\n'
                  ' assert a\n'
                  ' return a if a else [x for x in a if x]\n')
        self.assertEqual(self.branch_sites(source), [])
        self.assertEqual(len(self.branch_sites('if 0: pass\nif None: pass\nif "": pass\n')), 6)
        self.write('tests/test_sample.py', 'def test_nested():\n if 1: assert 1\n')
        self.assertEqual(len(self.branch_sites('if 0: pass\n')), 2)

    def test_branches_skip_effects_and_truthiness_preserve_body_values(self):
        class BoolBomb:
            def __bool__(self): raise RuntimeError('truthiness')
        class LenBomb:
            def __len__(self): raise RuntimeError('length')
        for expression in ('a', 'call()', 'a and call()', 'a or call()',
                           'a.missing', '1 / 0', 'a < call()'):
            source = 'def choose(a,call,yes,no):\n if ' + expression + ': return yes\n return no\n'
            for site in self.branch_sites(source):
                data = source.encode()
                namespace = {}
                exec(data[:site['start']] + site['replacement'].encode() + data[site['end']:], namespace)
                for value in (BoolBomb(), LenBomb(), [], [1], None):
                    calls = []
                    def call():
                        calls.append(1)
                        raise RuntimeError('condition called')
                    yes, no = object(), object()
                    result = namespace['choose'](value, call, yes, no)
                    self.assertIs(result, yes if site['replacement'].startswith('(True or') else no)
                    self.assertEqual(calls, [])
        source = 'def choose(first,second):\n if first(): return 1\n elif second(): return 2\n return 3\n'
        for site in self.branch_sites(source)[2:]:
            data = source.encode(); namespace = {}; calls = []
            exec(data[:site['start']] + site['replacement'].encode() + data[site['end']:], namespace)
            def first(): calls.append('first'); return False
            def second(): raise RuntimeError('skipped elif')
            self.assertEqual(namespace['choose'](first, second), 2 if site['replacement'].startswith('(True or') else 3)
            self.assertEqual(calls, ['first'])

    def test_branch_generator_kind_and_body_exception_are_preserved(self):
        source = 'def choose():\n if (yield 1): raise ValueError("body")\n return 3\n'
        for site in self.branch_sites(source):
            data = source.encode(); namespace = {}
            exec(data[:site['start']] + site['replacement'].encode() + data[site['end']:], namespace)
            generator = namespace['choose']()
            if site['replacement'].startswith('(True or'):
                with self.assertRaisesRegex(ValueError, 'body'): next(generator)
            else:
                with self.assertRaises(StopIteration) as raised: next(generator)
                self.assertEqual(raised.exception.value, 3)

    def test_audited_dependency_requires_declaration_and_narrow_imports(self):
        manifest = '[project]\nname="sample"\ndependencies=["webencodings >=0.4"]\n'
        self.write('pyproject.toml', manifest)
        self.write('sample/__init__.py', 'from webencodings import decode, UTF8, ascii_lower\n')
        self.assertEqual(scope.inspect(self.root)['application_dependencies'], {'webencodings':'0.5.1'})
        for source in ('import webencodings', 'from webencodings import encode',
                       'from webencodings.tests import test_lookup', 'from webencodings import *',
                       'from webencodings import lookup\nlookup = None'):
            self.write('sample/__init__.py', source)
            with self.assertRaises(ValueError): scope.inspect(self.root)
        self.write('sample/__init__.py', 'from webencodings import lookup')
        for dep in ('[]', '["webencodings>=1"]', '["webencodings[extra]>=0.4"]',
                    '["webencodings @ file:///tmp/evil"]', '["webencodings>=0.4", "other"]'):
            self.write('pyproject.toml', manifest.replace('["webencodings >=0.4"]', dep))
            with self.assertRaises(ValueError): scope.inspect(self.root)

    def test_local_base_initialization_and_method_namespace(self):
        source = """from collections import deque
class Base:
    def __init__(self, value): self.value = value
class Child(Base):
    def __init__(self, value): Base.__init__(self, value)
    def deque(self): return deque()
"""
        self.write('sample/__init__.py', source)
        scope.inspect(self.root)
        for bad in (source + '\ndef deque(): pass', source.replace('Base.__init__(self, value)', 'Base.__init__'),
                    source.replace('class Child(Base)', 'class Child(object)'),
                    source.replace('Base.__init__(self, value)', 'Base.__dict__')):
            self.write('sample/__init__.py', bad)
            with self.assertRaises(ValueError): scope.inspect(self.root)

    def test_data_only_exclusions_and_git_pointer_do_not_hide_tests(self):
        self.write('pyproject.toml', '[project]\nname="sample"\n[tool.pytest.ini_options]\nnorecursedirs=["tests/css-parsing-tests"]\n')
        helper = 'tests/css-parsing-tests/make_color3_hsl.py'
        self.write(helper, 'import colorsys\nprint(colorsys.hls_to_rgb(0, 0, 0))')
        self.write('tests/css-parsing-tests/.git', 'gitdir: ../../.git/modules/data\n')
        document = scope.inspect(self.root)
        self.assertIn(helper, document['files'])
        self.assertNotIn(helper, document['tests'])
        self.write(helper, 'def test_hidden(): assert False')
        with self.assertRaises(ValueError): scope.inspect(self.root)
        self.write(helper, 'import pytest')
        with self.assertRaises(ValueError): scope.inspect(self.root)
        self.write(helper, 'pass')
        self.write('tests/css-parsing-tests/test_hidden.py', 'def test_hidden(): pass')
        with self.assertRaisesRegex(ValueError, 'excluded data directory'): scope.inspect(self.root)

    def test_readonly_unicode_bound_and_test_function_names(self):
        self.write('sample/__init__.py', 'import sys\nvalue = sys.maxunicode\n')
        self.write('tests/test_sample.py', 'import pprint\ndef decorate(function): return function.__name__\ndef test_a(): pprint.pprint(1)\n')
        scope.inspect(self.root)
        for source in ('import pprint as p', 'import pprint\npprint._sys.exit()',
                       'import pprint\npprint = None', 'import pprint\nalias = pprint'):
            self.write('tests/test_sample.py', source)
            with self.assertRaises(ValueError): scope.inspect(self.root)
        self.write('tests/test_sample.py', 'def test_a(): pass')
        for source in ('import sys\nsys.maxunicode = 1', 'import sys\nvalue = sys.modules',
                       'import sys\nvalue = sys', 'import sys as other'):
            self.write('sample/__init__.py', source)
            with self.assertRaises(ValueError): scope.inspect(self.root)

    def test_independent_random_generator_without_global_rng_access(self):
        self.write('sample/__init__.py', 'import random as rng\ndef draw(seed):\n    return rng.Random(seed).random()\n')
        scope.inspect(self.root)
        for source in ('import random\nrandom.seed(1)',
                       'import random\nrandom.random()',
                       'import random\nrandom.SystemRandom()',
                       'import random\nrandom.Random = None',
                       'import random\nconstructor = random.Random',
                       'import random\nrandom.Random.seed = None',
                       'import random\nalias = random',
                       'from random import Random',
                       'import random\nvalue = random.__dict__'):
            with self.subTest(source=source):
                self.write('sample/__init__.py', source)
                with self.assertRaises(ValueError):
                    scope.inspect(self.root)

    def test_flit_and_inert_tools_do_not_execute_or_enable_plugins(self):
        config = '''[project]
name="sample"
version="1.0"
[build-system]
requires=["flit_core>=3.11.0,<4"]
build-backend="flit_core.buildapi"
[tool.isort]
profile="black"
[tool.tox.env_run_base]
commands=[["this-command-must-never-run"]]
deps=["pytest-profiling"]
[tool.coverage.run]
branch=true
source=["sample"]
[tool.mypy]
warn_unreachable=true
incremental=false
[[tool.mypy.overrides]]
module="tests.*"
disallow_untyped_defs=false
[tool.pytest.ini_options]
addopts="--strict-markers --strict-config"
xfail_strict=true
'''
        self.write('pyproject.toml', config)
        result = scope.inspect(self.root)
        self.assertIn('pyproject.toml', result['files'])
        for old, new in [
            ('flit_core.buildapi', 'unreviewed.buildapi'),
            ('version="1.0"', 'dynamic=["version"]'),
            ('branch=true', 'branch=true\n[tool.unreviewed]\nplugin="bad"'),
            ('warn_unreachable=true', 'plugins=["bad"]'),
            ('incremental=false', 'incremental="false"'),
            ('disallow_untyped_defs=false', 'plugins=["bad"]'),
            ('--strict-markers --strict-config', '-p bad'),
            ('xfail_strict=true', 'xfail_strict="true"'),
        ]:
            with self.subTest(new=new):
                self.write('pyproject.toml', config.replace(old, new))
                with self.assertRaises(ValueError):
                    scope.inspect(self.root)

    def test_datetime_types_and_direct_type_display_name(self):
        self.write('sample/__init__.py', '''from datetime import date, datetime, time, timezone
def describe(value):
    return f"Unsupported {type(value).__qualname__}"
def offset_time():
    return time(23, 59, tzinfo=timezone.utc)
''')
        scope.inspect(self.root)
        for source in ('import datetime', 'from datetime import timedelta',
                       'def f(v): return v.__qualname__',
                       'def f(v): return type(v).__dict__',
                       'def f(v): return type(v).__mro__',
                       'def f(v): type(v).__qualname__ = "changed"'):
            with self.subTest(source=source):
                self.write('sample/__init__.py', source)
                with self.assertRaises(ValueError):
                    scope.inspect(self.root)

    def test_pinned_tomllib_import_branch_does_not_admit_general_sys_or_files(self):
        source = '''import sys
if sys.version_info >= (3, 11):
    import tomllib as tomli
else:
    import tomli
def test_toml():
    assert tomli.loads('key = 1') == {'key': 1}
'''
        self.write('tests/test_sample.py', source)
        scope.inspect(self.root)
        for old, new in [('(3, 11)', '(99, 0)'),
                         ('import tomli\n', 'import forbidden\n'),
                         ('import tomli\n', 'import tomli\n    raise Exception()\n'),
                         ('tomli.loads', 'tomli.load'),
                         ('assert tomli.loads', 'assert sys.modules or tomli.loads'),
                         ('assert tomli.loads', 'module = tomli\n    assert tomli.loads'),
                         ('assert tomli.loads', 'module = sys\n    assert tomli.loads')]:
            with self.subTest(new=new):
                self.write('tests/test_sample.py', source.replace(old, new))
                with self.assertRaises(ValueError):
                    scope.inspect(self.root)

    def test_only_explicit_literal_expected_failure_calls(self):
        source = 'import pytest\ndef test_expected():\n    pytest.xfail("known unsupported input")\n'
        self.write('tests/test_sample.py', source)
        self.assertTrue(scope.inspect(self.root)['expected_failure_calls'])
        for source in ('from pytest import xfail\n',
                       'import pytest\nxfail = pytest.xfail\n',
                       'import pytest\ndef test_a(): pytest.xfail(str(1))\n',
                       'import pytest\n@pytest.mark.xfail\ndef test_a(): pass\n'):
            with self.subTest(source=source):
                self.write('tests/test_sample.py', source)
                with self.assertRaises(ValueError):
                    scope.inspect(self.root)

    def test_review_metadata_is_not_application_data(self):
        before = scope.inspect(self.root)
        for name in ('mutie.dismissals.toml', '.mutie.dismissals.lock'):
            self.write(name, 'owned review metadata')
            self.assertEqual(scope.inspect(self.root), before)
            self.write(name, 'changed review metadata')
            self.assertEqual(scope.inspect(self.root), before)
            (self.root / name).unlink()
            (self.root / name).mkdir()
            self.write(name + '/test_hidden.py', 'def test_hidden(): pass')
            with self.assertRaisesRegex(ValueError, 'review metadata'):
                scope.inspect(self.root)
            (self.root / name / 'test_hidden.py').unlink()
            (self.root / name).rmdir()
            (self.root / name).symlink_to(self.root / 'pyproject.toml')
            with self.assertRaisesRegex(ValueError, 'review metadata'):
                scope.inspect(self.root)
            (self.root / name).unlink()
        self.write('tests/data/mutie.dismissals.toml', 'ordinary fixture')
        inspected = scope.inspect(self.root)
        self.assertIn('tests/data/mutie.dismissals.toml', inspected['data_files'])
        self.write('tests/data/mutie.dismissals.toml', 'changed fixture')
        self.assertNotEqual(scope.inspect(self.root)['files'], inspected['files'])

    def test_input_paths_and_binary_temporary_output_have_narrow_entry_points(self):
        source = '''from pathlib import Path
import os
DATA = Path(__file__).parent / "data"
FILES = tuple(DATA.glob("**/*.toml"))
def test_data(tmp_path):
    iterations = int(os.environ.get("PROFILER_ITERATIONS", 1))
    for path in FILES:
        assert path.read_bytes()
    path = tmp_path / "output"
    with open(path, "wb") as stream:
        stream.write(b"ok")
    assert path.read_bytes() == b"ok"
'''
        self.write('tests/test_sample.py', source)
        self.write('tests/data/sample.toml', 'value=1\n')
        result = scope.inspect(self.root)
        self.assertIn('tests/data/sample.toml', result['data_files'])
        self.assertIn('tests/data', result['directories'])
        (self.root / 'tests/data/empty').mkdir()
        self.assertNotEqual(scope.inspect(self.root)['directories'], result['directories'])
        for old, new in [('Path(__file__)', 'Path("/tmp")'),
                         ('from pathlib import Path', 'from pathlib import PurePath as Path'),
                         ('"wb"', '"rb"'),
                         ('open(path, "wb")', 'open(path, "wb", opener=lambda *a: 1)'),
                         ('"PROFILER_ITERATIONS"', '"SECRET"'),
                         ('"PROFILER_ITERATIONS", 1', '"PROFILER_ITERATIONS", 100'),
                         ('int(os.environ.get("PROFILER_ITERATIONS", 1))', 'os.environ'),
                         ('int(os.environ.get("PROFILER_ITERATIONS", 1))', 'os.system("true")')]:
            with self.subTest(new=new):
                self.write('tests/test_sample.py', source.replace(old, new))
                with self.assertRaises(ValueError):
                    scope.inspect(self.root)

    def test_versioned_development_extras_and_inert_numeric_tool_settings(self):
        config = ('[project]\nname="sample"\n'
                  '[project.optional-dependencies]\ndev=["pytest>=7.0", "mypy>=1.0", "ruff>=0.4"]\n'
                  '[tool.ruff]\ntarget-version="py310"\nline-length=88\n'
                  '[tool.mypy]\npython_version="3.10"\nwarn_return_any=true\n'
                  'warn_unused_configs=true\ndisallow_untyped_defs=true\n')
        self.write('pyproject.toml', config)
        result = scope.inspect(self.root)
        self.assertIn('pyproject.toml', result['files'])
        for old, new in [('pytest>=7.0', 'pytest>=99.0'),
                         ('mypy>=1.0', 'unreviewed>=1.0'),
                         ('ruff>=0.4', 'ruff @ https://example.invalid/tool'),
                         ('warn_return_any=true', 'warn_return_any="true"'),
                         ('python_version="3.10"', 'plugins=["evil"]'),
                         ('target-version="py310"', 'target-version="unknown"')]:
            with self.subTest(new=new):
                self.write('pyproject.toml', config.replace(old, new))
                with self.assertRaises(ValueError):
                    scope.inspect(self.root)

    def test_comparison_tokens_preserve_unicode_comments_and_native_evaluation(self):
        source = ('label = "π < >"\n'
                  'def less(left, right): return (left()) < (right())\n'
                  'def le(left, right): return left() <= right()\n'
                  'def greater(left, right): return left() > right()\n'
                  'def ge(left, right): return left() >= right()\n'
                  'def multiline(a,b):\n return (a # < ignored\n         <= b)\n'
                  'def chained(a,b,c): return a < b <= c\n'
                  'def other(a,b): return a == b or a is b or a in b\n'
                  'annotation: list[1 < 2]\n')
        self.write('sample/__init__.py', source)
        sites = [m for m in scope.inspect(self.root)['mutations'] if (m['original'], m['replacement']) in {('<','<='), ('<=','<'), ('>','>='), ('>=','>')}]
        self.assertEqual([m['original'] for m in sites], ['<', '<=', '>', '>=', '<='])
        for site in sites:
            raw = source.encode()
            self.assertEqual(raw[site['start']:site['end']].decode(), site['original'])
            changed = raw[:site['start']] + site['replacement'].encode() + raw[site['end']:]
            namespace = {}
            exec(changed, namespace)
            trace = []
            def left():
                trace.append('left')
                return 4
            def right():
                trace.append('right')
                return 4
            if site['line'] < 6:
                name = ['less', 'le', 'greater', 'ge'][site['line'] - 2]
                self.assertEqual(namespace[name](left, right), site['replacement'] in ('<=', '>='))
                self.assertEqual(trace, ['left', 'right'])
        self.write('tests/test_sample.py', 'def test_boundary(): assert 1 < 2\n')
        self.assertEqual([m for m in scope.inspect(self.root)['mutations'] if (m['original'], m['replacement']) in {('<','<='), ('<=','<'), ('>','>='), ('>=','>')}], sites)

    def test_comparison_utf8_nested_and_string_tokens(self):
        source = ('label = "π"; result = (1) < (2)\n'
                  'nested = (1 < 2) >= (3 > 4)\n'
                  'formatted = f"value {1 <= 2}"\n'
                  'unequal = 1 != 2\n')
        self.write('sample/__init__.py', source)
        sites = [m for m in scope.inspect(self.root)['mutations'] if (m['original'], m['replacement']) in {('<','<='), ('<=','<'), ('>','>='), ('>=','>')}]
        self.assertEqual([m['original'] for m in sites], ['<', '<', '>=', '>', '<='])
        for site in sites:
            raw = source.encode()
            self.assertEqual(raw[site['start']:site['end']].decode(), site['original'])
            changed = raw[:site['start']] + site['replacement'].encode() + raw[site['end']:]
            compile(changed, 'changed', 'exec')

    def test_comparison_physical_newlines_and_unicode_separators(self):
        for newline in ('\n', '\r\n'):
            source = 'label = "π\u2028x"; value = 1 < 2' + newline + 'other = 3 >= 4' + newline
            self.write('sample/__init__.py', source)
            sites = [m for m in scope.inspect(self.root)['mutations'] if (m['original'], m['replacement']) in {('<','<='), ('<=','<'), ('>','>='), ('>=','>')}]
            self.assertEqual([m['line'] for m in sites], [1, 2])
            for site in sites:
                self.assertEqual(source.encode()[site['start']:site['end']].decode(), site['original'])

    def test_comparisons_in_type_parameter_bounds_and_defaults_are_excluded(self):
        self.write('sample/__init__.py',
                   'def bounded[T: (1 < 2)](value: T) -> T: return value\n'
                   'def defaulted[T = (1 <= 2)](value: T) -> T: return value\n')
        self.assertEqual([m for m in scope.inspect(self.root)['mutations'] if (m['original'], m['replacement']) in {('<','<='), ('<=','<'), ('>','>='), ('>=','>')}], [])

    def test_comparison_native_custom_dispatch_and_exception_order(self):
        source = 'def compare(left, right): return left() < right()\n'
        self.write('sample/__init__.py', source)
        site = [m for m in scope.inspect(self.root)['mutations'] if (m['original'], m['replacement']) in {('<','<='), ('<=','<'), ('>','>='), ('>=','>')}][0]
        changed = source[:site['start']] + site['replacement'] + source[site['end']:]
        namespace = {}
        exec(changed, namespace)
        trace = []
        class Operand:
            def __lt__(self, other):
                raise AssertionError('wrong dispatch')
            def __le__(self, other):
                trace.append('le')
                raise RuntimeError('custom comparison')
        def left():
            trace.append('left')
            return Operand()
        def right():
            trace.append('right')
            return Operand()
        with self.assertRaisesRegex(RuntimeError, 'custom comparison'):
            namespace['compare'](left, right)
        self.assertEqual(trace, ['left', 'right', 'le'])
        trace.clear()
        def failing_left():
            trace.append('left')
            raise ValueError('operand')
        with self.assertRaisesRegex(ValueError, 'operand'):
            namespace['compare'](failing_left, right)
        self.assertEqual(trace, ['left'])

    def test_logical_binary_scope_and_coordinates(self):
        source = ('label = "π and or"\n'
                  'def both(a,b): return a and b\n'
                  'def either(a,b): return (a)or(b)\n'
                  'def multiline(a,b): return (a # and in comment\n'
                  ' or b)\n'
                  'def excluded(a,b,c): return a and b and c, a or b or c, a & b, a | b\n'
                  'def nested(a,b,c): return (a and b) or c\n'
                  'annotation: list[1 and 2]\n'
                  'def bounded[T: (1 or 2)](x:T) -> list[1 and 2]: return x\n'
                  'def formatted(a,b): return f"{a and b}"\n')
        for newline in ('\n','\r\n'):
            text = source.replace('\n',newline)
            self.write('sample/__init__.py',text)
            sites = scope.inspect(self.root)['mutations']
            self.assertEqual([m['original'] for m in sites], ['and','or','or','and','or','and'])
            raw = text.encode()
            for site in sites:
                self.assertEqual(raw[site['start']:site['end']].decode(),site['original'])
                compile(raw[:site['start']] + site['replacement'].encode() + raw[site['end']:], 'mutated', 'exec')
        self.write('tests/test_sample.py','def test_logical(): assert 1 and 2\n')
        self.assertEqual(scope.inspect(self.root)['mutations'],sites)

    def test_logical_grouping_and_stateful_short_circuit_match_ast_swap(self):
        import ast
        expressions = [
            'a() and b()', 'a() or b()',
            'a() and b() or c()', 'a() or b() and c()',
            '(a() or b()) and c()', 'a() and (b() or c())',
            '(a() and b()) or c()', 'a() or (b() and c())',
            'a() and b() or c() and d()',
            '(a() or b()) and (c() or d())',
            'a() or b() and c() or d()',
            '(a() and b()) and c()', 'a() or (b() or c())',
        ]
        contexts = ['return EXPR', 'return [EXPR]',
                    "if EXPR: return 'yes'\n return 'no'", 'return not (EXPR)']
        checked = 0
        for expression in expressions:
            for context in contexts:
                source = 'def value(a,b,c,d):\n ' + context.replace('EXPR',expression) + '\n'
                self.write('sample/__init__.py',source)
                sites = [m for m in scope.inspect(self.root)['mutations'] if m['original'] in ('and','or')]
                for site in sites:
                    expected = ast.parse(source)
                    candidates = []
                    for node in ast.walk(expected):
                        if isinstance(node,ast.BoolOp) and len(node.values) == 2:
                            left,right = node.values
                            if left.end_lineno == site['line'] == right.lineno and left.end_col_offset <= site['column']-1 < right.col_offset:
                                candidates.append(node)
                    self.assertEqual(len(candidates),1)
                    node = candidates[0]
                    node.op = ast.Or() if isinstance(node.op,ast.And) else ast.And()
                    changed = source[:site['start']] + site['replacement'] + source[site['end']:]
                    for mask in range(16):
                        for mode in ('stable','toggle','raise_second','raise_right'):
                            def execute(code):
                                trace = []
                                class Value:
                                    def __init__(self,name,truth): self.name,self.truth,self.calls = name,truth,0
                                    def __bool__(self):
                                        self.calls += 1
                                        trace.append(('bool',self.name,self.calls))
                                        if mode == 'raise_second' and self.calls == 2: raise ValueError('second truthiness')
                                        return self.truth if mode != 'toggle' or self.calls % 2 else not self.truth
                                def operand(name,index):
                                    value = Value(name,bool(mask & (1 << index)))
                                    def get():
                                        trace.append(('operand',name))
                                        if mode == 'raise_right' and name == 'b': raise ValueError('right operand')
                                        return value
                                    return get
                                namespace = {}
                                exec(code,namespace)
                                try:
                                    result = namespace['value'](*(operand(n,i) for i,n in enumerate('abcd')))
                                    if isinstance(result,list): result = [('raw',v.name) if isinstance(v,Value) else v for v in result]
                                    elif isinstance(result,Value): result = ('raw',result.name)
                                except ValueError as error: result = ('error',str(error))
                                return result,trace
                            self.assertEqual(execute(changed),execute(compile(expected,'expected','exec')), (expression,context,site,mask,mode))
                            checked += 1
        self.assertGreater(checked,4000)

    def test_logical_raw_values_and_truthiness_failures(self):
        for original,replacement in [('and','or'),('or','and')]:
            source = f'def value(a,b): return a {original} b\n'
            self.write('sample/__init__.py',source)
            site, = scope.inspect(self.root)['mutations']
            namespace = {}
            exec(source[:site['start']] + site['replacement'] + source[site['end']:],namespace)
            for left in ([], [1], 0, 7, '', 'value', None):
                right = object()
                expected = (left or right) if replacement == 'or' else (left and right)
                self.assertIs(namespace['value'](left,right),expected)
            class Bad:
                def __bool__(self): raise RuntimeError('truthiness')
            with self.assertRaisesRegex(RuntimeError,'truthiness'): namespace['value'](Bad(),object())

    def test_inversion_native_dispatch_order_nan_and_raw_results(self):
        import math
        pairs = [('==','!='), ('!=','=='), ('<','>='), ('<=','>'), ('>','<='), ('>=','<')]
        methods = {'==':'__eq__', '!=':'__ne__', '<':'__lt__', '<=':'__le__', '>':'__gt__', '>=':'__ge__'}
        for original, replacement in pairs:
            source = f'def compare(left,right): return left() {original} right()\n'
            self.write('sample/__init__.py', source)
            sites = scope.inspect(self.root)['mutations']
            site, = [m for m in sites if m['replacement'] == replacement]
            namespace = {}
            exec(source[:site['start']] + replacement + source[site['end']:], namespace)
            trace = []
            marker = object()
            def method(self, other):
                trace.append(replacement)
                return marker
            def wrong(self, other): raise AssertionError('old operator invoked')
            Operand = type('Operand', (), {methods[original]:wrong, methods[replacement]:method})
            def left(): trace.append('left'); return Operand()
            def right(): trace.append('right'); return Operand()
            self.assertIs(namespace['compare'](left,right), marker)
            self.assertEqual(trace, ['left','right',replacement])
            trace.clear()
            def fail(): trace.append('left'); raise RuntimeError('operand')
            with self.assertRaisesRegex(RuntimeError, 'operand'): namespace['compare'](fail,right)
            self.assertEqual(trace, ['left'])
            if original not in ('==','!='):
                self.assertIs(namespace['compare'](lambda:math.nan,lambda:1), False)
            self.assertEqual(namespace['compare'](lambda:-0.0,lambda:0.0), eval('0.0 ' + replacement + ' 0.0'))
        # NotImplemented and subclass priority remain native reflected dispatch.
        self.write('sample/__init__.py', 'def compare(a,b): return a < b\n')
        site, = [m for m in scope.inspect(self.root)['mutations'] if m['replacement'] == '>=']
        namespace = {}
        source = 'def compare(a,b): return a < b\n'
        exec(source[:site['start']] + site['replacement'] + source[site['end']:], namespace)
        class Left:
            def __ge__(self, other): return NotImplemented
        class Right:
            def __le__(self, other): return 'reflected'
        self.assertEqual(namespace['compare'](Left(),Right()), 'reflected')
        class Parent:
            def __ge__(self, other): raise AssertionError('subclass should take priority')
        class Child(Parent):
            def __le__(self, other): return 'subclass'
        self.assertEqual(namespace['compare'](Parent(),Child()), 'subclass')


    def test_negation_ast_grouping_trivia_nested_and_exclusions(self):
        import ast
        source = ('label = "π not"\n'
                  'def f(a,b):\n'
                  ' not(a())\n'
                  ' return not (a() # comment\n'
                  '             < b())\n'
                  'def nested(a): return not not a\n'
                  'def mixed(a,b): return (not a) == b\n'
                  'def chain(a,b,c): return a < b < c\n'
                  'def excluded(a,b): return ~a, a is not b, a not in b\n'
                  'annotation: list[not 1]\n'
                  'def bound[T: (not 1)](x:T): return x\n'
                  'def formatted(a): return f"{not a}"\n')
        for newline in ('\n','\r\n'):
            text = source.replace('\n',newline)
            self.write('sample/__init__.py',text)
            sites = scope.inspect(self.root)['mutations']
            negations = [m for m in sites if m['original'].startswith('not')]
            self.assertEqual(len(negations),6)
            for site in negations:
                raw = text.encode()
                self.assertEqual(raw[site['start']:site['end']].decode(),site['original'])
                changed = raw[:site['start']] + site['replacement'].encode() + raw[site['end']:]
                class Remove(ast.NodeTransformer):
                    def visit_UnaryOp(self,node):
                        if (node.lineno,node.col_offset + 1) == (site['line'],site['column']):
                            return node.operand
                        return self.generic_visit(node)
                expected = Remove().visit(ast.parse(text))
                self.assertEqual(ast.dump(ast.parse(changed)), ast.dump(expected))
            self.assertFalse(any(m['line'] in (8,9,10,11) for m in sites))

    def test_negation_keyword_boundaries(self):
        import ast
        for expression in ('not[1]', 'not{1}', 'not"text"', 'not~1', 'not-1', 'not+1', 'not...', 'not# comment\n 1', 'not\x0c 1', 'not\\\n 1'):
            source = 'def value(): return (' + expression + ')\n'
            self.write('sample/__init__.py', source)
            site, = scope.inspect(self.root)['mutations']
            raw = source.encode()
            changed = raw[:site['start']] + site['replacement'].encode() + raw[site['end']:]
            expected = ast.parse(source)
            expected.body[0].body[0].value = expected.body[0].body[0].value.operand
            self.assertEqual(ast.dump(ast.parse(changed)), ast.dump(expected))

    def test_negation_removal_raw_value_and_no_truthiness(self):
        source = 'def negate(operand): return not operand()\n'
        self.write('sample/__init__.py',source)
        site, = scope.inspect(self.root)['mutations']
        namespace = {}
        exec(source[:site['start']] + site['replacement'] + source[site['end']:],namespace)
        class Value:
            def __bool__(self): raise RuntimeError('truthiness must be removed')
        for value in (Value(), [], [1], 0, 8, None):
            trace = []
            def operand(): trace.append('operand'); return value
            self.assertIs(namespace['negate'](operand),value)
            self.assertEqual(trace,['operand'])
        def fail(): raise ValueError('operand')
        with self.assertRaisesRegex(ValueError,'operand'): namespace['negate'](fail)

    def test_arithmetic_tokens_exclusions_and_grouping(self):
        import ast
        source = ('label = "π + \u2028 -"\n'
                  'def expression(a,b,c): return (a + b) * (-c) / (a - b)\n'
                  'def multiline(a,b): return (a # + ignored\n - b)\n'
                  'def excluded(a,b):\n a += b\n return -a, +b, a // b, a % b, a ** b\n'
                  'annotation: list[1 + 2]\n'
                  'def bound[T: (1 * 2)](x:T) -> list[1 / 2]: return x\n'
                  'def unpack(*args, **kwargs): return args\n'
                  'formatted = f"{1 + 2}"\n')
        for newline in ('\n', '\r\n'):
            text = source.replace('\n', newline)
            self.write('sample/__init__.py', text)
            sites = scope.inspect(self.root)['mutations']
            self.assertEqual([m['original'] for m in sites], ['+', '*', '/', '-', '-', '%', '+'])
            for site in sites:
                raw = text.encode()
                self.assertEqual(raw[site['start']:site['end']].decode(), site['original'])
                changed = raw[:site['start']] + site['replacement'].encode() + raw[site['end']:]
                expected = ast.parse(text)
                mapping = {ast.Add: ast.Sub, ast.Sub: ast.Add, ast.Mult: ast.Div, ast.Div: ast.Mult, ast.Mod: ast.Mult}
                # Only the native operator class changes; all grouping and operands remain.
                matched = []
                for node in ast.walk(expected):
                    if isinstance(node, ast.BinOp):
                        lines = raw.splitlines(keepends=True)
                        low = sum(map(len, lines[:node.left.end_lineno-1])) + node.left.end_col_offset
                        high = sum(map(len, lines[:node.right.lineno-1])) + node.right.col_offset
                        if low <= site['start'] < high:
                            node.op = mapping[type(node.op)]()
                            matched.append(node)
                self.assertEqual(len(matched), 1)
                self.assertEqual(ast.dump(ast.parse(changed)), ast.dump(expected))
        self.write('tests/test_sample.py', 'def test_math(): assert 1 + 2 == 3\n')
        self.assertEqual(scope.inspect(self.root)['mutations'], sites)

    def test_arithmetic_native_order_dispatch_and_errors(self):
        import math
        for original, replacement, expected in [('+','-',4), ('-','+',12), ('*','/',2), ('/','*',32)]:
            source = f'def calculate(left,right): return left() {original} right()\n'
            self.write('sample/__init__.py', source)
            site, = scope.inspect(self.root)['mutations']
            self.assertEqual(site['replacement'], replacement)
            changed = source[:site['start']] + replacement + source[site['end']:]
            namespace = {}
            exec(changed, namespace)
            trace = []
            def left():
                trace.append('left')
                return 8
            def right():
                trace.append('right')
                return 4
            self.assertEqual(namespace['calculate'](left, right), expected)
            self.assertEqual(trace, ['left','right'])
            trace.clear()
            def failing():
                trace.append('left')
                raise ValueError('operand')
            with self.assertRaisesRegex(ValueError, 'operand'):
                namespace['calculate'](failing, right)
            self.assertEqual(trace, ['left'])
            if original == '*':
                with self.assertRaises(ZeroDivisionError):
                    namespace['calculate'](lambda: 1, lambda: 0)
                self.assertEqual(math.copysign(1.0, namespace['calculate'](lambda: -0.0, lambda: 2)), -1.0)
                self.assertIsInstance(namespace['calculate'](lambda: 8, lambda: 4), float)
                self.assertTrue(math.isnan(namespace['calculate'](lambda: float('nan'), lambda: 2)))
            if original == '+':
                class Left:
                    def __sub__(self, other): return NotImplemented
                class Right:
                    def __rsub__(self, other): raise RuntimeError('reflected dispatch')
                with self.assertRaisesRegex(RuntimeError, 'reflected dispatch'):
                    namespace['calculate'](Left, Right)
                with self.assertRaises(TypeError):
                    namespace['calculate'](lambda: 'a', lambda: 'b')

    def test_modulo_grouping_coordinates_and_exclusions(self):
        import ast
        source = ('label = "π % text"\n'
                  'def f(a,b,c): return a % b * c, a * (b % c), a % (b % c), a ** b % c, -a % b\n'
                  'def multiline(a,b): return (a # % trivia\n % b)\n'
                  'def augmented(a,b):\n a %= b\n return a // b\n'
                  'annotation: list[1 % 2]\n'
                  'def bound[T: (1 % 2)](a: list[1 % 2]) -> list[1 % 2]: return a\n'
                  'def formatted(a,b): return f"{a % b}"\n'
                  'def string(a): return "%s" % a\n')
        for newline in ('\n', '\r\n'):
            text = source.replace('\n', newline)
            self.write('sample/__init__.py', text)
            self.write('tests/test_sample.py', 'def test_modulo(): assert 3 % 2 == 1\n')
            sites = [m for m in scope.inspect(self.root)['mutations'] if m['original'] == '%']
            self.assertEqual(len(sites), 9)
            raw = text.encode()
            for site in sites:
                self.assertEqual(raw[site['start']:site['end']], b'%')
                self.assertEqual(site['replacement'], '*')
                changed = raw[:site['start']] + b'*' + raw[site['end']:]
                expected = ast.parse(text)
                matched = []
                for node in ast.walk(expected):
                    if isinstance(node, ast.BinOp) and isinstance(node.op, ast.Mod):
                        lines = raw.splitlines(keepends=True)
                        low = sum(map(len, lines[:node.left.end_lineno-1])) + node.left.end_col_offset
                        high = sum(map(len, lines[:node.right.lineno-1])) + node.right.col_offset
                        if low <= site['start'] < high:
                            node.op = ast.Mult()
                            matched.append(node)
                self.assertEqual(len(matched), 1)
                self.assertEqual(ast.dump(ast.parse(changed)), ast.dump(expected))

    def test_modulo_native_values_formatting_dispatch_and_errors(self):
        import math
        source = 'def calculate(left,right): return left() % right()\n'
        self.write('sample/__init__.py', source)
        site, = scope.inspect(self.root)['mutations']
        namespace = {}
        exec(source[:site['start']] + site['replacement'] + source[site['end']:], namespace)
        calculate = namespace['calculate']
        for left, right in [(8,3), (-8,3), (8,-3), (-8,-3), (8.5,2.0), (True,3),
                            (2**100,7), (3,0), (-0.0,2), (math.inf,2), (math.nan,2),
                            ('%s',3), (b'%d',2), ([1],3)]:
            trace = []
            def lhs(): trace.append('left'); return left
            def rhs(): trace.append('right'); return right
            result = calculate(lhs,rhs)
            expected = left * right
            if isinstance(expected,float) and math.isnan(expected): self.assertTrue(math.isnan(result))
            else: self.assertEqual(result,expected)
            if isinstance(result,float) and result == 0: self.assertEqual(math.copysign(1,result), math.copysign(1,expected))
            self.assertEqual(type(result),type(expected))
            self.assertEqual(trace,['left','right'])
        # Formatting is deliberately syntactic: a numeric RHS repeats a string;
        # mapping/tuple/string RHS incompatibility remains a native TypeError.
        for left,right in [('%s','text'), ('%(x)s',{'x':'text'}), ('%s',(1,))]:
            with self.assertRaises(TypeError): calculate(lambda:left,lambda:right)
        trace = []
        marker = object()
        class Left:
            def __mod__(self, other): raise AssertionError('old dispatch')
            def __mul__(self, other): trace.append('mul'); return NotImplemented
        class Right:
            def __rmul__(self, other): trace.append('rmul'); return marker
        def lhs(): trace.append('left'); return Left()
        def rhs(): trace.append('right'); return Right()
        self.assertIs(calculate(lhs,rhs),marker)
        self.assertEqual(trace,['left','right','mul','rmul'])
        class Child(Left):
            def __rmul__(self, other): trace.append('subclass'); return marker
        trace.clear()
        self.assertIs(calculate(Left,Child),marker)
        self.assertEqual(trace,['subclass'])
        class Failing:
            def __mul__(self,other): raise ValueError('dispatch')
        with self.assertRaisesRegex(ValueError,'dispatch'): calculate(Failing,Right)
        for position in ('left','right'):
            trace.clear()
            def bad(): trace.append(position); raise RuntimeError('operand')
            with self.assertRaisesRegex(RuntimeError,'operand'):
                calculate(bad if position == 'left' else lhs, bad if position == 'right' else rhs)
            self.assertEqual(trace, ['left'] if position == 'left' else ['left','right'])

    def test_conftest_helpers_are_audited_and_imports_are_bounded(self):
        self.write('tests/__init__.py', '"""Local tests."""\n')
        self.write('tests/conftest.py', 'import pytest\ndef helper(value=True): return value\n'
                   '@pytest.fixture\ndef shared(): return helper()\n')
        self.write('tests/test_sample.py', 'from .conftest import helper\n'
                   'def test_helper(shared): assert helper(shared)\n')
        result = scope.inspect(self.root)
        self.assertEqual(result['conftests'], ['tests/conftest.py'])
        self.assertEqual(len(result['mutations']), 1)
        for header in ['from .conftest import shared', 'from .conftest import helper as renamed',
                       'from tests.conftest import helper', 'from ..conftest import helper',
                       'from .conftest import *', 'from .missing import helper']:
            self.write('tests/test_sample.py', header + '\ndef test_helper(): pass\n')
            with self.subTest(header=header), self.assertRaises(ValueError):
                scope.inspect(self.root)
        self.write('tests/test_sample.py', 'from .conftest import helper\ndef test_helper(): pass\n')
        (self.root / 'tests/__init__.py').unlink()
        with self.assertRaises(ValueError):
            scope.inspect(self.root)
        self.write('tests/__init__.py', '')
        for body in ['def helper(): return open("outside")', 'async def helper(): pass',
                     'def helper(value=calculate()): pass', 'def pytest_configure(): pass',
                     'def test_hidden(): pass', 'class Helper: pass']:
            self.write('tests/conftest.py', body)
            with self.subTest(body=body), self.assertRaises(ValueError):
                scope.inspect(self.root)

    def test_route_stdlib_access_is_narrow(self):
        self.write('sample/__init__.py', 'from abc import ABC, abstractmethod\n'
                   'from time import sleep, time\nfrom types import MappingProxyType\n'
                   'class Cost:\n input: float = 0.0\n'
                   'def enabled(): return MappingProxyType({"value": True})\n')
        self.write('tests/test_sample.py', 'import json\ndef test_json():\n'
                   ' assert json.loads(json.dumps({"value": True}))["value"]\n')
        scope.inspect(self.root)
        for text in ['from types import FunctionType', 'from abc import get_cache_token',
                     'import time', 'from time import clock_settime', 'value=input',
                     'class Cost:\n input: float = input()']:
            self.write('sample/__init__.py', text)
            with self.subTest(text=text), self.assertRaises(ValueError):
                scope.inspect(self.root)
        self.write('sample/__init__.py', 'value=True')
        for text in ['import json.tool', 'import json\njson.load(None)',
                     'import json\nvalue=json', 'import json\njson.dumps=None']:
            self.write('tests/test_sample.py', text)
            with self.subTest(text=text), self.assertRaises(ValueError):
                scope.inspect(self.root)

    def test_static_route_metadata_does_not_admit_plugins(self):
        manifest = ('[project]\nname="sample"\n[project.optional-dependencies]\n'
                    'dev=["mypy","pytest","pytest-cov","ruff"]\n'
                    '[tool.hatch.build.targets.wheel]\npackages=["sample"]\n'
                    '[tool.ruff]\ntarget-version="py311"\n'
                    '[tool.mypy]\npython_version="3.11"\nstrict=true\nfiles=["src","tests","examples"]\n')
        self.write('pyproject.toml', manifest)
        scope.inspect(self.root)
        for old, new in [('"mypy","pytest"', '"plugin","pytest"'),
                         ('packages=["sample"]', 'packages=["outside"]'),
                         ('strict=true', 'plugins=["hook.py"]'),
                         ('target-version="py311"', 'extend="external.toml"')]:
            self.write('pyproject.toml', manifest.replace(old, new))
            with self.subTest(new=new), self.assertRaises(ValueError):
                scope.inspect(self.root)

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
        for source in ["import random\nrandom.sample(range(3), 1)", "value=Exception.__init__", "Exception.__init__(object())",
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
            "class TestHidden: pass", "def helper(value=make_value()): pass", "raise RuntimeError('imported')",
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
        for path in ['doc/source/conf.py', '.circleci/build_docs.py', 'examples/demo.py']:
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
        for path in ['.circleci/test_hidden.py', '.circleci/hidden_test.py', 'doc/source/test_hidden.py', 'examples/test_hidden.py']:
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
