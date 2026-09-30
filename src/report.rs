use std::fmt;

use serde::Serialize;

use crate::{mutation::Mutation, process::CommandOutput};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
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
    pub outcome: Outcome,
    pub output: CommandOutput,
    pub error: Option<String>,
}

#[derive(Default, Serialize)]
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
            Outcome::Killed => self.killed += 1,
            Outcome::Missed => self.missed += 1,
            Outcome::Timeout => self.timeout += 1,
            Outcome::BuildFailed => self.build_failed += 1,
            Outcome::Error => self.errors += 1,
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
