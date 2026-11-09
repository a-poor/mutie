use crate::{
    cli::{Invocation, JsRunner, Lang, Language, RustBuildCache, TestScope, UiMode},
    mutation::Operator,
    rerun::{RerunFilter, RerunMode, RerunRequest},
    score::{MinimumScore, ScoreScope},
};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::ErrorKind,
    num::{NonZeroU64, NonZeroUsize},
    path::{Component, Path, PathBuf},
    time::{Duration, Instant},
};

// Each table has an explicit schema. Unknown keys (including UI options and
// settings belonging to another runner) fail even in unselected sections.
macro_rules! settings_table {
    ($name:ident { $($extra:ident : $ty:ty),* $(,)? }) => {
        #[derive(Default, Deserialize)]
        #[serde(deny_unknown_fields)]
        struct $name {
            jobs: Option<NonZeroUsize>,
            timeout: Option<NonZeroU64>,
            baseline_timeout: Option<NonZeroU64>,
            max_log_bytes: Option<NonZeroUsize>,
            max_capture_bytes: Option<NonZeroUsize>,
            test_scope: Option<TestScope>,
            include: Option<Vec<String>>,
            exclude: Option<Vec<String>>,
            operators: Option<Vec<Operator>>,
            fail_on_missed: Option<bool>,
            min_score: Option<MinimumScore>,
            output_dir: Option<PathBuf>,
            $( $extra: Option<$ty>, )*
        }
        impl $name {
            fn settings(&self) -> Settings {
                Settings {
                    jobs: self.jobs.clone(),
                    timeout: self.timeout.clone(),
                    baseline_timeout: self.baseline_timeout.clone(),
                    max_log_bytes: self.max_log_bytes.clone(),
                    max_capture_bytes: self.max_capture_bytes.clone(),
                    test_scope: self.test_scope.clone(),
                    include: self.include.clone(),
                    exclude: self.exclude.clone(),
                    operators: self.operators.clone(),
                    fail_on_missed: self.fail_on_missed.clone(),
                    min_score: self.min_score.clone(),
                    output_dir: self.output_dir.clone(),
                }
            }
        }
    };
}
#[derive(Default)]
struct Settings {
    jobs: Option<NonZeroUsize>,
    timeout: Option<NonZeroU64>,
    baseline_timeout: Option<NonZeroU64>,
    max_log_bytes: Option<NonZeroUsize>,
    max_capture_bytes: Option<NonZeroUsize>,
    test_scope: Option<TestScope>,
    include: Option<Vec<String>>,
    exclude: Option<Vec<String>>,
    operators: Option<Vec<Operator>>,
    fail_on_missed: Option<bool>,
    min_score: Option<MinimumScore>,
    output_dir: Option<PathBuf>,
}
impl Settings {
    fn inherit(self, defaults: Self) -> Self {
        Self {
            jobs: self.jobs.or(defaults.jobs),
            timeout: self.timeout.or(defaults.timeout),
            baseline_timeout: self.baseline_timeout.or(defaults.baseline_timeout),
            max_log_bytes: self.max_log_bytes.or(defaults.max_log_bytes),
            max_capture_bytes: self.max_capture_bytes.or(defaults.max_capture_bytes),
            test_scope: self.test_scope.or(defaults.test_scope),
            include: self.include.or(defaults.include),
            exclude: self.exclude.or(defaults.exclude),
            operators: self.operators.or(defaults.operators),
            fail_on_missed: self.fail_on_missed.or(defaults.fail_on_missed),
            min_score: self.min_score.or(defaults.min_score),
            output_dir: self.output_dir.or(defaults.output_dir),
        }
    }
}
settings_table!(RepositoryConfig {
    go: GoConfig,
    rust: RustConfig,
    js: JsConfig,
    python: PythonConfig
});
settings_table!(GoConfig {
    enabled: bool,
    path: PathBuf
});
settings_table!(RustConfig { enabled: bool, path: PathBuf, packages: Vec<String>, build_cache: RustBuildCache });
settings_table!(JsConfig {
    enabled: bool,
    path: PathBuf,
    runner: JsRunner
});
settings_table!(PythonConfig {
    enabled: bool,
    path: PathBuf,
    executable: PathBuf,
    continue_on_error: NonZeroUsize
});

