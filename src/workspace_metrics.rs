//! Cumulative workspace costs. These are operation times, not run wall-clock time.
use std::{path::PathBuf, time::Instant};

use anyhow::Result;
use serde::Serialize;

use crate::cancellation::Interrupted;

#[derive(Clone, Copy, Debug)]
pub enum WorkspaceKind {
    Snapshot,
    BaselineSetup,
    DiscoveryReset,
    WorkerSetup,
    WorkerReset,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OperationStatus {
    Completed,
    Failed,
    Interrupted,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct CopyCounts {
    pub reflink_files: u64,
    pub copied_files: u64,
    pub copied_bytes: u64,
}

impl CopyCounts {
    pub fn record(&mut self, copied: Option<u64>) {
        match copied {
            None => self.reflink_files += 1,
            Some(bytes) => {
                self.copied_files += 1;
                self.copied_bytes += bytes;
            }
        }
    }

    fn add(&mut self, other: &Self) {
        self.reflink_files += other.reflink_files;
        self.copied_files += other.copied_files;
        self.copied_bytes += other.copied_bytes;
    }
}

#[derive(Clone, Debug)]
pub struct WorkspaceMeasurement {
    pub duration_seconds: f64,
    pub status: OperationStatus,
    pub copies: CopyCounts,
    pub temporary_root: Option<PathBuf>,
}

impl WorkspaceMeasurement {
    /// Keep successful file transfers even if a later file or cancellation ends the operation.
    pub fn measure<T>(operation: impl FnOnce(&mut Self) -> Result<T>) -> (Result<T>, Self) {
        let started = Instant::now();
        let mut measurement = Self {
            duration_seconds: 0.0,
            status: OperationStatus::Completed,
            copies: CopyCounts::default(),
            temporary_root: None,
        };
        let result = operation(&mut measurement);
        measurement.duration_seconds = started.elapsed().as_secs_f64();
        measurement.status = match &result {
            Ok(_) => OperationStatus::Completed,
            Err(error) if error.is::<Interrupted>() => OperationStatus::Interrupted,
            Err(_) => OperationStatus::Failed,
        };
        (result, measurement)
    }
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct OperationTotals {
    pub operations: u64,
    pub completed: u64,
    pub failed: u64,
    pub interrupted: u64,
    pub duration_seconds: f64,
    #[serde(flatten)]
    pub copies: CopyCounts,
}

impl OperationTotals {
    fn record(&mut self, measurement: &WorkspaceMeasurement) {
        self.operations += 1;
        match measurement.status {
            OperationStatus::Completed => self.completed += 1,
            OperationStatus::Failed => self.failed += 1,
            OperationStatus::Interrupted => self.interrupted += 1,
        }
        self.duration_seconds += measurement.duration_seconds;
        self.copies.add(&measurement.copies);
    }
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct WorkspaceMetrics {
    pub temporary_root: Option<PathBuf>,
    pub snapshot: OperationTotals,
    pub baseline_setup: OperationTotals,
    pub discovery_reset: OperationTotals,
    pub worker_setup: OperationTotals,
    pub worker_reset: OperationTotals,
}

impl WorkspaceMetrics {
    pub fn record(&mut self, kind: WorkspaceKind, measurement: &WorkspaceMeasurement) {
        if self.temporary_root.is_none() {
            self.temporary_root.clone_from(&measurement.temporary_root);
        }
        match kind {
            WorkspaceKind::Snapshot => &mut self.snapshot,
            WorkspaceKind::BaselineSetup => &mut self.baseline_setup,
            WorkspaceKind::DiscoveryReset => &mut self.discovery_reset,
            WorkspaceKind::WorkerSetup => &mut self.worker_setup,
            WorkspaceKind::WorkerReset => &mut self.worker_reset,
        }
        .record(measurement);
    }

    pub fn summary(&self) -> String {
        let mut copies = CopyCounts::default();
        let mut operations = 0;
        let mut failed = 0;
        let mut interrupted = 0;
        for phase in [
            &self.snapshot,
            &self.baseline_setup,
            &self.discovery_reset,
            &self.worker_setup,
            &self.worker_reset,
        ] {
            copies.add(&phase.copies);
            operations += phase.operations;
            failed += phase.failed;
            interrupted += phase.interrupted;
        }
        format!(
            "Workspaces: snapshot {:.3}s | setup {:.3}s | reset {:.3}s (cumulative)\nCopies: {} reflinked | {} copied ({} bytes) | {} ops, {} failed, {} interrupted",
            self.snapshot.duration_seconds,
            self.baseline_setup.duration_seconds + self.worker_setup.duration_seconds,
            self.discovery_reset.duration_seconds + self.worker_reset.duration_seconds,
            copies.reflink_files,
            copies.copied_files,
            copies.copied_bytes,
            operations,
            failed,
            interrupted,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aggregates_by_phase_with_separate_failure_counts_and_cumulative_times() {
        let mut metrics = WorkspaceMetrics::default();
        for (kind, status, duration_seconds) in [
            (WorkspaceKind::Snapshot, OperationStatus::Completed, 1.0),
            (
                WorkspaceKind::BaselineSetup,
                OperationStatus::Completed,
                2.0,
            ),
            (
                WorkspaceKind::DiscoveryReset,
                OperationStatus::Completed,
                3.0,
            ),
            (WorkspaceKind::WorkerSetup, OperationStatus::Completed, 4.0),
            (WorkspaceKind::WorkerSetup, OperationStatus::Failed, 5.0),
            (WorkspaceKind::WorkerReset, OperationStatus::Completed, 6.0),
            (
                WorkspaceKind::WorkerReset,
                OperationStatus::Interrupted,
                7.0,
            ),
        ] {
            metrics.record(
                kind,
                &WorkspaceMeasurement {
                    duration_seconds,
                    status,
                    temporary_root: Some("/tmp/metrics".into()),
                    copies: CopyCounts {
                        reflink_files: 2,
                        copied_files: 3,
                        copied_bytes: 9,
                    },
                },
            );
        }
        let json = serde_json::to_value(&metrics).unwrap();
        assert_eq!(json["temporary_root"], "/tmp/metrics");
        assert_eq!(json["worker_setup"]["operations"], 2);
        assert_eq!(json["worker_setup"]["completed"], 1);
        assert_eq!(json["worker_setup"]["failed"], 1);
        assert_eq!(json["worker_setup"]["duration_seconds"], 9.0);
        assert_eq!(json["worker_reset"]["interrupted"], 1);
        assert_eq!(json["worker_reset"]["reflink_files"], 4);
        assert_eq!(json["worker_reset"]["copied_files"], 6);
        assert_eq!(json["worker_reset"]["copied_bytes"], 18);
        assert_eq!(
            metrics.summary(),
            "Workspaces: snapshot 1.000s | setup 11.000s | reset 16.000s (cumulative)\nCopies: 14 reflinked | 21 copied (63 bytes) | 7 ops, 1 failed, 1 interrupted"
        );
    }
}
