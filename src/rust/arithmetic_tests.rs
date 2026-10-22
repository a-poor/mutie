use super::*;

#[test]
fn arithmetic_edits_only_binary_tokens_and_preserves_original_bytes() -> Result<()> {
    let temp = tempfile::tempdir_in(std::env::temp_dir().canonicalize()?)?;
    fs::create_dir(temp.path().join("src"))?;
    let source = "fn f(a: i32, b: i32, c: i32) {\r\n\
        let π = a /* + */ + b - c;\r\n\
        let mixed = a * b / c % b; let comment = a*/*right*/b; let line = a*//right\n b;\r\n\
        let closure = || a + b;\r\n\
        match a { n if n - b > c => (), _ => () }\r\n\
        let negative = -a; let deref = *&a;\r\n\
        let mut x = a; x += b; x -= b; x *= b; x /= b; x %= b;\r\n\
        let shifts = (a << b, a >> b); let text = \"a+b*c\";\r\n\
        assert_eq!(a+b, c); const C: i32 = 1 + 2;\r\n\
        let const_block = const { 1 + 2 }; let array = [0; 1 + 2];\r\n\
        let generic = g::<{1 + 2}>();\r\n\
        #[cfg(unix)] let excluded = a + b;\r\n\
    }\r\n\
    static S: i32 = 1 + 2;\r\n\
    #[test] fn test_only() { let x = 1 + 2; }\r\n";
    fs::write(temp.path().join("src/main.rs"), source)?;
    let found = discover(
        temp.path(),
        &["src/main.rs".into()],
        &Scope::new(&[], &[])?,
        &Cancellation::default(),
    )?;
    let arithmetic: Vec<_> = found
        .mutations
        .iter()
        .filter(|m| m.operator == Operator::ArithmeticReplace)
        .collect();
    assert_eq!(
        arithmetic
            .iter()
            .map(|m| (m.original(), m.replacement.as_ref()))
            .collect::<Vec<_>>(),
        [
            ("+", "-"),
            ("-", "+"),
            ("*", "/"),
            ("/", "*"),
            ("%", "*"),
            ("*", "/ "),
            ("*", "/ "),
            ("+", "-"),
            ("-", "+")
        ]
    );
    let mut parser = Parser::new();
    parser.set_language(&tree_sitter_rust::LANGUAGE.into())?;
    for m in arithmetic {
        m.apply(temp.path())?;
        let changed = fs::read_to_string(temp.path().join("src/main.rs"))?;
        assert!(
            !parser
                .parse(&changed, None)
                .unwrap()
                .root_node()
                .has_error()
        );
        assert_eq!(&changed[..m.range.start], &source[..m.range.start]);
        assert_eq!(
            &changed[m.range.start + m.replacement.len()..],
            &source[m.range.end..]
        );
        assert_eq!(m.range.len(), 1);
        fs::write(temp.path().join("src/main.rs"), source)?;
    }
    Ok(())
}

