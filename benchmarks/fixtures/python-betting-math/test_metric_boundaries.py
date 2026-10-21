"""Public closed probability intervals and single-bin calibration contracts."""
import pytest
from betting_math_kit.exceptions import InvalidProbabilityError
from betting_math_kit.metrics import calibration_buckets, clv, edge_bucket_analysis


@pytest.mark.parametrize(('opening', 'closing', 'expected'), [
    (0.0, 0.5, 0.5), (1.0, 0.5, -0.5),
    (0.5, 0.0, -0.5), (0.5, 1.0, 0.5),
])
def test_clv_accepts_probability_endpoints(opening, closing, expected):
    try:
        actual = clv(opening, closing)
    except InvalidProbabilityError as error:
        pytest.fail(f'valid endpoint probability was rejected: {error}')
    assert actual == expected


def test_calibration_accepts_one_bin():
    try:
        buckets = calibration_buckets([0.25, 0.75], [0, 1], n_bins=1)
    except ValueError as error:
        pytest.fail(f'one calibration bin must be valid: {error}')
    assert len(buckets) == 1
    assert buckets[0]['count'] == 2
    assert buckets[0]['avg_predicted'] == 0.5
    assert buckets[0]['avg_actual'] == 0.5


def test_edge_analysis_accepts_one_bin():
    try:
        buckets = edge_bucket_analysis([0.25, 0.75], [0, 1], [0.0, 0.5], n_bins=1)
    except ValueError as error:
        pytest.fail(f'one edge bin must be valid: {error}')
    assert len(buckets) == 1
    assert buckets[0]['count'] == 2
    assert buckets[0]['avg_edge'] == 0.25
    assert buckets[0]['win_rate'] == 0.5
    assert buckets[0]['expected_win_rate'] == 0.5


def test_certain_prediction_is_in_last_calibration_bin():
    try:
        buckets = calibration_buckets([1.0], [1], n_bins=2)
    except IndexError as error:
        pytest.fail(f'probability one must fit the final calibration bin: {error}')
    assert len(buckets) == 1
    assert buckets[0]['bin_lower'] == 0.5
    assert buckets[0]['bin_upper'] == 1.0
    assert buckets[0]['count'] == 1
    assert buckets[0]['avg_predicted'] == 1.0
    assert buckets[0]['avg_actual'] == 1.0
