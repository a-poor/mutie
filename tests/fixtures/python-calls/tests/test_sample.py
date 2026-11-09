from sample import checked, weak


def test_checked():
    assert checked() == [1]


def test_weak():
    assert weak(False) == []
