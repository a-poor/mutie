//! Static onboarding only: generated settings do not certify adapter compatibility.
use std::{
    fs,
    io::{self, IsTerminal, Write},
    path::{Path, PathBuf},
};

use crate::cli::Lang;
use anyhow::{Context, Result, bail, ensure};

fn label(lang: Lang) -> &'static str {
    match lang {
        Lang::Auto => "auto",
        Lang::Go => "go",
        Lang::Rust => "rust",
        Lang::Python => "python",
        Lang::Vitest => "vitest",
        Lang::Node => "node",
        Lang::Bun => "bun",
    }
}

fn detect(root: &Path) -> Result<Vec<Lang>> {
    let mut choices = Vec::new();
    for (file, lang) in [
        ("go.mod", Lang::Go),
        ("Cargo.toml", Lang::Rust),
        ("pyproject.toml", Lang::Python),
    ] {
        if root.join(file).is_file() {
            choices.push(lang);
        }
    }
    let package = root.join("package.json");
    if package.is_file() {
        let value: serde_json::Value = serde_json::from_str(&fs::read_to_string(&package)?)
            .context("reading package.json for tooling detection")?;
        let script = value["scripts"]["test"].as_str().unwrap_or("");
        reject_vite_plus_test(&value)?;
        // Recognize exact command prefixes, never execute scripts or infer Bun from a lockfile.
        if script == "node --test" || script.starts_with("node --test ") {
            choices.push(Lang::Node);
        }
        if script == "bun test" || script.starts_with("bun test ") {
            choices.push(Lang::Bun);
        }
        if script == "vitest"
            || script.starts_with("vitest ")
            || value["devDependencies"].get("vitest").is_some()
            || value["dependencies"].get("vitest").is_some()
        {
            choices.push(Lang::Vitest);
        }
    }
    Ok(choices)
}

fn choose(choices: &[Lang], interactive: bool) -> Result<Lang> {
    if let [lang] = choices {
        return Ok(*lang);
    }
    ensure!(
        !choices.is_empty(),
        "could not infer a supported test runner; choose --lang go|rust|python|vitest|node|bun at its project root (package managers do not identify test runners)"
    );
    let names = choices
        .iter()
        .map(|lang| label(*lang))
        .collect::<Vec<_>>()
        .join(", ");
    ensure!(
        interactive,
        "ambiguous project tooling ({names}); pass --lang explicitly"
    );
    eprintln!("Choose execution adapter: {names}");
    eprint!("Adapter: ");
    io::stderr().flush()?;
    let mut answer = String::new();
    io::stdin().read_line(&mut answer)?;
    choices
        .iter()
        .copied()
        .find(|lang| label(*lang) == answer.trim())
        .context("no matching adapter selected; rerun with --lang")
}

fn reject_vite_plus_test(package: &serde_json::Value) -> Result<()> {
    let script = package["scripts"]["test"].as_str().unwrap_or("");
    ensure!(
        script != "vp test" && !script.starts_with("vp test "),
        "Vite+ vp test uses bundled runner APIs that need a separate attribution audit; package-manager delegation is distinct from test-runner support"
    );
    Ok(())
}

fn tooling_notes(root: &Path, lang: Lang) -> Result<()> {
    let runtime = match lang {
        Lang::Python => "CPython 3.14.7 / pytest 9.0.2 in an explicitly supplied venv",
        Lang::Node => "Node 24.22.0",
        Lang::Bun => "Bun 1.3.11+af24e281e",
        Lang::Vitest => "Node 24.22.0 / Vitest 4.1.9 / an audited package-manager installation",
        Lang::Rust => {
            "host Cargo toolchain; ordinary single package (workspaces not supported yet)"
        }
        _ => "Go 1.24+",
    };
    eprintln!("Required execution environment: {runtime}");
    if matches!(lang, Lang::Node | Lang::Bun | Lang::Vitest) {
        let package: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(root.join("package.json"))?)
                .context("reading package.json tooling metadata")?;
        let manager = package["packageManager"].as_str();
        let locks: Vec<_> = [
            "package-lock.json",
            "npm-shrinkwrap.json",
            "pnpm-lock.yaml",
            "yarn.lock",
            "bun.lock",
            "bun.lockb",
        ]
        .into_iter()
        .filter(|name| root.join(name).exists())
        .collect();
        eprintln!(
            "Package manager declaration: {}; lockfiles: {}",
            manager.unwrap_or("unspecified"),
            locks.join(", ")
        );
        eprintln!(
            "Package-manager metadata does not select or certify a test runner. No install will be performed."
        );
        if lang == Lang::Vitest {
            reject_vite_plus_test(&package)?;
            ensure!(
                manager
                    .is_some_and(|name| crate::vitest::project::PACKAGE_MANAGERS.contains(&name)),
                "unsupported packageManager; supported installation selectors: {}; other versions/layouts are not implemented; project files were not changed",
                crate::vitest::project::PACKAGE_MANAGERS.join(", ")
            );
            eprintln!(
                "pnpm requires its isolated layout; npm, Yarn and Bun require the audited hoisted layout. Yarn PnP/workspaces are not supported. Full installation validation happens on a run."
            );
        }
        if matches!(lang, Lang::Node | Lang::Bun) {
            eprintln!(
                "Runtime dependency imports remain unsupported. Declared development installations may be retained as owned, fingerprinted artifacts; PnP loaders and external dependency links are rejected. Full scope validation happens on a run."
            );
        }
    }
    Ok(())
}

