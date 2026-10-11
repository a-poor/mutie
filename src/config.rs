use std::{
    fs,
    io::ErrorKind,
    num::{NonZeroU64, NonZeroUsize},
    path::{Component, Path, PathBuf},
    time::{Duration, Instant},
};

use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};

use crate::{
    cli::{Cli, Lang, TestScope, UiMode},
    mutation::Operator,
    rerun::{RerunFilter, RerunMode, RerunRequest},
    score::{MinimumScore, ScoreScope},
};

/// Parsed file values have no defaults; only explicitly supplied CLI values
/// replace them. Invocation-only options such as `list` are not file settings.
#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct RepositoryConfig {
    lang: Option<Lang>,
    jobs: Option<NonZeroUsize>,
    test_scope: Option<TestScope>,
    include: Option<Vec<String>>,
    exclude: Option<Vec<String>>,
    operators: Option<Vec<Operator>>,
    timeout: Option<NonZeroU64>,
    max_log_bytes: Option<NonZeroUsize>,
    baseline_timeout: Option<NonZeroU64>,
    fail_on_missed: Option<bool>,
    min_score: Option<MinimumScore>,
    output_dir: Option<PathBuf>,
    verbose: Option<bool>,
    ui: Option<UiMode>,
}

/// The resolved settings used by both execution and saved reports. Paths are
/// absolute, `auto` language selection is resolved, and `None` timeout means
/// the automatic policy, whose duration is determined after the baseline.
#[derive(Clone, Debug, Serialize)]
pub struct RunConfig {
    pub module_root: PathBuf,
    pub config_file: Option<PathBuf>,
    pub lang: Lang,
    pub jobs: NonZeroUsize,
    pub test_scope: TestScope,
    pub include: Vec<String>,
    pub exclude: Vec<String>,
    /// Empty means all operators; selection is applied after dismissal matching.
    pub operators: Vec<Operator>,
    pub timeout: Option<NonZeroU64>,
    pub max_log_bytes: Option<NonZeroUsize>,
    pub baseline_timeout: NonZeroU64,
    pub fail_on_missed: bool,
    pub min_score: Option<MinimumScore>,
    pub output_dir: PathBuf,
    pub verbose: bool,
    pub list: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rerun: Option<RerunRequest>,
    pub ui: UiMode,
}

impl RunConfig {
    pub fn resolve(cli: Cli) -> Result<Self> {
        Self::resolve_from(
            cli,
            &std::env::current_dir().context("reading working directory")?,
        )
    }

