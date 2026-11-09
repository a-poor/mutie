"""Check documented public defaults without changing the implementation."""
from betting_math_kit.kelly import full_kelly_fraction, pool_size_limit, compute_kelly_bet


def test_default_takeout_matches_documented_sixteen_percent():
    assert full_kelly_fraction(0.6, 3.0) == full_kelly_fraction(0.6, 3.0, takeout=0.16)


def test_default_pool_impact_matches_documented_five_percent():
    assert pool_size_limit(1000.0, 3.0) == pool_size_limit(1000.0, 3.0, impact_limit=0.05)


def test_default_minimum_rejects_a_one_unit_size():
    result = compute_kelly_bet(1, prob=0.6, odds_decimal=3.0, bankroll=10.0,
                              fraction=0.25, takeout=0.0, max_fraction=1.0)
    assert result.bet_size == 0.0
    assert 'below minimum' in result.reason
