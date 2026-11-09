use super::*;

fn scan(source: &str) -> Result<(tempfile::TempDir, Vec<Mutation>)> {
    let root = tempfile::tempdir_in(std::env::temp_dir().canonicalize()?)?;
    fs::create_dir(root.path().join("src"))?;
    fs::write(root.path().join("src/main.rs"), source)?;
    let found = discover(
        root.path(),
        &["src/main.rs".into()],
        &Scope::new(&[], &[])?,
        &Cancellation::default(),
    )?;
    Ok((
        root,
        found
            .mutations
            .into_iter()
            .filter(|m| m.operator == Operator::StringReplace)
            .collect(),
    ))
}

#[test]
fn string_sites_preserve_spans_and_context_exclusions() -> Result<()> {
    let source = r###"
//! documentation
const C: &str = "const";
static S: &str = "static";
#[doc = "attribute"]
fn f() {
    // 🦀
    let values = ("value", r#"raw"#, "");
    let bytes = (b"bytes", br#"raw bytes"#, c"cstr", cr#"raw cstr"#, 'a', b'b');
    let c = const { "inline const" };
    let a: [u8; "type".len()] = [0; "length".len()];
    match x { "pattern" => "arm", y if y == "guard" => "guarded", _ => "fallback" }
    if let "if pattern" = x { call("argument"); }
    let "let pattern" = x;
    let closure = || "closure";
    let member = "member".len();
    macro_call!("macro");
    #[cfg(test)] let excluded = "cfg";
}
#[test] fn test_only() { let value = "test"; }
#[cfg(test)] mod tests { fn f() { let value = "test mod"; } }
"###
    .replace('\n', "\r\n");
    let (root, found) = scan(&source)?;
    assert_eq!(
        found.iter().map(Mutation::original).collect::<Vec<_>>(),
        [
            "\"value\"",
            "r#\"raw\"#",
            "\"\"",
            "\"arm\"",
            "\"guard\"",
            "\"guarded\"",
            "\"fallback\"",
            "\"argument\"",
            "\"closure\"",
            "\"member\""
        ]
    );
    for mutation in found {
        assert!(valid_string_replacement(
            mutation.original(),
            &mutation.replacement
        ));
        crate::identity::MutationIdentity::from(&mutation).validate()?;
        mutation.apply(root.path())?;
        assert_eq!(
            fs::read_to_string(root.path().join("src/main.rs"))?,
            format!(
                "{}{}{}",
                &source[..mutation.range.start],
                mutation.replacement,
                &source[mutation.range.end..]
            )
        );
        assert!(mutation.apply(root.path()).is_err());
        fs::write(root.path().join("src/main.rs"), &source)?;
    }
    Ok(())
}

#[test]
fn string_recipes_reject_invalid_literals_noops_and_injection() {
    for empty in [
        "\"\"",
        "r\"\"",
        "r###\"\"###",
        "\"\\\n\t \"",
        "\"\\\r\n \"",
        "\"\\\n \n\t\"",
        "\"\\\n\\\n\"",
    ] {
        assert_eq!(string::replacement(empty), Some("\"x\""), "{empty:?}");
    }
    for value in [
        "\"a\"",
        "\"\0\"",
        "\"🦀\"",
        "\"\r\n\"",
        r#""\0""#,
        r#""\x00""#,
        r#""\u{0_0_}""#,
        r#""\u{10FFFF}""#,
        r##"r#"\n"#"##,
        r###"r##""#"##"###,
    ] {
        assert_eq!(string::replacement(value), Some("\"\""), "{value:?}");
        assert!(!valid_string_replacement(value, "\"x\""));
    }
    for bad in [
        "",
        " \"a\"",
        "\"a\" ",
        "\"a\";f()",
        "\"a\" + \"b\"",
        "(\"a\")",
        "b\"\"",
        "br\"\"",
        "c\"\"",
        "cr\"\"",
        "'a'",
        "\"a\"suffix",
        "r\"a\"suffix",
        "\"",
        "r#\"\"",
        "\"\r\"",
        "r\"\r\"",
        r#""\q""#,
        r#""\x80""#,
        r#""\u{D800}""#,
        r#""\u{110000}""#,
        r#""\u{0000000}""#,
        r#""\u{_0}""#,
        r#""\u{}""#,
        r#""\u{z}""#,
    ] {
        assert_eq!(string::replacement(bad), None, "{bad:?}");
    }
    let huge = format!("r{}\"\"{}", "#".repeat(256), "#".repeat(256));
    assert_eq!(string::replacement(&huge), None);
    for wrong in ["r\"\"", "(\"\")", "\"a\"", "b\"\"", "String::new()"] {
        assert!(!valid_string_replacement("\"a\"", wrong));
    }
}

async fn execute(
    root: &Path,
    program: &str,
    args: &[&str],
) -> Result<crate::process::CommandOutput> {
    let out = crate::process::run(
        program,
        args,
        root,
        std::time::Duration::from_secs(30),
        std::num::NonZeroUsize::new(65536),
        &Cancellation::default(),
    )
    .await?;
    ensure!(
        !out.timed_out && out.capture_error.is_none(),
        "{program} exceeded its bound"
    );
    Ok(out)
}

#[tokio::test]
#[ignore = "compiles original/mutant Rust strings, raw delimiters, escapes and continuation values"]
async fn native_string_values_and_types() -> Result<()> {
    let cases = [
        ("\"\"", ""),
        ("r\"\"", ""),
        ("r#\"\"#", ""),
        ("r###\"\"###", ""),
        ("\"text\"", "text"),
        ("\"🦀\"", "🦀"),
        ("\"\0\"", "\0"),
        (r#""\0""#, "\0"),
        (r#""\x00""#, "\0"),
        (r#""\x7F""#, "\x7f"),
        (r#""\u{000000}""#, "\0"),
        (r#""\u{10FFFF}""#, "\u{10ffff}"),
        (r#""\u{1f980}""#, "🦀"),
        (r#""\n\r\t\\\'\"""#, "\n\r\t\\'\""),
        ("\"a\nb\"", "a\nb"),
        ("\"a\r\nb\"", "a\nb"),
        ("\"\\\n\"", ""),
        ("\"\\\r\n\"", ""),
        ("\"\\\n \t\"", ""),
        ("\"\\\n \n \t\"", ""),
        ("\"\\\n\\\n\"", ""),
        ("\"a\\\n b\"", "ab"),
        ("\"\\\n\u{a0}\"", "\u{a0}"),
        ("\"\\\n\u{b}\"", "\u{b}"),
        ("\"\\\n\u{c}\"", "\u{c}"),
        ("r\"a\nb\"", "a\nb"),
        ("r\"a\r\nb\"", "a\nb"),
        ("r\"\0\"", "\0"),
        (r##"r#"\n"#"##, "\\n"),
        (r###"r##"a"#b"##"###, "a\"#b"),
        ("\"text\".trim()", "text"),
        ("\"\".trim()", ""),
    ];
    for newline in ["\n", "\r\n"] {
        let mut source = String::new();
        for (i, (expression, _)) in cases.iter().enumerate() {
            source.push_str(&format!(
                "fn value_{i}() -> &'static str {{ {expression} }}\n"
            ));
        }
        source = source.replace("\r\n", "\n").replace('\n', newline);
        let (root, found) = scan(&source)?;
        ensure!(
            found.len() == cases.len(),
            "expected {} sites, got {}",
            cases.len(),
            found.len()
        );
        for mutated in [false, true] {
            let mut code = source.clone();
            if mutated {
                for m in found.iter().rev() {
                    ensure!(
                        valid_string_replacement(m.original(), &m.replacement),
                        "invalid recipe"
                    );
                    code.replace_range(m.range.clone(), &m.replacement);
                }
            }
            code.push_str("\nfn main() {\n");
            for (i, (_, value)) in cases.iter().enumerate() {
                let expected = if mutated {
                    if value.is_empty() { "x" } else { "" }
                } else {
                    value
                };
                code.push_str(&format!("let actual: &'static str = value_{i}(); assert_eq!(actual, {expected:?}, \"case {i}\");\n"));
            }
            code.push_str("}\n");
            fs::write(root.path().join("probe.rs"), code)?;
            let compiled = execute(
                root.path(),
                "rustc",
                &["--edition=2024", "probe.rs", "-o", "probe"],
            )
            .await?;
            ensure!(
                compiled.status.success(),
                "{mutated}: {}",
                compiled.stderr.text()?
            );
            let ran = execute(root.path(), "./probe", &[]).await?;
            ensure!(ran.status.success(), "{mutated}: {}", ran.stderr.text()?);
        }
    }
    Ok(())
}

#[tokio::test]
#[ignore = "checks native Unicode-escape grammar, raw delimiter limit and parser compatibility"]
async fn native_string_grammar_boundaries() -> Result<()> {
    let root = tempfile::tempdir()?;
    let max_raw = format!("r{}\"\"{}", "#".repeat(255), "#".repeat(255));
    for (expression, accepted) in [
        (r#""\u{0_0_}""#.to_owned(), true),
        (max_raw, true),
        (
            format!("r{}\"\"{}", "#".repeat(256), "#".repeat(256)),
            false,
        ),
        (r#""\u{D800}""#.to_owned(), false),
        (r#""\x80""#.to_owned(), false),
        (r#""\q""#.to_owned(), false),
        ("\"\r\"".to_owned(), false),
    ] {
        let source = format!("pub fn chosen() -> &'static str {{ {expression} }}");
        fs::write(root.path().join("probe.rs"), &source)?;
        let compiled = execute(
            root.path(),
            "rustc",
            &[
                "--edition=2024",
                "--crate-type=lib",
                "probe.rs",
                "-o",
                "probe.rlib",
            ],
        )
        .await?;
        ensure!(
            compiled.status.success() == accepted,
            "{expression:?}: {}",
            compiled.stderr.text()?
        );
        assert_eq!(string::replacement(&expression).is_some(), accepted);
        if expression.contains('_') {
            // The pinned Tree-sitter grammar does not admit underscores in
            // Unicode escapes; preserve its source-level parse rejection.
            assert!(scan(&source).is_err());
        } else if accepted {
            let (_, found) = scan(&source)?;
            assert_eq!(found.len(), 1);
            assert_eq!(found[0].replacement, "\"x\"");
        }
    }
    Ok(())
}