fn render(lang: Lang, python: Option<&Path>) -> Result<String> {
    let mut table = toml::Table::new();
    table.insert("lang".into(), label(lang).into());
    if let Some(python) = python {
        table.insert(
            "python".into(),
            python
                .to_str()
                .context("Python launcher path must be UTF-8")?
                .into(),
        );
    }
    let mut text = String::from(
        "# Generated setup; project/runtime compatibility is checked when Mutie runs.\n# Precedence: defaults < this file < explicit CLI options.\n",
    );
    text.push_str(&toml::to_string(&table)?);
    text.push_str("\n# Source filters select mutations, not tests.\n# include = [\"src/**\"]\n# exclude = []\n# operators = [] # all operators supported by this adapter\n# jobs = 1\n# baseline_timeout = 300\n# ui = \"auto\"\n");
    Ok(text)
}

pub fn run(
    root: &Path,
    requested: Option<Lang>,
    python: Option<PathBuf>,
    non_interactive: bool,
    dry_run: bool,
) -> Result<()> {
    let root = root.canonicalize().context("opening project root")?;
    ensure!(root.is_dir(), "project root must be a directory");
    let destination = root.join("mutie.toml");
    if !dry_run && fs::symlink_metadata(&destination).is_ok() {
        bail!(
            "{} already exists; use --dry-run to preview settings and merge them manually",
            destination.display()
        );
    }
    let interactive = !non_interactive && io::stdin().is_terminal() && io::stderr().is_terminal();
    let lang = match requested {
        Some(lang) if lang != Lang::Auto => lang,
        _ => choose(&detect(&root)?, interactive)?,
    };
    crate::language::resolve(&root, lang)?;
    tooling_notes(&root, lang)?;
    ensure!(
        lang == Lang::Python || python.is_none(),
        "--python requires the Python adapter"
    );
    let python = if lang == Lang::Python {
        let input = match python {
            Some(path) => path,
            None if interactive => {
                eprint!("Pinned Python virtual-environment launcher: ");
                io::stderr().flush()?;
                let mut answer = String::new();
                io::stdin().read_line(&mut answer)?;
                ensure!(
                    !answer.trim().is_empty(),
                    "Python requires --python /path/to/venv/bin/python"
                );
                PathBuf::from(answer.trim())
            }
            None => bail!(
                "Python requires --python /path/to/pinned-venv/bin/python; init does not install environments"
            ),
        };
        let input = std::env::current_dir()?.join(input);
        ensure!(
            input.is_file(),
            "Python launcher does not exist: {}",
            input.display()
        );
        // Do not canonicalize the launcher: venv symlinks must retain their environment.
        Some(input)
    } else {
        None
    };
    let text = render(lang, python.as_deref())?;
    if dry_run {
        print!("{text}");
    } else {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&destination)
            .with_context(|| {
                format!(
                    "creating {} without overwriting existing files",
                    destination.display()
                )
            })?;
        file.write_all(text.as_bytes())
            .context("writing configuration")?;
        eprintln!("Created {}", destination.display());
    }
    eprintln!(
        "Selected adapter: {}. No tests, installs or compatibility validation were performed.",
        label(lang)
    );
    eprintln!(
        "Next: run mutie with this project path and --list, then a baseline/run. Unsupported project settings and runtime versions will be reported by the adapter."
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn lockfile_does_not_choose_runner_and_multiple_runners_are_ambiguous() -> Result<()> {
        let root = tempfile::tempdir()?;
        fs::write(root.path().join("bun.lock"), "")?;
        fs::write(
            root.path().join("package.json"),
            r#"{"scripts":{"test":"node --test"},"devDependencies":{"vitest":"4.1.9"}}"#,
        )?;
        assert_eq!(detect(root.path())?, vec![Lang::Node, Lang::Vitest]);
        assert!(choose(&detect(root.path())?, false).is_err());
        Ok(())
    }
    #[test]
    fn refuses_existing_config_and_dry_run_does_not_write() -> Result<()> {
        let root = tempfile::tempdir()?;
        fs::write(root.path().join("go.mod"), "module example.test\n")?;
        run(root.path(), None, None, true, true)?;
        assert!(!root.path().join("mutie.toml").exists());
        run(root.path(), None, None, true, false)?;
        let before = fs::read(root.path().join("mutie.toml"))?;
        assert!(run(root.path(), None, None, true, false).is_err());
        assert_eq!(fs::read(root.path().join("mutie.toml"))?, before);
        Ok(())
    }
    #[test]
    fn generated_config_uses_existing_resolution() -> Result<()> {
        use clap::Parser;
        for (lang, manifest) in [
            (Lang::Go, "go.mod"),
            (Lang::Rust, "Cargo.toml"),
            (Lang::Node, "package.json"),
            (Lang::Bun, "package.json"),
            (Lang::Vitest, "package.json"),
        ] {
            let root = tempfile::tempdir()?;
            fs::write(root.path().join(manifest), "")?;
            fs::write(root.path().join("mutie.toml"), render(lang, None)?)?;
            let cli = crate::cli::Cli::try_parse_from([
                "mutie",
                root.path().to_str().unwrap(),
                "--jobs",
                "1",
            ])?;
            assert_eq!(crate::config::RunConfig::resolve(cli)?.lang, lang);
        }
        Ok(())
    }
}
