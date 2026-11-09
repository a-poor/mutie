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
            .filter(|m| m.operator == Operator::NumberReplace)
            .collect(),
    ))
}

#[test]
fn numbers_have_one_exact_non_noop_recipe_and_exclude_syntax() -> Result<()> {
    for syntax in ["js", "ts"] {
        let source = "// 🦀\r\nexport const x = [0, -0, + /* sign */ 0, 7, 0x0, -0xff, 0b1_0, 0o0, .5, 1., 1e3, 1e-999, 1e999, 0n, -0n, 0x0n, 0x10n, 99999999999999999999999999999999999999n];\r\nconst keys={1:2, [3]:4}; class C { 5=6; [7]=8; 9(){return 10;} } const value=keys[11]; const string='12'; const template=`13`; const regex=/14/; const invalid=+1n;";
        let (root, found) = scan(source, syntax)?;
        assert_eq!(
            found.len(),
            24,
            "{syntax}: {:?}",
            found.iter().map(Mutation::original).collect::<Vec<_>>()
        );
        assert_eq!(
            found.iter().map(Mutation::original).collect::<Vec<_>>(),
            [
                "0",
                "-0",
                "+ /* sign */ 0",
                "7",
                "0x0",
                "-0xff",
                "0b1_0",
                "0o0",
                ".5",
                "1.",
                "1e3",
                "1e-999",
                "1e999",
                "0n",
                "-0n",
                "0x0n",
                "0x10n",
                "99999999999999999999999999999999999999n",
                "2",
                "4",
                "6",
                "8",
                "10",
                "11"
            ]
        );
        for m in found {
            assert!(valid_replacement(
                m.operator,
                m.original(),
                &m.replacement,
                syntax == "ts"
            ));
            assert!(!valid_replacement(
                Operator::ArithmeticReplace,
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
            fs::write(root.path().join(format!("src/value.{syntax}")), source)?;
        }
    }
    let (_, found) = scan(
        "type N=1|2n; interface I{ 3:4; fn(x:5):6; } const x:7=8; function f<T extends 9>(a:10=11):12{return 13 as 14;} const y=15 satisfies 16; const z=<17>18;",
        "ts",
    )?;
    assert_eq!(
        found.iter().map(Mutation::original).collect::<Vec<_>>(),
        ["8", "11", "13", "15", "18"]
    );
    for original in [
        "00",
        "01",
        "08",
        "+1n",
        "NaN",
        "Infinity",
        "1 + 2",
        "(1)",
        "1; f()",
        "1 // tail",
        " 1",
        "1n+1n",
    ] {
        assert!(
            !valid_replacement(Operator::NumberReplace, original, "(0)", false),
            "{original}"
        );
    }
    for (original, wrong) in [
        ("0", "(0)"),
        ("1", "(1)"),
        ("1", "0"),
        ("1n", "(0)"),
        ("0n", "(1)"),
        ("1e-999", "(0)"),
        ("-0", "(0)"),
    ] {
        assert!(!valid_replacement(
            Operator::NumberReplace,
            original,
            wrong,
            false
        ));
    }
    Ok(())
}

async fn native(runtime: &str) -> Result<()> {
    let cases = [
        ("0", "0", "1"),
        ("-0", "-0", "1"),
        ("+0", "0", "1"),
        ("1", "1", "0"),
        ("-1", "-1", "0"),
        ("+ /*🦀*/ 7", "7", "0"),
        ("0x0", "0", "1"),
        ("0x1_f", "31", "0"),
        ("-0b10", "-2", "0"),
        ("0o17", "15", "0"),
        ("0X0", "0", "1"),
        (".5", "0.5", "0"),
        ("1.", "1", "0"),
        ("1_000.5_0", "1000.5", "0"),
        ("2.5e+2", "250", "0"),
        ("-1e-999", "-0", "1"),
        ("1e999", "Infinity", "0"),
        ("9007199254740993", "9007199254740992", "0"),
        ("5e-324", "Number.MIN_VALUE", "0"),
        ("2e-324", "0", "1"),
        ("2.4703282292062327e-324", "0", "1"),
        ("2.4703282292062328e-324", "Number.MIN_VALUE", "0"),
        ("0n", "0n", "1n"),
        ("-0n", "0n", "1n"),
        ("7n", "7n", "0n"),
        ("-7n", "-7n", "0n"),
        ("0x0n", "0n", "1n"),
        ("0x1_fn", "31n", "0n"),
        ("0b1_0n", "2n", "0n"),
        ("0o17n", "15n", "0n"),
        (
            "99999999999999999999999999999n",
            "99999999999999999999999999999n",
            "0n",
        ),
        ("7..toString()", "'7'", "'0'"),
        ("0n.toString()", "'0'", "'1'"),
    ];
    for syntax in ["js", "ts"] {
        for (expression, expected, changed) in cases {
            let source = format!("export const chosen = {expression};\n");
            let (root, found) = scan(&source, syntax)?;
            assert_eq!(found.len(), 1, "{expression}");
            fs::write(root.path().join("package.json"), "{\"type\":\"module\"}")?;
            for mutated in [false, true] {
                if mutated {
                    found[0].apply(root.path())?;
                }
                let expected = if mutated { changed } else { expected };
                fs::write(
                    root.path().join(format!("check.{syntax}")),
                    format!(
                        "import assert from 'node:assert/strict'; import {{chosen}} from './src/value.{syntax}'; assert.ok(Object.is(chosen,{expected})); assert.equal(typeof chosen, typeof ({expected}));"
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
                    "{runtime} {syntax} {expression} ({mutated}): {}",
                    out.stderr.text()?
                );
            }
        }
    }
    Ok(())
}

#[tokio::test]
#[ignore = "requires pinned Node; original/mutant JS/TS numeric edge controls"]
async fn native_node_numbers() -> Result<()> {
    native("node").await
}
#[tokio::test]
#[ignore = "requires pinned Bun; original/mutant JS/TS numeric edge controls"]
async fn native_bun_numbers() -> Result<()> {
    native("bun").await
}
