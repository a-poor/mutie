import tomli_w


def test_root_values_do_not_receive_a_table_header():
    assert tomli_w.dumps({"value": 1}) == "value = 1\n"


def test_array_of_tables_retains_double_bracket_header():
    assert tomli_w.dumps({"rows": [{"values": [1, 2]}]}) == (
        "[[rows]]\nvalues = [\n    1,\n    2,\n]\n"
    )


def test_key_newline_is_escaped_even_when_multiline_values_are_allowed():
    assert tomli_w.dumps({"a\nb": 1}, multiline_strings=True) == '"a\\nb" = 1\n'


def test_escape_resumes_after_the_escaped_character():
    assert tomli_w.dumps({"value": "a\nb"}) == 'value = "a\\nb"\n'
