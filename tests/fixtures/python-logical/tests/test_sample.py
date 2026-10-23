from sample import both, either, nested, weak_default, weak_both


def test_both():
    assert both(0, 9) == 0
    assert both(7, 9) == 9


def test_either():
    assert either(0, 9) == 9
    assert either(7, 9) == 7


def test_nested():
    assert nested(0, 7, 9) == 9


def test_default_same_values():
    assert weak_default(0, 0) == 0


def test_both_same_values():
    assert weak_both(7, 7) == 7
