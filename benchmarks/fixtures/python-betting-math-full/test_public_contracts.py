"""Boundary values and result flags observable through the public APIs."""
from dataclasses import FrozenInstanceError
import pytest
from betting_math_kit.kelly import compute_kelly_bet, full_kelly_fraction, size_race_bets
from betting_math_kit.odds import implied_prob_to_decimal, kelly_calibrated
from betting_math_kit.exceptions import InvalidProbabilityError
from betting_math_kit.simulation import simulate_bankroll
from betting_math_kit.types import DevigMethod, DevigResult, MultiOutcomeDevigResult, EdgeResult, Side


def test_no_edge_has_no_constraint_flags():
    result = compute_kelly_bet(1, 0.25, 2.0, 1024.0, takeout=0.0)
    assert result.bet_size == 0.0
    assert result.pool_limited is False
    assert result.capped is False
    assert result.reason == 'no edge after friction'


def test_unconstrained_bet_has_no_constraint_flags():
    result = compute_kelly_bet(1, 0.75, 2.0, 1024.0, takeout=0.0, max_fraction=1.0)
    assert result.bet_size == 128.0
    assert result.pool_limited is False
    assert result.capped is False


def test_equal_pool_limit_does_not_claim_reduction():
    result = compute_kelly_bet(1, 0.75, 2.0, 1024.0, takeout=0.0,
                               pool_size=512.0, impact_limit=0.5, max_fraction=1.0)
    assert result.bet_size == 128.0
    assert result.pool_limited is False


def test_zero_pool_size_is_ignored():
    result = compute_kelly_bet(1, 0.75, 2.0, 1024.0, takeout=0.0,
                               pool_size=0.0, max_fraction=1.0)
    assert result.bet_size == 128.0
    assert result.pool_limited is False


def test_equal_bankroll_cap_does_not_claim_reduction():
    result = compute_kelly_bet(1, 0.75, 2.0, 1024.0, takeout=0.0, max_fraction=0.125)
    assert result.bet_size == 128.0
    assert result.capped is False


def test_exact_minimum_bet_is_accepted():
    result = compute_kelly_bet(1, 0.75, 2.0, 1024.0, takeout=0.0,
                               max_fraction=1.0, min_bet=128.0)
    assert result.bet_size == 128.0
    assert 'below minimum' not in result.reason


def test_equal_race_exposure_does_not_claim_scaling():
    # Default per-selection cap limits this bet to 102; equality is exact.
    result = size_race_bets([dict(selection_id=1, prob=0.75, odds_decimal=2.0)],
                            1020.0, takeout=0.0, max_race_exposure=0.1)
    assert len(result) == 1
    assert result[0].bet_size == 102.0
    assert 'race-exposure scaled' not in result[0].reason


def test_race_reduction_sets_capped_flag():
    result = size_race_bets([dict(selection_id=1, prob=0.6, odds_decimal=2.0)],
                            1000.0, takeout=0.0, max_race_exposure=0.02)
    assert len(result) == 1
    assert result[0].bet_size == 20.0
    assert result[0].capped is True
    assert 'race-exposure scaled' in result[0].reason


def test_exact_minimum_edge_can_be_selected():
    stake, edge, fair, should_bet = kelly_calibrated(
        0.75, 100, 100, min_edge=0.25, max_stake=1.0)
    assert edge == 0.25
    assert fair == 0.5
    assert stake == 0.125
    assert should_bet is True


def test_zero_kelly_is_not_selected_even_with_zero_edge_threshold():
    stake, edge, fair, should_bet = kelly_calibrated(0.5, 100, 100, min_edge=0.0)
    assert stake == 0.0
    assert edge == 0.0
    assert fair == 0.5
    assert should_bet is False


def test_certain_probability_has_unit_decimal_odds():
    try:
        actual = implied_prob_to_decimal(1.0)
    except InvalidProbabilityError as error:
        pytest.fail(f'closed upper probability endpoint rejected: {error}')
    assert actual == 1.0


def test_full_takeout_leaves_no_edge():
    try:
        actual = full_kelly_fraction(0.75, 2.0, takeout=1.0)
    except ZeroDivisionError as error:
        pytest.fail(f'full takeout must return no edge, not divide by zero: {error}')
    assert actual == 0.0


def test_exact_ruin_threshold_counts_as_ruin():
    result = simulate_bankroll(0.5, 2.0, 0.5, bankroll=100.0,
                                n_bets=1, n_trials=1, seed=1, ruin_threshold=150.0)
    assert result.ruin_rate == 1.0
    assert result.mean_final == 0.0


def test_simulation_result_is_immutable():
    result = simulate_bankroll(0.1, 2.0, 0.1, n_bets=1, n_trials=1, seed=1)
    with pytest.raises(FrozenInstanceError):
        result.mean_final = 0.0


def test_devig_result_is_immutable():
    result = DevigResult(0.5, 0.5, DevigMethod.MULTIPLICATIVE, 0.0)
    with pytest.raises(FrozenInstanceError):
        result.fair_home = 0.0


def test_multi_result_is_immutable():
    result = MultiOutcomeDevigResult((0.5, 0.5), DevigMethod.MULTIPLICATIVE, 0.0, 2)
    with pytest.raises(FrozenInstanceError):
        result.fair_probs = ()


def test_edge_result_is_immutable():
    result = EdgeResult(0.6, 0.5, 0.5, 0.1, 0.1, Side.HOME, DevigMethod.MULTIPLICATIVE)
    with pytest.raises(FrozenInstanceError):
        result.true_edge = 0.0
