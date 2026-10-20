//! Test-only phase timings of the actual isolated adapter.
use super::*;
use std::time::Instant;

pub(super) struct PhaseTimings {
    last: Instant,
    pub(super) values: Vec<(&'static str, f64)>,
}

impl PhaseTimings {
    pub(super) fn new() -> Self {
        Self {
            last: Instant::now(),
            values: Vec::new(),
        }
    }

    pub(super) fn finish(&mut self, name: &'static str) {
        let now = Instant::now();
        self.values
            .push((name, now.duration_since(self.last).as_secs_f64()));
        self.last = now;
    }
}

#[tokio::test]
#[ignore = "requires MUTIE_PYTHON and MUTIE_PYTHON_PROFILE_PROJECT; three actual baselines"]
async fn real_feedback_phase_profile() -> Result<()> {
    let root = std::path::PathBuf::from(std::env::var("MUTIE_PYTHON_PROFILE_PROJECT")?);
    let executable = std::path::PathBuf::from(std::env::var("MUTIE_PYTHON")?);
    let cancellation = Cancellation::default();
    let original = project::Project::load(&root, &executable, &cancellation)
        .await?
        .value()?;
    let mut identity = None;
    for repeat in 1..=3 {
        let start = Instant::now();
        let result = execute(
            &root,
            &executable,
            Duration::from_secs(20),
            NonZeroUsize::new(4 * 1024 * 1024).unwrap(),
            &cancellation,
        )
        .await?;
        let seconds = start.elapsed().as_secs_f64();
        ensure!(result.command.classify()? == crate::report::Outcome::Missed);
        ensure!(result.project == original, "profile changed project inputs");
        if let Some(expected) = &identity {
            ensure!(
                &result.identity == expected,
                "profile execution identity changed"
            );
        }
        identity = Some(result.identity.clone());
        println!(
            "python-profile {}",
            serde_json::json!({
                "repeat": repeat, "adapter_seconds": seconds, "phases": result.timings,
                "test_command_seconds": result.command.elapsed.as_secs_f64(),
                "identity": result.identity, "nodes": result.nodes,
            })
        );
    }
    ensure!(
        project::Project::load(&root, &executable, &cancellation)
            .await?
            .value()?
            == original
    );
    Ok(())
}
