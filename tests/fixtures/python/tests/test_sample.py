from sample import enabled, optional


def test_enabled():
    assert enabled() is True


def test_optional_exists():
    assert callable(optional)
