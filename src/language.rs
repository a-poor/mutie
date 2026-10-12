//! The small built-in boundary needed by the Go and Rust execution paths.
use std::{future::Future, num::NonZeroUsize, path::Path, time::Duration};

use anyhow::Result;

use crate::{
    cancellation::Cancellation,
    cli::{Lang, TestScope},
    command::{CommandOutput, TestPhase},
    go, rust,
};

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