    fn resolve_from(cli: Cli, invocation: &Path) -> Result<Self> {
        let root = invocation
            .join(&cli.path)
            .canonicalize()
            .with_context(|| format!("opening {}", cli.path.display()))?;
        ensure!(
            root.join("go.mod").is_file(),
            "{} must be a Go module root containing go.mod",
            root.display()
        );
        let path = root.join("mutie.toml");
        let (file, config_file) = match fs::symlink_metadata(&path) {
            Ok(_) => {
                let source = fs::read_to_string(&path)
                    .with_context(|| format!("reading configuration {}", path.display()))?;
                let file = toml::from_str::<RepositoryConfig>(&source)
                    .with_context(|| format!("parsing configuration {}", path.display()))?;
                (file, Some(path))
            }
            Err(error) if error.kind() == ErrorKind::NotFound => {
                (RepositoryConfig::default(), None)
            }
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("opening configuration {}", path.display()));
            }
        };
        let lang = cli.lang.or(file.lang).unwrap_or(Lang::Auto);
        ensure!(
            lang != Lang::Rust,
            "Rust support is planned; only Go is supported for now"
        );
        let output_dir = match cli.output_dir {
            Some(path) => nonempty_path(invocation, path)?,
            None => nonempty_path(&root, file.output_dir.unwrap_or_else(|| ".mutie".into()))?,
        };
        let output_dir = resolve_output_path(&output_dir)?;
        ensure!(
            !root.starts_with(&output_dir),
            "the report directory must not be the module root or one of its ancestors"
        );
        let selection = if cli.rerun_missed.is_some() {
            RerunMode::Missed
        } else {
            RerunMode::All
        };
        let mut operators = if cli.clear_operators {
            Vec::new()
        } else if !cli.operators.is_empty() {
            cli.operators
        } else {
            file.operators.unwrap_or_default()
        };
        operators.sort_unstable();
        operators.dedup();
        let rerun = cli
            .rerun
            .or(cli.rerun_missed)
            .map(|path| {
                ensure!(
                    !path.as_os_str().is_empty(),
                    "saved run path must not be empty"
                );
                let path = invocation
                    .join(&path)
                    .canonicalize()
                    .with_context(|| format!("opening saved run {}", path.display()))?;
                ensure!(
                    path.is_dir(),
                    "saved run {} is not a directory",
                    path.display()
                );
                ensure!(
                    !root.starts_with(&path),
                    "the saved run directory must not be the module root or one of its ancestors"
                );
                ensure!(
                    !output_dir.starts_with(&path),
                    "the output directory must not be inside the saved run being rerun"
                );
                Ok(RerunRequest {
                    source_run: path,
                    filter: RerunFilter::new(
                        selection,
                        cli.ids.into_iter().map(NonZeroUsize::get).collect(),
                        operators.clone(),
                    ),
                })
            })
            .transpose()?;
        let config = Self {
            module_root: root,
            config_file,
            lang: Lang::Go,
            jobs: cli
                .jobs
                .or(file.jobs)
                .unwrap_or(NonZeroUsize::new(1).unwrap()),
            test_scope: cli.test_scope.or(file.test_scope).unwrap_or_default(),
            include: merge_patterns(cli.include, cli.clear_include, file.include),
            exclude: merge_patterns(cli.exclude, cli.clear_exclude, file.exclude),
            operators,
            max_log_bytes: if cli.clear_max_log_bytes {
                None
            } else {
                cli.max_log_bytes.or(file.max_log_bytes)
            },
            timeout: if cli.clear_timeout {
                None
            } else {
                cli.timeout.or(file.timeout)
            },
            baseline_timeout: cli
                .baseline_timeout
                .or(file.baseline_timeout)
                .unwrap_or(NonZeroU64::new(300).unwrap()),
            fail_on_missed: !cli.no_fail_on_missed
                && (cli.fail_on_missed || file.fail_on_missed.unwrap_or(false)),
            min_score: if cli.clear_min_score {
                None
            } else {
                cli.min_score.or(file.min_score)
            },
            output_dir,
            verbose: if cli.no_verbose {
                false
            } else {
                cli.verbose || file.verbose.unwrap_or(false)
            },
            list: cli.list,
            rerun,
            ui: cli.ui.or(file.ui).unwrap_or_default(),
        };
        for (name, timeout) in [
            ("timeout", config.timeout),
            ("baseline_timeout", Some(config.baseline_timeout)),
        ] {
            if let Some(seconds) = timeout {
                ensure!(
                    Instant::now()
                        .checked_add(Duration::from_secs(seconds.get()))
                        .is_some(),
                    "{name} is too large for a command deadline"
                );
            }
        }
        Ok(config)
    }

    pub fn score_scope(&self) -> ScoreScope {
        ScoreScope::selected(
            self.rerun.is_some(),
            !self.include.is_empty() || !self.exclude.is_empty() || !self.operators.is_empty(),
        )
    }
}

fn merge_patterns(cli: Vec<String>, clear: bool, configured: Option<Vec<String>>) -> Vec<String> {
    if clear || !cli.is_empty() {
        cli
    } else {
        configured.unwrap_or_default()
    }
}

fn nonempty_path(base: &Path, path: PathBuf) -> Result<PathBuf> {
    ensure!(!path.as_os_str().is_empty(), "output_dir must not be empty");
    Ok(base.join(path))
}

