#[path = "installation.rs"]
mod installation;
#[cfg(test)]
use installation::json_trailing_commas;
use installation::{
    installation_fingerprint, validate_bun_installation, validate_npm_installation,
    validate_pnpm_installation, validate_yarn_installation,
};

use crate::cancellation::Cancellation;
use anyhow::{Context, Result, bail, ensure};
use serde_json::Value as Json;
use serde_yaml_ng::Value as Yaml;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

pub const PACKAGE_MANAGERS: &[&str] = &[
    "pnpm@11.10.0",
    "pnpm@10.33.2",
    "npm@11.19.1",
    "yarn@4.18.1",
    "bun@1.3.11",
];

pub(super) fn supported_runner(vitest: &str, vite: &str) -> bool {
    matches!(
        (vitest, vite),
        ("4.1.9", "8.1.3" | "8.3.4") | ("4.1.5", "8.0.10")
    )
}

#[derive(Debug)]
pub struct Project {
    pub root: PathBuf,
    pub project_config_fingerprints: BTreeMap<String, String>,
    pub name: String,
    pub package_manager: String,
    pub installation_fingerprint: String,
    pub vitest: String,
    pub vite: String,
    pub manifest_fingerprint: String,
    pub lockfile_fingerprint: String,
    pub recipe: super::recipe::Recipe,
}

fn read(path: &Path) -> Result<String> {
    ensure!(
        fs::symlink_metadata(path)
            .with_context(|| format!("reading {}", path.display()))?
            .file_type()
            .is_file(),
        "{} must be a regular file",
        path.display()
    );
    fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))
}
fn json(path: &Path) -> Result<Json> {
    serde_json::from_str(&read(path)?).with_context(|| format!("parsing {}", path.display()))
}
fn yaml(path: &Path) -> Result<Yaml> {
    serde_yaml_ng::from_str(&read(path)?).with_context(|| format!("parsing {}", path.display()))
}
fn fingerprint(text: &str) -> String {
    format!("sha256:{:x}", Sha256::digest(text.as_bytes()))
}

