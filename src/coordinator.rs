//! Execution events feed persistence and presentation. Only compact, replaceable
//! snapshots cross into the renderer; command output stays with the report writer.
use std::{
    collections::{BTreeMap, VecDeque},
    path::PathBuf,
    time::{Duration, Instant},
};

use anyhow::Result;
use tokio::sync::watch;

use crate::{
    cancellation::{Cancellation, Interrupted},
    config::RunConfig,
    go,
    mutation::Mutation,
    process::CommandOutput,
    report::{ExecutionState, MutationResult, Outcome, RunStatus, Summary},
    rerun::RerunProvenance,
    saved_report::SavedReport,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Snapshot,
    Baseline,
    Discovery,
    Inventory,
    Mutations,
}

impl Phase {
    pub fn name(self) -> &'static str {
        match self {
            Self::Snapshot => "snapshot",
            Self::Baseline => "baseline",
            Self::Discovery => "discovery",
            Self::Inventory => "inventory",
            Self::Mutations => "mutations",
        }
    }
}

pub enum RunEvent<'a> {
    Phase(Phase),
    Command {
        phase: Phase,
        output: &'a CommandOutput,
    },
    Plan {
        mutations: &'a [Mutation],
        workers: usize,
        timeout: Duration,
        rerun: Option<&'a RerunProvenance>,
    },
    Started {
        id: usize,
        worker: usize,
        mutation: &'a Mutation,
    },
    Finished {
        worker: usize,
        result: &'a MutationResult,
    },
}

#[derive(Clone, Debug)]
pub struct ActiveWorker {
    pub id: usize,
    pub description: String,
    pub started: Instant,
}

#[derive(Clone, Debug)]
pub struct RecentResult {
    pub description: String,
    pub outcome: Option<Outcome>,
    pub elapsed: Option<Duration>,
}

#[derive(Clone, Debug)]
pub struct RunState {
    pub started: Instant,
    pub phase: Phase,
    pub status: RunStatus,
    pub total: Option<usize>,
    pub workers: usize,
    pub summary: Summary,
    pub interrupted: usize,
    pub active: BTreeMap<usize, ActiveWorker>,
    pub recent: VecDeque<RecentResult>,
}

impl Default for RunState {
    fn default() -> Self {
        Self {
            started: Instant::now(),
            phase: Phase::Snapshot,
            status: RunStatus::Running,
            total: None,
            workers: 0,
            summary: Summary::default(),
            interrupted: 0,
            active: BTreeMap::new(),
            recent: VecDeque::new(),
        }
    }
}

impl RunState {
    fn apply(&mut self, event: &RunEvent<'_>) {
        match event {
            RunEvent::Phase(phase) => self.phase = *phase,
            RunEvent::Command { .. } => {}
            RunEvent::Plan {
                mutations, workers, ..
            } => {
                self.total = Some(mutations.len());
                self.workers = *workers;
            }
            RunEvent::Started {
                id,
                worker,
                mutation,
            } => {
                self.active.insert(
                    *worker,
                    ActiveWorker {
                        id: *id,
                        description: mutation.description(),
                        started: Instant::now(),
                    },
                );
            }
            RunEvent::Finished { worker, result } => {
                self.active.remove(worker);
                self.summary.record(result);
                if result.state == ExecutionState::Interrupted {
                    self.interrupted += 1;
                }
                self.recent.push_back(RecentResult {
                    description: result.mutation.description(),
                    outcome: result.outcome,
                    elapsed: result.output.as_ref().map(|o| o.elapsed),
                });
                // All results are persisted; only the recent display is bounded.
                if self.recent.len() > 5 {
                    self.recent.pop_front();
                }
            }
        }
    }

    pub fn pending(&self) -> usize {
        self.total
            .unwrap_or(0)
            .saturating_sub(self.summary.total() + self.interrupted + self.active.len())
    }