/// Resolve existing symlinks and normalize missing path components without
/// creating report directories. Listing and invalid configurations have no
/// filesystem side effects. Walk in order so `link/..` respects the link target.
fn resolve_output_path(path: &Path) -> Result<PathBuf> {
    let mut resolved = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => continue,
            Component::ParentDir => {
                resolved.pop();
            }
            other => resolved.push(other.as_os_str()),
        }
        match fs::symlink_metadata(&resolved) {
            Ok(_) => {
                resolved = resolved
                    .canonicalize()
                    .with_context(|| format!("resolving report directory {}", path.display()))?;
                ensure!(
                    resolved.is_dir(),
                    "report directory {} is not a directory",
                    resolved.display()
                );
            }
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("resolving report directory {}", path.display()));
            }
        }
    }
    Ok(resolved)
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    fn project() -> Result<tempfile::TempDir> {
        let temp = tempfile::tempdir_in(std::env::temp_dir().canonicalize()?)?;
        fs::create_dir(temp.path().join("module"))?;
        fs::write(
            temp.path().join("module/go.mod"),
            "module example.com/test\n",
        )?;
        Ok(temp)
    }

    fn resolve(base: &Path, args: &[&str]) -> Result<RunConfig> {
        RunConfig::resolve_from(
            Cli::try_parse_from(["mutie", "module"].into_iter().chain(args.iter().copied()))?,
            base,
        )
    }

    #[test]
    fn defaults_work_without_a_file_and_do_not_inherit_parent_config() -> Result<()> {
        let temp = project()?;
        fs::write(temp.path().join("mutie.toml"), "unknown = true")?;
        let config = resolve(temp.path(), &[])?;
        assert_eq!(config.lang, Lang::Go);
        assert_eq!(config.jobs.get(), 1);
        assert_eq!(config.baseline_timeout.get(), 300);
        assert_eq!(config.timeout, None);
        assert!(config.include.is_empty() && config.exclude.is_empty());
        assert!(config.operators.is_empty());
        assert!(!config.verbose && !config.list);
        assert_eq!(config.ui, UiMode::Auto);
        assert_eq!(config.test_scope, TestScope::Full);
        assert!(config.config_file.is_none());
        assert_eq!(config.output_dir, temp.path().join("module/.mutie"));
        assert!(!config.output_dir.exists());
        fs::write(temp.path().join("module/mutie.toml"), "")?;
        let empty = resolve(temp.path(), &[])?;
        assert_eq!(empty.jobs, config.jobs);
        assert_eq!(empty.output_dir, config.output_dir);
        assert_eq!(
            empty.config_file,
            Some(temp.path().join("module/mutie.toml"))
        );
        Ok(())
    }

    #[test]
    fn ui_preference_merges_explicit_overrides_and_rejects_unknown_modes() -> Result<()> {
        let temp = project()?;
        fs::write(temp.path().join("module/mutie.toml"), "ui = 'interactive'")?;
        assert_eq!(resolve(temp.path(), &[])?.ui, UiMode::Interactive);
        assert_eq!(resolve(temp.path(), &["--ui", "plain"])?.ui, UiMode::Plain);
        assert_eq!(resolve(temp.path(), &["--ui", "auto"])?.ui, UiMode::Auto);
        assert!(resolve(temp.path(), &["--ui", "unknown"]).is_err());
        fs::write(temp.path().join("module/mutie.toml"), "ui = 'unknown'")?;
        assert!(resolve(temp.path(), &["--ui", "plain"]).is_err());
        Ok(())
    }

    #[test]
    fn test_scope_merges_config_and_explicit_full_override() -> Result<()> {
        let temp = project()?;
        let file = temp.path().join("module/mutie.toml");
        fs::write(&file, "test_scope = 'affected'")?;
        assert_eq!(resolve(temp.path(), &[])?.test_scope, TestScope::Affected);
        assert_eq!(
            resolve(temp.path(), &["--test-scope", "full"])?.test_scope,
            TestScope::Full
        );
        assert!(resolve(temp.path(), &["--test-scope", "unknown"]).is_err());
        fs::write(&file, "test_scope = 'unknown'")?;
        assert!(resolve(temp.path(), &[]).is_err());
        Ok(())
    }

    #[test]
    fn minimum_score_config_overrides_and_validation() -> Result<()> {
        let temp = project()?;
        assert_eq!(resolve(temp.path(), &[])?.min_score, None);
        let file = temp.path().join("module/mutie.toml");
        for value in ["0", "80", "66.67", "100.0"] {
            fs::write(&file, format!("min_score = {value}"))?;
            assert_eq!(
                resolve(temp.path(), &[])?.min_score,
                Some(value.parse().unwrap())
            );
            assert_eq!(
                resolve(temp.path(), &["--min-score", "50.01"])?.min_score,
                Some("50.01".parse().unwrap())
            );
            assert_eq!(
                resolve(temp.path(), &["--clear-min-score"])?.min_score,
                None
            );
        }
        for value in ["-1", "101", "nan", "inf", "1.001", "'80'", "true"] {
            fs::write(&file, format!("min_score = {value}"))?;
            assert!(resolve(temp.path(), &[]).is_err(), "{value}");
        }
        fs::write(&file, "")?;
        for value in ["-1", "101", "NaN", "inf", "1.001"] {
            assert!(
                resolve(temp.path(), &["--min-score", value]).is_err(),
                "{value}"
            );
        }
        assert!(resolve(temp.path(), &["--min-score", "80", "--clear-min-score"]).is_err());
        Ok(())
    }

    #[test]
    fn findings_policy_defaults_and_explicit_overrides() -> Result<()> {
        let temp = project()?;
        assert!(!resolve(temp.path(), &[])?.fail_on_missed);
        let file = temp.path().join("module/mutie.toml");
        fs::write(&file, "fail_on_missed = true")?;
        assert!(resolve(temp.path(), &[])?.fail_on_missed);
        assert!(!resolve(temp.path(), &["--no-fail-on-missed"])?.fail_on_missed);
        fs::write(&file, "fail_on_missed = false")?;
        assert!(resolve(temp.path(), &["--fail-on-missed"])?.fail_on_missed);
        Ok(())
    }

    #[test]
    fn explicit_cli_values_replace_repository_values_including_defaults() -> Result<()> {
        let temp = project()?;
        fs::write(
            temp.path().join("module/mutie.toml"),
            r#"
lang = "go"
jobs = 4
include = ["internal/**"]
exclude = ["legacy/**"]
timeout = 17
baseline_timeout = 90
output_dir = "reports"
verbose = true
"#,
        )?;
        let from_file = resolve(temp.path(), &[])?;
        assert_eq!(from_file.jobs.get(), 4);
        assert_eq!(from_file.include, ["internal/**"]);
        assert_eq!(from_file.exclude, ["legacy/**"]);
        assert_eq!(from_file.timeout.unwrap().get(), 17);
        assert_eq!(from_file.baseline_timeout.get(), 90);
        assert!(from_file.verbose);
        assert_eq!(from_file.output_dir, temp.path().join("module/reports"));
        let overrides = resolve(
            temp.path(),
            &[
                "--lang",
                "auto",
                "--jobs",
                "1",
                "--include",
                "a.go",
                "--include",
                "b.go",
                "--exclude",
                "b.go",
                "--timeout",
                "10",
                "--baseline-timeout",
                "300",
                "--output-dir",
                "reports",
                "--no-verbose",
            ],
        )?;
        assert_eq!(overrides.lang, Lang::Go);
        assert_eq!(overrides.jobs.get(), 1);
        assert_eq!(overrides.include, ["a.go", "b.go"]);
        assert_eq!(overrides.exclude, ["b.go"]);
        assert_eq!(overrides.timeout.unwrap().get(), 10);
        assert_eq!(overrides.baseline_timeout.get(), 300);
        assert_eq!(overrides.output_dir, temp.path().join("reports"));
        assert!(!overrides.verbose);
        let cleared = resolve(
            temp.path(),
            &[
                "--clear-include",
                "--clear-exclude",
                "--clear-timeout",
                "--no-verbose",
                "--list",
            ],
        )?;
        assert!(cleared.include.is_empty() && cleared.exclude.is_empty());
        assert_eq!(cleared.timeout, None);
        assert!(!cleared.verbose);
        assert!(cleared.list);
        assert!(!cleared.output_dir.exists());
        fs::write(
            temp.path().join("module/mutie.toml"),
            "lang = 'rust'\nverbose = false",
        )?;
        assert!(resolve(temp.path(), &[]).is_err());
        assert!(resolve(temp.path(), &["--lang", "go", "--verbose"])?.verbose);
        Ok(())
    }

    #[test]
    fn rejects_bad_files_values_and_conflicting_overrides() -> Result<()> {
        let temp = project()?;
        for contents in [
            "unknown = 1",
            "[nested]\njobs = 1",
            "jobs = 0",
            "jobs = -1",
            "jobs = 1.5",
            "jobs = '2'",
            "timeout = 0",
            "max_log_bytes = 0",
            "max_log_bytes = -1",
            "max_log_bytes = '1KiB'",
            "baseline_timeout = 0",
            "lang = 'python'",
            "include = '*.go'",
            "exclude = [1]",
            "verbose = 'yes'",
            "fail_on_missed = 'yes'",
            "list = true",
            "rerun_missed = 'previous-run'",
            "rerun = 'previous-run'",
            "ids = [1]",
            "operators = ['unknown']",
            "operators = 'boolean_flip'",
            "operators = [1]",
            "jobs =",
            "jobs = 1\njobs = 2",
        ] {
            fs::write(temp.path().join("module/mutie.toml"), contents)?;
            let error = resolve(temp.path(), &[]).unwrap_err();
            assert!(
                format!("{error:#}").contains("mutie.toml"),
                "{contents}: {error:#}"
            );
        }
        fs::remove_file(temp.path().join("module/mutie.toml"))?;
        fs::create_dir(temp.path().join("module/mutie.toml"))?;
        assert!(
            format!("{:#}", resolve(temp.path(), &[]).unwrap_err())
                .contains("reading configuration")
        );
        for args in [
            vec!["--include", "a.go", "--clear-include"],
            vec!["--exclude", "a.go", "--clear-exclude"],
            vec!["--timeout", "1", "--clear-timeout"],
            vec!["--max-log-bytes", "1", "--clear-max-log-bytes"],
            vec!["--max-log-bytes", "0"],
            vec!["--verbose", "--no-verbose"],
            vec!["--fail-on-missed", "--no-fail-on-missed"],
            vec!["--operator", "call_remove", "--clear-operators"],
            vec!["--list", "--output-dir", "reports"],
            vec!["--jobs", "0"],
            vec!["--timeout", "0"],
            vec!["--baseline-timeout", "0"],
        ] {
            assert!(Cli::try_parse_from(["mutie"].into_iter().chain(args)).is_err());
        }
        Ok(())
    }

    #[test]
    fn rerun_ids_require_a_source_and_filters_resolve_to_normalized_settings() -> Result<()> {
        use crate::mutation::Operator;

        for args in [
            vec!["--id", "1"],
            vec!["--operator", "unknown"],
            vec!["--rerun", "a", "--rerun-missed", "b"],
            vec!["--rerun", "a", "--id", "0"],
            vec!["--rerun", "a", "--id", "-1"],
            vec!["--rerun", "a", "--id", "1,nope"],
            vec!["--rerun", "a", "--id", "1,"],
            vec!["--rerun", "a", "--operator", "unknown"],
        ] {
            assert!(
                Cli::try_parse_from(["mutie"].into_iter().chain(args.clone())).is_err(),
                "{args:?}"
            );
        }
        let temp = project()?;
        let source = temp.path().join("previous");
        fs::create_dir(&source)?;
        for (arg, selection) in [
            ("--rerun", RerunMode::All),
            ("--rerun-missed", RerunMode::Missed),
        ] {
            let config = resolve(
                temp.path(),
                &[
                    arg,
                    "previous",
                    "--id",
                    "3,1",
                    "--id",
                    "3",
                    "--operator",
                    "boolean_flip,arithmetic_replace",
                    "--operator",
                    "boolean_flip",
                ],
            )?;
            let rerun = config.rerun.context("missing rerun request")?;
            assert_eq!(rerun.source_run, source);
            assert_eq!(rerun.filter.selection, selection);
            assert_eq!(rerun.filter.source_ids, [1, 3]);
            assert_eq!(
                rerun.filter.operators,
                [Operator::ArithmeticReplace, Operator::BooleanFlip]
            );
            assert!(!config.output_dir.exists());
        }
        Ok(())
    }

    #[test]
    fn operator_selection_merges_normalizes_and_clears_for_fresh_runs_and_reruns() -> Result<()> {
        let temp = project()?;
        let file = temp.path().join("module/mutie.toml");
        fs::create_dir(temp.path().join("previous"))?;
        fs::write(
            &file,
            "operators = ['call_remove', 'boolean_flip', 'call_remove']",
        )?;
        for source in [vec![], vec!["--rerun", "previous"]] {
            for (extra, expected) in [
                (vec![], vec![Operator::BooleanFlip, Operator::CallRemove]),
                (vec!["--clear-operators"], vec![]),
                (
                    vec![
                        "--operator",
                        "arithmetic_replace,call_remove",
                        "--operator",
                        "call_remove",
                    ],
                    vec![Operator::ArithmeticReplace, Operator::CallRemove],
                ),
            ] {
                let args: Vec<_> = source.iter().chain(&extra).copied().collect();
                let config = resolve(temp.path(), &args)?;
                assert_eq!(config.operators, expected);
                if let Some(rerun) = config.rerun {
                    assert_eq!(rerun.filter.operators, expected);
                }
            }
        }
        fs::write(&file, "operators = []")?;
        assert!(resolve(temp.path(), &[])?.operators.is_empty());
        fs::write(&file, "operators = ['unknown']")?;
        assert!(resolve(temp.path(), &["--clear-operators"]).is_err());
        Ok(())
    }

    #[test]
    fn log_limit_defaults_to_unlimited_and_merges_explicit_overrides() -> Result<()> {
        let temp = project()?;
        assert_eq!(resolve(temp.path(), &[])?.max_log_bytes, None);
        let file = temp.path().join("module/mutie.toml");
        fs::write(&file, "max_log_bytes = 1024")?;
        assert_eq!(
            resolve(temp.path(), &[])?.max_log_bytes.unwrap().get(),
            1024
        );
        assert_eq!(
            resolve(temp.path(), &["--max-log-bytes", "1"])?
                .max_log_bytes
                .unwrap()
                .get(),
            1
        );
        assert_eq!(
            resolve(temp.path(), &["--clear-max-log-bytes"])?.max_log_bytes,
            None
        );
        fs::write(file, "max_log_bytes = 0")?;
        assert!(resolve(temp.path(), &["--clear-max-log-bytes"]).is_err());
        Ok(())
    }

    #[test]
    fn output_validation_has_no_side_effects_and_normalizes_paths() -> Result<()> {
        let temp = project()?;
        fs::write(temp.path().join("sentinel"), "keep me")?;
        for path in [
            "",
            ".",
            "module",
            "module/missing/..",
            "sentinel",
            "sentinel/reports",
        ] {
            assert!(
                resolve(temp.path(), &["--output-dir", path]).is_err(),
                "{path}"
            );
        }
        assert_eq!(fs::read_to_string(temp.path().join("sentinel"))?, "keep me");
        assert!(!temp.path().join("module/missing").exists());
        let config = resolve(
            temp.path(),
            &["--output-dir", "module/missing/../reports/new"],
        )?;
        assert_eq!(config.output_dir, temp.path().join("module/reports/new"));
        assert!(!config.output_dir.exists());
        assert!(resolve(temp.path(), &["--timeout", "18446744073709551615"]).is_err());
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn resolves_symlinks_before_parent_components_and_rejects_dangling_links() -> Result<()> {
        use std::os::unix::fs::symlink;
        let temp = project()?;
        fs::create_dir_all(temp.path().join("other/nested"))?;
        symlink(temp.path().join("other/nested"), temp.path().join("link"))?;
        let config = resolve(temp.path(), &["--output-dir", "link/../reports"])?;
        assert_eq!(config.output_dir, temp.path().join("other/reports"));
        symlink(temp.path().join("module"), temp.path().join("root-link"))?;
        assert!(resolve(temp.path(), &["--output-dir", "root-link"]).is_err());
        symlink(temp.path().join("missing"), temp.path().join("dangling"))?;
        assert!(resolve(temp.path(), &["--output-dir", "dangling"]).is_err());
        symlink(
            temp.path().join("missing.toml"),
            temp.path().join("module/mutie.toml"),
        )?;
        assert!(resolve(temp.path(), &[]).is_err());
        Ok(())
    }
}
