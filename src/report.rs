use std::fmt;

use serde::{Deserialize, Serialize};

use crate::{command::CommandOutput, mutation::Mutation};

pub struct RunPlan<'a> {
    pub mutations: &'a [Mutation],
    /// Exact package arguments for each mutation, in inventory order.
    pub test_packages: &'a [Vec<String>],
    pub workers: usize,
    pub timeout: std::time::Duration,
    pub rerun: Option<&'a crate::rerun::RerunProvenance>,
    pub dismissals: &'a crate::dismissals::DismissalSnapshot,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    Killed,
    Missed,
    Timeout,
    BuildFailed,
    Error,
}

impl fmt::Display for Outcome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Killed => "killed",
            Self::Missed => "missed",
            Self::Timeout => "timeout",
            Self::BuildFailed => "build failed",
            Self::Error => "error",
        })
    }
}

pub struct MutationResult {
    pub id: usize,
    pub mutation: Mutation,
    pub state: ExecutionState,
    pub outcome: Option<Outcome>,
    pub output: Option<CommandOutput>,
    pub error: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionState {
    Pending,
    Running,
    Completed,
    Interrupted,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    Running,
    Completed,
    Failed,
    Interrupted,
}

impl fmt::Display for RunStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Running => "running",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Interrupted => "interrupted",
        })
    }
}

/// Findings policy is separate from execution status. Only a successfully
/// completed run can pass or fail; errors and interruptions stay unevaluated.
#[derive(Clone, Debug, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum PolicyEvaluation {
    Disabled,
    NotEvaluated,
    Passed {
        undismissed_missed: usize,
        dismissed_missed: usize,
    },
    Failed {
        undismissed_missed: usize,
        dismissed_missed: usize,
    },
}

impl PolicyEvaluation {
    pub fn pending(enabled: bool) -> Self {
        if enabled {
            Self::NotEvaluated
        } else {
            Self::Disabled
        }
    }

    pub fn completed(undismissed_missed: usize, dismissed_missed: usize) -> Self {
        if undismissed_missed > 0 {
            Self::Failed {
                undismissed_missed,
                dismissed_missed,
            }
        } else {
            Self::Passed {
                undismissed_missed,
                dismissed_missed,
            }
        }
    }

    pub fn failed(&self) -> bool {
        matches!(self, Self::Failed { .. })
    }
}

impl fmt::Display for PolicyEvaluation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Disabled => f.write_str("Findings policy: disabled"),
            Self::NotEvaluated => f.write_str(
                "Findings policy: not evaluated (--fail-on-missed requires a completed run)",
            ),
            Self::Passed {
                undismissed_missed,
                dismissed_missed,
            }
            | Self::Failed {
                undismissed_missed,
                dismissed_missed,
            } => write!(
                f,
                "Findings policy: {} (--fail-on-missed; {undismissed_missed} undismissed missed, {dismissed_missed} dismissed missed)",
                if self.failed() { "failed" } else { "passed" }
            ),
        }
    }
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct Summary {
    pub killed: usize,
    pub missed: usize,
    pub timeout: usize,
    pub build_failed: usize,
    pub errors: usize,
}

impl Summary {
    pub fn record(&mut self, result: &MutationResult) {
        match result.outcome {
            Some(Outcome::Killed) => self.killed += 1,
            Some(Outcome::Missed) => self.missed += 1,
            Some(Outcome::Timeout) => self.timeout += 1,
            Some(Outcome::BuildFailed) => self.build_failed += 1,
            Some(Outcome::Error) => self.errors += 1,
            None => {}
        }
    }

    pub fn total(&self) -> usize {
        self.killed + self.missed + self.timeout + self.build_failed + self.errors
    }
}

impl fmt::Display for Summary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} mutations: {} killed, {} missed, {} timed out, {} failed to compile",
            self.total(),
            self.killed,
            self.missed,
            self.timeout,
            self.build_failed
        )?;
        if self.errors > 0 {
            write!(f, ", {} errors", self.errors)?;
        }
        Ok(())
    }
}
