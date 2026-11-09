//! Adapter regression fixtures describe execution settings as option/value pairs.
//! Materialize those settings in an external config and invoke the public CLI.
//! This keeps source fixtures unchanged across focused reruns. CLI/config contract
//! tests use std::process::Command directly and do not pass through this helper.
#![allow(dead_code)]
use std::{
    ffi::{OsStr, OsString},
    fs, io,
    ops::{Deref, DerefMut},
    path::Path,
    process::{self, Output, Stdio},
};

#[derive(Debug)]
pub struct Command {
    inner: process::Command,
    args: Vec<OsString>,
    config: Option<tempfile::NamedTempFile>,
    prepared: bool,
}
impl Command {
    pub fn new(program: impl AsRef<OsStr>) -> Self {
        Self {
            inner: process::Command::new(program),
            args: vec![],
            config: None,
            prepared: false,
        }
    }
    pub fn arg(&mut self, arg: impl AsRef<OsStr>) -> &mut Self {
        self.args.push(arg.as_ref().to_owned());
        self
    }
    pub fn args<I, S>(&mut self, args: I) -> &mut Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        for arg in args {
            self.arg(arg);
        }
        self
    }
    pub fn current_dir(&mut self, dir: impl AsRef<Path>) -> &mut Self {
        self.inner.current_dir(dir);
        self
    }
    pub fn env(&mut self, key: impl AsRef<OsStr>, value: impl AsRef<OsStr>) -> &mut Self {
        self.inner.env(key, value);
        self
    }
    pub fn envs<I, K, V>(&mut self, vars: I) -> &mut Self
    where
        I: IntoIterator<Item = (K, V)>,
        K: AsRef<OsStr>,
        V: AsRef<OsStr>,
    {
        self.inner.envs(vars);
        self
    }
    pub fn env_remove(&mut self, key: impl AsRef<OsStr>) -> &mut Self {
        self.inner.env_remove(key);
        self
    }
    pub fn env_clear(&mut self) -> &mut Self {
        self.inner.env_clear();
        self
    }
    pub fn stdin(&mut self, value: impl Into<Stdio>) -> &mut Self {
        self.inner.stdin(value);
        self
    }
    pub fn stdout(&mut self, value: impl Into<Stdio>) -> &mut Self {
        self.inner.stdout(value);
        self
    }
    pub fn stderr(&mut self, value: impl Into<Stdio>) -> &mut Self {
        self.inner.stderr(value);
        self
    }
    #[cfg(unix)]
    pub fn process_group(&mut self, group: i32) -> &mut Self {
        use std::os::unix::process::CommandExt;
        self.inner.process_group(group);
        self
    }
    #[cfg(unix)]
    pub unsafe fn pre_exec<F>(&mut self, f: F) -> &mut Self
    where
        F: FnMut() -> io::Result<()> + Send + Sync + 'static,
    {
        use std::os::unix::process::CommandExt;
        unsafe {
            self.inner.pre_exec(f);
        }
        self
    }
    pub fn output(&mut self) -> io::Result<Output> {
        self.prepare()?;
        self.inner.output()
    }
    pub fn status(&mut self) -> io::Result<process::ExitStatus> {
        self.prepare()?;
        self.inner.status()
    }
    pub fn spawn(&mut self) -> io::Result<Child> {
        self.prepare()?;
        Ok(Child {
            inner: self.inner.spawn()?,
            _config: self.config.take(),
        })
    }
    fn prepare(&mut self) -> io::Result<()> {
        if self.prepared {
            return Ok(());
        }
        self.prepared = true;
        if Path::new(self.inner.get_program()).file_name()
            != Path::new(env!("CARGO_BIN_EXE_mutie")).file_name()
        {
            self.inner.args(&self.args);
            return Ok(());
        }
        let args: Vec<_> = self
            .args
            .iter()
            .map(|v| v.to_string_lossy().into_owned())
            .collect();
        let first = args.first().map(String::as_str).unwrap_or("");
        if [
            "init",
            "report",
            "runs",
            "prune",
            "help",
            "--help",
            "--version",
        ]
        .contains(&first)
        {
            self.inner.args(&self.args);
            return Ok(());
        }
        let cwd = self
            .inner
            .get_current_dir()
            .map(Path::to_owned)
            .unwrap_or(std::env::current_dir()?);
        let mut root = cwd.clone();
        let mut action = "run";
        let mut lang = None;
        let mut json = false;
        let mut listing = false;
        let mut rerun = None;
        let mut missed = false;
        let mut overrides = Vec::<(String, Option<String>)>::new();
        let mut passed = Vec::<String>::new();
        let mut dismiss = Vec::<String>::new();
        let mut i = 0;
        if first == "dismiss" {
            action = "dismiss";
            i = 1;
        }
        for (prefix, runner) in [("python", "python"), ("bun", "bun"), ("vitest", "vitest")] {
            if first == format!("{prefix}-list") {
                action = "list";
                lang = Some(runner.to_owned());
                i = 1;
            }
            if first == format!("{prefix}-check") {
                action = "check";
                lang = Some(runner.to_owned());
                i = 1;
            }
        }
        while i < args.len() {
            let arg = &args[i];
            i += 1;
            match arg.as_str() {
                "--reason" | "--note" => {
                    dismiss.extend([arg.clone(), args[i].clone()]);
                    i += 1;
                }
                "--clear-note" => dismiss.push(arg.clone()),
                "--lang" => {
                    lang = Some(args[i].clone());
                    i += 1;
                }
                "--module" => {
                    root = cwd.join(&args[i]);
                    i += 1;
                }
                "--list" => listing = true,
                "--json" => json = true,
                "--rerun" | "--rerun-missed" => {
                    rerun = Some(cwd.join(&args[i]));
                    missed = arg == "--rerun-missed";
                    i += 1;
                }
                "--ui" => {
                    if args[i] == "plain" {
                        passed.push("--plain".into());
                    }
                    i += 1;
                }
                "--id" | "--timeout" => {
                    if action == "check" && arg == "--timeout" {
                        overrides.push(("baseline_timeout".into(), Some(args[i].clone())));
                    } else {
                        passed.extend([arg.clone(), args[i].clone()]);
                    }
                    i += 1;
                }
                "--jobs" | "-j" => {
                    overrides.push(("jobs".into(), Some(args[i].clone())));
                    i += 1;
                }
                "--verbose" | "-v" => passed.push("--verbose".into()),
                "--no-verbose" => passed.retain(|s| s != "--verbose"),
                "--fail-on-missed" => {
                    overrides.push(("fail_on_missed".into(), Some("true".into())))
                }
                "--no-fail-on-missed" => {
                    overrides.push(("fail_on_missed".into(), Some("false".into())))
                }
                value if value.starts_with("--clear-") => {
                    overrides.push((value.trim_start_matches("--clear-").replace('-', "_"), None))
                }
                "--operator"
                | "--include"
                | "--exclude"
                | "--rust-package"
                | "--rust-build-cache"
                | "--python"
                | "--continue-on-error"
                | "--test-scope"
                | "--baseline-timeout"
                | "--max-log-bytes"
                | "--max-capture-bytes"
                | "--min-score"
                | "--output-dir" => {
                    let key = arg.trim_start_matches("--").replace('-', "_");
                    let value = if ["--python", "--output-dir"].contains(&arg.as_str()) {
                        cwd.join(&args[i]).to_string_lossy().into_owned()
                    } else {
                        args[i].clone()
                    };
                    overrides.push((key, Some(value)));
                    i += 1;
                }
                value if action == "dismiss" => dismiss.push(value.to_owned()),
                value if !value.starts_with('-') => root = cwd.join(value),
                _ => passed.push(arg.clone()),
            }
        }
        if listing && rerun.is_none() {
            action = "list";
        }
        let file = root.join("mutie.toml");
        let source = fs::read_to_string(&file).ok();
        let mut table: toml::Table = source
            .as_deref()
            .and_then(|s| toml::from_str(s).ok())
            .unwrap_or_default();
        // Existing adapter fixtures predate the grouped config; their on-disk
        // bytes stay unchanged while their execution settings are materialized.
        if let Some(value) = table
            .remove("lang")
            .and_then(|v| v.as_str().map(str::to_owned))
            && lang.is_none()
        {
            lang = Some(value);
        }
        let lang = lang.unwrap_or_else(|| {
            if root.join("Cargo.toml").exists() {
                "rust".into()
            } else {
                "go".into()
            }
        });
        let section = if ["vitest", "node", "bun"].contains(&lang.as_str()) {
            "js"
        } else {
            &lang
        };
        let mut settings = table
            .remove(section)
            .and_then(|v| v.as_table().cloned())
            .unwrap_or_default();
        if section == "js" {
            settings.insert("runner".into(), lang.clone().into());
        }
        for (old, new) in [
            ("rust_packages", "packages"),
            ("rust_build_cache", "build_cache"),
            ("python", "executable"),
        ] {
            if let Some(value) = table.remove(old) {
                settings.insert(new.into(), value);
            }
        }
        // Preserve old fixture-relative paths when the temporary config is elsewhere.
        if let Some(value) = table
            .get("output_dir")
            .and_then(|v| v.as_str())
            .map(str::to_owned)
        {
            table.insert(
                "output_dir".into(),
                root.join(value).to_string_lossy().into_owned().into(),
            );
        }
        if let Some(value) = settings
            .get("executable")
            .and_then(|v| v.as_str())
            .map(str::to_owned)
        {
            settings.insert(
                "executable".into(),
                root.join(value).to_string_lossy().into_owned().into(),
            );
        }
        let mut lists = std::collections::BTreeSet::new();
        for (key, value) in overrides {
            let key = match key.as_str() {
                "operator" => "operators",
                "rust_package" | "rust_packages" => "packages",
                "rust_build_cache" => "build_cache",
                "python" => "executable",
                key => key,
            }
            .to_owned();
            table.remove(&key);
            match value {
                None => {
                    settings.remove(&key);
                    if ["include", "exclude", "operators", "packages"].contains(&key.as_str()) {
                        settings.insert(key, Vec::<toml::Value>::new().into());
                    }
                }
                Some(value) => {
                    let value = if ["include", "exclude", "operators", "packages"]
                        .contains(&key.as_str())
                    {
                        let mut values = if lists.insert(key.clone()) {
                            vec![]
                        } else {
                            settings
                                .remove(&key)
                                .and_then(|v| v.as_array().cloned())
                                .unwrap_or_default()
                        };
                        if key == "operators" {
                            values.extend(value.split(',').map(|v| v.to_owned().into()));
                        } else {
                            values.push(value.into());
                        }
                        toml::Value::Array(values)
                    } else if [
                        "jobs",
                        "timeout",
                        "baseline_timeout",
                        "max_log_bytes",
                        "max_capture_bytes",
                        "continue_on_error",
                    ]
                    .contains(&key.as_str())
                    {
                        value
                            .parse::<i64>()
                            .map(toml::Value::Integer)
                            .unwrap_or_else(|_| value.into())
                    } else if key == "min_score" {
                        value.parse::<f64>().unwrap().into()
                    } else if key == "fail_on_missed" {
                        (value == "true").into()
                    } else {
                        value.into()
                    };
                    settings.insert(key, value);
                }
            }
        }
        settings.insert("path".into(), root.to_string_lossy().into_owned().into());
        table.insert(section.into(), toml::Value::Table(settings));
        if source
            .as_ref()
            .is_some_and(|s| toml::from_str::<toml::Table>(s).is_err())
        {
            // Malformed-config controls must still reach the real parser.
            self.inner.arg("--path").arg(&root).arg(action);
            if action == "dismiss" {
                self.inner.args(dismiss);
            }
            self.inner.args(passed);
            return Ok(());
        }
        let config = tempfile::Builder::new()
            .prefix("mutie-fixture-")
            .suffix(".toml")
            .tempfile()?;
        fs::write(config.path(), toml::to_string(&table).unwrap())?;
        if let Some(rerun) = rerun {
            self.inner.arg("rerun").arg(rerun);
            if missed {
                self.inner.arg("--missed");
            }
            if listing {
                self.inner.arg("--dry-run");
            }
        } else {
            self.inner.arg(action);
        }
        self.inner.arg("--config").arg(config.path());
        if action == "dismiss" {
            self.inner.args(dismiss);
        }
        if json {
            self.inner.arg("--json");
        }
        if action == "list" || action == "check" {
            let mut j = 0;
            while j < passed.len() {
                if ["--jobs", "-j", "--timeout"].contains(&passed[j].as_str()) {
                    j += 2;
                } else if ["--verbose", "--plain"].contains(&passed[j].as_str()) {
                    j += 1;
                } else {
                    self.inner.arg(&passed[j]);
                    j += 1;
                }
            }
        } else {
            self.inner.args(passed);
        }
        self.config = Some(config);
        Ok(())
    }
}
#[derive(Debug)]
pub struct Child {
    inner: process::Child,
    _config: Option<tempfile::NamedTempFile>,
}
impl Deref for Child {
    type Target = process::Child;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}
impl DerefMut for Child {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}
impl Child {
    pub fn wait_with_output(self) -> io::Result<Output> {
        self.inner.wait_with_output()
    }
}
