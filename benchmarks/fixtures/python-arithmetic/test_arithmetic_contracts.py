"""Exact public outputs complement upstream sign/invariant checks."""
from betting_math_kit.kelly import full_kelly_fraction, expected_roi, compute_kelly_bet


def test_exact_full_kelly_fraction():
    # Net odds 2, weighted win 1.5, loss probability .25: fraction 1.25 / 2.
    assert full_kelly_fraction(0.75, 3.0, takeout=0.0) == 0.625


def test_exact_expected_roi_after_takeout():
    # Effective odds 1 + 2*.75 = 2.5; expected return .5*2.5 - 1 = .25.
    assert expected_roi(0.5, 3.0, takeout=0.25) == 0.25


def test_edge_is_consistent_across_sizing_paths():
    no_edge = compute_kelly_bet(1, 0.125, 4.0, 1024.0, takeout=0.0)
    assert no_edge.bet_size == 0.0
    assert no_edge.reason == 'no edge after friction'
    assert no_edge.edge == -0.125
    below = compute_kelly_bet(1, 0.75, 2.0, 16.0, fraction=0.01, takeout=0.0)
    assert below.bet_size == 0.0
    assert 'below minimum' in below.reason
    assert below.edge == 0.25
    sized = compute_kelly_bet(1, 0.75, 2.0, 1024.0, takeout=0.0, max_fraction=1.0)
    assert sized.bet_size == 128.0
    assert sized.edge == 0.25
