from betting_math_kit.simulation import simulate_bankroll


def test_even_trials_keep_losses_and_average_middle_bankrolls():
    # Seed 1 gives win, loss, loss, win. Sorted finals: 750,750,1250,1250.
    result = simulate_bankroll(
        edge=0.05, odds_decimal=2.0, fraction=0.25, bankroll=1000.0,
        n_bets=1, n_trials=4, seed=1,
    )
    assert result.mean_final == 1000.0
    assert result.median_final == 1000.0


def test_odd_trials_select_middle_bankroll():
    # Seed 7 gives win, win, loss. Sorted finals: 750,1250,1250.
    result = simulate_bankroll(
        edge=0.05, odds_decimal=2.0, fraction=0.25, bankroll=1000.0,
        n_bets=1, n_trials=3, seed=7,
    )
    assert result.median_final == 1250.0
