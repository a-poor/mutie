//! The small built-in boundary needed by the Go and Rust execution paths.
use std::{future::Future, num::NonZeroUsize, path::Path, time::Duration};

use anyhow::{Result, bail, ensure};

use crate::{
    cancellation::Cancellation,
    cli::{Lang, TestScope},
    command::{CommandOutput, TestPhase},
    go, rust,
};

/// Resolve a manifest without loading run settings or invoking language tools.
pub fn resolve(root: &Path, requested: Lang) -> Result<Lang> {
    let lang = match requested {
        Lang::Auto => match (
            root.join("go.mod").is_file(),
            root.join("Cargo.toml").is_file(),
        ) {
            (true, false) => Lang::Go,
            (false, true) => Lang::Rust,
            (true, true) => {
                bail!("both go.mod and Cargo.toml exist; select --lang go or --lang rust")
            }
            (false, false) => bail!("{} must contain go.mod or Cargo.toml", root.display()),
        },
        lang => lang,
    };
    let manifest = if lang == Lang::Go {
        "go.mod"
    } else {
        "Cargo.toml"
    };
    ensure!(
        root.join(manifest).is_file(),
        "{} must be a project root containing {manifest}",
        root.display()
    );
    Ok(lang)
}

pub fn test_command(lang: Lang) -> Vec<&'static str> {
    match lang {
        Lang::Rust => std::iter::once("cargo")
            .chain(rust::TEST_ARGS.iter().copied())
            .collect(),
        _ => std::iter::once("go")
            .chain(go::TEST_ARGS.iter().copied())
            .collect(),
    }
}

pub fn discovery_command(lang: Lang, scope: TestScope) -> Vec<&'static str> {
    match lang {
        Lang::Rust => std::iter::once("cargo")
            .chain(rust::METADATA_ARGS.iter().copied())
            .collect(),
        _ => std::iter::once("go")
            .chain(go::list_args(scope).iter().copied())
            .collect(),
    }
}

#[allow(clippy::too_many_arguments)]
pub async fn test<F, Fut>(
    lang: Lang,
    root: &Path,
    packages: &[String],
    limit: Duration,
    log_limit: Option<NonZeroUsize>,
    capture_limit: Option<NonZeroUsize>,
    cancellation: &Cancellation,
    progress: F,
) -> Result<CommandOutput>
where
    F: FnMut(TestPhase) -> Fut + Send,
    Fut: Future<Output = ()> + Send,
{
    match lang {
        Lang::Rust => {
            rust::test(
                root,
                limit,
                log_limit,
                capture_limit,
                cancellation,
                progress,
            )
            .await
        }
        _ => {
            go::test_observed(
                root,
                packages,
                limit,
                log_limit,
                capture_limit,
                cancellation,
                progress,
            )
            .await
        }
    }
}