impl Project {
    pub fn load(root: &Path, cancellation: &Cancellation) -> Result<Self> {
        cancellation.check()?;
        let root = root.canonicalize().context("resolving Vitest project")?;
        let manifest_text = read(&root.join("package.json"))?;
        let manifest: Json =
            serde_json::from_str(&manifest_text).context("parsing package.json")?;
        if manifest.get("type").is_none() {
            super::recipe::validate_implicit_typescript(&root, cancellation)?;
        } else {
            ensure!(
                manifest["type"] == "module",
                "Vitest requires ESM or an audited implicit TypeScript package"
            );
        }
        let name = manifest["name"]
            .as_str()
            .filter(|s| !s.trim().is_empty())
            .context("package.json requires a name")?
            .to_owned();
        let package_manager = manifest["packageManager"]
            .as_str()
            .context("Vitest requires an explicit packageManager")?
            .to_owned();
        ensure!(
            PACKAGE_MANAGERS.contains(&package_manager.as_str()),
            "unsupported Vitest package manager: {package_manager}"
        );
        for key in [
            "workspaces",
            "pnpm",
            "vitest",
            "resolutions",
            "overrides",
            "imports",
            "peerDependencies",
            "bundledDependencies",
            "bundleDependencies",
        ] {
            ensure!(
                manifest.get(key).is_none(),
                "unsupported package.json setting: {key}"
            );
        }
        for parent in root.ancestors().skip(1) {
            ensure!(
                !parent.join("pnpm-workspace.yaml").exists(),
                "enclosing pnpm workspace is unsupported"
            );
            if parent.join("package.json").is_file() {
                ensure!(
                    json(&parent.join("package.json"))?
                        .get("workspaces")
                        .is_none(),
                    "enclosing package workspace is unsupported"
                );
            }
        }
        let mut project_config_fingerprints = BTreeMap::new();
        let recipe = super::recipe::load(
            &root,
            &manifest,
            &mut project_config_fingerprints,
            cancellation,
        )?;
        let mut runner_config_seen = false;
        for entry in fs::read_dir(&root)? {
            let entry = entry?;
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if super::config::FILES.contains(&name.as_ref()) {
                ensure!(
                    !runner_config_seen,
                    "multiple Vite/Vitest config files are unsupported"
                );
                runner_config_seen = true;
                let source = read(&entry.path())?;
                super::config::validate(&source)
                    .with_context(|| format!("unsupported Vitest project configuration: {name}"))?;
                project_config_fingerprints.insert(name.to_string(), fingerprint(&source));
                continue;
            }
            ensure!(
                !name.starts_with("vite.config.")
                    && !name.starts_with("vitest.config.")
                    && !name.starts_with("vitest.workspace.")
                    && !matches!(
                        name.as_ref(),
                        ".pnpmfile.cjs"
                            | ".npmrc"
                            | "bun.lockb"
                            | ".pnp.cjs"
                            | ".pnp.loader.mjs"
                            | "bunfig.toml"
                    ),
                "unsupported Vitest project configuration: {name}"
            );
        }
        if root.join("pnpm-workspace.yaml").exists() {
            let settings = yaml(&root.join("pnpm-workspace.yaml"))?;
            let allowed: Yaml = serde_yaml_ng::from_str("allowBuilds:\n  esbuild: false\n")?;
            ensure!(
                settings == allowed,
                "only settings-only pnpm-workspace.yaml with allowBuilds.esbuild: false is supported"
            );
        }
        if root.join("tsconfig.json").exists() {
            // Only the audited plain-JSON compiler settings are accepted.
            validate_tsconfig(&root.join("tsconfig.json"), cancellation)?;
        }
        for name in ["tsconfig.json", "pnpm-workspace.yaml", ".yarnrc.yml"] {
            if root.join(name).exists() {
                project_config_fingerprints
                    .insert(name.to_owned(), fingerprint(&read(&root.join(name))?));
            }
        }
        let expected_lock = match package_manager.as_str() {
            "npm@11.19.1" => "package-lock.json",
            "yarn@4.18.1" => "yarn.lock",
            "bun@1.3.11" => "bun.lock",
            _ => "pnpm-lock.yaml",
        };
        for name in [
            "package-lock.json",
            "yarn.lock",
            "bun.lock",
            "pnpm-lock.yaml",
            "npm-shrinkwrap.json",
        ] {
            ensure!(
                name == expected_lock || !root.join(name).exists(),
                "mixed or unsupported lockfiles: {name}"
            );
        }
        ensure!(
            package_manager.starts_with("pnpm@") || !root.join("pnpm-workspace.yaml").exists(),
            "pnpm configuration requires pnpm"
        );
        ensure!(
            package_manager.starts_with("yarn@") || !root.join(".yarnrc.yml").exists(),
            "Yarn configuration requires Yarn"
        );
        let (lock_text, locked_vitest, packages) = match package_manager.as_str() {
            "npm@11.19.1" => validate_npm_installation(&root, &manifest, cancellation)?,
            "yarn@4.18.1" => validate_yarn_installation(&root, &manifest, cancellation)?,
            "bun@1.3.11" => validate_bun_installation(&root, &manifest, cancellation)?,
            _ => validate_pnpm_installation(&root, &manifest, cancellation)?,
        };
        let modules = root.join("node_modules");
        let vitest_root = modules
            .join("vitest")
            .canonicalize()
            .context("missing installed Vitest")?;
        let vitest = json(&vitest_root.join("package.json"))?;
        let vitest_version = vitest["version"]
            .as_str()
            .context("Vitest version missing")?;
        ensure!(
            vitest["name"] == "vitest" && matches!(vitest_version, "4.1.9" | "4.1.5"),
            "Vitest requires installed Vitest 4.1.9 or 4.1.5"
        );
        ensure!(
            vitest_root.join("vitest.mjs").is_file(),
            "installed Vitest CLI is missing"
        );
        ensure!(
            locked_vitest.split('(').next() == Some(vitest_version),
            "locked Vitest must match the installed version"
        );
        let vite_root = resolve_dependency(&root, &vitest_root, "vite")?;
        let vite = json(&vite_root.join("package.json"))?;
        let vite_version = vite["version"].as_str().context("Vite version missing")?;
        ensure!(
            !recipe.typecheck() || vitest_version == "4.1.5",
            "typecheck recipes are audited with Vitest 4.1.5 only"
        );
        ensure!(
            vite["name"] == "vite" && supported_runner(vitest_version, vite_version),
            "unsupported Vitest/Vite version pair"
        );
        ensure!(
            packages.contains(&format!("vite@{vite_version}")),
            "installed Vite is not in the lockfile"
        );
        ensure!(
            root.join("src").is_dir(),
            "Vitest preview requires a src directory"
        );
        Ok(Self {
            installation_fingerprint: installation_fingerprint(&root, cancellation)?,
            project_config_fingerprints,
            package_manager,
            root,
            name,
            vitest: vitest_version.into(),
            vite: vite_version.into(),
            manifest_fingerprint: fingerprint(&manifest_text),
            lockfile_fingerprint: fingerprint(&lock_text),
            recipe,
        })
    }
}

