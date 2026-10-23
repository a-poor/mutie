from betting_math_kit.simulation import simulate_bankroll


def test_odd_trial_count_returns_middle_final_bankroll():
    # With the pinned Random seed, the one-bet trials are win, win, loss.
    # Their final bankrolls are 1250, 1250, 750: the middle is 1250, not
    # the average of the lower two values (1000).
    result = simulate_bankroll(
        edge=0.05,
        odds_decimal=2.0,
        fraction=0.25,
        bankroll=1000.0,
        n_bets=1,
        n_trials=3,
        seed=7,
    )
    assert result.n_trials == 3
    assert result.median_final == 1250.0
