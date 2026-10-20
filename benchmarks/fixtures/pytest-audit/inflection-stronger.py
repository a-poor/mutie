import inflection


def test_underscore_separates_unicode_digit_from_following_capital():
    assert inflection.underscore("version٣HTTP") == "version٣_http"
