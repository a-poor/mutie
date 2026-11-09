"""Native string-value and static AST controls, using owned source only."""
import ast
import importlib.util
from pathlib import Path
import sys
import tempfile
import unittest

SOURCE = Path(__file__).resolve().parents[2] / 'src/python/scope.py'
spec = importlib.util.spec_from_file_location('scope', SOURCE)
scope = importlib.util.module_from_spec(spec)
spec.loader.exec_module(scope)

CASES = [
    ("''", '', 'x'), ('""', '', 'x'), ("r''", '', 'x'), ('R""', '', 'x'),
    ("u''", '', 'x'), ('U""', '', 'x'), ("''''''", '', 'x'), ('""""""', '', 'x'),
    ("'abc'", 'abc', ''), ('"abc"', 'abc', ''), ("'🦀'", '🦀', ''),
    (r"'\0'", '\0', ''), (r"'\000'", '\0', ''), (r"'\377'", '\xff', ''),
    (r"'\x00'", '\0', ''), (r"'\u0000'", '\0', ''), (r"'\ud800'", '\ud800', ''),
    (r"'\udfff'", '\udfff', ''), (r"'\U0010FFFF'", '\U0010FFFF', ''),
    (r"'\N{SNOWMAN}'", '☃', ''), (r"'\N{NULL}'", '\0', ''),
    (r"'\N{LATIN SMALL LETTER A}'", 'a', ''),
    (r"'\a\b\f\n\r\t\v'", '\a\b\f\n\r\t\v', ''),
    (r"'\\'", '\\', ''), (r"'\''", "'", ''), (r'''"\""''', '"', ''),
    (r"r'\n'", '\\n', ''), (r"r'\x'", '\\x', ''), (r"r'\''", "\\'", ''),
    ("'\\\n'", '', 'x'), ("'\\\r'", '', 'x'), ("'\\\r\n'", '', 'x'),
    ("r'\\\n'", '\\\n', ''), ("'a\\\nb'", 'ab', ''),
    ("'''a\nb'''", 'a\nb', ''), ("r'''a\nb'''", 'a\nb', ''),
    ("'\u2028\u2029'", '\u2028\u2029', ''),
    ("'' ''", '', 'x'), ("r''u''", '', 'x'), ("'a' 'b'", 'ab', ''),
    ("'' # π\n r''", '', 'x'), ("'' # π\n r'\\n'", '\\n', ''),
    ("''\\\n''", '', 'x'), ("'a'\\\n'b'", 'ab', ''),
    ("''.upper()", '', 'X'), ("'ab'.upper()", 'AB', ''),
]


def sites(source):
    return [m for m in scope.operator_sites(ast.parse(source), source, source.encode(), set(), 'sample/__init__.py')
            if m['replacement'] in ('("")', '("x")')]


class Strings(unittest.TestCase):
    def test_native_pairs(self):
        for newline in ('\n', '\r\n'):
            for expression, original, replacement in CASES:
                with self.subTest(expression=expression, newline=newline):
                    # Normalize the case's explicit physical line endings first.
                    expression = expression.replace('\r\n', '\n').replace('\r', '\n')
                    source = ('# π\nvalue = (' + expression + ')\n').replace('\n', newline)
                    site, = sites(source)
                    raw = source.encode()
                    self.assertEqual(raw[site['start']:site['end']].decode(), site['original'])
                    changed = raw[:site['start']] + site['replacement'].encode() + raw[site['end']:]
                    for code, expected in [(raw, original), (changed, replacement)]:
                        values = {}
                        exec(compile(code, '<string-control>', 'exec'), values)
                        self.assertIs(type(values['value']), str)
                        self.assertEqual(values['value'], expected)

    def test_context_exclusions_and_preservation(self):
        source = '''"module doc"
from __future__ import annotations
'additional doc'
value: 'annotation' = 'initial'
class C:
 "class doc"
 value: 'field annotation' = 'field value'
 'attribute doc'
def bound[T: 'bound'](value: 'arg' = 'default') -> 'result':
 "function doc"
 return 'body'
def match_value(value):
 match value:
  case 'pattern': return 'case body'
  case {'pattern key': name} if name == 'guard': return name
mapping = {'key': 'value'}
lookup = mapping['lookup']
raw = r'raw'
bytes_value = b'bytes'
formatted = f"text {'formatted expression'}"
template = t"text {'template expression'}"
'''
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root/'sample').mkdir()
            (root/'tests').mkdir()
            (root/'pyproject.toml').write_text('[project]\nname="sample"\ndependencies=[]\n')
            (root/'sample/__init__.py').write_text(source)
            (root/'tests/test_sample.py').write_text("def test_value(): assert 'test' == 'test'\n")
            result = scope.inspect(root)
            selected = [m for m in result['mutations'] if m['replacement'] in ('("")', '("x")')]
            self.assertEqual([m['original'] for m in selected], ["'initial'", "'field value'", "'default'", "'body'", "'case body'", "'guard'", "'key'", "'value'", "'lookup'", "r'raw'"])
            self.assertEqual((root/'sample/__init__.py').read_text(), source)

    def test_malformed_literals_remain_native_errors(self):
        for expression in [r"'\x0'", r"'\u123'", r"'\U00110000'", r"'\N{NOT A UNICODE NAME}'", r"'\N{KEYCAP NUMBER SIGN}'", "'\0'", "b'a' 'b'", r"r'\'", "'unterminated"]:
            with self.subTest(expression=expression):
                with self.assertRaises((SyntaxError, ValueError)):
                    ast.parse('value = (' + expression + ')')


if __name__ == '__main__':
    if len(sys.argv) == 2 and sys.argv[1] == '--source':
        print('\n'.join('value_' + str(i) + ' = (' + case[0].replace('\r\n', '\n').replace('\r', '\n') + ')' for i, case in enumerate(CASES)))
    else:
        unittest.main()
