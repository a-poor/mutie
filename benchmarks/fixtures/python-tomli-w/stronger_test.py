import tomli_w


def test_dumps_default_preserves_escaped_newline_form():
    assert tomli_w.dumps({"value": "one\ntwo"}) == 'value = "one\\ntwo"\n'
