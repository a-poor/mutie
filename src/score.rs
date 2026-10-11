//! Raw mutation scores and an independent CI threshold. Review decisions never
//! change the numerator or denominator, and unfinished runs have no final score.
use std::{fmt, str::FromStr};

use serde::{Deserialize, Serialize};

use crate::report::{Outcome, RunStatus};

/// Hundredths of a percentage point; compare ratios using integers, not a
/// rounded display percentage. Config/report serialization uses percentage units.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(try_from = "f64", into = "f64")]
pub struct MinimumScore(u16);

impl TryFrom<f64> for MinimumScore {
    type Error = &'static str;

    fn try_from(value: f64) -> Result<Self, Self::Error> {
        let scaled = value * 100.0;
        // Allow representation error from decimal decoding, but never round a
        // genuinely finer-grained threshold (especially a tiny positive one) to zero.
        let canonical = scaled.round() / 100.0;
        if !value.is_finite()
            || !(0.0..=100.0).contains(&value)
            || (value - canonical).abs() > 2.0 * f64::EPSILON * value.abs()
        {
            return Err("minimum score must be 0 through 100 with at most two decimal places");
        }
        Ok(Self(scaled.round() as u16))
    }
}

impl From<MinimumScore> for f64 {
    fn from(value: MinimumScore) -> Self {
        f64::from(value.0) / 100.0
    }
}

impl FromStr for MinimumScore {
    type Err = &'static str;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let invalid = "minimum score must be 0 through 100 with at most two decimal places";
        let (whole, fraction) = value.split_once('.').unwrap_or((value, ""));
        if whole.is_empty()
            || !whole.bytes().all(|b| b.is_ascii_digit())
            || fraction.len() > 2
            || !fraction.bytes().all(|b| b.is_ascii_digit())
        {
            return Err(invalid);
        }
        Self::try_from(value.parse::<f64>().map_err(|_| invalid)?)
    }
}

impl fmt::Display for MinimumScore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{:02}%", self.0 / 100, self.0 % 100)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ScoreScope {
    AllDiscovered,
    Filtered,
    Rerun,
}

impl ScoreScope {
    pub fn selected(rerun: bool, filtered: bool) -> Self {
        if rerun {
            Self::Rerun
        } else if filtered {
            Self::Filtered
        } else {
            Self::AllDiscovered
        }
    }
}

impl fmt::Display for ScoreScope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::AllDiscovered => "all discovered mutations",
            Self::Filtered => "filtered selection only",
            Self::Rerun => "rerun selection only",
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Unavailable {
    IncompleteRun,
    ExecutionErrors,
    NoScoredMutations,
}

impl fmt::Display for Unavailable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::IncompleteRun => "run did not complete successfully",
            Self::ExecutionErrors => "execution errors",
            Self::NoScoredMutations => "no killed or missed mutations",
        })
    }
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MutationScore {
    pub scope: ScoreScope,
    pub killed: usize,
    pub missed: usize,
    pub timeout: usize,
    pub build_failed: usize,
    pub errors: usize,
    pub unfinished: usize,
    /// Matched dismissals among killed/missed entries, still included in score.
    pub dismissed_in_score: usize,
    pub percentage: Option<f64>,
    pub unavailable_reason: Option<Unavailable>,
}

impl MutationScore {
    pub fn calculate(
        scope: ScoreScope,
        status: RunStatus,
        inventory_complete: bool,
        executions: impl IntoIterator<Item = (Option<Outcome>, bool)>,
    ) -> Self {
        let mut score = Self {
            scope,
            killed: 0,
            missed: 0,
            timeout: 0,
            build_failed: 0,
            errors: 0,
            unfinished: 0,
            dismissed_in_score: 0,
            percentage: None,
            unavailable_reason: None,
        };
        for (outcome, dismissed) in executions {
            match outcome {
                Some(Outcome::Killed) => score.killed += 1,
                Some(Outcome::Missed) => score.missed += 1,
                Some(Outcome::Timeout) => score.timeout += 1,
                Some(Outcome::BuildFailed) => score.build_failed += 1,
                Some(Outcome::Error) => score.errors += 1,
                None => score.unfinished += 1,
            }
            if dismissed && matches!(outcome, Some(Outcome::Killed | Outcome::Missed)) {
                score.dismissed_in_score += 1;
            }
        }
        score.unavailable_reason =
            if status != RunStatus::Completed || !inventory_complete || score.unfinished > 0 {
                Some(Unavailable::IncompleteRun)
            } else if score.errors > 0 {
                Some(Unavailable::ExecutionErrors)
            } else if score.killed + score.missed == 0 {
                Some(Unavailable::NoScoredMutations)
            } else {
                score.percentage =
                    Some(100.0 * score.killed as f64 / (score.killed + score.missed) as f64);
                None
            };
        score
    }