    fn finish(&mut self, error: Option<&anyhow::Error>) {
        self.status = match error {
            Some(error) if error.is::<Interrupted>() => RunStatus::Interrupted,
            Some(_) => RunStatus::Failed,
            None => RunStatus::Completed,
        };
        self.interrupted += self.active.len();
        self.active.clear();
    }
}

pub struct Coordinator {
    saved: Option<SavedReport>,
    pub report_finished: bool,
    pub state: RunState,
    updates: watch::Sender<RunState>,
    interactive: bool,
    verbose: bool,
}

impl Coordinator {
    pub fn new(config: &RunConfig, interactive: bool) -> Result<(Self, watch::Receiver<RunState>)> {
        let saved = if config.list {
            None
        } else {
            Some(SavedReport::create(config)?)
        };
        if let Some(report) = &saved {
            println!("Saving report to {}", report.path().display());
        }
        let state = RunState::default();
        let (updates, receiver) = watch::channel(state.clone());
        Ok((
            Self {
                saved,
                report_finished: false,
                state,
                updates,
                interactive,
                verbose: config.verbose,
            },
            receiver,
        ))
    }

    pub fn report_path(&self) -> Option<PathBuf> {
        self.saved.as_ref().map(|r| r.path().to_owned())
    }

    pub fn line(&self, line: impl std::fmt::Display) {
        if !self.interactive {
            println!("{line}");
        }
    }

    pub fn use_plain_output(&mut self) {
        self.interactive = false;
    }

