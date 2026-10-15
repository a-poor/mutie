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

#[derive(Debug)]
pub struct Project {
    pub root: PathBuf,
    pub project_config_fingerprints: BTreeMap<String, String>,
    pub name: String,
    pub vitest: String,
    pub vite: String,
    pub manifest_fingerprint: String,
    pub lockfile_fingerprint: String,
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
        ensure!(
            manifest["type"] == "module",
            "Vitest preview requires package.json type: module"
        );
        let name = manifest["name"]
            .as_str()
            .filter(|s| !s.trim().is_empty())
            .context("package.json requires a name")?
            .to_owned();
        ensure!(
            manifest["packageManager"] == "pnpm@11.10.0",
            "Vitest preview requires packageManager pnpm@11.10.0"
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
                            | "package-lock.json"
                            | "yarn.lock"
                            | "bun.lock"
                            | "bun.lockb"
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
            validate_tsconfig(&root.join("tsconfig.json"))?;
        }
        for name in ["tsconfig.json", "pnpm-workspace.yaml"] {
            if root.join(name).exists() {
                project_config_fingerprints
                    .insert(name.to_owned(), fingerprint(&read(&root.join(name))?));
            }
        }
        let lock_text = read(&root.join("pnpm-lock.yaml"))?;
        let lock: Yaml = serde_yaml_ng::from_str(&lock_text).context("parsing pnpm-lock.yaml")?;
        ensure!(
            lock["lockfileVersion"].as_str() == Some("9.0"),
            "expected pnpm lockfileVersion 9.0"
        );
        let importers = lock["importers"]
            .as_mapping()
            .context("lockfile requires importers")?;
        ensure!(
            importers.len() == 1 && importers.contains_key(Yaml::String(".".into())),
            "only one root lockfile importer is supported"
        );
        let importer = &lock["importers"]["."];
        for group in ["dependencies", "devDependencies", "optionalDependencies"] {
            let deps = manifest
                .get(group)
                .map(|v| v.as_object().context("dependency group must be an object"))
                .transpose()?;
            let locked = importer[group].as_mapping();
            ensure!(
                deps.map_or(0, |d| d.len()) == locked.map_or(0, |d| d.len()),
                "manifest/lockfile {group} mismatch; install with the frozen lockfile"
            );
            if let Some(deps) = deps {
                for (name, spec) in deps {
                    let spec = spec
                        .as_str()
                        .context("dependency specifier must be a string")?;
                    ensure!(!spec.is_empty() && spec.chars().all(|c| c.is_ascii_digit() || " .^~<>=|*-+xXvabcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ".contains(c)), "only registry version dependencies are supported: {name}");
                    ensure!(
                        importer[group][name.as_str()]["specifier"].as_str() == Some(spec),
                        "manifest/lockfile specifier mismatch for {name}"
                    );
                }
            }
        }
        let packages = lock["packages"]
            .as_mapping()
            .context("lockfile requires registry packages")?;
        for (_, package) in packages {
            let resolution = package["resolution"]
                .as_mapping()
                .context("package requires a registry integrity resolution")?;
            ensure!(
                resolution.len() == 1
                    && package["resolution"]["integrity"]
                        .as_str()
                        .is_some_and(|s| s.starts_with("sha512-")),
                "only integrity-pinned registry dependencies are supported"
            );
        }
        let modules = root.join("node_modules");
        ensure!(fs::symlink_metadata(&modules).context("install dependencies with pnpm install --frozen-lockfile --ignore-scripts before listing")?.file_type().is_dir(), "node_modules must be a local directory");
        validate_tree(&modules, &modules, cancellation, &mut 0)?;
        let layout = yaml(&modules.join(".modules.yaml"))?;
        ensure!(
            layout["packageManager"].as_str() == Some("pnpm@11.10.0")
                && layout["nodeLinker"].as_str() == Some("isolated")
                && layout["virtualStoreDir"].as_str() == Some(".pnpm"),
            "installed dependencies require pnpm 11.10.0 with a local isolated virtual store"
        );
        for group in ["dependencies", "devDependencies", "optionalDependencies"] {
            if let Some(deps) = importer[group].as_mapping() {
                for (name, entry) in deps {
                    let name = name.as_str().context("dependency name must be a string")?;
                    ensure!(
                        !name.contains("..") && !name.starts_with('/') && !name.contains('\\'),
                        "invalid dependency name: {name}"
                    );
                    let version = entry["version"]
                        .as_str()
                        .context("locked dependency version is missing")?
                        .split('(')
                        .next()
                        .unwrap();
                    ensure!(
                        packages.contains_key(Yaml::String(format!("{name}@{version}"))),
                        "dependency is not a locked registry package: {name}"
                    );
                    let installed = modules.join(name).join("package.json");
                    if group == "optionalDependencies" && !installed.exists() {
                        continue;
                    }
                    let data = json(
                        &installed
                            .canonicalize()
                            .with_context(|| format!("missing installed dependency {name}"))?,
                    )?;
                    ensure!(
                        data["name"] == name && data["version"] == version,
                        "installed dependency differs from lockfile: {name}"
                    );
                }
            }
        }
        let installed_lock = yaml(&modules.join(".pnpm/lock.yaml"))?;
        ensure!(
            installed_lock == lock,
            "installed dependency lock differs from pnpm-lock.yaml; reinstall with the frozen lockfile"
        );
        let vitest_root = modules
            .join("vitest")
            .canonicalize()
            .context("missing installed Vitest")?;
        let vitest = json(&vitest_root.join("package.json"))?;
        ensure!(
            vitest["name"] == "vitest" && vitest["version"] == "4.1.9",
            "Vitest preview requires installed Vitest 4.1.9"
        );
        ensure!(
            vitest_root.join("vitest.mjs").is_file(),
            "installed Vitest CLI is missing"
        );
        let locked_vitest = ["devDependencies", "dependencies"]
            .iter()
            .find_map(|group| importer[*group]["vitest"]["version"].as_str())
            .context("Vitest must be a direct dependency")?;
        ensure!(
            locked_vitest == "4.1.9" || locked_vitest.starts_with("4.1.9("),
            "locked Vitest must match installed 4.1.9"
        );
        let vite_root = vitest_root
            .parent()
            .context("invalid installed Vitest path")?
            .join("vite")
            .canonicalize()
            .context("missing Vitest's Vite dependency")?;
        ensure!(
            vite_root.starts_with(&modules),
            "Vite dependency escapes node_modules"
        );
        let vite = json(&vite_root.join("package.json"))?;
        let vite_version = vite["version"].as_str().context("Vite version missing")?;
        ensure!(
            vite["name"] == "vite" && matches!(vite_version, "8.1.3" | "8.3.4"),
            "Vitest preview requires verified Vite 8.1.3 or 8.3.4"
        );
        ensure!(
            packages.contains_key(Yaml::String(format!("vite@{vite_version}"))),
            "installed Vite is not in the lockfile"
        );
        ensure!(
            root.join("src").is_dir(),
            "Vitest preview requires a src directory"
        );
        Ok(Self {
            project_config_fingerprints,
            root,
            name,
            vitest: "4.1.9".into(),
            vite: vite_version.into(),
            manifest_fingerprint: fingerprint(&manifest_text),
            lockfile_fingerprint: fingerprint(&lock_text),
        })
    }
}

fn validate_tsconfig(path: &Path) -> Result<()> {
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
            ensure!(
                module.as_str() == Some("ESNext"),
                "only ESNext TypeScript modules are supported"
            );
        }
        for key in options.keys() {
            ensure!(
                matches!(
                    key.as_str(),
                    "target"
                        | "module"
                        | "moduleResolution"
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
