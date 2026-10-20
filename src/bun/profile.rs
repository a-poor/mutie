//! Opt-in phase observations; no production timers or CLI/report fields.
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
#[ignore = "requires pinned Bun and MUTIE_BUN_PROFILE_PROJECT; three actual baseline phase profiles"]
async fn real_feedback_phase_profile() -> Result<()> {
    let source = std::env::var_os("MUTIE_BUN_PROFILE_PROJECT")
        .map(std::path::PathBuf::from)
        .context("set MUTIE_BUN_PROFILE_PROJECT")?;
    let cancellation = Cancellation::default();
    let timeout = Duration::from_secs(15);
    let limit = NonZeroUsize::new(4 * 1024 * 1024).unwrap();
    let original = project::Project::load(&source, &cancellation)?.files;
    let scope_start = Instant::now();
    let expected = execution_scope(&source, timeout, limit, &cancellation).await?;
    let scope_seconds = scope_start.elapsed().as_secs_f64();
    for repeat in 1..=3 {
        // Separate warm-file calibration distinguishes I/O from SHA-256 CPU
        // time. These samples are NOT included in actual adapter phase totals.
        let start = Instant::now();
        let bytes = fs::read(&expected.executable)?;
        let read_seconds = start.elapsed().as_secs_f64();
        let start = Instant::now();
        let executable_hash = fingerprint(std::hint::black_box(&bytes));
        let hash_seconds = start.elapsed().as_secs_f64();
        drop(bytes);
        ensure!(serde_json::to_value(&expected)?["executable_fingerprint"] == executable_hash);
        let start = Instant::now();
        let recipe = identity::recipe_fingerprint();
        let recipe_seconds = start.elapsed().as_secs_f64();
        ensure!(serde_json::to_value(&expected)?["recipe_fingerprint"] == recipe);
        let total = Instant::now();
        let output = execute_expected(
            &source,
            utf8(&expected.executable)?,
            timeout,
            limit,
            &cancellation,
            Some(&expected),
        )
        .await?;
        let total_seconds = total.elapsed().as_secs_f64();
        ensure!(
            output.classify()? == Outcome::Missed,
            "profile baseline must pass"
        );
        ensure!(output.identity == expected, "profile identity changed");
        println!(
            "bun-profile {}",
            serde_json::json!({
                "repeat": repeat, "debug_assertions": cfg!(debug_assertions),
                "scope_seconds": scope_seconds, "adapter_seconds": total_seconds,
                "phases": output.timings, "test_command_seconds": output.raw.elapsed.as_secs_f64(),
                "calibration": {"read_seconds":read_seconds,"hash_seconds":hash_seconds,"recipe_seconds":recipe_seconds},
                "identity": output.identity,
                "baseline_stderr": output.raw.stderr.text()?,
            })
        );
    }
    ensure!(
        project::Project::load(&source, &cancellation)?.files == original,
        "profile changed inputs"
    );
    Ok(())
}
