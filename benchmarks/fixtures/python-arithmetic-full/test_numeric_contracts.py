"""Exact public outputs beyond normalization and broad range assertions."""
import math
import pytest
from betting_math_kit.devig import (
    devig, devig_multiplicative, devig_power, devig_additive, devig_shin, devig_multi,
)
from betting_math_kit.types import DevigMethod
from betting_math_kit.metrics import calibration_buckets, expected_calibration_error, edge_bucket_analysis, log_loss
from betting_math_kit.odds import kelly_fraction, kelly_calibrated
from betting_math_kit.simulation import simulate_bankroll, optimal_fraction_search


@pytest.mark.parametrize('calculate', [devig, devig_multiplicative, devig_power, devig_additive, devig_shin])
def test_original_margin_is_reported(calculate):
    # American -150 and +100 imply .6 and .5: margin .1, independent of method.
    result = calculate(-150, 100)
    assert result.vig == pytest.approx(0.1, abs=1e-12)
    assert result.fair_home > result.fair_away


@pytest.mark.parametrize('method', [DevigMethod.MULTIPLICATIVE, DevigMethod.POWER, DevigMethod.SHIN])
def test_multi_outcome_margin_is_reported(method):
    # Three even-money outcomes imply 1.5 in total, hence a .5 margin.
    assert devig_multi([100, 100, 100], method=method).vig == pytest.approx(0.5, abs=1e-12)


def test_additive_removes_equal_margin_from_unequal_sides():
    result = devig_additive(-150, 100)
    assert result.fair_home == pytest.approx(0.55, abs=1e-12)
    assert result.fair_away == pytest.approx(0.45, abs=1e-12)
    # At an extreme valid skew, the .01 floor requires final normalization.
    try:
        clipped = devig_additive(-19900, 199900)
    except ValueError as error:
        pytest.fail(f'valid odds must remain normalized after flooring: {error}')
    assert clipped.fair_home + clipped.fair_away == pytest.approx(1.0, abs=1e-12)
    assert 0.0 < clipped.fair_away < clipped.fair_home < 1.0



def test_calibration_bin_edges_means_and_weights():
    probs = [0.125, 0.375, 0.625, 0.875]
    outcomes = [0, 1, 1, 1]
    assert calibration_buckets(probs, outcomes, n_bins=2) == [
        dict(bin_lower=0.0, bin_upper=0.5, count=2, avg_predicted=0.25, avg_actual=0.5, gap=-0.25),
        dict(bin_lower=0.5, bin_upper=1.0, count=2, avg_predicted=0.75, avg_actual=1.0, gap=-0.25),
    ]
    assert expected_calibration_error(probs, outcomes, n_bins=2) == 0.25


def test_edge_bins_preserve_offsets_and_average_counts():
    assert edge_bucket_analysis([0.25, 0.5, 0.75, 1.0], [0, 1, 1, 1],
                                [0.25, 0.375, 0.625, 0.75], n_bins=2) == [
        dict(bin_lower=0.25, bin_upper=0.5, count=2, avg_edge=0.3125, win_rate=0.5, expected_win_rate=0.375),
        dict(bin_lower=0.5, bin_upper=0.75, count=2, avg_edge=0.6875, win_rate=1.0, expected_win_rate=0.875),
    ]


def test_identical_edges_still_return_rates_not_totals():
    assert edge_bucket_analysis([0.25, 0.5, 0.75, 1.0], [0, 1, 1, 1], [0.25]*4) == [
        dict(bin_lower=0.25, bin_upper=0.25, count=4, avg_edge=0.25, win_rate=0.75, expected_win_rate=0.625),
    ]


def test_non_even_odds_kelly_magnitude():
    assert kelly_fraction(0.5, 200) == 0.125
    stake, edge, fair, should_bet = kelly_calibrated(0.5, 200, -200,
        kelly_fraction_mult=0.5, max_stake=1.0)
    assert stake == 0.125
    assert edge == pytest.approx(1.0/6.0)
    assert fair == pytest.approx(1.0/3.0)
    assert should_bet is True


@pytest.mark.parametrize('odds, final', [(2.0, 75.0), (3.0, 100.0)])
def test_seeded_two_step_trajectory_and_drawdown(odds, final):
    # Seed 1 draws .134... then .847...: win then loss for both probability values.
    result = simulate_bankroll(0.25, odds, 0.5, bankroll=100.0,
                               n_bets=2, n_trials=1, seed=1)
    assert result.mean_final == result.median_final == result.p5_final == result.p95_final == final
    assert result.max_drawdown_median == 0.5
    assert result.growth_rate == pytest.approx(math.log(final/100.0)/2.0)
    assert result.ruin_rate == 0.0


def test_seeded_two_trials_percentiles_and_median():
    # The same two draws now belong to separate one-bet trials: finals 150 and 50.
    result = simulate_bankroll(0.25, 2.0, 0.5, bankroll=100.0,
                               n_bets=1, n_trials=2, seed=1)
    assert result.mean_final == result.median_final == 100.0
    assert result.p5_final == 55.0
    assert result.p95_final == 145.0
    assert result.max_drawdown_median == 0.25
    assert result.growth_rate == pytest.approx((math.log(1.5)+math.log(0.5))/2.0)


def test_fraction_search_uses_documented_incrementing_seeds():
    results = optimal_fraction_search(0.4, 2.0, fractions=[0.25, 0.5],
                                       n_bets=2, n_trials=2, seed=1)
    assert results[0.25] == simulate_bankroll(0.4, 2.0, 0.25,
                                             n_bets=2, n_trials=2, seed=1)
    assert results[0.5] == simulate_bankroll(0.4, 2.0, 0.5,
                                           n_bets=2, n_trials=2, seed=2)


def test_three_trial_percentiles_interpolate_adjacent_values():
    # Finals sorted as [50, 50, 150]; low interpolation must use the next item,
    # not a wrapped negative index selecting the final item.
    result = simulate_bankroll(0.25, 2.0, 0.5, bankroll=100.0,
                               n_bets=1, n_trials=3, seed=1)
    assert result.p5_final == 50.0
    assert result.p95_final == 140.0
    assert result.median_final == 50.0
    assert result.mean_final == pytest.approx(250.0/3.0)


def test_log_loss_clamps_a_valid_wrong_endpoint():
    try:
        actual = log_loss([1.0], [0], eps=0.25)
    except ValueError as error:
        pytest.fail(f'valid probability endpoint must be clamped: {error}')
    assert actual == pytest.approx(math.log(4.0))


@pytest.mark.parametrize('method', [DevigMethod.POWER, DevigMethod.SHIN])
def test_unequal_multi_outcomes_keep_favorite_order(method):
    result = devig_multi([150, -200, 300], method=method)
    assert result.fair_probs[1] > result.fair_probs[0] > result.fair_probs[2]
