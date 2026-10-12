//! Shared, derived operator totals for text reports and saved-report browsing.
use std::collections::BTreeMap;

use crate::{
    mutation::Operator,
    report::{ExecutionState, Outcome, Summary},
};

#[derive(Default)]
struct Totals {
    planned: usize,
    outcomes: Summary,
    pending: usize,
    running: usize,
    interrupted: usize,
    seconds: f64,
    timed: usize,
    missing: usize,
}

impl Totals {
    fn record(&mut self, state: ExecutionState, outcome: Option<Outcome>, seconds: Option<f64>) {
        self.planned += 1;
        match state {
            ExecutionState::Pending => self.pending += 1,
            ExecutionState::Running => self.running += 1,
            ExecutionState::Interrupted => self.interrupted += 1,
            ExecutionState::Completed => {}
        }
        match outcome {
            Some(Outcome::Killed) => self.outcomes.killed += 1,
            Some(Outcome::Missed) => self.outcomes.missed += 1,
            Some(Outcome::Timeout) => self.outcomes.timeout += 1,
            Some(Outcome::BuildFailed) => self.outcomes.build_failed += 1,
            Some(Outcome::Error) => self.outcomes.errors += 1,
            None => {}
        }
        // Pending mutations have not run, even if an old report has a timing.
        if state != ExecutionState::Pending {
            if let Some(seconds) = seconds.filter(|s| s.is_finite() && *s >= 0.0) {
                self.seconds += seconds;
                self.timed += 1;
            } else {
                self.missing += 1;
            }
        }
    }

    fn time(&self) -> String {
        if self.timed == 0 {
            "not recorded".into()
        } else if !self.seconds.is_finite() {
            "unavailable (sum overflow)".into()
        } else {
            format!("{:.2}s", self.seconds)
        }
    }
}

/// Entries must join the complete inventory and executions by ID, with validated
/// state/outcome pairs. Only operators present in that saved selection appear.
pub fn summarize(
    complete: bool,
    entries: impl IntoIterator<Item = (Operator, ExecutionState, Option<Outcome>, Option<f64>)>,
) -> String {
    let mut text = String::from("Operator breakdown (saved run selection):\n");
    if !complete {
        text.push_str("Not available: inventory was not published.\n");
        return text;
    }
    let mut operators = BTreeMap::<Operator, Totals>::new();
    for (operator, state, outcome, seconds) in entries {
        operators
            .entry(operator)
            .or_default()
            .record(state, outcome, seconds);
    }
    if operators.is_empty() {
        text.push_str("No mutations in this selection.\n");
        return text;
    }
    for (operator, totals) in operators {
        text.push_str(&format!(
            "\n{operator}\n  Planned: {} | Completed: {}\n  Killed: {} | Missed: {} | Timeout: {}\n  Build failed: {} | Error: {}\n  Pending: {} | Running: {} | Interrupted: {}\n  Time: {} ({} recorded, {} missing)\n",
            totals.planned, totals.outcomes.total(),
            totals.outcomes.killed, totals.outcomes.missed, totals.outcomes.timeout,
            totals.outcomes.build_failed, totals.outcomes.errors,
            totals.pending, totals.running, totals.interrupted,
            totals.time(), totals.timed, totals.missing,
        ));
    }
    text.push_str(
        "\nTotals use the entire saved run selection.\nFilters, marks and dismissals do not change them.\nTime sums recorded mutation command durations,\nincluding interrupted commands. It excludes baseline,\ndiscovery and workspace work; it may exceed wall time.\nMissing/invalid timings are unknown, not zero.\nPending mutations are not counted as missing timings.\n",
    );
    text
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn groups_outcomes_and_unfinished_work_in_catalog_order() {
        use ExecutionState::*;
        use Operator::*;
        use Outcome::*;
        let entries = [
            (BooleanFlip, Completed, Some(Killed), Some(0.25)),
            (BooleanFlip, Completed, Some(Missed), Some(0.5)),
            (BooleanFlip, Completed, Some(Timeout), Some(1.0)),
            (BooleanFlip, Completed, Some(BuildFailed), Some(0.0)),
            (BooleanFlip, Completed, Some(Error), None),
            (BooleanFlip, Interrupted, None, Some(0.75)),
            (BooleanFlip, Interrupted, None, None),
            (BooleanFlip, Pending, None, Some(99.0)),
            (BooleanFlip, Running, None, None),
            (ArithmeticReplace, Pending, None, None),
        ];
        let summary = summarize(true, entries);
        assert_eq!(summary, summarize(true, entries.into_iter().rev()));
        assert!(
            summary.find("arithmetic_replace").unwrap() < summary.find("boolean_flip").unwrap()
        );
        assert!(summary.contains("Planned: 9 | Completed: 5"));
        assert!(summary.contains("Killed: 1 | Missed: 1 | Timeout: 1"));
        assert!(summary.contains("Build failed: 1 | Error: 1"));
        assert!(summary.contains("Pending: 1 | Running: 1 | Interrupted: 2"));
        assert!(summary.contains("Time: 2.50s (5 recorded, 3 missing)"));
        assert!(summary.contains("Time: not recorded (0 recorded, 0 missing)"));
    }

    #[test]
    fn distinguishes_missing_invalid_and_zero_durations() {
        let entry = |seconds| {
            (
                Operator::LoopSkip,
                ExecutionState::Completed,
                Some(Outcome::Killed),
                seconds,
            )
        };
        let summary = summarize(
            true,
            [None, Some(-1.0), Some(f64::NAN), Some(f64::INFINITY)].map(entry),
        );
        assert!(summary.contains("Time: not recorded (0 recorded, 4 missing)"));
        assert!(
            summarize(true, [entry(Some(0.0))]).contains("Time: 0.00s (1 recorded, 0 missing)")
        );
        assert!(
            summarize(true, [entry(Some(f64::MAX)); 2])
                .contains("Time: unavailable (sum overflow)")
        );
    }

    #[test]
    fn distinguishes_empty_selection_from_unpublished_inventory() {
        assert_eq!(
            summarize(true, []),
            "Operator breakdown (saved run selection):\nNo mutations in this selection.\n"
        );
        assert_eq!(
            summarize(false, []),
            "Operator breakdown (saved run selection):\nNot available: inventory was not published.\n"
        );
    }
}
