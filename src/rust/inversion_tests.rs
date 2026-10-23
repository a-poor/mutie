use super::*;

fn scan_source(source: &str) -> Result<(tempfile::TempDir, Vec<Mutation>)> {
    let temp = tempfile::tempdir_in(std::env::temp_dir().canonicalize()?)?;
    fs::create_dir(temp.path().join("src"))?;
    fs::write(temp.path().join("src/main.rs"), source)?;
    let found = discover(
        temp.path(),
        &["src/main.rs".into()],
        &Scope::new(&[], &[])?,
        &Cancellation::default(),
    )?;
    Ok((temp, found.mutations))
}

#[test]
fn inversion_and_negation_preserve_recipes_exclusions_and_shared_spans() -> Result<()> {
    let source = "fn f(a: bool,b: bool) {\r\n\
        let π = (a==b,a!=b,a<b,a<=b,a>b,a>=b);\r\n\
        let negated = ! /*keep*/a; let nested = !!b; let bits = !42u8;\r\n\
        let borrow = &!a; let minus = -1; let deref = *&a;\r\n\
        let closure = || !a; match a { x if !x => (), _ => () };\r\n\
        let text = \"! == !=\"; let call = stringify!(!a);\r\n\
        const C: bool = !false; let c = const { !false }; let n = [0; (!0u8) as usize];\r\n\
        let g = g::<{!false}>();\r\n\
        #[cfg(unix)] let skipped = !a;\r\n\
    }\r\n\
    fn never()->! { loop {} }\r\n\
    #[test] fn test_only() { let x = !true; }\r\n";
    let (temp, found) = scan_source(source)?;
    let comparisons: Vec<_> = found
        .iter()
        .filter(|m| m.operator == Operator::ComparisonInvert)
        .collect();
    assert_eq!(
        comparisons
            .iter()
            .map(|m| (m.original(), m.replacement.as_ref()))
            .collect::<Vec<_>>(),
        [
            ("==", "!="),
            ("!=", "=="),
            ("<", ">="),
            ("<=", ">"),
            (">", "<="),
            (">=", "<")
        ]
    );
    for m in comparisons.iter().skip(2) {
        assert_eq!(found.iter().filter(|n| n.range == m.range).count(), 2);
        assert!(
            found
                .iter()
                .any(|n| n.range == m.range && n.operator == Operator::ComparisonBoundary)
        );
    }
    let negations: Vec<_> = found
        .iter()
        .filter(|m| m.operator == Operator::NegationRemove)
        .collect();
    assert_eq!(negations.len(), 7);
    assert_eq!(negations[0].replacement, "( /*keep*/a,).0");
    assert_eq!(negations[1].replacement, "(!b,).0");
    assert_eq!(negations[2].replacement, "(b,).0");
    for m in &negations {
        assert!(valid_negation_replacement(m.original(), &m.replacement));
        assert!(!valid_negation_replacement(m.original(), "(a)"));
        assert!(!valid_negation_replacement(m.original(), "a"));
    }
    for original in ["a", "-a", "call!()", "!a; let _ = !b", "!a /*trailing*/"] {
        assert!(!valid_negation_replacement(original, "(a,).0"));
    }
    let mut parser = Parser::new();
    parser.set_language(&tree_sitter_rust::LANGUAGE.into())?;
    for m in found.iter().filter(|m| {
        matches!(
            m.operator,
            Operator::ComparisonInvert | Operator::NegationRemove
        )
    }) {
        m.apply(temp.path())?;
        let changed = fs::read_to_string(temp.path().join("src/main.rs"))?;
        assert!(
            !parser
                .parse(&changed, None)
                .unwrap()
                .root_node()
                .has_error(),
            "{changed}"
        );
        assert_eq!(&changed[..m.range.start], &source[..m.range.start]);
        assert_eq!(
            &changed[m.range.start + m.replacement.len()..],
            &source[m.range.end..]
        );
        fs::write(temp.path().join("src/main.rs"), source)?;
    }
    let repeated = scan_source(source)?.1;
    assert_eq!(
        found
            .iter()
            .map(|m| (m.range.clone(), m.operator))
            .collect::<Vec<_>>(),
        repeated
            .iter()
            .map(|m| (m.range.clone(), m.operator))
            .collect::<Vec<_>>()
    );
    Ok(())
}

