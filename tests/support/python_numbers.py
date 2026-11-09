"""Pinned AST discovery and native numeric semantics; never runs upstream code."""
import ast
import importlib.util
import math
from pathlib import Path
import sys
import tempfile
import unittest

SOURCE = Path(__file__).resolve().parents[2] / 'src/python/scope.py'
spec = importlib.util.spec_from_file_location('scope', SOURCE)
scope = importlib.util.module_from_spec(spec)
spec.loader.exec_module(scope)

CASES = [
    ('0', 0, 1), ('-0', 0, 1), ('+ 7', 7, 0), ('-7', -7, 0),
    ('00_0', 0, 1), ('0x_0', 0, 1), ('-0Xf_f', -255, 0),
    ('0b1_0', 2, 0), ('0o17', 15, 0), ('0B_0', 0, 1),
    ('999999999999999999999999999999999999999', 999999999999999999999999999999999999999, 0),
    ('9007199254740993', 9007199254740993, 0),
    ('0.0', 0.0, 1.0), ('-0.0', -0.0, 1.0), ('.5', .5, 0.0), ('1.', 1.0, 0.0),
    ('000.0', 0.0, 1.0), ('1_000.5_0', 1000.5, 0.0), ('2.5e+2', 250.0, 0.0),
    ('1e-999', 0.0, 1.0), ('-1e-999', -0.0, 1.0), ('1e999', math.inf, 0.0),
    ('-1e999', -math.inf, 0.0), ('5e-324', 5e-324, 0.0), ('2e-324', 0.0, 1.0),
    ('2.4703282292062327e-324', 0.0, 1.0), ('2.4703282292062328e-324', 5e-324, 0.0),
    ('9007199254740993.0', 9007199254740992.0, 0.0),
    ('- # sign π\n 0.0', -0.0, 1.0), ('+\\\n 7', 7, 0),
    ('7 .bit_length()', 3, 0), ('-(0.0)', -0.0, -1.0), ('-(-7)', 7, 0),
]


def numeric_sites(source):
    return [m for m in scope.operator_sites(ast.parse(source), source, source.encode(), set(), 'sample/__init__.py')
            if m['replacement'] in ('(0)', '(1)', '(0.0)', '(1.0)')]


class Numbers(unittest.TestCase):
    def test_native_pairs(self):
        for newline in ('\n', '\r\n'):
            for expression, original, replacement in CASES:
                with self.subTest(expression=expression, newline=newline):
                    source = ('# π\nvalue = (' + expression + ')\n').replace('\n', newline)
                    site, = numeric_sites(source)
                    raw = source.encode()
                    self.assertEqual(raw[site['start']:site['end']].decode(), site['original'])
                    changed = raw[:site['start']] + site['replacement'].encode() + raw[site['end']:]
                    for code, expected in [(raw, original), (changed, replacement)]:
                        values = {}
                        exec(compile(code, '<numeric-control>', 'exec'), values)
                        actual = values['value']
                        self.assertIs(type(actual), type(expected))
                        self.assertEqual(actual, expected)
                        if type(actual) is float:
                            self.assertEqual(math.copysign(1, actual), math.copysign(1, expected))

    def test_exclusions_and_context(self):
        source = '''flag = True
imaginary = -2j
text = '123'
annotation: list[1]
def bound[T: list[2]](x: list[3] = 4) -> list[5]: return 6
def match_value(x):
 match x:
  case 7 | -8: return 9
  case {'value': 10}: return 11
  case _: return 12
mapping = {13: 14}
value = mapping[15]
formatted = f'{16}'
'''
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root/'sample').mkdir()
            (root/'tests').mkdir()
            (root/'pyproject.toml').write_text('[project]\nname="sample"\ndependencies=[]\n')
            (root/'sample/__init__.py').write_text(source)
            (root/'tests/test_sample.py').write_text('def test_value(): assert 17 == 17\n')
            sites = [m for m in scope.inspect(root)['mutations'] if m['replacement'] in ('(0)', '(1)', '(0.0)', '(1.0)')]
            self.assertEqual([m['original'] for m in sites], ['4','6','9','11','12','13','14','15','16'])


if __name__ == '__main__':
    if len(sys.argv) == 2 and sys.argv[1] == '--source':
        # The scored test's static listing feeds all these recipes through Rust
        # too, so the two independent readers must agree on native edge cases.
        print('\n'.join('value_' + str(i) + ' = (' + case[0] + ')' for i, case in enumerate(CASES)))
    else:
        unittest.main()