/// Resolve a dependency using native node_modules ancestry without executing JS.
/// Canonical package directories must stay in the owned installation.
pub(super) fn resolve_dependency(root: &Path, from: &Path, name: &str) -> Result<PathBuf> {
    let modules = root.join("node_modules").canonicalize()?;
    let from = from.canonicalize()?;
    ensure!(
        from.starts_with(&modules),
        "dependency origin escapes node_modules"
    );
    ensure!(
        !name.is_empty() && !name.contains("..") && !name.starts_with('/') && !name.contains('\\'),
        "invalid dependency name"
    );
    for parent in from.ancestors() {
        if !parent.starts_with(root) {
            break;
        }
        if parent
            .file_name()
            .is_some_and(|part| part == "node_modules")
        {
            continue;
        }
        let candidate = parent.join("node_modules").join(name);
        match fs::symlink_metadata(&candidate) {
            Ok(_) => {
                let resolved = candidate
                    .canonicalize()
                    .with_context(|| format!("resolving dependency {name}"))?;
                ensure!(
                    resolved.starts_with(&modules) && resolved.is_dir(),
                    "dependency {name} escapes node_modules or is not a directory"
                );
                return Ok(resolved);
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error).context("reading dependency path"),
        }
    }
    bail!("missing installed dependency {name}")
}

fn validate_tsconfig(path: &Path, cancellation: &Cancellation) -> Result<()> {
    let config = json(path).context("only plain JSON tsconfig is supported")?;
    let object = config.as_object().context("tsconfig must be an object")?;
    for key in object.keys() {
        ensure!(
            matches!(key.as_str(), "compilerOptions" | "include" | "exclude"),
            "unsupported tsconfig setting: {key}"
        );
    }
    if let Some(options) = config.get("compilerOptions") {
        let options = options
            .as_object()
            .context("compilerOptions must be an object")?;
        if let Some(module) = options.get("module") {
            if matches!(module.as_str(), Some("nodenext" | "NodeNext")) {
                ensure!(
                    ["allowJs", "checkJs", "noEmit"]
                        .iter()
                        .all(|key| options.get(*key).and_then(serde_json::Value::as_bool)
                            == Some(true)),
                    "NodeNext requires JavaScript-only type checking with allowJs, checkJs and noEmit"
                );
                let root = path.parent().context("tsconfig has no parent")?;
                validate_javascript_only(root, root, cancellation, &mut 0)?;
            } else {
                ensure!(
                    module.as_str() == Some("ESNext"),
                    "only ESNext TypeScript modules are supported"
                );
            }
        }
        for key in ["allowJs", "checkJs"] {
            if let Some(value) = options.get(key) {
                ensure!(value.is_boolean(), "tsconfig {key} must be boolean");
            }
        }
        for key in options.keys() {
            ensure!(
                matches!(
                    key.as_str(),
                    "target"
                        | "module"
                        | "moduleResolution"
                        | "allowJs"
                        | "checkJs"
                        | "strict"
                        | "noEmit"
                        | "skipLibCheck"
                        | "types"
                        | "lib"
                        | "esModuleInterop"
                        | "allowImportingTsExtensions"
                        | "resolveJsonModule"
                        | "forceConsistentCasingInFileNames"
                        | "isolatedModules"
                        | "noImplicitOverride"
                        | "noUncheckedIndexedAccess"
                        | "noUnusedLocals"
                        | "verbatimModuleSyntax"
                ),
                "unsupported tsconfig compiler option: {key}"
            );
        }
    }
    Ok(())
}

