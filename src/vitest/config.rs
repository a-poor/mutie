//! Static recognition of coverage-reporter-only config; never execute project code.
use anyhow::{Context, Result, ensure};

pub const FILES: &[&str] = &[
    "vite.config.ts",
    "vite.config.js",
    "vitest.config.ts",
    "vitest.config.js",
];

pub fn validate(source: &str) -> Result<()> {
    ensure!(
        source.len() <= 64 * 1024,
        "coverage-only config exceeds 64 KiB"
    );
    let mut parser = tree_sitter::Parser::new();
    parser.set_language(&tree_sitter_javascript::LANGUAGE.into())?;
    let tree = parser
        .parse(source, None)
        .context("config parser returned no tree")?;
    ensure!(
        !tree.root_node().has_error(),
        "invalid coverage-only config syntax"
    );
    let mut tokens = Vec::new();
    collect(tree.root_node(), source, 0, &mut tokens)?;
    let prefix = [
        "import",
        "{",
        "defineConfig",
        "}",
        "from",
        "string:vitest/config",
        "export",
        "default",
        "defineConfig",
        "(",
        "{",
        "test",
        ":",
        "{",
        "coverage",
        ":",
        "{",
        "reporter",
        ":",
        "[",
    ];
    let suffix = ["]", "}", "}", "}", ")"];
    ensure!(
        tokens.starts_with(&prefix.iter().map(|s| s.to_string()).collect::<Vec<_>>())
            && tokens.ends_with(&suffix.iter().map(|s| s.to_string()).collect::<Vec<_>>())
            && tokens.len() > prefix.len() + suffix.len(),
        "only defineConfig({{test:{{coverage:{{reporter:[...]}}}}}}) imported from vitest/config is supported"
    );
    ensure!(
        tokens[prefix.len()..tokens.len() - suffix.len()]
            .iter()
            .all(|token| matches!(
                token.as_str(),
                "string:text" | "string:json" | "string:html" | "string:clover"
            )),
        "unsupported coverage reporter; supported literals: text, json, html, clover"
    );
    Ok(())
}

pub(super) fn collect(
    node: tree_sitter::Node<'_>,
    source: &str,
    depth: usize,
    tokens: &mut Vec<String>,
) -> Result<()> {
    ensure!(depth < 32, "coverage config nesting is too deep");
    match node.kind() {
        "comment" | "," | ";" => {}
        "string" => {
            let raw = &source[node.byte_range()];
            ensure!(
                raw.len() >= 2 && !raw.contains('\\'),
                "escaped config strings are unsupported"
            );
            tokens.push(format!("string:{}", &raw[1..raw.len() - 1]));
        }
        _ if node.child_count() == 0 => tokens.push(source[node.byte_range()].to_owned()),
        _ => {
            for child in node.children(&mut node.walk()) {
                collect(child, source, depth + 1, tokens)?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    const CONFIG: &str = "import { defineConfig } from 'vitest/config'; export default defineConfig({test:{coverage:{reporter:['text','json','html','clover'],},},});";
    #[test]
    fn accepts_only_static_coverage_reporter_shape() {
        validate(CONFIG).unwrap();
        validate(
            &CONFIG
                .replace(";", "; /* formatting */\n")
                .replace('\'', "\""),
        )
        .unwrap();
        for source in [
            format!("{CONFIG}\nthrow new Error('must not execute');"),
            CONFIG.replace("coverage:", "environment:'jsdom',coverage:"),
            CONFIG.replace("reporter:", "enabled:true,reporter:"),
            CONFIG.replace("'text'", "process.env.REPORTER"),
            CONFIG.replace("'text'", "'lcov'"),
            CONFIG.replace("defineConfig({", "defineConfig(()=>({"),
            CONFIG.replace("test:{", "...settings,test:{"),
            CONFIG.replace("test:{", "get test(){return {"),
            CONFIG.replace("defineConfig", "other"),
            CONFIG.replace("'vitest/config'", "'vite'"),
            CONFIG.replace("['text','json','html','clover']", "[]"),
        ] {
            assert!(validate(&source).is_err(), "{source}");
        }
    }
}
