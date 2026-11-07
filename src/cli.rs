use std::{
    num::{NonZeroU64, NonZeroUsize},
    path::PathBuf,
};

use clap::{Args, Parser, Subcommand, ValueEnum};
use serde::{Deserialize, Serialize};

#[derive(Debug, Parser)]
#[command(
    version,
    about = "Find gaps in tests by making small changes to your code"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum Language {
    Go,
    Rust,
    Js,
    Python,
}

impl Language {
    pub fn name(self) -> &'static str {
        match self {
            Self::Go => "go",
            Self::Rust => "rust",
            Self::Js => "js",
            Self::Python => "python",
        }
    }
    pub fn of(lang: Lang) -> Self {
        match lang {
            Lang::Go | Lang::Auto => Self::Go,
            Lang::Rust => Self::Rust,
            Lang::Python => Self::Python,
            Lang::Vitest | Lang::Node | Lang::Bun => Self::Js,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum JsRunner {
    Vitest,
    Node,
    Bun,
}
impl JsRunner {
    pub fn lang(self) -> Lang {
        match self {
            Self::Vitest => Lang::Vitest,
            Self::Node => Lang::Node,
            Self::Bun => Lang::Bun,
        }
    }
}

#[derive(Clone, Debug, Args)]
pub struct ProjectArgs {
    /// Directory containing mutie.toml (or the project, without a config)
    #[arg(default_value = ".", value_name = "PATH", conflicts_with = "config")]
    pub path: PathBuf,
    /// Select one enabled language when the config contains several
    #[arg(long, value_enum)]
    pub lang: Option<Language>,
    /// Use this configuration file; configured paths are relative to its directory
    #[arg(long, value_name = "FILE")]
    pub config: Option<PathBuf>,
}

#[derive(Debug, Default, Args)]
pub struct PresentationArgs {
    /// Terminal status view
    #[arg(long, value_enum, default_value = "auto")]
    pub ui: UiMode,
    /// Show captured test/build output in plain mode
    #[arg(short, long)]
    pub verbose: bool,
}

#[derive(Debug, Args)]
pub struct RunArgs {
    #[command(flatten)]
    pub project: ProjectArgs,
    /// Override the configured number of mutation workers
    #[arg(short, long)]
    pub jobs: Option<NonZeroUsize>,
    /// Override the timeout per mutation, in seconds
    #[arg(long, value_name = "SECONDS")]
    pub timeout: Option<NonZeroU64>,
    #[command(flatten)]
    pub presentation: PresentationArgs,
}

#[derive(Debug, Args)]
pub struct PreviewArgs {
    #[command(flatten)]
    pub project: ProjectArgs,
    /// Print machine-readable JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Args)]
pub struct RerunArgs {
    /// Finalized saved run to select mutations from
    pub run_dir: PathBuf,
    #[command(flatten)]
    pub run: RunArgs,
    /// Select only completed missed mutations
    #[arg(long)]
    pub missed: bool,
    /// Select mutation IDs from the saved run (repeatable or comma-separated)
    #[arg(long, value_delimiter = ',')]
    pub id: Vec<NonZeroUsize>,
    /// Preview the selected mutations without running tests or saving a report
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Run mutation testing
    Run(RunArgs),
    /// List mutations without running tests or saving a report
    List(PreviewArgs),
    /// Validate the project and run its isolated baseline without mutations or reports
    Check(PreviewArgs),
    /// Rerun mutations selected from a saved run
    Rerun(RerunArgs),
    /// Create mutie.toml from detected project tooling (does not run tests)
    Init {
        #[arg(default_value = ".", value_name = "PATH")]
        path: PathBuf,
        #[arg(long, value_enum)]
        lang: Option<Language>,
        /// JavaScript/TypeScript test runner
        #[arg(long, value_enum)]
        runner: Option<JsRunner>,
        /// Pinned Python virtual-environment launcher
        #[arg(long, value_name = "EXECUTABLE")]
        python: Option<PathBuf>,
        #[arg(long)]
        non_interactive: bool,
        #[arg(long)]
        dry_run: bool,
    },
    /// List saved runs, their status, age, and logical disk usage
    Runs {
        #[arg(default_value = ".mutie", value_name = "DIR")]
        directory: PathBuf,
    },
    /// Preview removal of finalized saved runs; deletion requires --apply
    #[command(group(clap::ArgGroup::new("retention").required(true).multiple(true).args(["keep_last", "older_than"])))]
    Prune {
        #[arg(default_value = ".mutie", value_name = "DIR")]
        directory: PathBuf,
        #[arg(long, value_name = "N")]
        keep_last: Option<usize>,
        #[arg(long, value_name = "DAYS")]
        older_than: Option<NonZeroU64>,
        #[arg(long)]
        apply: bool,
    },
    /// Manage repository-owned mutation dismissals
    Dismiss {
        /// Directory containing mutie.toml (or the project, without a config)
        #[arg(
            long,
            global = true,
            default_value = ".",
            value_name = "PATH",
            conflicts_with = "config"
        )]
        path: PathBuf,
        #[arg(long, global = true, value_enum)]
        lang: Option<Language>,
        #[arg(long, global = true, value_name = "FILE")]
        config: Option<PathBuf>,
        #[command(subcommand)]
        action: DismissAction,
    },
    /// Browse a finalized saved run, its diffs, and captured logs
    Report {
        run_dir: PathBuf,
        /// Project root to use in exported rerun commands after moving a checkout
        #[arg(long, value_name = "PATH")]
        module: Option<PathBuf>,
    },
}

/// Invocation intent, separate from repository execution settings.
#[derive(Debug)]
pub struct Invocation {
    pub project: ProjectArgs,
    pub jobs: Option<NonZeroUsize>,
    pub timeout: Option<NonZeroU64>,
    pub presentation: PresentationArgs,
    pub list: bool,
    pub check: bool,
    pub json: bool,
    pub metadata_only: bool,
    pub rerun: Option<PathBuf>,
    pub missed: bool,
    pub ids: Vec<NonZeroUsize>,
}
impl Invocation {
    pub fn project(project: ProjectArgs) -> Self {
        Self {
            project,
            jobs: None,
            timeout: None,
            presentation: PresentationArgs::default(),
            list: false,
            check: false,
            json: false,
            metadata_only: false,
            rerun: None,
            missed: false,
            ids: Vec::new(),
        }
    }
    pub fn run(args: RunArgs) -> Self {
        Self {
            jobs: args.jobs,
            timeout: args.timeout,
            presentation: args.presentation,
            ..Self::project(args.project)
        }
    }
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
    Node,
    Bun,
    Python,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize, ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum RustBuildCache {
    #[default]
    Fresh,
    Worker,
}

#[cfg(test)]
impl From<Cli> for Invocation {
    fn from(cli: Cli) -> Self {
        match cli.command.unwrap() {
            Command::Run(args) => Self::run(args),
            _ => panic!("expected run arguments"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn commands_require_explicit_actions_and_keep_settings_in_config() {
        assert!(Cli::try_parse_from(["mutie"]).unwrap().command.is_none());
        for args in [
            vec!["mutie", "."],
            vec!["mutie", "--list"],
            vec!["mutie", "python-list"],
            vec!["mutie", "bun-check"],
            vec!["mutie", "vitest-list"],
            vec!["mutie", "run", "--operator", "boolean_flip"],
            vec!["mutie", "run", "--include", "*.rs"],
            vec!["mutie", "run", "--python", "/python"],
            vec!["mutie", "run", "--clear-timeout"],
            vec!["mutie", "run", "--lang", "vitest"],
            vec!["mutie", "run", "--rerun", "report"],
            vec!["mutie", "rerun"],
            vec!["mutie", "list", "--jobs", "2"],
            vec!["mutie", "check", "--ui", "interactive"],
        ] {
            assert!(Cli::try_parse_from(&args).is_err(), "{args:?}");
        }
        for command in ["run", "list", "check"] {
            Cli::try_parse_from(["mutie", command, "--lang", "js", "--config", "ci.toml"]).unwrap();
        }
        let cli = Cli::try_parse_from([
            "mutie",
            "rerun",
            "saved",
            "repo",
            "--missed",
            "--id",
            "2,4",
            "--dry-run",
        ])
        .unwrap();
        assert!(matches!(
            cli.command,
            Some(Command::Rerun(RerunArgs {
                missed: true,
                dry_run: true,
                ..
            }))
        ));
    }
}