impl RepositoryConfig {
    fn languages(&self) -> Vec<Language> {
        [
            (Language::Go, self.go.as_ref().map(|v| v.enabled)),
            (Language::Rust, self.rust.as_ref().map(|v| v.enabled)),
            (Language::Js, self.js.as_ref().map(|v| v.enabled)),
            (Language::Python, self.python.as_ref().map(|v| v.enabled)),
        ]
        .into_iter()
        .filter_map(|(lang, enabled)| enabled.filter(|v| v.unwrap_or(true)).map(|_| lang))
        .collect()
    }
    fn has_languages(&self) -> bool {
        self.go.is_some() || self.rust.is_some() || self.js.is_some() || self.python.is_some()
    }
}

/// The resolved settings used by both execution and saved reports. Paths are
/// absolute, `auto` language selection is resolved, and `None` timeout means
/// the automatic policy, whose duration is determined after the baseline.
#[derive(Clone, Debug, Serialize)]
pub struct RunConfig {
    pub module_root: PathBuf,
    pub config_file: Option<PathBuf>,
    pub lang: Lang,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub python: Option<PathBuf>,
    pub rust_build_cache: RustBuildCache,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub rust_packages: Vec<String>,
    pub jobs: NonZeroUsize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub continue_on_error: Option<NonZeroUsize>,
    pub test_scope: TestScope,
    pub include: Vec<String>,
    pub exclude: Vec<String>,
    /// Empty means all operators; selection is applied after dismissal matching.
    pub operators: Vec<Operator>,
    pub timeout: Option<NonZeroU64>,
    pub max_log_bytes: Option<NonZeroUsize>,
    pub max_capture_bytes: Option<NonZeroUsize>,
    pub baseline_timeout: NonZeroU64,
    pub fail_on_missed: bool,
    pub min_score: Option<MinimumScore>,
    pub output_dir: PathBuf,
    pub verbose: bool,
    pub list: bool,
    #[serde(skip)]
    pub check: bool,
    #[serde(skip)]
    pub json: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rerun: Option<RerunRequest>,
    pub ui: UiMode,
}

impl RunConfig {
    pub fn resolve(cli: impl Into<Invocation>) -> Result<Self> {
        Self::resolve_from(
            cli.into(),
            &std::env::current_dir().context("reading working directory")?,
        )
    }