    pub fn value_label(&self) -> String {
        self.percentage
            .map(|value| format!("{value:.2}%"))
            .unwrap_or_else(|| "unavailable".into())
    }

    /// Counts and status must match exactly. JSON float decoding can shift the
    /// derived percentage by a few ULPs; it never supplies threshold evidence.
    pub fn matches_saved(&self, saved: &Self) -> bool {
        let percentage_matches = match (self.percentage, saved.percentage) {
            (None, None) => true,
            (Some(expected), Some(actual)) => {
                (0.0..=100.0).contains(&actual) && (expected - actual).abs() <= 1e-12
            }
            _ => false,
        };
        percentage_matches
            && *self
                == Self {
                    percentage: self.percentage,
                    ..saved.clone()
                }
    }

    pub fn details(&self) -> String {
        let mut text = format!(
            "Mutation score: {}\nScope: {}\nKilled / (killed + missed): {} / {}\nExcluded: {} timeout, {} build failed\nExecution errors: {}; unfinished: {}\nDismissed in score: {} (included)\n",
            self.value_label(),
            self.scope,
            self.killed,
            self.killed + self.missed,
            self.timeout,
            self.build_failed,
            self.errors,
            self.unfinished,
            self.dismissed_in_score,
        );
        if let Some(reason) = self.unavailable_reason {
            text.push_str(&format!("Unavailable: {reason}\n"));
        }
        text
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum ScorePolicy {
    Disabled,
    NotEvaluated { minimum: MinimumScore },
    Passed { minimum: MinimumScore },
    Failed { minimum: MinimumScore },
}

impl ScorePolicy {
    pub fn evaluate(minimum: Option<MinimumScore>, score: &MutationScore) -> Self {
        let Some(minimum) = minimum else {
            return Self::Disabled;
        };
        match score.unavailable_reason {
            Some(Unavailable::IncompleteRun | Unavailable::ExecutionErrors) => {
                Self::NotEvaluated { minimum }
            }
            Some(Unavailable::NoScoredMutations) => Self::Failed { minimum },
            None => {
                if (score.killed as u128) * 10_000
                    >= (score.killed as u128 + score.missed as u128) * u128::from(minimum.0)
                {
                    Self::Passed { minimum }
                } else {
                    Self::Failed { minimum }
                }
            }
        }
    }

    pub fn failed(&self) -> bool {
        matches!(self, Self::Failed { .. })
    }
}

impl fmt::Display for ScorePolicy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (status, minimum) = match self {
            Self::Disabled => return f.write_str("Minimum-score policy: disabled"),
            Self::NotEvaluated { minimum } => ("not evaluated", minimum),
            Self::Passed { minimum } => ("passed", minimum),
            Self::Failed { minimum } => ("failed", minimum),
        };
        write!(f, "Minimum-score policy: {status} (minimum {minimum})")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn score(outcomes: &[Option<Outcome>]) -> MutationScore {
        MutationScore::calculate(
            ScoreScope::AllDiscovered,
            RunStatus::Completed,
            true,
            outcomes.iter().map(|&o| (o, false)),
        )
    }

    #[test]
    fn threshold_boundaries_compare_exact_ratios_not_display_rounding() {
        let two_thirds = score(&[
            Some(Outcome::Killed),
            Some(Outcome::Killed),
            Some(Outcome::Missed),
        ]);
        assert_eq!(two_thirds.value_label(), "66.67%");
        for (minimum, failed) in [
            ("0", false),
            ("66.66", false),
            ("66.67", true),
            ("100", true),
        ] {
            assert_eq!(
                ScorePolicy::evaluate(Some(minimum.parse().unwrap()), &two_thirds).failed(),
                failed
            );
        }
        let exact = score(&[Some(Outcome::Killed), Some(Outcome::Missed)]);
        assert!(!ScorePolicy::evaluate(Some("50".parse().unwrap()), &exact).failed());
        assert!(ScorePolicy::evaluate(Some("50.01".parse().unwrap()), &exact).failed());
    }

    #[test]
    fn exclusions_and_dismissals_never_turn_into_kills() {
        let score = MutationScore::calculate(
            ScoreScope::Filtered,
            RunStatus::Completed,
            true,
            [
                Outcome::Killed,
                Outcome::Missed,
                Outcome::Timeout,
                Outcome::BuildFailed,
            ]
            .map(|o| (Some(o), true)),
        );
        assert_eq!(score.percentage, Some(50.0));
        assert_eq!(
            (score.timeout, score.build_failed, score.dismissed_in_score),
            (1, 1, 2)
        );
        assert!(score.details().contains("filtered selection only"));
    }

    #[test]
    fn unavailable_scores_do_not_silently_pass() {
        let minimum = Some("0".parse().unwrap());
        for outcomes in [
            vec![],
            vec![Some(Outcome::Timeout), Some(Outcome::BuildFailed)],
        ] {
            let score = score(&outcomes);
            assert_eq!(score.percentage, None);
            assert_eq!(
                score.unavailable_reason,
                Some(Unavailable::NoScoredMutations)
            );
            assert!(ScorePolicy::evaluate(minimum, &score).failed());
        }
        for status in [
            RunStatus::Running,
            RunStatus::Failed,
            RunStatus::Interrupted,
        ] {
            let score = MutationScore::calculate(
                ScoreScope::Rerun,
                status,
                true,
                [(Some(Outcome::Killed), false)],
            );
            assert_eq!(score.percentage, None);
            assert!(matches!(
                ScorePolicy::evaluate(minimum, &score),
                ScorePolicy::NotEvaluated { .. }
            ));
        }
        for outcomes in [vec![None], vec![Some(Outcome::Error)]] {
            assert!(matches!(
                ScorePolicy::evaluate(minimum, &score(&outcomes)),
                ScorePolicy::NotEvaluated { .. }
            ));
        }
    }

    #[test]
    fn saved_score_percentages_round_trip_for_fractional_ratios() {
        for total in 1..100 {
            for killed in 0..=total {
                let score = MutationScore::calculate(
                    ScoreScope::AllDiscovered,
                    RunStatus::Completed,
                    true,
                    (0..total).map(|i| {
                        (
                            Some(if i < killed {
                                Outcome::Killed
                            } else {
                                Outcome::Missed
                            }),
                            false,
                        )
                    }),
                );
                let saved: MutationScore =
                    serde_json::from_slice(&serde_json::to_vec(&score).unwrap()).unwrap();
                assert!(score.matches_saved(&saved), "{killed}/{total}: {saved:?}");
            }
        }
    }

    #[test]
    fn minimum_score_rejects_invalid_percentages() {
        for value in [
            "-1", "101", "NaN", "inf", "1e2", "1.001", "", " 50", "1.2.3",
        ] {
            assert!(value.parse::<MinimumScore>().is_err(), "{value}");
        }
        for value in [
            f64::NAN,
            f64::INFINITY,
            -1.0,
            100.01,
            1.001,
            1e-15,
            80.000000000001,
        ] {
            assert!(MinimumScore::try_from(value).is_err());
        }
        for value in ["0", "0.01", "66.67", "99.99", "100"] {
            let minimum = value.parse::<MinimumScore>().unwrap();
            let json = serde_json::to_string(&minimum).unwrap();
            assert_eq!(
                serde_json::from_str::<MinimumScore>(&json).unwrap(),
                minimum
            );
        }
        for hundredths in 0..=10_000 {
            let minimum = MinimumScore(hundredths);
            let saved = serde_json::to_string(&minimum).unwrap();
            assert_eq!(
                serde_json::from_str::<MinimumScore>(&saved).unwrap(),
                minimum
            );
        }
    }
}
