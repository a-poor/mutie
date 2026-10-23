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
            .filter(|m| m.operator == Operator::LoopSkip)
            .collect(),
    ))
}

#[test]
fn loop_spans_binding_exclusions_and_saved_recipes() -> Result<()> {
    let source = "fn f(a:bool,b:bool,x:Option<u8>) {\r\n// 🦀\r\nwhile a /*keep*/ && b {} while (a) {}\r\nwhile true {} while ((/*literal*/false)) {}\r\nwhile let Some(v)=x {while v>0 {}}\r\nwhile let Some(v)=x && v>0 {}\r\nwhile matches!(x,Some(_)) {} if a {} loop {break} for v in x {} let _ = const {while a {} 1};\r\nlet _ = stringify!(while a {}); }\r\n#[test] fn t(){while x {}}\r\n";
    let (root, found) = scan(source)?;
    assert_eq!(found.len(), 4);
    assert_eq!(found[0].original(), "a /*keep*/ && b");
    assert_eq!(found[0].replacement, "false && (a /*keep*/ && b)");
    for m in &found {
        assert!(valid_loop_replacement(m.original(), &m.replacement));
        assert!(!valid_loop_replacement(m.original(), "false"));
        m.apply(root.path())?;
        let changed = fs::read_to_string(root.path().join("src/main.rs"))?;
        assert_eq!(
            changed,
            format!(
                "{}{}{}",
                &source[..m.range.start],
                m.replacement,
                &source[m.range.end..]
            )
        );
        assert!(m.apply(root.path()).is_err());
        fs::write(root.path().join("src/main.rs"), source)?;
    }
    for original in [
        "true",
        "((false))",
        "let Some(x)=value",
        "let Some(x)=value && x>0",
        "x {} while y",
        "x; injected()",
    ] {
        assert!(!valid_loop_replacement(
            original,
            &format!("false && ({original})")
        ));
    }
    Ok(())
}

#[tokio::test]
#[ignore = "compiles Rust loop ownership, borrowing, drops and skipped-effect controls"]
async fn native_loops_preserve_ownership_and_skip_conditions() -> Result<()> {
    for (setup, condition, after, drops) in [
        ("", "panic_condition()", "", false),
        (
            "let mut value=7;",
            "{value=9;value>0}",
            "assert_eq!(value,7);",
            false,
        ),
        (
            "let a=String::from(\"a\");let b=String::from(\"b\");",
            "a==b",
            "assert_eq!(a,\"a\");assert_eq!(b,\"b\");",
            false,
        ),
        (
            "let value=Owned(trace.clone());",
            "consume(value)",
            "",
            true,
        ),
        (
            "let mut value=String::from(\"before\");",
            "change(&mut value)",
            "assert_eq!(value,\"before\");",
            false,
        ),
        ("", "consume(Owned(trace.clone()))", "", false),
    ] {
        let source = format!(
            r#"
use std::cell::RefCell; use std::rc::Rc;
#[allow(dead_code)] struct Owned(Rc<RefCell<Vec<&'static str>>>);
impl Drop for Owned {{fn drop(&mut self){{self.0.borrow_mut().push("drop");}}}}
#[allow(dead_code)] fn panic_condition()->bool {{panic!("skipped")}}
#[allow(dead_code)] fn consume(_:Owned)->bool {{panic!("must not consume")}}
#[allow(dead_code)] fn change(v:&mut String)->bool {{v.clear();panic!("must not borrow/call")}}
fn main() {{
 let trace=Rc::new(RefCell::new(Vec::new()));
 {{ {setup}
 while {condition} {{trace.borrow_mut().push("body");break;}}
 trace.borrow_mut().push("after"); {after}
 assert_eq!(*trace.borrow(),vec!["after"]);
 }}
 assert_eq!(*trace.borrow(),{expected});
}}
"#,
            expected = if drops {
                "vec![\"after\",\"drop\"]"
            } else {
                "vec![\"after\"]"
            }
        );
        let (root, found) = scan(&source)?;
        assert_eq!(found.len(), 1);
        found[0].apply(root.path())?;
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
            let out = crate::process::run(
                command,
                &args,
                root.path(),
                std::time::Duration::from_secs(30),
                std::num::NonZeroUsize::new(65536),
                &Cancellation::default(),
            )
            .await?;
            ensure!(
                out.status.success() && !out.timed_out && out.capture_error.is_none(),
                "{command}: {}",
                out.stderr.text()?
            );
        }
    }
    Ok(())
}