// NodeNext is accepted only as an audited JavaScript type-checking configuration.
// Scan the whole project, not just discovery's src directory: tests and helpers
// are executed too. Declarations have no runtime semantics.
fn validate_javascript_only(
    root: &Path,
    path: &Path,
    cancellation: &Cancellation,
    count: &mut usize,
) -> Result<()> {
    cancellation.check()?;
    *count += 1;
    ensure!(
        *count <= 200_000 && path.components().count() - root.components().count() < 100,
        "JavaScript-only project exceeds validation limit"
    );
    let metadata = fs::symlink_metadata(path)?;
    ensure!(
        !metadata.file_type().is_symlink(),
        "JavaScript-only validation does not support project symlinks: {}",
        path.display()
    );
    if metadata.is_dir() {
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            if matches!(
                entry.file_name().to_str(),
                Some("node_modules" | ".git" | ".mutie")
            ) {
                continue;
            }
            validate_javascript_only(root, &entry.path(), cancellation, count)?;
        }
    } else {
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("");
        let declaration = [".d.ts", ".d.mts", ".d.cts"]
            .iter()
            .any(|suffix| name.ends_with(suffix));
        ensure!(
            declaration
                || !matches!(
                    path.extension().and_then(|ext| ext.to_str()),
                    Some("ts" | "tsx" | "mts" | "cts")
                ),
            "NodeNext type checking requires a JavaScript-only project; found {}",
            path.display()
        );
    }
    Ok(())
}

