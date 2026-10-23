from betting_math_kit.simulation import _median


def test_median_averages_middle_pair_without_reordering_input():
    values = [40.0, 10.0, 30.0, 20.0]
    assert _median(values) == 25.0
    assert values == [40.0, 10.0, 30.0, 20.0]


def test_median_selects_middle_for_odd_length():
    assert _median([30.0, 10.0, 20.0]) == 20.0
