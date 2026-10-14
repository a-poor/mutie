//! Opt-in diagnostic for the complete Cargo adapter, without source mutations.
use super::*;

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "requires MUTIE_REPRO_ROOT with project-0/project-1 and logs directories"]
async fn unmutated_cargo_adapter() -> Result<()> {
    let root = std::path::PathBuf::from(
        std::env::var_os("MUTIE_REPRO_ROOT").context("set MUTIE_REPRO_ROOT")?,
    );
    ensure!(root.is_absolute(), "reproduction root must be absolute");
    ensure!(root.join("logs").is_dir(), "create an empty logs directory");
    for jobs in [1, 2] {
        for cached in [false, true] {
            let mut tasks = tokio::task::JoinSet::new();
            for worker in 0..jobs {
                let root = root.clone();
                tasks.spawn(async move {
                    let project = root.join(format!("project-{worker}"));
                    let cache = cached.then(|| BuildCache::new("mutie".into())).transpose()?;
                    for round in 0..if cached { 2 } else { 1 } {
                        let label = format!("jobs-{jobs}-cached-{cached}-round-{round}-worker-{worker}");
                        let stem = root.join("logs").join(&label);
                        // Reserve each label before execution to preserve earlier evidence.
                        std::fs::File::create_new(stem.with_extension("reserved"))?;
                        let output = test(
                            &project,
                            Duration::from_secs(300),
                            None,
                            None,
                            cache.as_ref(),
                            &Cancellation::default(),
                            |phase| {
                                eprintln!("{label}: {phase:?}");
                                std::future::ready(())
                            },
                        ).await?;
                        std::io::copy(&mut output.stdout.reader()?, &mut std::fs::File::create_new(stem.with_extension("stdout.log"))?)?;
                        std::io::copy(&mut output.stderr.reader()?, &mut std::fs::File::create_new(stem.with_extension("stderr.log"))?)?;
                        let result = serde_json::json!({
                            "label": label, "success": output.status.success(),
                            "timed_out": output.timed_out, "interrupted": output.interrupted,
                            "capture_error": output.capture_error, "seconds": output.elapsed.as_secs_f64(),
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
