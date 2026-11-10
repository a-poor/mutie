//! Recognized test recipes, without invoking package scripts or installing tools.
use crate::cancellation::Cancellation;
use anyhow::{Context, Result, ensure};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fs, path::Path};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Recipe {
    #[default]
    Runtime,
    Typecheck,
    LintTypecheck,
}
impl Recipe {
    pub fn is_runtime(&self) -> bool {
        *self == Self::Runtime
    }
    pub fn typecheck(self) -> bool {
        self != Self::Runtime
    }
}

fn read(root: &Path, name: &str, fingerprints: &mut BTreeMap<String, String>) -> Result<String> {
    let path = root.join(name);
    let meta =
        fs::symlink_metadata(&path).with_context(|| format!("missing recipe input {name}"))?;
    ensure!(
        meta.is_file() && meta.len() <= 64 * 1024,
        "invalid or oversized recipe input {name}"
    );
    let source = fs::read_to_string(path)?;
    fingerprints.insert(
        name.into(),
        format!("sha256:{:x}", Sha256::digest(source.as_bytes())),
    );
    Ok(source)
}

pub(super) fn load(
    root: &Path,
    manifest: &Value,
    fingerprints: &mut BTreeMap<String, String>,
    cancellation: &Cancellation,
) -> Result<Recipe> {
    let script = manifest["scripts"]["test"].as_str().unwrap_or("");
    let recipe = match script {
        "vitest run --typecheck" => Recipe::Typecheck,
        "pnpm lint && vitest run --typecheck" => {
            ensure!(
                manifest["packageManager"]
                    .as_str()
                    .is_some_and(|p| p.starts_with("pnpm@"))
                    && manifest["scripts"]["lint"] == "eslint . && prettier -c src test",
                "unsupported Vitest lint prerequisite; expected eslint . && prettier -c src test"
            );
            Recipe::LintTypecheck
        }
        _ => {
            ensure!(
                !script.contains("typecheck"),
                "unsupported Vitest typecheck script"
            );
            ensure!(
                manifest["packageManager"] != "pnpm@10.33.2" || script == "vitest run",
                "pnpm 10 Vitest support requires a recognized complete test recipe"
            );
            Recipe::Runtime
        }
    };
    if recipe.typecheck() {
        read(root, "tsconfig.json", fingerprints)?;
        tool(root, "typescript", "6.0.3", "bin/tsc")?;
        type_files(root, cancellation)?;
    }
    if recipe == Recipe::LintTypecheck {
        tool(root, "eslint", "10.2.1", "bin/eslint.js")?;
        tool(root, "prettier", "3.8.3", "bin/prettier.cjs")?;
        tool(root, "eslint-config-unjs", "0.6.2", "package.json")?;
        validate_eslint(&read(root, "eslint.config.mjs", fingerprints)?)?;
        ensure!(
            serde_json::from_str::<Value>(&read(root, ".prettierrc", fingerprints)?)?
                == serde_json::json!({}),
            "the audited lint recipe requires an empty JSON .prettierrc"
        );
        let editor = read(root, ".editorconfig", fingerprints)?;
        validate_editorconfig(&editor)?;
        ensure!(
            manifest.get("prettier").is_none() && manifest.get("eslintConfig").is_none(),
            "multiple lint configurations are unsupported"
        );
        walk(root, root, cancellation, &mut 0, &mut |path| {
            let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
            let config = name.starts_with("eslint.config.")
                || name.starts_with(".eslintrc")
                || name.starts_with(".prettier")
                || name.starts_with("prettier.config.")
                || name == ".editorconfig";
            if config {
                ensure!(
                    path.parent() == Some(root)
                        && matches!(name, "eslint.config.mjs" | ".prettierrc" | ".editorconfig"),
                    "unsupported additional lint configuration: {}",
                    path.display()
                );
            }
            Ok(())
        })?;
    }
    Ok(recipe)
}

/// These are direct tools whose complete dependency tree is fingerprinted by Project.
pub(super) fn tool(
    root: &Path,
    name: &str,
    version: &str,
    relative: &str,
) -> Result<std::path::PathBuf> {
    let package = root
        .join("node_modules")
        .join(name)
        .canonicalize()
        .with_context(|| format!("missing installed recipe tool {name}"))?;
    ensure!(
        package.starts_with(root.join("node_modules")),
        "recipe tool escapes owned dependencies"
    );
    let manifest: Value = serde_json::from_slice(&fs::read(package.join("package.json"))?)?;
    ensure!(
        manifest["name"] == name && manifest["version"] == version,
        "the audited recipe requires {name} {version}"
    );
    let executable = package.join(relative).canonicalize()?;
    ensure!(
        executable.starts_with(root.join("node_modules")) && executable.is_file(),
        "recipe tool escapes owned dependencies"
    );
    Ok(executable)
}

