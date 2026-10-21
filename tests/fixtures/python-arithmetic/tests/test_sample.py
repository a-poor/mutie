import pytest
from sample import add, subtract, multiply, divide, net


@pytest.mark.parametrize('calculate, expected', [(add, 12), (subtract, 4),
                                                (multiply, 32), (divide, 2)])
def test_result_and_order(calculate, expected):
    calls = []
    def left():
        calls.append('left')
        return 8
    def right():
        calls.append('right')
        return 4
    assert calculate(left, right) == expected
    assert calls == ['left', 'right']


def test_no_fee():
    assert net(10, 0) == 10
