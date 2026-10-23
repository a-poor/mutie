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
            .filter(|m| {
                matches!(
                    m.operator,
                    Operator::BranchForceTrue | Operator::BranchForceFalse
                )
            })
            .collect(),
    ))
}

#[test]
fn branch_spans_exclusions_and_saved_recipes() -> Result<()> {
    let source = "fn f(a:bool,b:bool,x:Option<u8>) {\r\n// 🦀\r\nif a /*keep*/ && b { } else if (a) {}\r\nif true {} if ((/*literal*/false)) {}\r\nif let Some(v)=x {if v>0 {}}\r\nif let Some(v)=x && v>0 {}\r\nif matches!(x,Some(_)) {} while a {} let _ = const {if a {1}else{2}};\r\nlet _ = stringify!(if a {}); }\r\n#[test] fn t(){if x {}}\r\n";
    let (root, found) = scan(source)?;
    assert_eq!(found.len(), 8);
    assert_eq!(found[0].original(), "a /*keep*/ && b");
    assert_eq!(found[0].replacement, "true || (a /*keep*/ && b)");
    for m in &found {
        assert!(valid_branch_replacement(
            m.operator,
            m.original(),
            &m.replacement
        ));
        assert!(!valid_branch_replacement(m.operator, m.original(), "true"));
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
        "x {} if y",
        "x; injected()",
    ] {
        assert!(!valid_branch_replacement(
            Operator::BranchForceTrue,
            original,
            &format!("true || ({original})")
        ));
    }
    Ok(())
}

#[tokio::test]
#[ignore = "compiles native Rust branch-forcing ownership, borrowing and skipped-effect controls"]
async fn native_branches_preserve_ownership_and_skip_condition_effects() -> Result<()> {
    for (setup, condition, after) in [
        ("", "panic_condition()", ""),
        (
            "let mut value=7;",
            "{ value=9; value>0 }",
            "assert_eq!(value,7);",
        ),
        (
            "let a=String::from(\"a\");let b=String::from(\"b\");",
            "a==b",
            "assert_eq!(a,\"a\");assert_eq!(b,\"b\");",
        ),
        (
            "let value=Owned(trace.clone());",
            "consume(value)",
            "assert_eq!(*trace.borrow(),vec![\"body\"]);",
        ),
        (
            "let mut value=String::from(\"before\");",
            "change(&mut value)",
            "assert_eq!(value,\"before\");",
        ),
    ] {
        for operator in [Operator::BranchForceTrue, Operator::BranchForceFalse] {
            let answer = if operator == Operator::BranchForceTrue {
                1
            } else {
                2
            };
            let source = format!(
                r#"
use std::cell::RefCell;
use std::rc::Rc;
#[allow(dead_code)] struct Owned(Rc<RefCell<Vec<&'static str>>>);
impl Drop for Owned {{fn drop(&mut self){{self.0.borrow_mut().push("drop");}}}}
#[allow(dead_code)] fn panic_condition()->bool {{panic!("skipped")}}
#[allow(dead_code)] fn consume(_:Owned)->bool {{panic!("must not consume")}}
#[allow(dead_code)] fn change(v:&mut String)->bool {{v.clear();panic!("must not borrow/call")}}
fn main() {{
 let trace=Rc::new(RefCell::new(Vec::new()));
 {{ {setup}
 let answer=if {condition} {{trace.borrow_mut().push("body");1}}else{{trace.borrow_mut().push("body");2}};
 assert_eq!(answer,{answer}); {after}
 }}
 assert_eq!(trace.borrow()[0],"body");
 {drop_check}
}}
"#,
                drop_check = if condition == "consume(value)" {
                    "assert_eq!(*trace.borrow(),vec![\"body\",\"drop\"]);"
                } else {
                    "assert_eq!(*trace.borrow(),vec![\"body\"]);"
                }
            );
            let (root, found) = scan(&source)?;
            assert_eq!(found.len(), 2);
            found
                .iter()
                .find(|m| m.operator == operator)
                .unwrap()
                .apply(root.path())?;
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
    }
    Ok(())
}
