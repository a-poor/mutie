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
            .filter(|m| m.operator == Operator::CallRemove)
            .collect(),
    ))
}

#[test]
fn calls_require_complete_standalone_statements_and_exact_saved_edits() -> Result<()> {
    let source = r#"
fn f() {
    // 🦀 comments and trivia stay outside the selected statement
    plain /*inside*/ ();
    (obj.method::<u8>(argument()));
    <Thing as Trait>::method(&thing);
    factory()(1);
    (|| 7)();
    if flag { nested(); } else { other(); }
    let value = used(); assigned = used(); return used();
}
fn excluded() {
    awaited().await; propagated()?;
    take(awaited().await); take(propagated()?);
    take({return}); take({break}); take({continue});
    take(format!("x")); macro_call!();
    #[cfg(test)] hidden();
    let _ = const { const_call(); 1 };
    let _ = stringify!(tokens());
    tail()
}
#[test] fn test_only() { test_call(); }
#[cfg(test)] mod tests { fn f() { test_call(); } }
const C: () = const_call();
"#
    .replace('\n', "\r\n");
    let (root, found) = scan(&source)?;
    assert_eq!(
        found.iter().map(|m| m.original()).collect::<Vec<_>>(),
        [
            "plain /*inside*/ ();",
            "(obj.method::<u8>(argument()));",
            "<Thing as Trait>::method(&thing);",
            "factory()(1);",
            "(|| 7)();",
            "nested();",
            "other();"
        ]
    );
    let (_, again) = scan(&source)?;
    assert_eq!(
        found
            .iter()
            .map(|m| (&m.range, m.operator))
            .collect::<Vec<_>>(),
        again
            .iter()
            .map(|m| (&m.range, m.operator))
            .collect::<Vec<_>>()
    );
    for m in &found {
        assert_eq!(m.replacement, "{}");
        assert!(valid_call_replacement(m.original(), &m.replacement));
        m.apply(root.path())?;
        let changed = fs::read_to_string(root.path().join("src/main.rs"))?;
        assert_eq!(
            changed,
            format!("{}{{}}{}", &source[..m.range.start], &source[m.range.end..])
        );
        let mut parser = Parser::new();
        parser.set_language(&tree_sitter_rust::LANGUAGE.into())?;
        assert!(
            !parser
                .parse(&changed, None)
                .unwrap()
                .root_node()
                .has_error()
        );
        assert!(m.apply(root.path()).is_err());
        fs::write(root.path().join("src/main.rs"), &source)?;
    }
    for original in [
        "f()",
        "f(); g();",
        "let _ = f();",
        "return f();",
        "f()?;",
        "f().await;",
        "f(g()?);",
        "f(g().await);",
        "f!();",
        "f(g!());",
        "#[cfg(test)] f();",
        "f(); // tail",
        " f();",
        "f({return});",
        "f({break});",
        "f({continue});",
        "{}",
        ";",
    ] {
        assert!(!valid_call_replacement(original, "{}"), "{original}");
    }
    for replacement in ["", ";", "();", "{f();}", "{};", "{panic!()}"] {
        assert!(
            !valid_call_replacement("f();", replacement),
            "{replacement}"
        );
    }
    Ok(())
}

#[tokio::test]
#[ignore = "compiles native Rust call effects, receiver evaluation, ownership and drop controls"]
async fn native_calls_skip_receivers_arguments_and_bodies_with_native_drop_timing() -> Result<()> {
    for (setup, call, original, mutated) in [
        (
            "",
            "take(argument(&trace), &trace);",
            vec!["arg", "body", "after"],
            vec!["after"],
        ),
        (
            "",
            "receiver(trace.clone()).touch(argument(&trace));",
            vec!["receiver", "arg", "body", "drop", "after"],
            vec!["after"],
        ),
        (
            "let _owned=Owned(trace.clone());",
            "consume(_owned);",
            vec!["body", "drop", "after"],
            vec!["after", "drop"],
        ),
        (
            "let _owned=Owned(trace.clone());",
            "drop(_owned);",
            vec!["drop", "after"],
            vec!["after", "drop"],
        ),
        (
            "let _owned=Owned(trace.clone()); let _closure=move || consume(_owned);",
            "_closure();",
            vec!["body", "drop", "after"],
            vec!["after", "drop"],
        ),
        (
            "let _callback:fn(u8,&Rc<RefCell<Vec<&'static str>>>)=take;",
            "_callback(argument(&trace), &trace);",
            vec!["arg", "body", "after"],
            vec!["after"],
        ),
        (
            "",
            "generic::<u8>(argument(&trace), &trace);",
            vec!["arg", "body", "after"],
            vec!["after"],
        ),
        (
            "",
            "#[allow(unused_must_use)] fallible(&trace);",
            vec!["body", "after"],
            vec!["after"],
        ),
    ] {
        let source = format!(
            r#"
#![allow(dead_code)]
use std::cell::RefCell; use std::rc::Rc;
struct Owned(Rc<RefCell<Vec<&'static str>>>);
impl Drop for Owned {{fn drop(&mut self){{self.0.borrow_mut().push("drop");}}}}
impl Owned {{fn touch(&self,_:u8){{self.0.borrow_mut().push("body");}}}}
fn argument(t:&Rc<RefCell<Vec<&'static str>>>)->u8{{t.borrow_mut().push("arg"); 7}}
fn receiver(t:Rc<RefCell<Vec<&'static str>>>)->Owned{{t.borrow_mut().push("receiver"); Owned(t)}}
fn take(_:u8,t:&Rc<RefCell<Vec<&'static str>>>){{t.borrow_mut().push("body");}}
fn generic<T>(_:T,t:&Rc<RefCell<Vec<&'static str>>>){{t.borrow_mut().push("body");}}
fn consume(o:Owned){{o.0.borrow_mut().push("body");}}
fn fallible(t:&Rc<RefCell<Vec<&'static str>>>)->Result<(),()>{{t.borrow_mut().push("body"); Ok(())}}
fn main() {{
 let trace=Rc::new(RefCell::new(Vec::new()));
 {{ {setup} {call} (|| trace.borrow_mut().push("after"))(); }}
 assert_eq!(*trace.borrow(), if cfg!(mutated) {{vec!{mutated:?}}} else {{vec!{original:?}}});
}}
"#
        );
        let (root, found) = scan(&source)?;
        let target = call
            .strip_prefix("#[allow(unused_must_use)] ")
            .unwrap_or(call);
        let mutation = found
            .iter()
            .find(|m| m.original() == target)
            .context("missing call candidate")?;
        for changed in [false, true] {
            if changed {
                mutation.apply(root.path())?;
            }
            let mut args = vec![
                "--edition=2024",
                "-D",
                "warnings",
                "src/main.rs",
                "-o",
                "probe",
            ];
            if changed {
                args.extend(["--cfg", "mutated"]);
            }
            for (command, args) in [("rustc", args), ("./probe", vec![])] {
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
                    "{call} ({changed}) {command}: {}",
                    out.stderr.text()?
                );
            }
        }
    }
    Ok(())
}