    pub(crate) fn resolve_from(cli: impl Into<Invocation>, invocation: &Path) -> Result<Self> {
        let cli = cli.into();
        let explicit_config = cli.project.config.is_some();
        let path = cli
            .project
            .config
            .as_ref()
            .map(|p| invocation.join(p))
            .unwrap_or_else(|| invocation.join("mutie.toml"));
        let (file, config_file, base) = match fs::read_to_string(&path) {
            Ok(source) => {
                let file = toml::from_str::<RepositoryConfig>(&source)
                    .with_context(|| format!("parsing configuration {}; use [go], [rust], [js] or [python] for project settings", path.display()))?;
                // Resolve the directory, not the file's symlink: paths belong to the
                // location from which this configuration was selected.
                let base = path
                    .parent()
                    .context("configuration has no parent")?
                    .canonicalize()?;
                let config_file =
                    base.join(path.file_name().context("configuration has no filename")?);
                (file, Some(config_file), base)
            }
            Err(error)
                if error.kind() == ErrorKind::NotFound
                    && !explicit_config
                    && fs::symlink_metadata(&path)
                        .is_err_and(|e| e.kind() == ErrorKind::NotFound) =>
            {
                (
                    RepositoryConfig::default(),
                    None,
                    invocation
                        .canonicalize()
                        .context("opening project directory")?,
                )
            }
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("reading configuration {}", path.display()));
            }
        };
        let saved = cli
            .rerun
            .as_ref()
            .map(|p| crate::report_reader::LoadedReport::load(&invocation.join(p)))
            .transpose()?;
        let languages = file.languages();
        let selected = if file.has_languages() {
            ensure!(
                !languages.is_empty(),
                "all configured languages are disabled"
            );
            if let Some(lang) = cli.project.lang {
                ensure!(
                    languages.contains(&lang),
                    "[{}] is missing or disabled",
                    lang.name()
                );
                lang
            } else {
                ensure!(
                    languages.len() == 1,
                    "multiple languages are enabled; select one with --lang {} (one project per invocation)",
                    languages
                        .iter()
                        .map(|v| v.name())
                        .collect::<Vec<_>>()
                        .join("|")
                );
                languages[0]
            }
        } else if let Some(lang) = cli.project.lang {
            lang
        } else if let Some(saved) = &saved {
            Language::of(saved.run.language())
        } else {
            Language::of(crate::init::infer(&base, None)?)
        };
        let (settings, project_path) = match selected {
            Language::Go => file.go.as_ref().map(|v| (v.settings(), v.path.clone())),
            Language::Rust => file.rust.as_ref().map(|v| (v.settings(), v.path.clone())),
            Language::Js => file.js.as_ref().map(|v| (v.settings(), v.path.clone())),
            Language::Python => file.python.as_ref().map(|v| (v.settings(), v.path.clone())),
        }
        .unwrap_or_default();
        let settings = settings.inherit(file.settings());
        let root = nonempty_path(&base, project_path.unwrap_or_else(|| ".".into()))?
            .canonicalize()
            .context("opening configured project path")?;
        let lang = match selected {
            Language::Go => Lang::Go,
            Language::Rust => Lang::Rust,
            Language::Python => Lang::Python,
            Language::Js => match file.js.as_ref().and_then(|v| v.runner) {
                Some(runner) => runner.lang(),
                None => saved
                    .as_ref()
                    .filter(|s| Language::of(s.run.language()) == Language::Js)
                    .map(|s| s.run.language())
                    .map(Ok)
                    .unwrap_or_else(|| crate::init::infer(&root, Some(Language::Js)))?,
            },
        };
        crate::language::resolve(&root, lang)?;
        let output_dir = resolve_output_path(&match settings.output_dir {
            Some(path) => nonempty_path(&base, path)?,
            None => root.join(".mutie"),
        })?;
        ensure!(
            !root.starts_with(&output_dir),
            "the report directory must not be the module root or one of its ancestors"
        );
        let mut operators = settings.operators.unwrap_or_default();
        operators.sort_unstable();
        operators.dedup();
        let rerun = cli
            .rerun
            .map(|path| {
                ensure!(
                    !path.as_os_str().is_empty(),
                    "saved run path must not be empty"
                );
                let source_run = invocation
                    .join(path)
                    .canonicalize()
                    .context("opening saved run directory")?;
                ensure!(
                    !root.starts_with(&source_run),
                    "the saved run directory must not be the module root or one of its ancestors"
                );
                ensure!(
                    !output_dir.starts_with(&source_run),
                    "the output directory must not be inside the saved run being rerun"
                );
                Ok(RerunRequest {
                    source_run,
                    filter: RerunFilter::new(
                        if cli.missed {
                            RerunMode::Missed
                        } else {
                            RerunMode::All
                        },
                        cli.ids.into_iter().map(NonZeroUsize::get).collect(),
                        operators.clone(),
                    ),
                })
            })
            .transpose()?;
        let mut rust_packages = if lang == Lang::Rust {
            file.rust
                .as_ref()
                .and_then(|v| v.packages.clone())
                .unwrap_or_else(|| {
                    saved
                        .as_ref()
                        .and_then(|s| s.run.rust_scope.as_ref())
                        .and_then(|s| s.workspace.as_ref())
                        .map(|s| s.selected.clone())
                        .unwrap_or_default()
                })
        } else {
            Vec::new()
        };
        rust_packages.sort();
        rust_packages.dedup();
        let python = if lang == Lang::Python {
            file.python
                .as_ref()
                .and_then(|v| v.executable.clone())
                .map(|p| nonempty_path(&base, p))
                .transpose()?
                .or_else(|| {
                    saved
                        .as_ref()
                        .and_then(|s| s.run.python_scope.as_ref())
                        .map(|s| s.executable().to_owned())
                })
        } else {
            None
        };
        ensure!(
            cli.metadata_only || lang != Lang::Python || python.is_some(),
            "Python requires [python].executable pointing to its pinned virtual-environment launcher"
        );
        let config = Self {
            module_root: root,
            config_file,
            lang,
            python,
            rust_build_cache: file
                .rust
                .as_ref()
                .and_then(|v| v.build_cache)
                .filter(|_| lang == Lang::Rust)
                .unwrap_or_default(),
            rust_packages,
            continue_on_error: file
                .python
                .as_ref()
                .and_then(|v| v.continue_on_error)
                .filter(|_| lang == Lang::Python),
            jobs: cli
                .jobs
                .or(settings.jobs)
                .unwrap_or(NonZeroUsize::new(1).unwrap()),
            timeout: cli.timeout.or(settings.timeout),
            baseline_timeout: settings
                .baseline_timeout
                .unwrap_or(NonZeroU64::new(300).unwrap()),
            max_log_bytes: settings.max_log_bytes,
            max_capture_bytes: settings.max_capture_bytes.or_else(|| {
                matches!(lang, Lang::Bun | Lang::Python)
                    .then(|| NonZeroUsize::new(4 * 1024 * 1024).unwrap())
            }),
            test_scope: settings.test_scope.unwrap_or_default(),
            include: settings.include.unwrap_or_default(),
            exclude: settings.exclude.unwrap_or_default(),
            operators,
            fail_on_missed: settings.fail_on_missed.unwrap_or(false),
            min_score: settings.min_score,
            output_dir,
            verbose: cli.presentation.verbose,
            ui: if cli.presentation.plain || cli.json {
                UiMode::Plain
            } else {
                UiMode::Auto
            },
            list: cli.list,
            check: cli.check,
            json: cli.json,
            rerun,
        };
        crate::mutation::Scope::new(&config.include, &config.exclude)?;
        if matches!(
            config.lang,
            Lang::Vitest | Lang::Node | Lang::Bun | Lang::Python
        ) {
            let runner = if config.lang == Lang::Python {
                "Python"
            } else if config.lang == Lang::Bun {
                "Bun"
            } else if config.lang == Lang::Node {
                "Node"
            } else {
                "Vitest"
            };
            ensure!(
                config.test_scope == TestScope::Full,
                "{runner} supports only full test scope"
            );
            ensure!(
                config.jobs.get() == 1,
                "{runner} currently supports only jobs = 1"
            );
            ensure!(
                config.operators.iter().all(|op| (*op == Operator::LoopSkip)
                    || (*op == Operator::CallRemove)
                    || (config.lang == Lang::Python && *op == Operator::AssignmentRemove)
                    || matches!(
                        op,
                        Operator::BooleanFlip
                            | Operator::ComparisonInvert
                            | Operator::ComparisonBoundary
                            | Operator::LogicalSwap
                            | Operator::NegationRemove
                            | Operator::ArithmeticReplace
                            | Operator::BranchForceTrue
                            | Operator::BranchForceFalse
                    )),
                "unsupported {runner} operator"
            );
        }
        ensure!(
            config.lang == Lang::Python || !config.operators.contains(&Operator::AssignmentRemove),
            "assignment_remove currently supports only Python attribute/item assignments"
        );
        if config.lang == Lang::Rust {
            ensure!(
                config.test_scope == TestScope::Full,
                "test_scope = affected is only supported for Go"
            );
            ensure!(
                config.operators.iter().all(|op| matches!(
                    op,
                    Operator::BooleanFlip
                        | Operator::ComparisonInvert
                        | Operator::NegationRemove
                        | Operator::ComparisonBoundary
                        | Operator::LogicalSwap
                        | Operator::ArithmeticReplace
                        | Operator::BranchForceTrue
                        | Operator::BranchForceFalse
                        | Operator::LoopSkip
                        | Operator::CallRemove
                )),
                "unsupported Rust operator"
            );
        }
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
            !self.include.is_empty()
                || !self.exclude.is_empty()
                || !self.operators.is_empty()
                || !self.rust_packages.is_empty(),
        )
    }
}