#[tokio::test]
#[ignore = "compiles and runs native Rust semantic controls"]
async fn native_arithmetic_preserves_evaluation_moves_borrows_and_numeric_semantics() -> Result<()>
{
    for (op, replacement, integer, negative, float, overflow, zero) in [
        (
            "+",
            "-",
            5,
            -9,
            5.0,
            "i(i32::MIN, 1)",
            "assert_eq!(i(1,0),1); assert!(f(-0.0,0.0).is_sign_negative());",
        ),
        (
            "-",
            "+",
            9,
            -5,
            9.0,
            "i(i32::MAX, 1)",
            "assert_eq!(i(1,0),1); assert!(!f(-0.0,0.0).is_sign_negative());",
        ),
        (
            "*",
            "/",
            3,
            -3,
            3.5,
            "i(i32::MIN, -1)",
            "assert!(std::panic::catch_unwind(||i(1,0)).is_err()); assert_eq!(f(1.0,0.0),f64::INFINITY); assert!(f(0.0,0.0).is_nan());",
        ),
        (
            "/",
            "*",
            14,
            -14,
            14.0,
            "i(i32::MAX, 2)",
            "assert_eq!(i(1,0),0); assert!(f(-0.0,0.0).is_sign_negative());",
        ),
        (
            "%",
            "*",
            14,
            -14,
            14.0,
            "i(i32::MAX, 2)",
            "assert_eq!(i(1,0),0); assert!(f(-0.0,0.0).is_sign_negative());",
        ),
    ] {
        let temp = tempfile::tempdir_in(std::env::temp_dir().canonicalize()?)?;
        fs::create_dir(temp.path().join("src"))?;
        let gap = if op == "/" { " " } else { "" };
        let source = format!(
            r#"
use std::cell::RefCell;
use std::ops::*;
struct Value<'a>(&'static str, &'a RefCell<Vec<&'static str>>);
impl Drop for Value<'_> {{ fn drop(&mut self) {{ self.1.borrow_mut().push(self.0); }} }}
macro_rules! implementation {{ ($trait:ident, $method:ident, $token:literal) => {{
    impl $trait for Value<'_> {{ type Output = i32; fn $method(self, rhs:Self)->i32 {{ self.1.borrow_mut().push($token); drop(rhs); drop(self); 42 }} }}
    impl $trait for &Value<'_> {{ type Output = i32; fn $method(self, _rhs:Self)->i32 {{ self.1.borrow_mut().push($token); 42 }} }}
}}; }}
implementation!(Add, add, "+"); implementation!(Sub, sub, "-");
implementation!(Mul, mul, "*"); implementation!(Div, div, "/"); implementation!(Rem, rem, "%");
fn i(a:i32,b:i32)->i32 {{ a {op}{gap}/*right*/b }}
fn f(a:f64,b:f64)->f64 {{ a {op}{gap}/*right*/b }}
fn owned(a:Value<'_>,b:Value<'_>)->i32 {{ a {op}{gap}/*right*/b }}
fn borrowed(a:&Value<'_>,b:&Value<'_>)->i32 {{ a {op}{gap}/*right*/b }}
fn make<'a>(name:&'static str, trace:&'a RefCell<Vec<&'static str>>)->Value<'a> {{
    trace.borrow_mut().push(name); Value(name,trace)
}}
fn main() {{
    assert_eq!(i(7,2),{integer}); assert_eq!(i(-7,2),{negative}); assert_eq!(f(7.0,2.0),{float:?});
    {zero}
    assert!(f(f64::NAN,1.0).is_nan());
    assert!(std::panic::catch_unwind(||{overflow}).is_err());
    let trace=RefCell::new(Vec::new());
    assert_eq!(owned(make("left",&trace),make("right",&trace)),42);
    assert_eq!(*trace.borrow(),["left","right","{replacement}","right","left"]);
    trace.borrow_mut().clear();
    {{
        let left=make("left",&trace); let right=make("right",&trace);
        assert_eq!(borrowed(&left,&right),42);
        assert_eq!(*trace.borrow(),["left","right","{replacement}"]);
        assert_eq!(left.0,"left"); assert_eq!(right.0,"right");
    }}
    assert_eq!(*trace.borrow(),["left","right","{replacement}","right","left"]);
}}
"#
        );
        fs::write(temp.path().join("src/main.rs"), &source)?;
        let found = discover(
            temp.path(),
            &["src/main.rs".into()],
            &Scope::new(&[], &[])?,
            &Cancellation::default(),
        )?;
        let arithmetic: Vec<_> = found
            .mutations
            .iter()
            .filter(|m| m.operator == Operator::ArithmeticReplace)
            .collect();
        assert_eq!(arithmetic.len(), 4);
        // These disjoint one-byte edits exercise all four operand types per native
        // case. Scored workflows below still apply exactly one mutation at a time.
        let mut changed = source.clone();
        for m in arithmetic.into_iter().rev() {
            assert_eq!(m.replacement.trim_end(), replacement);
            changed.replace_range(m.range.clone(), &m.replacement);
        }
        fs::write(temp.path().join("src/main.rs"), changed)?;
        for (command, args) in [
            (
                "rustc",
                vec![
                    "--edition=2024",
                    "-C",
                    "overflow-checks=yes",
                    "src/main.rs",
                    "-o",
                    "probe",
                ],
            ),
            ("./probe", vec![]),
        ] {
            let output = crate::process::run(
                command,
                &args,
                temp.path(),
                std::time::Duration::from_secs(30),
                std::num::NonZeroUsize::new(65536),
                &Cancellation::default(),
            )
            .await?;
            ensure!(
                output.status.success() && !output.timed_out && output.capture_error.is_none(),
                "{op}/{command}: {}",
                output.stderr.text()?
            );
        }
    }
    Ok(())
}
