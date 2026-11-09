use super::*;

fn scan(source: &str, syntax: &str) -> Result<(tempfile::TempDir, Vec<Mutation>)> {
    let root = tempfile::tempdir()?;
    fs::create_dir(root.path().join("src"))?;
    fs::write(root.path().join(format!("src/value.{syntax}")), source)?;
    let found = discover(
        root.path(),
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
fn strings_preserve_spans_and_exclude_nonvalue_contexts() -> Result<()> {
    for syntax in ["js", "ts"] {
        let source = r#"// 🦀
'use client';
import {value} from './other.js';
export {value as 'export name'} from './other.js';
export * from './other.js';
export const text = 'payload';
const empty = '';
const object = {'key':'value', ['computed']: 'computed value'};
class C { 'field name'='field value'; ['computed field']='other value'; 'method'(){'use strict';return 'method value';} }
const lookup=object['lookup'];
function f(){'use strict'; 'another directive'; ('wrapped statement'); void 0; 'ordinary statement'; return 'returned';}
f('argument');
const lazy=import('./dynamic.js');
const template=`raw ${'substitution'}`;
const tagged=String.raw`tag ${'tagged substitution'}`;
switch(text){case 'case value': f('case body');}
export default 'default value';
"#.replace('\n', "\r\n");
        let (root, found) = scan(&source, syntax)?;
        assert_eq!(
            found.iter().map(Mutation::original).collect::<Vec<_>>(),
            [
                "'payload'",
                "''",
                "'value'",
                "'computed value'",
                "'field value'",
                "'other value'",
                "'method value'",
                "'lookup'",
                "'returned'",
                "'argument'",
                "'case value'",
                "'case body'",
                "'default value'"
            ]
        );
        for m in found {
            crate::identity::MutationIdentity::from(&m).validate()?;
            assert!(valid_replacement(
                m.operator,
                m.original(),
                &m.replacement,
                syntax == "ts"
            ));
            assert!(!valid_replacement(
                Operator::NumberReplace,
                m.original(),
                &m.replacement,
                syntax == "ts"
            ));
            m.apply(root.path())?;
            assert_eq!(
                fs::read_to_string(root.path().join(format!("src/value.{syntax}")))?,
                format!(
                    "{}{}{}",
                    &source[..m.range.start],
                    m.replacement,
                    &source[m.range.end..]
                )
            );
            assert!(m.apply(root.path()).is_err());
            fs::write(root.path().join(format!("src/value.{syntax}")), &source)?;
        }
        fs::write(
            root.path().join(format!("src/ignored.test.{syntax}")),
            "const s='test';",
        )?;
        fs::write(
            root.path().join(format!("src/generated.{syntax}")),
            "// @generated\nconst s='generated';",
        )?;
        assert_eq!(
            discover(
                root.path(),
                &Scope::new(&[], &[])?,
                &Cancellation::default()
            )?
            .mutations
            .iter()
            .filter(|m| m.operator == Operator::StringReplace)
            .count(),
            13
        );
    }
    let (_, found) = scan(
        "type T='type'; interface I{'key':'type';} const a:'annotation'='initial'; function f<T extends 'bound'>(x:'param'='default'):'result'{return 'body' as 'cast';} const v='satisfied' satisfies 'type'; const q=<'asserted'>'value';",
        "ts",
    )?;
    assert_eq!(
        found.iter().map(Mutation::original).collect::<Vec<_>>(),
        ["'initial'", "'default'", "'body'", "'satisfied'", "'value'"]
    );
    for syntax in ["js", "ts"] {
        for (source, expected) in [
            ("import data from './data.json' with {type:'json'};", vec![]),
            ("export {data as 'name'};", vec![]),
            ("const {'key': value='default'}=data;", vec!["'default'"]),
            (
                "const object={['computed'+f('hidden')]: 'visible'};",
                vec!["'visible'"],
            ),
        ] {
            let (_, found) = scan(source, syntax).with_context(|| format!("{syntax}: {source}"))?;
            assert_eq!(
                found.iter().map(Mutation::original).collect::<Vec<_>>(),
                expected
            );
        }
        // The pinned grammars do not admit export attributes. Preserve the
        // existing parse rejection instead of admitting or mutating that syntax.
        assert!(scan("export * from './other.json' with {type:'json'};", syntax).is_err());
    }
    Ok(())
}

#[test]
fn string_recipes_use_values_not_source_length_and_reject_invalid_escapes() -> Result<()> {
    for syntax in ["js", "ts"] {
        for token in [
            "''",
            "\"\"",
            "'\\\n'",
            "'\\\r'",
            "'\\\r\n'",
            "'\\\u{2028}'",
            "'\\\u{2029}'",
            "'\\\n\\\r\n'",
        ] {
            assert!(
                valid_replacement(Operator::StringReplace, token, "(\"x\")", syntax == "ts"),
                "{token:?}"
            );
            let (_, found) = scan(&format!("export const s={token};"), syntax)?;
            assert_eq!(found.len(), 1);
            assert_eq!(&*found[0].replacement, "(\"x\")");
        }
        for token in [
            "'a'",
            r"'\0'",
            r"'\x00'",
            r"'\u0000'",
            r"'\u{0}'",
            r"'\uD800'",
            r"'\u{D800}'",
            r"'\u{10FFFF}'",
            r"'\q'",
            "'\0'",
            "'\u{2028}'",
            "'\u{2029}'",
            "'\\\nX'",
            r"'\\n'",
            r#"'\"'"#,
        ] {
            assert!(
                valid_replacement(Operator::StringReplace, token, "(\"\")", syntax == "ts"),
                "{token:?}"
            );
            assert!(!valid_replacement(
                Operator::StringReplace,
                token,
                "(\"x\")",
                syntax == "ts"
            ));
        }
        for token in [
            r"'\1'",
            r"'\07'",
            r"'\8'",
            r"'\9'",
            r"'\x0'",
            r"'\u123'",
            r"'\u{}'",
            r"'\u{110000}'",
            r"'\u{xyz}'",
            "'\n'",
            "'\r'",
            "`x`",
            "'a'+'b'",
            "'a'; f()",
            "('a')",
            " 'a'",
            "'a' ",
            "'a' // tail",
        ] {
            for changed in ["(\"\")", "(\"x\")"] {
                assert!(
                    !valid_replacement(Operator::StringReplace, token, changed, syntax == "ts"),
                    "{token:?}"
                );
            }
        }
        for wrong in ["''", "\"\"", "('')", "(\"x\")", "(0)"] {
            assert!(!valid_replacement(
                Operator::StringReplace,
                "'payload'",
                wrong,
                syntax == "ts"
            ));
        }
    }
    Ok(())
}

async fn native(runtime: &str) -> Result<()> {
    let cases = [
        ("''", "''", "'x'"),
        ("\"\"", "''", "'x'"),
        ("'abc'", "'abc'", "''"),
        ("\"abc\"", "'abc'", "''"),
        ("'🦀'", "String.fromCodePoint(0x1f980)", "''"),
        (r"'\u{1F980}'", "String.fromCodePoint(0x1f980)", "''"),
        (r"'\uD800'", "String.fromCharCode(0xd800)", "''"),
        (r"'\u{D800}'", "String.fromCharCode(0xd800)", "''"),
        (r"'\uDC00'", "String.fromCharCode(0xdc00)", "''"),
        (r"'\u{10ffff}'", "String.fromCodePoint(0x10ffff)", "''"),
        (r"'\0'", "String.fromCharCode(0)", "''"),
        ("'\0'", "String.fromCharCode(0)", "''"),
        (r"'\x00'", "String.fromCharCode(0)", "''"),
        (r"'\u0000'", "String.fromCharCode(0)", "''"),
        (r"'\u{0}'", "String.fromCharCode(0)", "''"),
        (
            r"'\n\r\t\b\f\v'",
            "String.fromCharCode(10,13,9,8,12,11)",
            "''",
        ),
        (r"'\q'", "'q'", "''"),
        (r"'\''", "String.fromCharCode(39)", "''"),
        (r#"'\"'"#, "String.fromCharCode(34)", "''"),
        (r"'\\'", "String.fromCharCode(92)", "''"),
        ("'\\\n'", "''", "'x'"),
        ("'\\\r'", "''", "'x'"),
        ("'\\\r\n'", "''", "'x'"),
        ("'\\\u{2028}'", "''", "'x'"),
        ("'\\\u{2029}'", "''", "'x'"),
        ("'\\\n\\\r\n'", "''", "'x'"),
        ("'a\\\r\nb'", "'ab'", "''"),
        (
            "'\u{2028}\u{2029}'",
            "String.fromCharCode(0x2028,0x2029)",
            "''",
        ),
        ("'abc'.length", "3", "0"),
        ("''.length", "0", "1"),
    ];
    for syntax in ["js", "ts"] {
        for (expression, expected, changed) in cases {
            let source = format!("export const chosen={expression};\n");
            let (root, found) = scan(&source, syntax)?;
            assert_eq!(found.len(), 1, "{expression:?}");
            fs::write(root.path().join("package.json"), "{\"type\":\"module\"}")?;
            for mutated in [false, true] {
                if mutated {
                    found[0].apply(root.path())?;
                }
                let expected = if mutated { changed } else { expected };
                fs::write(
                    root.path().join(format!("check.{syntax}")),
                    format!(
                        "import assert from 'node:assert/strict'; import {{chosen}} from './src/value.{syntax}'; assert.ok(Object.is(chosen,{expected})); assert.equal(typeof chosen,typeof ({expected}));"
                    ),
                )?;
                let out = crate::process::run(
                    runtime,
                    &[&format!("check.{syntax}")],
                    root.path(),
                    std::time::Duration::from_secs(10),
                    std::num::NonZeroUsize::new(65536),
                    &Cancellation::default(),
                )
                .await?;
                ensure!(
                    out.status.success() && !out.timed_out && out.capture_error.is_none(),
                    "{runtime} {syntax} {expression:?} ({mutated}): {}",
                    out.stderr.text()?
                );
            }
        }
    }
    Ok(())
}

#[tokio::test]
#[ignore = "requires pinned Node; original/mutant JS/TS string values and escaping"]
async fn native_node_strings() -> Result<()> {
    native("node").await
}
#[tokio::test]
#[ignore = "requires pinned Bun; original/mutant JS/TS string values and escaping"]
async fn native_bun_strings() -> Result<()> {
    native("bun").await
}
