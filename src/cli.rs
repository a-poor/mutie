use std::{num::NonZeroU64, num::NonZeroUsize, path::PathBuf};

use clap::{Parser, Subcommand, ValueEnum};
use serde::{Deserialize, Serialize};

use crate::mutation::Operator;

#[derive(Debug, Parser)]
#[command(
    version,
    args_conflicts_with_subcommands = true,
    subcommand_negates_reqs = true,
    about = "Find gaps in Go, Rust and Vitest tests by making small changes to your code"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,

    /// Project root (go.mod, Cargo.toml, or package.json with --lang vitest)
    #[arg(default_value = ".")]
    pub path: PathBuf,

    /// Project language (default: auto)
    #[arg(long, value_enum)]
    pub lang: Option<Lang>,

    /// Cargo artifact mode: fresh builds or worker-local dependency reuse (default: fresh)
    #[arg(long, value_enum)]
    pub rust_build_cache: Option<RustBuildCache>,

    /// Number of mutation workers (default: 1)
    #[arg(short, long)]
    pub jobs: Option<NonZeroUsize>,

    /// Mutation tests to run (default: full; baseline always tests the full module)
    #[arg(long, value_enum)]
    pub test_scope: Option<TestScope>,

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

    /// Retain at most BYTES of each saved log (head and tail; default: unlimited)
    #[arg(long, value_name = "BYTES")]
    pub max_log_bytes: Option<NonZeroUsize>,

    /// Disable the configured log limit
    #[arg(long, conflicts_with = "max_log_bytes")]
    pub clear_max_log_bytes: bool,

    /// Stop a command if combined stdout/stderr exceeds BYTES (default: unlimited)
    #[arg(long, value_name = "BYTES")]
    pub max_capture_bytes: Option<NonZeroUsize>,

    /// Disable the configured command output capture limit
    #[arg(long, conflicts_with = "max_capture_bytes")]
    pub clear_max_capture_bytes: bool,

    /// Timeout for the baseline and package discovery, in seconds (default: 300)
    #[arg(long, value_name = "SECONDS")]
    pub baseline_timeout: Option<NonZeroU64>,

    /// Exit with code 3 after a completed run with undismissed missed mutations
    #[arg(long)]
    pub fail_on_missed: bool,

    /// Disable configured failure on undismissed missed mutations
    #[arg(long, conflicts_with = "fail_on_missed")]
    pub no_fail_on_missed: bool,

    /// Minimum raw mutation score, 0-100 with up to two decimals (exit 3 if unmet)
    #[arg(long, value_name = "PERCENT")]
    pub min_score: Option<crate::score::MinimumScore>,

    /// Disable the configured minimum mutation score
    #[arg(long, conflicts_with = "min_score")]
    pub clear_min_score: bool,

    /// List mutations without running tests
    #[arg(long)]
    pub list: bool,

    /// Rerun saved mutations regardless of their previous outcome
    #[arg(long, value_name = "RUN_DIR", group = "rerun_source")]
    pub rerun: Option<PathBuf>,

    /// Rerun only completed missed mutations from a saved run directory
    #[arg(long, value_name = "RUN_DIR", group = "rerun_source")]
    pub rerun_missed: Option<PathBuf>,

    /// Select these source-run mutation IDs (repeatable or comma-separated)
    #[arg(
        long = "id",
        value_name = "ID",
        value_delimiter = ',',
        requires = "rerun_source"
    )]
    pub ids: Vec<NonZeroUsize>,

    /// Select mutation operators (repeatable or comma-separated; replaces config)
    #[arg(long = "operator", value_enum, value_delimiter = ',')]
    pub operators: Vec<Operator>,

    /// Clear configured operator selection, allowing all operators
    #[arg(long, conflicts_with = "operators")]
    pub clear_operators: bool,

    /// Parent directory for saved runs (default: <module>/.mutie)
    #[arg(long, value_name = "DIR", conflicts_with = "list")]
    pub output_dir: Option<PathBuf>,

    /// Show captured test/build output in plain mode
    #[arg(short, long)]
    pub verbose: bool,

    /// Disable configured verbose output
    #[arg(long, conflicts_with = "verbose")]
    pub no_verbose: bool,

    /// Terminal status view (default: auto; --list always uses plain output)
    #[arg(long, value_enum)]
    pub ui: Option<UiMode>,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Check an isolated Vitest baseline (no mutations, saved reports or reruns)
    VitestCheck {
        #[arg(default_value = ".", value_name = "PATH")]
        path: PathBuf,
        /// Command deadline in seconds
        #[arg(long, default_value = "60")]
        timeout: NonZeroU64,
        /// Combined raw stdout/stderr byte limit
        #[arg(long, default_value = "4194304")]
        max_capture_bytes: NonZeroUsize,
    },
    /// Preview Vitest boolean mutations (read-only; execution is not implemented)
    VitestList {
        #[arg(default_value = ".", value_name = "PATH")]
        path: PathBuf,
        /// Include production source paths matching this glob
        #[arg(long)]
        include: Vec<String>,
        /// Exclude production source paths matching this glob
        #[arg(long)]
        exclude: Vec<String>,
    },
    /// List saved runs, their status, age, and logical disk usage
    Runs {
        /// Parent directory of saved runs (relative to the current directory)
        #[arg(default_value = ".mutie", value_name = "DIR")]
        directory: PathBuf,
    },
    /// Preview removal of finalized saved runs; deletion requires --apply
    #[command(group(clap::ArgGroup::new("retention").required(true).multiple(true).args(["keep_last", "older_than"])))]
    Prune {
        /// Parent directory of saved runs (relative to the current directory)
        #[arg(default_value = ".mutie", value_name = "DIR")]
        directory: PathBuf,
        /// Keep the newest N eligible finalized runs (0 keeps none)
        #[arg(long, value_name = "N")]
        keep_last: Option<usize>,
        /// Select runs finished more than DAYS * 24 hours ago; intersects --keep-last
        #[arg(long, value_name = "DAYS")]
        older_than: Option<NonZeroU64>,
        /// Delete selected runs after revalidation (default: preview only)
        #[arg(long)]
        apply: bool,
    },
    /// Manage repository-owned mutation dismissals
    Dismiss {
        /// Project root where dismissals are stored (go.mod or Cargo.toml)
        #[arg(long, global = true, default_value = ".", value_name = "PATH")]
        module: PathBuf,
        /// Project language (default: auto; check also uses repository configuration)
        #[arg(long, global = true, value_enum)]
        lang: Option<Lang>,
        #[command(subcommand)]
        action: DismissAction,
    },
    /// Browse a finalized saved run, its diffs, and captured logs
    Report {
        /// Saved run directory (no Go module or checkout required)
        run_dir: PathBuf,
        /// Module root to use in generated rerun commands instead of the recorded path
        #[arg(long, value_name = "PATH")]
        module: Option<PathBuf>,
    },
}

