from sample import compare, negate, nested, weak_compare, weak_negate


def test_comparisons():
    assert compare(1, 2) == (False, True, True, True, False, False)


def test_negate():
    assert negate([]) is True
    assert negate([1]) is False


def test_nested():
    assert nested(True) is True


def test_nan():
    assert weak_compare(float('nan'), 1) is False


def test_ignored_result():
    weak_negate(0)
