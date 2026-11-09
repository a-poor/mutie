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
            .filter(|m| m.operator == Operator::NumberReplace)
            .collect(),
    ))
}

#[test]
fn numeric_sites_preserve_spans_types_and_exclusions() -> Result<()> {
    let source = r#"
const C: usize = 90;
static S: usize = 91;
type A = [u8; 92];
fn f<const N: usize>() {
    // 🦀
    let values = (0, -0, - /* comment */ 128i8, 0xff32, 0x_0u64, 1_2usize,
        0.0, -0.0f32, 1f64, 1e-100, 1e-100f64, 1e-100f32);
    let x: [u8; 93] = [1; 94];
    let x = const { 95 };
    let y = x.0;
    let z = Tuple { 0: 2 };
    match x { 96 => 3, _ if x < 4 => 5, _ => 6 }
    let 97 = x;
    call::<98>();
    macro_call!(99);
    #[cfg(test)] let excluded = 100;
    let closure = || 7;
    let text = "101";
    let byte = b'9';
}
#[test] fn test_only() { let x = 102; }
#[cfg(test)] mod tests { fn f() { let x = 103; } }
"#
    .replace('\n', "\r\n");
    let (root, found) = scan(&source)?;
    assert_eq!(
        found.iter().map(Mutation::original).collect::<Vec<_>>(),
        [
            "0",
            "-0",
            "- /* comment */ 128i8",
            "0xff32",
            "0x_0u64",
            "1_2usize",
            "0.0",
            "-0.0f32",
            "1f64",
            "1e-100f64",
            "1e-100f32",
            "1",
            "2",
            "3",
            "4",
            "5",
            "6",
            "7"
        ]
    );
    assert_eq!(
        found
            .iter()
            .map(|m| m.replacement.as_ref())
            .collect::<Vec<_>>(),
        [
            "1", "1", "0i8", "0", "1u64", "0usize", "1.0", "1.0f32", "0.0f64", "0.0f64", "1.0f32",
            "0", "0", "0", "0", "0", "0", "0"
        ]
    );
    for mutation in found {
        assert!(
            valid_number_replacement(mutation.original(), &mutation.replacement),
            "{}",
            mutation.original()
        );
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
fn numeric_saved_recipes_reject_injected_code_noops_and_changed_types() {
    for original in [
        "True",
        "true",
        "b'0'",
        "0xffu256",
        "1foo",
        "1.0f128",
        "1e-100",
        "-1e-100",
        "(1)",
        "1 + 2",
        "1; f()",
        " 1",
        "1 ",
        "1 // tail",
    ] {
        assert!(!valid_number_replacement(original, "0"), "{original}");
        assert!(!valid_number_replacement(original, "0.0"), "{original}");
    }
    for (original, wrong) in [
        ("0", "0"),
        ("1", "1"),
        ("1u8", "0"),
        ("1.0", "0"),
        ("1f32", "0.0f64"),
        ("1e-100f32", "0.0f32"),
        ("1u8", "(0u8)"),
    ] {
        assert!(
            !valid_number_replacement(original, wrong),
            "{original} -> {wrong}"
        );
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
#[ignore = "compiles original/mutant Rust numeric types, boundaries, signs and syntax"]
async fn native_numeric_values_and_types() -> Result<()> {
    let cases = [
        ("0", "i32", "0", "1"),
        ("-0", "i32", "0", "1"),
        ("-(0i32)", "i32", "0", "-1"),
        ("- /* comment */ (0.0f64)", "f64", "-0.0", "-1.0"),
        ("-(-0.0f32)", "f32", "0.0", "-1.0"),
        ("- /* 🦀 */ 128i8", "i8", "i8::MIN", "0"),
        ("-32768i16", "i16", "i16::MIN", "0"),
        ("-2147483648i32", "i32", "i32::MIN", "0"),
        ("-9223372036854775808i64", "i64", "i64::MIN", "0"),
        (
            "-170141183460469231731687303715884105728i128",
            "i128",
            "i128::MIN",
            "0",
        ),
        ("1isize", "isize", "1", "0"),
        ("255u8", "u8", "u8::MAX", "0"),
        ("65535u16", "u16", "u16::MAX", "0"),
        ("4294967295u32", "u32", "u32::MAX", "0"),
        ("18446744073709551615u64", "u64", "u64::MAX", "0"),
        (
            "340282366920938463463374607431768211455u128",
            "u128",
            "u128::MAX",
            "0",
        ),
        ("0x_0usize", "usize", "0", "1"),
        ("0xff32", "i32", "65330", "0"),
        ("0b1_0u8", "u8", "2", "0"),
        ("0o17u8", "u8", "15", "0"),
        ("0___u8", "u8", "0", "1"),
        ("1_2usize", "usize", "12", "0"),
        ("0.0", "f64", "0.0", "1.0"),
        ("-0.0f32", "f32", "-0.0", "1.0"),
        ("-0.0f64", "f64", "-0.0", "1.0"),
        ("1f32", "f32", "1.0", "0.0"),
        ("1_f64", "f64", "1.0", "0.0"),
        ("1.", "f64", "1.0", "0.0"),
        ("1_000.5_0f32", "f32", "1000.5", "0.0"),
        ("1e-100f32", "f32", "0.0", "1.0"),
        ("-1e-100f32", "f32", "-0.0", "1.0"),
        ("1e-100f64", "f64", "1e-100", "0.0"),
        ("7.006492321624085e-46f32", "f32", "0.0", "1.0"),
        (
            "7.006492321624086e-46f32",
            "f32",
            "f32::from_bits(1)",
            "0.0",
        ),
        ("2.4703282292062327e-324f64", "f64", "0.0", "1.0"),
        (
            "2.4703282292062328e-324f64",
            "f64",
            "f64::from_bits(1)",
            "0.0",
        ),
        ("9007199254740993f64", "f64", "9007199254740992.0", "0.0"),
        ("1e-999", "f32", "0.0", "1.0"),
        ("1e-999", "f64", "0.0", "1.0"),
        ("7u32.count_ones()", "u32", "3", "0"),
        ("1.5f64.floor()", "f64", "1.0", "0.0"),
    ];
    for (expression, ty, original, changed) in cases {
        let source = format!("fn chosen() -> {ty} {{ {expression} }}\n");
        let (root, found) = scan(&source)?;
        ensure!(
            found.len() == 1,
            "{expression}: {:?}",
            found.iter().map(Mutation::original).collect::<Vec<_>>()
        );
        for mutated in [false, true] {
            if mutated {
                found[0].apply(root.path())?;
            }
            let expected = if mutated { changed } else { original };
            let check = if ty.starts_with('f') {
                "assert_eq!(actual.to_bits(),expected.to_bits());"
            } else {
                "assert_eq!(actual,expected);"
            };
            let program = format!(
                "{}\nfn main(){{let actual: {ty}=chosen(); let expected: {ty}={expected}; {check}}}",
                fs::read_to_string(root.path().join("src/main.rs"))?
            );
            fs::write(root.path().join("probe.rs"), program)?;
            let compiled = execute(
                root.path(),
                "rustc",
                &["--edition=2024", "-Dwarnings", "probe.rs", "-o", "probe"],
            )
            .await?;
            ensure!(
                compiled.status.success(),
                "{expression} ({mutated}): {}",
                compiled.stderr.text()?
            );
            let out = execute(root.path(), "./probe", &[]).await?;
            ensure!(
                out.status.success(),
                "{expression} ({mutated}): {}",
                out.stderr.text()?
            );
        }
    }
    Ok(())
}

#[tokio::test]
#[ignore = "compiles numeric division/index errors and verifies inferred-width exclusions"]
async fn native_numeric_compile_failures_and_inferred_widths() -> Result<()> {
    for (source, original, diagnostic) in [
        ("fn chosen()->u8 { 2 / 1 }", "1", "by zero"),
        ("fn chosen()->u8 { [7][0] }", "0", "index out of bounds"),
    ] {
        let (root, found) = scan(source)?;
        let m = found.iter().find(|m| m.original() == original).unwrap();
        let args = [
            "--edition=2024",
            "--crate-type=lib",
            "src/main.rs",
            "-o",
            "probe.rlib",
        ];
        let before = execute(root.path(), "rustc", &args).await?;
        ensure!(before.status.success(), "{}", before.stderr.text()?);
        m.apply(root.path())?;
        let after = execute(root.path(), "rustc", &args).await?;
        ensure!(
            !after.status.success() && after.stderr.text()?.contains(diagnostic),
            "{}",
            after.stderr.text()?
        );
    }
    let source = "fn small()->f32{1e-100} fn large()->f64{1e-100} fn main(){assert_eq!(small(),0.0);assert!(large()>0.0);}";
    let (root, found) = scan(source)?;
    assert!(found.is_empty()); // Macro bodies are excluded too.
    let before = execute(
        root.path(),
        "rustc",
        &["--edition=2024", "src/main.rs", "-o", "probe"],
    )
    .await?;
    ensure!(before.status.success(), "{}", before.stderr.text()?);
    ensure!(execute(root.path(), "./probe", &[]).await?.status.success());
    Ok(())
}
