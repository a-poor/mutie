from sample import checked, weak

def test_checked():
    assert checked() == {'count': 1}

def test_weak():
    assert weak(False) == {}
