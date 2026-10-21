import pytest
from sample import less, less_equal, greater, greater_equal, within_budget, chained


@pytest.mark.parametrize('compare, expected', [(less, False), (less_equal, True),
                                              (greater, False), (greater_equal, True)])
def test_equality_and_operand_order(compare, expected):
    calls = []
    def left():
        calls.append('left')
        return 4
    def right():
        calls.append('right')
        return 4
    assert compare(left, right) is expected
    assert calls == ['left', 'right']


@pytest.mark.parametrize('compare', [less, less_equal, greater, greater_equal])
def test_nan_remains_unordered(compare):
    assert compare(lambda: float('nan'), lambda: 1.0) is False
    assert compare(lambda: 1.0, lambda: float('nan')) is False


def test_budget_away_from_boundary():
    assert within_budget(1.0, 2.0)
    assert not within_budget(3.0, 2.0)


def test_chain_unchanged():
    assert chained(1, 2, 2)
    assert not chained(2, 2, 3)