fn validate_tree(
    root: &Path,
    path: &Path,
    cancellation: &Cancellation,
    count: &mut usize,
) -> Result<()> {
    cancellation.check()?;
    *count += 1;
    ensure!(
        path.components().count() - root.components().count() < 100,
        "dependency directory nesting exceeds preview limit"
    );
    ensure!(
        *count <= 200_000,
        "dependency tree exceeds preview validation limit"
    );
    let meta = fs::symlink_metadata(path)?;
    if meta.file_type().is_symlink() {
        let target = path
            .canonicalize()
            .with_context(|| format!("broken dependency link {}", path.display()))?;
        ensure!(
            target.starts_with(root),
            "dependency link escapes node_modules: {}",
            path.display()
        );
    } else if meta.is_dir() {
        for entry in fs::read_dir(path)? {
            validate_tree(root, &entry?.path(), cancellation, count)?;
        }
    } else if !meta.is_file() {
        bail!("unsupported dependency file: {}", path.display());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn write(root: &Path, path: &str, text: &str) {
        let path = root.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }
    fn fixture() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        write(
            root,
            "package.json",
            r#"{"name":"fixture","type":"module","packageManager":"pnpm@11.10.0","devDependencies":{"vitest":"4.1.9"}}"#,
        );
        let lock = "lockfileVersion: '9.0'\nimporters:\n  .:\n    devDependencies:\n      vitest:\n        specifier: 4.1.9\n        version: 4.1.9\npackages:\n  vitest@4.1.9:\n    resolution: {integrity: sha512-test}\n  vite@8.3.4:\n    resolution: {integrity: sha512-test}\n";
        write(root, "pnpm-lock.yaml", lock);
        write(root, "node_modules/.pnpm/lock.yaml", lock);
        write(
            root,
            "node_modules/.modules.yaml",
            "packageManager: pnpm@11.10.0\nnodeLinker: isolated\nvirtualStoreDir: .pnpm\n",
        );
        write(
            root,
            "node_modules/vitest/package.json",
            r#"{"name":"vitest","version":"4.1.9"}"#,
        );
        write(root, "node_modules/vitest/vitest.mjs", "");
        write(
            root,
            "node_modules/vite/package.json",
            r#"{"name":"vite","version":"8.3.4"}"#,
        );
        write(root, "src/a.js", "export const flag=true;");
        dir
    }
    fn load(root: &Path) -> Result<Project> {
        Project::load(root, &Cancellation::default())
    }
    #[test]
    fn validates_metadata_and_fingerprints_without_running_project_code() {
        let dir = fixture();
        let a = load(dir.path()).unwrap();
        assert_eq!(a.vite, "8.3.4");
        write(
            dir.path(),
            "pnpm-workspace.yaml",
            "allowBuilds:\n  esbuild: false\n",
        );
        assert_eq!(
            load(dir.path()).unwrap().lockfile_fingerprint,
            a.lockfile_fingerprint
        );
        let manifest = fs::read_to_string(dir.path().join("package.json")).unwrap();
        write(dir.path(), "package.json", &(manifest + "\n"));
        assert_ne!(
            load(dir.path()).unwrap().manifest_fingerprint,
            a.manifest_fingerprint
        );
    }
    #[test]
    fn older_runner_keeps_exact_manager_lock_and_vite_pair_checks() {
        let dir = fixture();
        let root = dir.path();
        for name in [
            "package.json",
            "pnpm-lock.yaml",
            "node_modules/.pnpm/lock.yaml",
            "node_modules/.modules.yaml",
            "node_modules/vitest/package.json",
            "node_modules/vite/package.json",
        ] {
            let text = fs::read_to_string(root.join(name))
                .unwrap()
                .replace("11.10.0", "10.33.2")
                .replace("4.1.9", "4.1.5")
                .replace("8.3.4", "8.0.10");
            write(root, name, &text);
        }
        let mut manifest: Json =
            serde_json::from_str(&fs::read_to_string(root.join("package.json")).unwrap()).unwrap();
        manifest["scripts"] = serde_json::json!({"test":"vitest run"});
        write(root, "package.json", &manifest.to_string());
        assert_eq!(load(root).unwrap().vitest, "4.1.5");
        write(
            root,
            "node_modules/vite/package.json",
            r#"{"name":"vite","version":"8.3.4"}"#,
        );
        assert!(load(root).is_err());
        write(
            root,
            "node_modules/vite/package.json",
            r#"{"name":"vite","version":"8.0.10"}"#,
        );
        write(
            root,
            "node_modules/.modules.yaml",
            "packageManager: pnpm@11.10.0\nnodeLinker: isolated\nvirtualStoreDir: .pnpm\n",
        );
        assert!(load(root).is_err());
    }
    #[test]
    fn coverage_only_configs_are_fingerprinted_and_ambiguity_is_rejected() {
        let dir = fixture();
        let source = "import {defineConfig} from 'vitest/config'; export default defineConfig({test:{coverage:{reporter:['text','json','html','clover']}}});";
        for name in super::super::config::FILES {
            write(dir.path(), name, source);
            let original = load(dir.path()).unwrap();
            assert_eq!(
                original.project_config_fingerprints[*name],
                fingerprint(source)
            );
            write(dir.path(), name, &format!("{source}\n"));
            assert_ne!(
                load(dir.path()).unwrap().project_config_fingerprints,
                original.project_config_fingerprints
            );
            fs::remove_file(dir.path().join(name)).unwrap();
        }
        write(dir.path(), "vite.config.ts", source);
        write(dir.path(), "vitest.config.ts", source);
        assert!(
            load(dir.path())
                .unwrap_err()
                .to_string()
                .contains("multiple")
        );
    }

    #[test]
    fn nodenext_type_checking_requires_javascript_only_and_tracks_config() {
        let config = r#"{"compilerOptions":{"module":"nodenext","allowJs":true,"checkJs":true,"noEmit":true}}"#;
        let dir = fixture();
        write(dir.path(), "tsconfig.json", config);
        write(
            dir.path(),
            "types/index.d.ts",
            "export declare const flag: boolean;",
        );
        assert_eq!(
            load(dir.path()).unwrap().project_config_fingerprints["tsconfig.json"],
            fingerprint(config)
        );
        for path in [
            "src/a.ts",
            "test/a.test.ts",
            "scripts/helper.mts",
            "helper.cts",
            "view.tsx",
        ] {
            write(dir.path(), path, "export const flag = true;");
            assert!(
                load(dir.path())
                    .unwrap_err()
                    .to_string()
                    .contains("JavaScript-only"),
                "{path}"
            );
            fs::remove_file(dir.path().join(path)).unwrap();
        }
        for key in ["allowJs", "checkJs", "noEmit"] {
            for value in [
                serde_json::Value::Bool(false),
                serde_json::Value::String("true".into()),
                serde_json::Value::Null,
            ] {
                let mut invalid: serde_json::Value = serde_json::from_str(config).unwrap();
                invalid["compilerOptions"][key] = value;
                write(dir.path(), "tsconfig.json", &invalid.to_string());
                assert!(load(dir.path()).is_err(), "{key}");
            }
        }
    }

    #[cfg(unix)]
    #[test]
    fn javascript_only_guard_rejects_symlinks_and_honors_cancellation() {
        let dir = fixture();
        let cancellation = Cancellation::default();
        std::os::unix::fs::symlink("a.js", dir.path().join("src/link.js")).unwrap();
        assert!(
            validate_javascript_only(dir.path(), dir.path(), &cancellation, &mut 0)
                .unwrap_err()
                .to_string()
                .contains("symlinks")
        );
        fs::remove_file(dir.path().join("src/link.js")).unwrap();
        cancellation.cancel();
        assert!(validate_javascript_only(dir.path(), dir.path(), &cancellation, &mut 0).is_err());
    }

    #[test]
    fn rejects_manifest_config_and_installation_mismatches() {
        for (path, contents) in [
            ("package.json", r#"{"name":"fixture","type":"commonjs"}"#),
            ("vitest.config.ts", "throw new Error('must not execute');"),
            ("vite.config.js", ""),
            (".npmrc", "node-linker=hoisted"),
            ("pnpm-workspace.yaml", "packages: ['packages/*']"),
            (
                "tsconfig.json",
                r#"{"compilerOptions":{"paths":{"x":["../x"]}}}"#,
            ),
            ("node_modules/.pnpm/lock.yaml", "{}"),
            ("node_modules/.modules.yaml", "packageManager: pnpm@12.0.0"),
            (
                "node_modules/vitest/package.json",
                r#"{"name":"vitest","version":"5.0.0"}"#,
            ),
            (
                "node_modules/vite/package.json",
                r#"{"name":"vite","version":"8.9.0"}"#,
            ),
        ] {
            let dir = fixture();
            write(dir.path(), path, contents);
            assert!(load(dir.path()).is_err(), "{path}");
        }
        let dir = fixture();
        fs::remove_file(dir.path().join("node_modules/vitest/vitest.mjs")).unwrap();
        assert!(load(dir.path()).is_err());
    }
    #[test]
    fn rejects_stale_manifest_and_nonregistry_lock_resolutions() {
        for (from, to) in [
            ("specifier: 4.1.9", "specifier: ^4.1.9"),
            ("version: 4.1.9", "version: link:../other"),
            (
                "integrity: sha512-test",
                "tarball: https://example.com/a.tgz",
            ),
            ("  .:\n", "  other: {}\n  .:\n"),
        ] {
            let dir = fixture();
            let text = fs::read_to_string(dir.path().join("pnpm-lock.yaml"))
                .unwrap()
                .replace(from, to);
            write(dir.path(), "pnpm-lock.yaml", &text);
            write(dir.path(), "node_modules/.pnpm/lock.yaml", &text);
            assert!(load(dir.path()).is_err(), "{to}");
        }
    }
    #[cfg(unix)]
    #[test]
    fn dependency_links_must_resolve_inside_the_tree() {
        let dir = fixture();
        let root = dir.path();
        std::os::unix::fs::symlink("vite", root.join("node_modules/internal")).unwrap();
        assert!(load(root).is_ok());
        std::os::unix::fs::symlink("../src", root.join("node_modules/external")).unwrap();
        assert!(load(root).is_err());
        fs::remove_file(root.join("node_modules/external")).unwrap();
        std::os::unix::fs::symlink("missing", root.join("node_modules/broken")).unwrap();
        assert!(load(root).is_err());
    }
    #[test]
    fn rejects_enclosing_workspace() {
        let parent = tempfile::tempdir().unwrap();
        write(parent.path(), "pnpm-workspace.yaml", "packages: ['*']");
        let fixture = fixture();
        let nested = parent.path().join("project");
        fs::create_dir(&nested).unwrap();
        fs::copy(
            fixture.path().join("package.json"),
            nested.join("package.json"),
        )
        .unwrap();
        assert!(format!("{:#}", load(&nested).unwrap_err()).contains("enclosing"));
    }
}

#[cfg(test)]
mod dependency_resolution_tests {
    use super::*;

    #[test]
    fn resolves_hoisted_and_nearer_dependencies() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let root = temp.path().canonicalize()?;
        let vitest = root.join("node_modules/vitest");
        let hoisted = root.join("node_modules/@vitest/runner");
        fs::create_dir_all(&vitest)?;
        fs::create_dir_all(&hoisted)?;
        assert_eq!(
            resolve_dependency(&root, &vitest, "@vitest/runner")?,
            hoisted
        );
        let nested = vitest.join("node_modules/@vitest/runner");
        fs::create_dir_all(&nested)?;
        assert_eq!(
            resolve_dependency(&root, &vitest, "@vitest/runner")?,
            nested
        );
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn resolves_isolated_links_but_rejects_external_dependencies() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let root = temp.path().canonicalize()?;
        let store = root.join("node_modules/.pnpm/vitest@4.1.9/node_modules");
        let vitest = store.join("vitest");
        let runner = root.join("node_modules/.pnpm/runner/node_modules/@vitest/runner");
        fs::create_dir_all(&vitest)?;
        fs::create_dir_all(store.join("@vitest"))?;
        fs::create_dir_all(&runner)?;
        let link = store.join("@vitest/runner");
        std::os::unix::fs::symlink(&runner, &link)?;
        assert_eq!(
            resolve_dependency(&root, &vitest, "@vitest/runner")?,
            runner
        );
        fs::remove_file(&link)?;
        std::os::unix::fs::symlink(&root, &link)?;
        assert!(resolve_dependency(&root, &vitest, "@vitest/runner").is_err());
        Ok(())
    }
}

