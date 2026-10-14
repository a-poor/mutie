//! Opt-in diagnostics for the complete Cargo adapter on caller-prepared copies.
use super::*;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires MUTIE_REPRO_ROOT with project-0/project-1 and logs directories"]
async fn unmutated_cargo_adapter() -> Result<()> {
    adapter_matrix(&[1, 2], &[(false, 1), (true, 2)]).await
}

/// The caller prepares project-0/project-1 as control and candidate copies.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires MUTIE_REPRO_ROOT with isolated control/candidate projects and logs"]
async fn paired_cargo_adapter() -> Result<()> {
    adapter_matrix(&[2], &[(false, 1), (true, 2)]).await
}

/// One cold and two warm executions per worker, first serial then paired.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires MUTIE_REPRO_ROOT; profiles repeated full Cargo suites in isolated copies"]
async fn profile_cargo_adapter() -> Result<()> {
    adapter_matrix(&[1, 2], &[(true, 3)]).await
}

async fn adapter_matrix(job_counts: &[usize], modes: &[(bool, usize)]) -> Result<()> {
    let root = std::path::PathBuf::from(
        std::env::var_os("MUTIE_REPRO_ROOT").context("set MUTIE_REPRO_ROOT")?,
    );
    ensure!(root.is_absolute(), "reproduction root must be absolute");
    ensure!(root.join("logs").is_dir(), "create an empty logs directory");
    for &jobs in job_counts {
        for &(cached, rounds) in modes {
            let mut tasks = tokio::task::JoinSet::new();
            for worker in 0..jobs {
                let root = root.clone();
                tasks.spawn(async move {
                    let project = root.join(format!("project-{worker}"));
                    let cache = cached.then(|| BuildCache::new("mutie".into())).transpose()?;
                    for round in 0..rounds {
                        let label = format!("jobs-{jobs}-cached-{cached}-round-{round}-worker-{worker}");
                        let stem = root.join("logs").join(&label);
                        // Reserve each label before execution to preserve earlier evidence.
                        std::fs::File::create_new(stem.with_extension("reserved"))?;
                        let started = Instant::now();
                        let mut phases = Vec::new();
                        let output = test(
                            &project,
                            Duration::from_secs(300),
                            None,
                            None,
                            cache.as_ref(),
                            &Cancellation::default(),
                            |phase| {
                                phases.push(serde_json::json!({
                                    "phase": format!("{phase:?}"),
                                    "seconds": started.elapsed().as_secs_f64(),
                                }));
                                eprintln!("{label}: {phase:?}");
                                std::future::ready(())
                            },
                        ).await?;
                        let observed_seconds = started.elapsed().as_secs_f64();
                        std::io::copy(&mut output.stdout.reader()?, &mut std::fs::File::create_new(stem.with_extension("stdout.log"))?)?;
                        std::io::copy(&mut output.stderr.reader()?, &mut std::fs::File::create_new(stem.with_extension("stderr.log"))?)?;
                        let result = serde_json::json!({
                            "label": label, "success": output.status.success(),
                            "timed_out": output.timed_out, "interrupted": output.interrupted,
                            "capture_error": output.capture_error, "seconds": output.elapsed.as_secs_f64(),
                            "observed_seconds": observed_seconds, "phases": phases,
                        });
                        std::fs::write(stem.with_extension("json"), serde_json::to_vec_pretty(&result)?)?;
                        eprintln!("{result}");
                        ensure!(
                            output.status.success() && !output.timed_out && !output.interrupted
                                && output.capture_error.is_none(),
                            "{label} failed; captures saved under {}", root.join("logs").display()
                        );
                    }
                    Ok::<_, anyhow::Error>(())
                });
            }
            let mut failure = None;
            while let Some(result) = tasks.join_next().await {
                if let Err(error) = result
                    .context("adapter probe task panicked")
                    .and_then(|r| r)
                {
                    failure.get_or_insert(error);
                }
            }
            if let Some(error) = failure {
                return Err(error);
            }
        }
    }
    Ok(())
}