fn validate_eslint(source: &str) -> Result<()> {
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&tree_sitter_javascript::LANGUAGE.into())?;
    let tree = parser
        .parse(source, None)
        .context("missing ESLint syntax tree")?;
    ensure!(
        !tree.root_node().has_error(),
        "invalid ESLint configuration"
    );
    let mut tokens = Vec::new();
    super::config::collect(tree.root_node(), source, 0, &mut tokens)?;
    let prefix = [
        "import",
        "unjs",
        "from",
        "string:eslint-config-unjs",
        "export",
        "default",
        "unjs",
        "(",
        "{",
        "ignores",
        ":",
        "[",
        "]",
        "rules",
        ":",
        "{",
    ];
    let suffix = ["}", "}", ")"];
    ensure!(
        tokens.len() >= prefix.len() + suffix.len()
            && tokens
                .iter()
                .take(prefix.len())
                .map(String::as_str)
                .eq(prefix)
            && tokens
                .iter()
                .skip(tokens.len() - suffix.len())
                .map(String::as_str)
                .eq(suffix),
        "only static unjs({{ignores:[],rules:{{...}}}} ESLint configuration is supported"
    );
    let rules = &tokens[prefix.len()..tokens.len() - suffix.len()];
    ensure!(
        rules.len().is_multiple_of(3)
            && rules
                .as_chunks::<3>()
                .0
                .iter()
                .all(|p| p[0].starts_with("string:")
                    && p[1] == ":"
                    && matches!(p[2].as_str(), "0" | "1" | "2")),
        "ESLint rules must be literal numeric levels"
    );
    Ok(())
}

fn validate_editorconfig(source: &str) -> Result<()> {
    let mut section = false;
    let mut root = false;
    for line in source
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with(['#', ';']))
    {
        if line.starts_with('[') {
            ensure!(line.ends_with(']'), "invalid EditorConfig section");
            section = true;
        } else {
            let (key, value) = line
                .split_once('=')
                .context("invalid EditorConfig setting")?;
            if key.trim().eq_ignore_ascii_case("root") {
                ensure!(
                    !section && !root && value.trim() == "true",
                    "lint recipe requires one global EditorConfig root = true"
                );
                root = true;
            } else {
                ensure!(section, "unsupported global EditorConfig setting");
            }
        }
    }
    ensure!(root, "lint recipe requires root = true in .editorconfig");
    Ok(())
}

pub(super) fn validate_implicit_typescript(root: &Path, cancellation: &Cancellation) -> Result<()> {
    let mut typescript = false;
    walk(root, root, cancellation, &mut 0, &mut |path| {
        let extension = path.extension().and_then(|s| s.to_str());
        ensure!(
            !matches!(extension, Some("js" | "cjs" | "cts" | "jsx" | "tsx")),
            "Vitest without package type requires TypeScript sources and explicit .mjs tooling; found {}",
            path.display()
        );
        typescript |= extension == Some("ts") && path.starts_with(root.join("src"));
        Ok(())
    })?;
    ensure!(
        typescript,
        "Vitest without package type requires TypeScript sources"
    );
    Ok(())
}

pub(super) fn type_files(
    root: &Path,
    cancellation: &Cancellation,
) -> Result<Vec<std::path::PathBuf>> {
    let mut paths = Vec::new();
    walk(root, root, cancellation, &mut 0, &mut |path| {
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("");
        if name.contains(".test-d.") || name.contains(".spec-d.") {
            ensure!(
                name.ends_with(".test-d.ts") || name.ends_with(".spec-d.ts"),
                "only .test-d.ts and .spec-d.ts type tests are audited"
            );
            paths.push(path.to_owned());
        }
        Ok(())
    })?;
    paths.sort();
    ensure!(
        !paths.is_empty(),
        "typecheck recipe requires type-test files"
    );
    Ok(paths)
}

fn walk(
    root: &Path,
    path: &Path,
    cancellation: &Cancellation,
    count: &mut usize,
    visit: &mut impl FnMut(&Path) -> Result<()>,
) -> Result<()> {
    cancellation.check()?;
    *count += 1;
    ensure!(
        *count <= 200_000 && path.components().count() - root.components().count() < 100,
        "recipe input tree exceeds validation limit"
    );
    let metadata = fs::symlink_metadata(path)?;
    ensure!(
        !metadata.file_type().is_symlink(),
        "recipe inputs cannot be symlinks: {}",
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
            walk(root, &entry.path(), cancellation, count, visit)?;
        }
    } else {
        ensure!(metadata.is_file(), "unsupported recipe input");
        visit(path)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn lint_config_is_static_and_cannot_filter_files_or_execute_code() {
        let source = "import unjs from 'eslint-config-unjs'; export default unjs({ignores:[],rules:{'unicorn/prevent-abbreviations':0,}});";
        validate_eslint(source).unwrap();
        for bad in [
            format!("{source}; process.exit(0)"),
            source.replace("[]", "['src']"),
            source.replace(":0", ":process.env.LEVEL"),
            source.replace("unjs({", "unjs(()=>({"),
            source.replace("rules:{", "plugins:[],rules:{"),
        ] {
            assert!(validate_eslint(&bad).is_err(), "{bad}");
        }
        validate_editorconfig("root = true\n[*]\nend_of_line = lf\n").unwrap();
        for bad in [
            "[*]\nroot = true",
            "root = true\nroot = false",
            "# root = true\n[*]\nend_of_line = lf",
        ] {
            assert!(validate_editorconfig(bad).is_err());
        }
    }
    #[test]
    fn implicit_typescript_does_not_admit_commonjs_or_untyped_helpers() {
        let temp = tempfile::tempdir().unwrap();
        fs::create_dir(temp.path().join("src")).unwrap();
        fs::write(
            temp.path().join("src/index.ts"),
            "export const ready = true;",
        )
        .unwrap();
        validate_implicit_typescript(temp.path(), &Cancellation::default()).unwrap();
        fs::write(temp.path().join("helper.js"), "module.exports = true;").unwrap();
        assert!(validate_implicit_typescript(temp.path(), &Cancellation::default()).is_err());
    }
}
