use std::{num::NonZeroU64, num::NonZeroUsize, path::PathBuf};

use clap::{Parser, ValueEnum};

#[derive(Debug, Parser)]
#[command(
    version,
    about = "Find gaps in Go tests by making small changes to your code"
)]
pub struct Cli {
    /// Go module root (must contain go.mod)
    #[arg(default_value = ".")]
    pub path: PathBuf,

    #[arg(long, value_enum, default_value_t = Lang::Auto)]
    pub lang: Lang,

    /// Number of mutation workers (each runs the full test suite)
    #[arg(short, long, default_value = "1")]
    pub jobs: NonZeroUsize,

    /// Only mutate files matching these module-relative globs (repeatable)
    #[arg(long, value_name = "GLOB")]
    pub include: Vec<String>,

    /// Exclude files matching these module-relative globs (repeatable)
    #[arg(long, value_name = "GLOB")]
    pub exclude: Vec<String>,

    /// Timeout per mutation in seconds (default: max(10, 5 * baseline duration))
    #[arg(long, value_name = "SECONDS")]
    pub timeout: Option<NonZeroU64>,

    /// Timeout for the baseline and package discovery, in seconds
    #[arg(long, default_value = "300", value_name = "SECONDS")]
    pub baseline_timeout: NonZeroU64,

    /// List mutations without running tests
    #[arg(long)]
    pub list: bool,

    /// Show captured test/build output for every mutation
    #[arg(short, long)]
    pub verbose: bool,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug, ValueEnum)]
pub enum Lang {
    Auto,
    Go,
    Rust,
}
