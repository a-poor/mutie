from tinycss2 import parse_component_value_list, parse_stylesheet_bytes, serialize_identifier
from tinycss2.serializer import serialize_name


def test_decoded_comments_are_preserved_by_default():
    rules, encoding = parse_stylesheet_bytes(b'/*caf\xe9*/a{}', protocol_encoding='windows-1252')
    assert encoding.name == 'windows-1252'
    assert [rule.type for rule in rules] == ['comment', 'qualified-rule']
    assert rules[0].value == 'caf\xe9'


def test_decoded_whitespace_is_preserved_by_default():
    rules, encoding = parse_stylesheet_bytes(b' \na{}')
    assert encoding.name == 'utf-8'
    assert [rule.type for rule in rules] == ['whitespace', 'qualified-rule']
    assert rules[0].value == ' \n'


def test_identifier_escapes_ascii_delete_at_start():
    rendered = serialize_identifier('\x7f')
    assert rendered == '\\\x7f'
    tokens = parse_component_value_list(rendered)
    assert len(tokens) == 1
    assert tokens[0].type == 'ident'
    assert tokens[0].value == '\x7f'


def test_name_escapes_ascii_delete_after_first_character():
    assert serialize_name('a\x7f') == 'a\\\x7f'
    assert serialize_identifier('a\x7f') == 'a\\\x7f'