async fn compile_run(root: &Path) -> Result<()> {
    for (command, args) in [
        (
            "rustc",
            vec![
                "--edition=2024",
                "-D",
                "warnings",
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
            root,
            std::time::Duration::from_secs(30),
            std::num::NonZeroUsize::new(65536),
            &Cancellation::default(),
        )
        .await?;
        ensure!(
            output.status.success() && !output.timed_out && output.capture_error.is_none(),
            "{command}: {}",
            output.stderr.text()?
        );
    }
    Ok(())
}

#[tokio::test]
#[ignore = "compiles native Rust comparison inversion semantic controls"]
async fn native_comparisons_preserve_borrowing_dispatch_order_and_unordered_values() -> Result<()> {
    for (op, method, less, equal, nan) in [
        ("==", "ne", true, false, true),
        ("!=", "eq", false, true, false),
        ("<", "ge", false, true, false),
        ("<=", "gt", false, false, false),
        (">", "le", true, true, false),
        (">=", "lt", true, false, false),
    ] {
        let source = format!(
            r#"
use std::cell::RefCell;
use std::cmp::Ordering;
struct Value<'a>(&'static str,&'a RefCell<Vec<&'static str>>);
macro_rules! equality {{ () => {{
impl PartialEq for Value<'_> {{
 fn eq(&self,_:&Self)->bool {{ self.1.borrow_mut().push("eq");true }}
 fn ne(&self,_:&Self)->bool {{ self.1.borrow_mut().push("ne");true }}
}}
impl PartialOrd for Value<'_> {{
 fn partial_cmp(&self,_:&Self)->Option<Ordering> {{ panic!("no inferred comparison") }}
 fn lt(&self,_:&Self)->bool {{self.1.borrow_mut().push("lt");true}}
 fn le(&self,_:&Self)->bool {{self.1.borrow_mut().push("le");true}}
 fn gt(&self,_:&Self)->bool {{self.1.borrow_mut().push("gt");true}}
 fn ge(&self,_:&Self)->bool {{self.1.borrow_mut().push("ge");true}}
}}
}}; }} equality!();
fn generic<T:PartialOrd>(a:T,b:T)->(bool,T,T) {{ let result=a {op} b; (result,a,b) }}
fn float(a:f64,b:f64)->bool {{ a {op} b }}
fn make<'a>(name:&'static str,trace:&'a RefCell<Vec<&'static str>>)->Value<'a> {{trace.borrow_mut().push(name);Value(name,trace)}}
fn main() {{
 assert_eq!(float(1.0,2.0),{less}); assert_eq!(float(-0.0,0.0),{equal});
 assert_eq!(float(f64::NAN,1.0),{nan}); assert_eq!(float(1.0,f64::NAN),{nan});
 assert_eq!(float(f64::NEG_INFINITY,f64::INFINITY),{less});
 let (answer,left,right)=generic(String::from("a"),String::from("b"));
 assert_eq!(answer,{less}); assert_eq!(left,"a");assert_eq!(right,"b");
 let trace=RefCell::new(Vec::new());
 let (answer,left,right)=generic(make("left",&trace),make("right",&trace));
 assert!(answer);assert_eq!(left.0,"left");assert_eq!(right.0,"right");
 assert_eq!(*trace.borrow(),["left","right","{method}"]);
}}
"#
        );
        let (temp, found) = scan_source(&source)?;
        let mutations: Vec<_> = found
            .iter()
            .filter(|m| m.operator == Operator::ComparisonInvert)
            .collect();
        assert_eq!(mutations.len(), 2);
        let mut changed = source.clone();
        for m in mutations.into_iter().rev() {
            changed.replace_range(m.range.clone(), &m.replacement);
        }
        fs::write(temp.path().join("src/main.rs"), changed)?;
        compile_run(temp.path()).await?;
    }
    Ok(())
}

#[tokio::test]
#[ignore = "compiles native Rust Boolean/integer/custom Not removal controls"]
async fn native_negation_removal_preserves_values_moves_and_single_evaluation() -> Result<()> {
    let source = r#"
use std::cell::{Cell,RefCell};
use std::ops::Not;
struct Value<'a>(&'static str,&'a RefCell<Vec<&'static str>>);
impl Drop for Value<'_> {fn drop(&mut self){self.1.borrow_mut().push("drop");}}
macro_rules! custom_not { () => {
impl Not for Value<'_> {type Output=Self;fn not(self)->Self {self.1.borrow_mut().push("not");self}}
impl Not for &Value<'_> {type Output=Self;fn not(self)->Self {self.1.borrow_mut().push("not");self}}
}; } custom_not!();
fn boolean(x:bool)->bool {!x}
fn integer(x:u8)->u8 {!x}
fn effects(f:impl FnOnce()->bool)->bool {!/* retain */f()}
fn owned(x:Value<'_>)->Value<'_> {!x}
fn borrowed<'a,'b>(x:&'a Value<'b>)->&'a Value<'b> {!x}
fn consume(x:&Value<'_>) {assert_eq!(x.0,"x");}
fn borrow_context(x:Value<'_>,trace:&RefCell<Vec<&'static str>>) {consume(&!x);trace.borrow_mut().push("after");}
fn main() {
 assert!(boolean(true));assert!(!boolean(false));assert_eq!(integer(42),42);
 let calls=Cell::new(0);assert!(effects(||{calls.set(1);true}));assert_eq!(calls.get(),1);
 let trace=RefCell::new(Vec::new());
 let x=owned(Value("x",&trace));assert_eq!(x.0,"x");assert!(trace.borrow().is_empty());
 assert_eq!(borrowed(&x).0,"x");assert!(trace.borrow().is_empty());
 drop(x);assert_eq!(*trace.borrow(),["drop"]);trace.borrow_mut().clear();
 borrow_context(Value("x",&trace),&trace);assert_eq!(*trace.borrow(),["drop","after"]);
}
"#;
    let (temp, found) = scan_source(source)?;
    let mutations: Vec<_> = found
        .iter()
        .filter(|m| m.operator == Operator::NegationRemove)
        .collect();
    assert_eq!(mutations.len(), 6);
    let mut changed = source.to_owned();
    for m in mutations.into_iter().rev() {
        changed.replace_range(m.range.clone(), &m.replacement);
    }
    fs::write(temp.path().join("src/main.rs"), changed)?;
    compile_run(temp.path()).await?;
    let source = "fn run(x:bool)->bool {!!x}\nfn main(){assert!(!run(true));assert!(run(false));}";
    let (temp, found) = scan_source(source)?;
    let mutations: Vec<_> = found
        .iter()
        .filter(|m| m.operator == Operator::NegationRemove)
        .collect();
    assert_eq!(mutations.len(), 2);
    for m in mutations {
        m.apply(temp.path())?;
        compile_run(temp.path()).await?;
        fs::write(temp.path().join("src/main.rs"), source)?;
    }
    Ok(())
}