#[derive(Debug, Subcommand)]
pub enum DismissAction {
    /// Record a dismissal using a finalized saved run's mutation identity
    Add {
        /// Finalized saved run with a complete mutation inventory
        run_dir: PathBuf,
        /// Mutation ID in the saved run
        #[arg(long)]
        id: NonZeroUsize,
        /// Required nonblank explanation of the review decision
        #[arg(long)]
        reason: String,
        /// Optional nonblank supporting note
        #[arg(long)]
        note: Option<String>,
    },
    /// Inspect stored dismissals without running tools or checking current source
    List,
    /// Change a stored dismissal's reason or note (keeps its source identity)
    Update {
        /// Dismissal ID from `dismiss list`, not a saved-run mutation ID
        id: NonZeroUsize,
        /// Replace the reason with nonblank text
        #[arg(long)]
        reason: Option<String>,
        /// Replace the note with nonblank text
        #[arg(long, conflicts_with = "clear_note")]
        note: Option<String>,
        /// Remove the existing note
        #[arg(long)]
        clear_note: bool,
    },
    /// Remove a stored dismissal by its dismissal ID
    Remove { id: NonZeroUsize },
    /// Rediscover mutations and flag stale, missing, or ambiguous dismissals
    Check,
}

#[derive(Copy, Clone, Default, PartialEq, Eq, Debug, ValueEnum, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum UiMode {
    #[default]
    Auto,
    Interactive,
    Plain,
}

#[derive(Copy, Clone, Default, PartialEq, Eq, Debug, ValueEnum, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum TestScope {
    #[default]
    Full,
    Affected,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug, ValueEnum, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Lang {
    Auto,
    Go,
    Rust,
    Vitest,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize, ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum RustBuildCache {
    #[default]
    Fresh,
    Worker,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_command_is_separate_from_run_arguments() {
        let cli = Cli::try_parse_from([
            "mutie",
            "report",
            "/saved/run",
            "--module",
            "/moved/checkout",
        ])
        .unwrap();
        assert!(
            matches!(cli.command, Some(Command::Report { run_dir, module })
            if run_dir == std::path::Path::new("/saved/run")
                && module.as_deref() == Some(std::path::Path::new("/moved/checkout")))
        );
        for args in [
            vec!["mutie", "report"],
            vec!["mutie", "report", "/saved/run", "--list"],
            vec!["mutie", "--list", "report", "/saved/run"],
        ] {
            assert!(Cli::try_parse_from(args).is_err());
        }
        let run = Cli::try_parse_from(["mutie", "./report", "--list"]).unwrap();
        assert!(run.command.is_none());
        assert!(run.list);
    }
}