#[cfg(test)]
mod npm_tests {
    use super::*;

    #[test]
    fn npm_lock_validation_rejects_drift_and_local_packages() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let root = temp.path();
        fs::create_dir_all(root.join("node_modules/vitest"))?;
        let manifest = serde_json::json!({"devDependencies":{"vitest":"4.1.9"}});
        let entry = serde_json::json!({"version":"4.1.9","resolved":"https://registry.npmjs.org/vitest/-/vitest-4.1.9.tgz","integrity":"sha512-test"});
        let lock = serde_json::json!({"lockfileVersion":3,"packages":{"":manifest,"node_modules/vitest":entry}});
        let hidden =
            serde_json::json!({"lockfileVersion":3,"packages":{"node_modules/vitest":entry}});
        fs::write(root.join("package-lock.json"), serde_json::to_vec(&lock)?)?;
        fs::write(
            root.join("node_modules/.package-lock.json"),
            serde_json::to_vec(&hidden)?,
        )?;
        fs::write(
            root.join("node_modules/vitest/package.json"),
            r#"{"name":"vitest","version":"4.1.9"}"#,
        )?;
        validate_npm_installation(root, &manifest, &Cancellation::default())?;
        let mut wrong = manifest.clone();
        wrong["devDependencies"]["vitest"] = "4.1.8".into();
        assert!(validate_npm_installation(root, &wrong, &Cancellation::default()).is_err());
        let mut wrong_lock = lock.clone();
        wrong_lock["packages"]["node_modules/vitest"]["resolved"] = "file:../vitest".into();
        fs::write(
            root.join("package-lock.json"),
            serde_json::to_vec(&wrong_lock)?,
        )?;
        assert!(validate_npm_installation(root, &manifest, &Cancellation::default()).is_err());
        fs::write(root.join("package-lock.json"), serde_json::to_vec(&lock)?)?;
        fs::write(
            root.join("node_modules/vitest/package.json"),
            r#"{"name":"vitest","version":"4.1.8"}"#,
        )?;
        assert!(validate_npm_installation(root, &manifest, &Cancellation::default()).is_err());
        Ok(())
    }

    #[test]
    fn yarn_and_bun_validate_installed_versions_and_manifest_drift() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let root = temp.path();
        fs::create_dir_all(root.join("node_modules/vitest"))?;
        fs::write(
            root.join("node_modules/vitest/package.json"),
            r#"{"name":"vitest","version":"4.1.9"}"#,
        )?;
        let manifest = serde_json::json!({"name":"fixture","devDependencies":{"vitest":"4.1.9"}});
        let lock = serde_json::json!({"lockfileVersion":1,"configVersion":1,"workspaces":{"":manifest},"packages":{"vitest":["vitest@4.1.9","",{},"sha512-test"]}});
        fs::write(root.join("bun.lock"), serde_json::to_vec(&lock)?)?;
        validate_bun_installation(root, &manifest, &Cancellation::default())?;
        fs::write(
            root.join(".yarnrc.yml"),
            "nodeLinker: node-modules\nenableScripts: false\n",
        )?;
        fs::write(
            root.join("yarn.lock"),
            "__metadata: {version: 10}\n'fixture@workspace:.':\n  dependencies: {vitest: 'npm:4.1.9'}\n'vitest@npm:4.1.9':\n  resolution: 'vitest@npm:4.1.9'\n  version: '4.1.9'\n  checksum: test\n",
        )?;
        fs::write(
            root.join("node_modules/.yarn-state.yml"),
            "__metadata: {version: 1, nmMode: classic}\n'vitest@npm:4.1.9':\n  locations: ['node_modules/vitest']\n",
        )?;
        validate_yarn_installation(root, &manifest, &Cancellation::default())?;
        let mut changed = manifest.clone();
        changed["devDependencies"]["vitest"] = "4.1.8".into();
        assert!(validate_bun_installation(root, &changed, &Cancellation::default()).is_err());
        assert!(validate_yarn_installation(root, &changed, &Cancellation::default()).is_err());
        fs::write(
            root.join("node_modules/vitest/package.json"),
            r#"{"name":"vitest","version":"4.1.8"}"#,
        )?;
        assert!(validate_bun_installation(root, &manifest, &Cancellation::default()).is_err());
        assert!(validate_yarn_installation(root, &manifest, &Cancellation::default()).is_err());
        Ok(())
    }

    #[test]
    fn installation_identity_detects_same_length_changes_and_ignores_owned_caches() -> Result<()> {
        let temp = tempfile::tempdir()?;
        let root = temp.path();
        fs::create_dir(root.join("node_modules"))?;
        fs::write(root.join("node_modules/module.js"), "true")?;
        let a = installation_fingerprint(root, &Cancellation::default())?;
        fs::create_dir(root.join("node_modules/.vite"))?;
        fs::write(root.join("node_modules/.vite/cache"), "temporary")?;
        assert_eq!(a, installation_fingerprint(root, &Cancellation::default())?);
        fs::write(root.join("node_modules/module.js"), "null")?;
        assert_ne!(a, installation_fingerprint(root, &Cancellation::default())?);
        Ok(())
    }

    #[test]
    fn bun_trailing_comma_parser_preserves_strings_and_rejects_code() -> Result<()> {
        assert_eq!(
            json_trailing_commas(r#"{"text":",}","array":[1,],}"#)?["text"],
            ",}"
        );
        assert!(json_trailing_commas("{x: process.exit()}").is_err());
        assert!(json_trailing_commas("{\"a\": 1,,}").is_err());
        Ok(())
    }

    #[test]
    #[ignore = "requires MUTIE_MANAGER_AUDIT with prepared npm/Yarn/Bun fixtures"]
    fn real_manager_project_metadata() -> Result<()> {
        let base =
            std::env::var_os("MUTIE_MANAGER_AUDIT").context("MUTIE_MANAGER_AUDIT required")?;
        for (manager, version) in [("npm", "11.19.1"), ("yarn", "4.18.1"), ("bun", "1.3.11")] {
            let project = Project::load(&Path::new(&base).join(manager), &Cancellation::default())
                .with_context(|| format!("loading {manager}"))?;
            assert_eq!(project.package_manager, format!("{manager}@{version}"));
        }
        Ok(())
    }

    #[test]
    #[ignore = "requires MUTIE_NPM_FIXTURE prepared with audited npm/Vitest versions"]
    fn real_npm_project_metadata() -> Result<()> {
        let root = std::env::var_os("MUTIE_NPM_FIXTURE").context("MUTIE_NPM_FIXTURE required")?;
        let project = Project::load(Path::new(&root), &Cancellation::default())?;
        assert_eq!(project.package_manager, "npm@11.19.1");
        assert_eq!(project.vitest, "4.1.9");
        Ok(())
    }
}
