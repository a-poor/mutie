use std::{fmt, time::Duration};

use crate::mutation::Mutation;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Killed,
    Missed,
    Timeout,
    BuildFailed,
}

impl fmt::Display for Outcome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Killed => "killed",
            Self::Missed => "missed",
            Self::Timeout => "timeout",
            Self::BuildFailed => "build failed",
        })
    }
}

pub struct MutationResult {
    pub mutation: Mutation,
    pub outcome: Outcome,
    pub duration: Duration,
    pub output: String,
}

#[derive(Default)]
pub struct Summary {
    pub killed: usize,
    pub missed: usize,
    pub timeout: usize,
    pub build_failed: usize,
}

impl Summary {
    pub fn record(&mut self, result: &MutationResult) {
        match result.outcome {
            Outcome::Killed => self.killed += 1,
            Outcome::Missed => self.missed += 1,
            Outcome::Timeout => self.timeout += 1,
            Outcome::BuildFailed => self.build_failed += 1,
        }
    }

    pub fn total(&self) -> usize {
        self.killed + self.missed + self.timeout + self.build_failed
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
        )
    }
}