    pub fn emit(&mut self, event: RunEvent<'_>, cancellation: &Cancellation) -> Result<()> {
        self.state.apply(&event);
        self.updates.send_replace(self.state.clone());
        if let Some(report) = &mut self.saved {
            match &event {
                RunEvent::Phase(phase) => report.phase(phase.name())?,
                RunEvent::Command { phase, output } => report.command(phase.name(), output)?,
                RunEvent::Plan {
                    mutations,
                    workers,
                    timeout,
                    rerun,
                } => report.plan(mutations, *workers, *timeout, *rerun, cancellation)?,
                RunEvent::Started { id, worker, .. } => report.started(*id, *worker)?,
                RunEvent::Finished { result, .. } => report.record(result)?,
            }
        }
        if !self.interactive {
            match event {
                RunEvent::Phase(Phase::Baseline) => {
                    self.line("Running baseline: go test ./... (test caching disabled)")
                }
                RunEvent::Command {
                    phase: Phase::Baseline,
                    output,
                } if output.status.success() && !output.interrupted && !output.timed_out => {
                    self.line(format!(
                        "Baseline passed in {:.2}s",
                        output.elapsed.as_secs_f64()
                    ));
                    if self.verbose {
                        print!("{}", go::text(output));
                    }
                }
                RunEvent::Finished { result, .. } => {
                    let outcome = result
                        .outcome
                        .map(|o| o.to_string())
                        .unwrap_or_else(|| "interrupted".into());
                    let duration = result
                        .output
                        .as_ref()
                        .map(|o| format!(" ({:.2}s)", o.elapsed.as_secs_f64()))
                        .unwrap_or_default();
                    self.line(format!(
                        "[{}/{}] {}: {}{}",
                        self.state.summary.total(),
                        self.state.total.unwrap_or(0),
                        outcome,
                        result.mutation.description(),
                        duration
                    ));
                    if self.verbose
                        && let Some(output) = &result.output
                    {
                        let text = go::text(output);
                        if !text.is_empty() {
                            self.line(text.trim_end());
                        }
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }

    pub fn finish(&mut self, mut result: Result<()>) -> Result<()> {
        if let Some(report) = &mut self.saved {
            match report.finish(result.as_ref().err()) {
                Ok(()) => self.report_finished = true,
                Err(save_error) => {
                    result = Err(match result {
                        Err(run_error) => run_error
                            .context(format!("could not finish saved report: {save_error:#}")),
                        Ok(()) => save_error.context("finishing saved report"),
                    });
                }
            }
        }
        self.state.finish(result.as_ref().err());
        self.updates.send_replace(self.state.clone());
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        cli::Cli,
        mutation::{self, Scope},
    };
    use clap::Parser;
    use std::fs;

    #[test]
    fn events_keep_reports_and_coalesced_state_consistent_through_interruption() -> Result<()> {
        let root = tempfile::tempdir()?;
        fs::write(root.path().join("go.mod"), "module example.com/test\n")?;
        fs::write(
            root.path().join("a.go"),
            "package p\nvar a = true\nvar b = false\nvar c = true\nvar d = false\n",
        )?;
        let mutations =
            mutation::discover(root.path(), vec!["a.go".into()], &Scope::new(&[], &[])?)?;
        let config = RunConfig::resolve(Cli::parse_from([
            std::ffi::OsStr::new("mutie"),
            root.path().as_os_str(),
        ]))?;
        let (mut coordinator, updates) = Coordinator::new(&config, true)?;
        let cancellation = Cancellation::default();
        coordinator.emit(
            RunEvent::Plan {
                mutations: &mutations,
                workers: 2,
                timeout: Duration::from_secs(10),
                rerun: None,
            },
            &cancellation,
        )?;
        coordinator.emit(RunEvent::Phase(Phase::Mutations), &cancellation)?;
        for (id, worker) in [(1, 1), (2, 2)] {
            coordinator.emit(
                RunEvent::Started {
                    id,
                    worker,
                    mutation: &mutations[id - 1],
                },
                &cancellation,
            )?;
        }
        for (id, worker, state, outcome) in [
            (2, 2, ExecutionState::Completed, Some(Outcome::Missed)),
            (1, 1, ExecutionState::Interrupted, None),
        ] {
            coordinator.emit(
                RunEvent::Finished {
                    worker,
                    result: &MutationResult {
                        id,
                        mutation: mutations[id - 1].clone(),
                        state,
                        outcome,
                        output: None,
                        error: None,
                    },
                },
                &cancellation,
            )?;
        }
        assert!(coordinator.finish(Err(Interrupted.into())).is_err());
        let state = updates.borrow();
        assert_eq!(state.status, RunStatus::Interrupted);
        assert_eq!(state.phase, Phase::Mutations);
        assert_eq!(state.summary.missed, 1);
        assert_eq!(state.interrupted, 1);
        assert_eq!(state.pending(), 2);
        assert!(state.active.is_empty());
        let outcomes: serde_json::Value = serde_json::from_slice(&fs::read(
            coordinator.report_path().unwrap().join("outcomes.json"),
        )?)?;
        assert_eq!(outcomes["summary"], serde_json::to_value(&state.summary)?);
        assert_eq!(outcomes["mutations"][0]["state"], "interrupted");
        assert_eq!(outcomes["mutations"][1]["state"], "completed");
        assert_eq!(outcomes["mutations"][2]["state"], "pending");
        Ok(())
    }

    #[test]
    fn recent_display_is_bounded_without_losing_summary_counts() -> Result<()> {
        let root = tempfile::tempdir()?;
        fs::write(root.path().join("a.go"), "package p\nvar a = true\n")?;
        let mutations =
            mutation::discover(root.path(), vec!["a.go".into()], &Scope::new(&[], &[])?)?;
        let mut state = RunState::default();
        for id in 1..=20 {
            state.apply(&RunEvent::Started {
                id,
                worker: 1,
                mutation: &mutations[0],
            });
            state.apply(&RunEvent::Finished {
                worker: 1,
                result: &MutationResult {
                    id,
                    mutation: mutations[0].clone(),
                    state: ExecutionState::Completed,
                    outcome: Some(Outcome::Killed),
                    output: None,
                    error: None,
                },
            });
        }
        assert_eq!(state.summary.killed, 20);
        assert_eq!(state.recent.len(), 5);
        assert!(state.active.is_empty());
        Ok(())
    }
}
