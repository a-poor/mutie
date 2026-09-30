use std::{num::NonZeroU64, num::NonZeroUsize, path::PathBuf};

use clap::{Parser, ValueEnum};
use serde::{Deserialize, Serialize};

#[derive(Debug, Parser)]
#[command(
    version,
    about = "Find gaps in Go tests by making small changes to your code"
)]
pub struct Cli {
    /// Go module root (must contain go.mod)
    #[arg(default_value = ".")]
    pub path: PathBuf,

    /// Project language (default: auto)
    #[arg(long, value_enum)]
    pub lang: Option<Lang>,

    /// Number of mutation workers (default: 1; each runs the full test suite)
    #[arg(short, long)]
    pub jobs: Option<NonZeroUsize>,

    /// Only mutate files matching these module-relative globs (repeatable)
    #[arg(long, value_name = "GLOB")]
    pub include: Vec<String>,

    /// Clear configured include patterns, allowing all eligible files
    #[arg(long, conflicts_with = "include")]
    pub clear_include: bool,

    /// Exclude files matching these module-relative globs (repeatable)
    #[arg(long, value_name = "GLOB")]
    pub exclude: Vec<String>,

    /// Clear configured exclude patterns
    #[arg(long, conflicts_with = "exclude")]
    pub clear_exclude: bool,

    /// Timeout per mutation in seconds (default: max(10, 5 * baseline duration))
    #[arg(long, value_name = "SECONDS")]
    pub timeout: Option<NonZeroU64>,

    /// Use the automatic mutation timeout instead of the configured value
    #[arg(long, conflicts_with = "timeout")]
    pub clear_timeout: bool,

    /// Timeout for the baseline and package discovery, in seconds (default: 300)
    #[arg(long, value_name = "SECONDS")]
    pub baseline_timeout: Option<NonZeroU64>,

    /// List mutations without running tests
    #[arg(long)]
    pub list: bool,

    /// Parent directory for saved runs (default: <module>/.mutie)
    #[arg(long, value_name = "DIR", conflicts_with = "list")]
    pub output_dir: Option<PathBuf>,

    /// Show captured test/build output for every mutation
    #[arg(short, long)]
    pub verbose: bool,

    /// Disable configured verbose output
    #[arg(long, conflicts_with = "verbose")]
    pub no_verbose: bool,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug, ValueEnum, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Lang {
    Auto,
    Go,
    Rust,
}