fn nonempty_path(base: &Path, path: PathBuf) -> Result<PathBuf> {
    ensure!(!path.as_os_str().is_empty(), "output_dir must not be empty");
    Ok(base.join(path))
}

/// Resolve existing symlinks and normalize missing path components without
/// creating report directories. Listing and invalid configurations have no
/// filesystem side effects. Walk in order so `link/..` respects the link target.
pub(crate) fn resolve_output_path(path: &Path) -> Result<PathBuf> {
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
    use crate::cli::{PresentationArgs, ProjectArgs};
    fn fixture(config: &str) -> tempfile::TempDir {
        let temp = tempfile::tempdir_in(std::env::temp_dir().canonicalize().unwrap()).unwrap();
        fs::write(temp.path().join("go.mod"), "module example.test\n").unwrap();
        fs::write(temp.path().join("mutie.toml"), config).unwrap();
        temp
    }
    fn invocation(lang: Option<Language>) -> Invocation {
        Invocation::project(ProjectArgs { lang, config: None })
    }
    fn resolve(root: &Path) -> Result<RunConfig> {
        RunConfig::resolve_from(invocation(None), root)
    }
    #[test]
    fn assignment_removal_is_python_only() -> Result<()> {
        for config in ["[go]\n", "[rust]\n", "[js]\nrunner='node'\n"] {
            let root = fixture(&(config.to_owned() + "operators=['assignment_remove']\n"));
            fs::write(
                root.path().join("Cargo.toml"),
                "[package]\nname='fixture'\nversion='0.1.0'\n",
            )?;
            fs::write(root.path().join("package.json"), "{}")?;
            let error = resolve(root.path()).unwrap_err().to_string();
            assert!(
                error.contains("assignment_remove") || error.contains("unsupported Node operator"),
                "{error}"
            );
        }
        let root =
            fixture("[python]\nexecutable='venv/bin/python'\noperators=['assignment_remove']\n");
        fs::write(
            root.path().join("pyproject.toml"),
            "[project]\nname='fixture'\n",
        )?;
        assert_eq!(
            resolve(root.path())?.operators,
            [Operator::AssignmentRemove]
        );
        Ok(())
    }
    #[test]
    fn defaults_language_settings_and_invocation_overrides_are_distinct() -> Result<()> {
        let root = fixture(
            "jobs=4\ntimeout=30\ninclude=['global/**']\n[go]\njobs=2\ninclude=['local/**']\noperators=['boolean_flip','boolean_flip']\n",
        );
        let config = resolve(root.path())?;
        assert_eq!(config.jobs.get(), 2);
        assert_eq!(config.timeout.unwrap().get(), 30);
        assert_eq!(config.include, ["local/**"]);
        assert_eq!(config.operators, [Operator::BooleanFlip]);
        assert_eq!(config.score_scope(), ScoreScope::Filtered);
        let mut cli = invocation(None);
        cli.jobs = NonZeroUsize::new(3);
        cli.timeout = NonZeroU64::new(7);
        cli.presentation = PresentationArgs {
            plain: true,
            json: false,
            verbose: true,
        };
        let config = RunConfig::resolve_from(cli, root.path())?;
        assert_eq!(config.jobs.get(), 3);
        assert_eq!(config.timeout.unwrap().get(), 7);
        assert_eq!(config.ui, UiMode::Plain);
        assert!(config.verbose);
        assert!(!root.path().join(".mutie").exists());
        Ok(())
    }
    #[test]
    fn section_presence_enabled_switch_and_explicit_selection() -> Result<()> {
        let root = fixture("[go]\n[rust]\npath='missing'\n");
        assert!(
            resolve(root.path())
                .unwrap_err()
                .to_string()
                .contains("multiple languages")
        );
        assert_eq!(
            RunConfig::resolve_from(invocation(Some(Language::Go)), root.path())?.lang,
            Lang::Go
        );
        assert!(RunConfig::resolve_from(invocation(Some(Language::Js)), root.path()).is_err());
        fs::write(
            root.path().join("mutie.toml"),
            "[go]\n[rust]\nenabled=false\npath='missing'\n",
        )?;
        assert_eq!(resolve(root.path())?.lang, Lang::Go);
        assert!(RunConfig::resolve_from(invocation(Some(Language::Rust)), root.path()).is_err());
        fs::write(root.path().join("mutie.toml"), "[go]\nenabled=false\n")?;
        assert!(
            resolve(root.path())
                .unwrap_err()
                .to_string()
                .contains("disabled")
        );
        Ok(())
    }
    #[test]
    fn alternate_config_anchors_project_output_and_python_paths() -> Result<()> {
        let root = fixture("");
        fs::create_dir(root.path().join("config"))?;
        fs::create_dir(root.path().join("backend"))?;
        fs::write(root.path().join("backend/go.mod"), "module example.test\n")?;
        let path = root.path().join("config/ci.toml");
        fs::write(&path, "output_dir='../reports'\n[go]\npath='../backend'\n")?;
        let mut cli = invocation(None);
        cli.project.config = Some(path.clone());
        let config = RunConfig::resolve_from(cli, root.path())?;
        assert_eq!(config.module_root, root.path().join("backend"));
        assert_eq!(config.output_dir, root.path().join("reports"));
        assert_eq!(config.config_file, Some(path.clone()));
        fs::write(root.path().join("backend/pyproject.toml"), "")?;
        fs::write(
            &path,
            "[python]\npath='../backend'\nexecutable='../venv/bin/python'\n",
        )?;
        let mut cli = invocation(None);
        cli.project.config = Some(path);
        let config = RunConfig::resolve_from(cli, root.path())?;
        // Preserve the venv launcher rather than following its symlink.
        assert_eq!(
            config.python,
            Some(root.path().join("config/../venv/bin/python"))
        );
        Ok(())
    }
    #[test]
    fn strict_schemas_reject_old_ui_and_foreign_runner_settings() {
        for source in [
            "lang='go'",
            "python='/python'",
            "rust_packages=[]",
            "ui='plain'",
            "verbose=true",
            "[go]\nui='plain'",
            "[go]\nbuild_cache='worker'",
            "[js]\nrunner='jest'",
            "[js]\nexecutable='node'",
            "[go]\n[rust]\nenabled=false\ntypo=true",
            "jobs=0",
            "timeout=0",
            "jobs='4'",
            "[go]\noperators=['unknown']",
            "[go]\ninclude=['[']",
            "[go]\npath=''",
            "output_dir=''",
            "output_dir='.'",
        ] {
            let root = fixture(source);
            assert!(resolve(root.path()).is_err(), "{source}");
            assert!(!root.path().join(".mutie").exists());
        }
    }
    #[test]
    fn explicit_missing_config_fails_and_parent_configs_are_not_searched() -> Result<()> {
        let root = fixture("invalid = true");
        fs::create_dir(root.path().join("child"))?;
        fs::write(root.path().join("child/go.mod"), "module example.test\n")?;
        assert_eq!(resolve(&root.path().join("child"))?.lang, Lang::Go);
        let mut cli = invocation(None);
        cli.project.config = Some(root.path().join("absent"));
        assert!(RunConfig::resolve_from(cli, root.path()).is_err());
        Ok(())
    }
    #[cfg(unix)]
    #[test]
    fn dangling_config_symlinks_are_errors_not_missing_defaults() -> Result<()> {
        let root = fixture("");
        fs::remove_file(root.path().join("mutie.toml"))?;
        std::os::unix::fs::symlink("absent.toml", root.path().join("mutie.toml"))?;
        assert!(resolve(root.path()).is_err());
        Ok(())
    }

    #[test]
    fn js_runner_and_serial_limits_are_separate_from_language() -> Result<()> {
        let root = fixture("[js]\nrunner='bun'\n");
        fs::write(
            root.path().join("package.json"),
            r#"{"scripts":{"test":"node --test"}}"#,
        )?;
        assert_eq!(resolve(root.path())?.lang, Lang::Bun);
        fs::write(
            root.path().join("mutie.toml"),
            "jobs=4\n[js]\njobs=1\nrunner='node'\n",
        )?;
        assert_eq!(resolve(root.path())?.lang, Lang::Node);
        fs::write(
            root.path().join("mutie.toml"),
            "jobs=4\n[js]\nrunner='vitest'\n",
        )?;
        assert!(
            resolve(root.path())
                .unwrap_err()
                .to_string()
                .contains("jobs = 1")
        );
        fs::write(root.path().join("mutie.toml"), "[js]\n")?;
        assert_eq!(resolve(root.path())?.lang, Lang::Node);
        Ok(())
    }
    #[test]
    fn rust_and_python_settings_stay_in_their_own_sections() -> Result<()> {
        let root = fixture(
            "[rust]\npackages=['core','core']\nbuild_cache='worker'\n[python]\nenabled=false\nexecutable='venv/bin/python'\ncontinue_on_error=2\n",
        );
        fs::write(root.path().join("Cargo.toml"), "[workspace]\nmembers=[]\n")?;
        let config = resolve(root.path())?;
        assert_eq!(config.rust_packages, ["core"]);
        assert_eq!(config.rust_build_cache, RustBuildCache::Worker);
        assert!(config.python.is_none());
        assert!(config.continue_on_error.is_none());
        fs::write(
            root.path().join("mutie.toml"),
            "[rust]\noperators=['call_remove']\n",
        )?;
        assert_eq!(resolve(root.path())?.operators, [Operator::CallRemove]);
        fs::write(
            root.path().join("mutie.toml"),
            "[rust]\ntest_scope='affected'\n",
        )?;
        assert!(resolve(root.path()).is_err());
        fs::write(root.path().join("mutie.toml"), "[python]\n")?;
        fs::write(root.path().join("pyproject.toml"), "")?;
        assert!(
            resolve(root.path())
                .unwrap_err()
                .to_string()
                .contains("executable")
        );
        Ok(())
    }
    #[test]
    fn empty_language_selections_replace_global_defaults() -> Result<()> {
        let root = fixture(
            "include=['src/**']\nexclude=['generated/**']\noperators=['boolean_flip']\n[go]\ninclude=[]\nexclude=[]\noperators=[]\n",
        );
        let config = resolve(root.path())?;
        assert!(
            config.include.is_empty() && config.exclude.is_empty() && config.operators.is_empty()
        );
        assert_eq!(config.score_scope(), ScoreScope::AllDiscovered);
        Ok(())
    }
    #[test]
    fn automatic_detection_rejects_ambiguity() -> Result<()> {
        let root = fixture("");
        fs::write(root.path().join("Cargo.toml"), "")?;
        assert!(
            resolve(root.path())
                .unwrap_err()
                .to_string()
                .contains("ambiguous")
        );
        assert_eq!(
            RunConfig::resolve_from(invocation(Some(Language::Rust)), root.path())?.lang,
            Lang::Rust
        );
        Ok(())
    }
    #[cfg(unix)]
    #[test]
    fn output_symlinks_cannot_point_to_the_project_or_its_ancestor() -> Result<()> {
        let root = fixture("output_dir='alias'\n[go]\n");
        std::os::unix::fs::symlink(root.path(), root.path().join("alias"))?;
        assert!(resolve(root.path()).is_err());
        Ok(())
    }
}
