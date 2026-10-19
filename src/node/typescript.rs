//! Native stripping only: configuration is audited and fingerprinted, never executed.
use anyhow::{Context, Result, ensure};
use std::{fs, path::Path};

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TypeScriptScope {
    pub test_pattern: TestPattern,
    pub tsconfig_fingerprint: Option<String>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum TestPattern {
    #[serde(rename = "native-default")]
    Default,
    #[serde(rename = "test/*.test.js")]
    JavaScript,
    #[serde(rename = "test/*.test.ts")]
    TypeScript,
}
impl TestPattern {
    pub fn glob(self) -> &'static str {
        match self {
            Self::Default => super::discovery::PATTERN,
            Self::JavaScript => "test/*.test.js",
            Self::TypeScript => "test/*.test.ts",
        }
    }
    pub fn suffix(self) -> &'static str {
        match self {
            Self::Default => unreachable!("default discovery does not use a suffix"),
            Self::JavaScript => ".test.js",
            Self::TypeScript => ".test.ts",
        }
    }
}
impl TypeScriptScope {
    pub fn load(root: &Path, test_pattern: TestPattern) -> Result<Self> {
        let path = root.join("tsconfig.json");
        let tsconfig_fingerprint = match fs::symlink_metadata(&path) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            metadata => {
                ensure!(metadata?.is_file(), "Node tsconfig must be a regular file");
                let bytes = fs::read(&path)?;
                let config: serde_json::Value = serde_json::from_slice(&bytes)
                    .context("Node scope supports only plain JSON tsconfig")?;
                let object = config.as_object().context("tsconfig must be an object")?;
                for (key, value) in object {
                    match key.as_str() {
                        "include" | "exclude" => ensure!(
                            value
                                .as_array()
                                .is_some_and(|a| a.iter().all(|v| v.is_string())),
                            "tsconfig {key} must be a string array"
                        ),
                        "compilerOptions" => validate_options(value)?,
                        _ => anyhow::bail!("unsupported Node tsconfig setting: {key}"),
                    }
                }
                Some(super::fingerprint(&bytes))
            }
        };
        Ok(Self {
            test_pattern,
            tsconfig_fingerprint,
        })
    }
    pub fn validate_record(&self) -> Result<()> {
        ensure!(
            self.tsconfig_fingerprint
                .as_deref()
                .is_none_or(super::valid_fingerprint),
            "invalid saved Node tsconfig fingerprint"
        );
        Ok(())
    }
}
fn validate_options(value: &serde_json::Value) -> Result<()> {
    let options = value
        .as_object()
        .context("compilerOptions must be an object")?;
    for (key, value) in options {
        let supported = match key.as_str() {
            "module" | "moduleResolution" => value
                .as_str()
                .is_some_and(|s| s.eq_ignore_ascii_case("nodenext")),
            "target" => value
                .as_str()
                .is_some_and(|s| matches!(s.to_ascii_lowercase().as_str(), "es2023" | "esnext")),
            "types" | "lib" => value
                .as_array()
                .is_some_and(|a| a.iter().all(|v| v.is_string())),
            "noEmit"
            | "allowImportingTsExtensions"
            | "erasableSyntaxOnly"
            | "verbatimModuleSyntax" => value == true,
            "strict"
            | "skipLibCheck"
            | "noUncheckedIndexedAccess"
            | "exactOptionalPropertyTypes"
            | "noImplicitOverride"
            | "noUnusedLocals"
            | "noUnusedParameters"
            | "forceConsistentCasingInFileNames"
            | "allowJs"
            | "checkJs" => value.is_boolean(),
            _ => false,
        };
        ensure!(
            supported,
            "unsupported Node tsconfig compiler option: {key}; native execution strips types without applying tsconfig"
        );
    }
    Ok(())
}

pub fn validate_syntax(node: tree_sitter::Node<'_>, file: &Path) -> Result<()> {
    let kind = node.kind();
    ensure!(
        !matches!(
            kind,
            "enum_declaration"
                | "internal_module"
                | "module"
                | "ambient_declaration"
                | "import_alias"
                | "import_require_clause"
                | "export_assignment"
                | "type_assertion"
                | "decorator"
                | "jsx_element"
                | "jsx_self_closing_element"
        ),
        "unsupported native Node TypeScript {kind} in {}",
        file.display()
    );
    if matches!(kind, "required_parameter" | "optional_parameter") {
        let mut cursor = node.walk();
        ensure!(
            !node.children(&mut cursor).any(|n| matches!(
                n.kind(),
                "accessibility_modifier" | "readonly" | "override_modifier"
            )),
            "Node type stripping cannot execute parameter properties in {}",
            file.display()
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn audits_configuration_without_interpreting_it() {
        let root = tempfile::tempdir().unwrap();
        let load = || TypeScriptScope::load(root.path(), TestPattern::TypeScript);
        assert_eq!(load().unwrap().tsconfig_fingerprint, None);
        for config in [
            r#"{"extends":"./base.json"}"#,
            r#"{"references":[]}"#,
            r#"{"compilerOptions":{"paths":{}}}"#,
            r#"{"compilerOptions":{"module":"commonjs"}}"#,
            r#"{"compilerOptions":{"noEmit":false}}"#,
            r#"{"compilerOptions":{"experimentalDecorators":true}}"#,
            r#"{"compilerOptions":{"rewriteRelativeImportExtensions":true}}"#,
            r#"{"include":"src"}"#,
            "{/*comment*/}",
        ] {
            fs::write(root.path().join("tsconfig.json"), config).unwrap();
            assert!(load().is_err(), "{config}");
        }
        let config = include_bytes!("../../tests/fixtures/node-typescript/tsconfig.json");
        fs::write(root.path().join("tsconfig.json"), config).unwrap();
        let saved = load().unwrap();
        saved.validate_record().unwrap();
        fs::write(
            root.path().join("tsconfig.json"),
            [config.as_slice(), b"\n"].concat(),
        )
        .unwrap();
        assert_ne!(load().unwrap(), saved);
        fs::remove_file(root.path().join("tsconfig.json")).unwrap();
        assert_ne!(load().unwrap(), saved);
        #[cfg(unix)]
        {
            let original = root.path().join("config.json");
            fs::write(&original, config).unwrap();
            std::os::unix::fs::symlink(original, root.path().join("tsconfig.json")).unwrap();
            assert!(load().is_err());
        }
    }
}
