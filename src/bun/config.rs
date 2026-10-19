//! Narrow plain-JSON tsconfig acceptance for the pinned Composer execution audit.
use anyhow::{Context, Result, ensure};
use serde::Deserialize;
use std::{fs, path::Path};

// Typed objects deliberately reject duplicate as well as unknown keys. Do not
// validate through Value maps: a duplicate could hide a different runtime setting.
#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Config {
    #[serde(default)]
    compiler_options: Options,
    #[serde(default, rename = "include")]
    _include: Vec<String>,
    #[serde(default, rename = "exclude")]
    _exclude: Vec<String>,
}
#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct Options {
    module: Option<String>,
    module_resolution: Option<String>,
    target: Option<String>,
    module_detection: Option<String>,
    lib: Option<Vec<String>>,
    no_emit: Option<bool>,
    allow_importing_ts_extensions: Option<bool>,
    #[serde(rename = "strict")]
    _strict: Option<bool>,
    #[serde(rename = "skipLibCheck")]
    _skip_lib_check: Option<bool>,
    #[serde(rename = "noFallthroughCasesInSwitch")]
    _no_fallthrough: Option<bool>,
    #[serde(rename = "declaration")]
    _declaration: Option<bool>,
    #[serde(rename = "forceConsistentCasingInFileNames")]
    _consistent_casing: Option<bool>,
}

pub(super) fn validate(root: &Path) -> Result<()> {
    let path = root.join("tsconfig.json");
    if !path.exists() {
        return Ok(());
    }
    let config: Config = serde_json::from_slice(&fs::read(path)?).context(
        "unsupported Bun tsconfig; requires plain JSON with audited options and no duplicate keys",
    )?;
    let options = config.compiler_options;
    for (key, actual, expected) in [
        ("module", options.module, "NodeNext"),
        ("moduleResolution", options.module_resolution, "NodeNext"),
        ("target", options.target, "ES2022"),
        ("moduleDetection", options.module_detection, "force"),
    ] {
        ensure!(
            actual.is_none_or(|v| v == expected),
            "unsupported Bun tsconfig {key}"
        );
    }
    ensure!(
        options
            .lib
            .is_none_or(|a| a.iter().all(|v| v == "ESNext" || v == "DOM")),
        "unsupported Bun tsconfig lib"
    );
    ensure!(
        options.no_emit != Some(false) && options.allow_importing_ts_extensions != Some(false),
        "unsupported Bun tsconfig emit/import setting"
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_resolution_transforms_inheritance_duplicates_and_invalid_types() -> Result<()> {
        let root = tempfile::tempdir()?;
        for text in [
            r#"{"extends":"./base.json"}"#,
            r#"{"references":[]}"#,
            r#"{"compilerOptions":{"paths":{}}}"#,
            r#"{"compilerOptions":{"baseUrl":"."}}"#,
            r#"{"compilerOptions":{"jsx":"react-jsx"}}"#,
            r#"{"compilerOptions":{"experimentalDecorators":true}}"#,
            r#"{"compilerOptions":{"module":"CommonJS"}}"#,
            r#"{"compilerOptions":{"strict":"true"}}"#,
            r#"{"include":"src"}"#,
            r#"{"compilerOptions":{"module":"CommonJS","module":"NodeNext"}}"#,
            r#"{"compilerOptions":{},"compilerOptions":{}}"#,
            "{/* comments */}",
        ] {
            fs::write(root.path().join("tsconfig.json"), text)?;
            assert!(validate(root.path()).is_err(), "{text}");
        }
        Ok(())
    }
}
