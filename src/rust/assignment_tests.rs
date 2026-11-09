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
            .filter(|m| m.operator == Operator::AssignmentRemove)
            .collect(),
    ))
}

#[test]
fn assignments_preserve_whole_statements_and_exclude_other_contexts() -> Result<()> {
    let source = r#"
fn f() {
    // 🦀 before
    a.value /* target */ =
        rhs(); // after
    a.0 = x;
    (a.value) = x;
    (a.value = x);
    a[i] = x;
    (*a).value = x;
    if flag { a.value = x; } else { a.value = y; }
    match v { 0 => a.value = x, _ => { a.value = y; } }
    let callback = || { a.value = x; };
    a.value = (|| x)();
    let _ = a.value = x;
    return a.value = x;
}
fn excluded() {
    let x; x = y; *p = x;
    (a.value, a.other) = (x, y);
    [a[0], a[1]] = [x,y];
    a.value += 1;
    a.value = b.value = ();
    a.value = { b.value = 1; x };
    a.value = { b.value += 1; x };
    a.value = awaited().await;
    a.value = propagated()?;
    a.value = {return};
    a.value = {break};
    a.value = {continue};
    a.value = make!();
    a.value = (|| macro_call!())();
    let _ = const { a.value = x; 0 };
    #[cfg(test)] a.value = x;
    a.value = x
}
#[test] fn only_test() { a.value = x; }
#[cfg(test)] mod tests { fn f() { a.value = x; } }
static S: () = { a.value = x; };
const C: () = { a.value = x; };
"#
    .replace('\n', "\r\n");
    let (root, found) = scan(&source)?;
    assert_eq!(
        found.iter().map(Mutation::original).collect::<Vec<_>>(),
        [
            "a.value /* target */ =\r\n        rhs();",
            "a.0 = x;",
            "(a.value) = x;",
            "(a.value = x);",
            "a[i] = x;",
            "(*a).value = x;",
            "a.value = x;",
            "a.value = y;",
            "a.value = y;",
            "a.value = x;",
            "a.value = (|| x)();",
            "b.value = 1;"
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
    for m in found {
        assert!(
            valid_assignment_replacement(m.original(), "{}"),
            "{}",
            m.original()
        );
        assert!(!valid_call_replacement(m.original(), "{}"));
        crate::identity::MutationIdentity::from(&m).validate()?;
        m.apply(root.path())?;
        let actual = fs::read_to_string(root.path().join("src/main.rs"))?;
        assert_eq!(
            actual,
            format!("{}{{}}{}", &source[..m.range.start], &source[m.range.end..])
        );
        assert!(m.apply(root.path()).is_err());
        let mut parser = Parser::new();
        parser.set_language(&tree_sitter_rust::LANGUAGE.into())?;
        assert!(!parser.parse(&actual, None).unwrap().root_node().has_error());
        assert!(actual.contains("// after"));
        fs::write(root.path().join("src/main.rs"), &source)?;
    }
    Ok(())
}

#[test]
fn saved_assignments_require_exact_field_index_statement_recipes() {
    for original in [
        "x = 1;",
        "let x = 1;",
        "a.x += 1;",
        "*a = 1;",
        "a.x = 1",
        "a.x = 1; b.x = 2;",
        "a.x = y?;",
        "a.x = y.await;",
        "a.x = f!();",
        "a.x = (|| {return y})();",
        "a.x = b.x = ();",
        "(a.x, a.y) = (1,2);",
        "f();",
        " a.x=1;",
        "a.x=1; // tail",
        "#[cfg(test)] a.x=1;",
    ] {
        assert!(!valid_assignment_replacement(original, "{}"), "{original}");
    }
    for replacement in ["", ";", "{};", "();", "{f();}"] {
        assert!(!valid_assignment_replacement("a.x=1;", replacement));
    }
}

async fn execute(
    root: &Path,
    program: &str,
    args: &[&str],
) -> Result<crate::process::CommandOutput> {
    let output = crate::process::run(
        program,
        args,
        root,
        std::time::Duration::from_secs(30),
        std::num::NonZeroUsize::new(65536),
        &Cancellation::default(),
    )
    .await?;
    ensure!(
        !output.timed_out && output.capture_error.is_none(),
        "{program} exceeded its bound"
    );
    Ok(output)
}

#[tokio::test]
#[ignore = "compiles paired Rust assignment effects, ownership, borrowing, panic and drop controls"]
async fn native_assignment_effects_and_drop_order() -> Result<()> {
    for (setup, statement, original, mutated) in [
        (
            "let mut slot = Slot(owned(&t, \"old\"));",
            "place(&mut slot, &t).0 = rhs(&t);",
            vec!["rhs", "place", "old", "after", "new"],
            vec!["after", "old"],
        ),
        (
            "let mut slot = Slot(owned(&t, \"old\")); let incoming = owned(&t, \"new\");",
            "slot.0 = incoming;",
            vec!["old", "after", "new"],
            vec!["after", "new", "old"],
        ),
        (
            "let mut items = [owned(&t, \"old\")];",
            "items[key(&t)] = rhs(&t);",
            vec!["rhs", "key", "old", "after", "new"],
            vec!["after", "old"],
        ),
        (
            "let mut items = Indexed { value: owned(&t, \"old\"), t:t.clone() };",
            "items[key(&t)] = rhs(&t);",
            vec!["rhs", "key", "index_mut", "old", "after", "new"],
            vec!["after", "old"],
        ),
        (
            "let mut slot = Wrapped(Slot(owned(&t, \"old\")), t.clone());",
            "slot.0.0 = rhs(&t);",
            vec!["rhs", "old", "after", "new"],
            vec!["after", "old"],
        ),
        (
            "let mut slot = Named { value: owned(&t, \"old\") }; let mut wrap = DerefNamed(&mut slot, t.clone());",
            "wrap.value = rhs(&t);",
            vec!["rhs", "deref_mut", "old", "after", "new"],
            vec!["after", "old"],
        ),
        (
            "let mut slot = Slot(owned(&t, \"old\"));",
            "(slot.0 = rhs(&t));",
            vec!["rhs", "old", "after", "new"],
            vec!["after", "old"],
        ),
        (
            "",
            "temporary(&t).0 = rhs(&t);",
            vec!["rhs", "place", "old", "new", "after"],
            vec!["after"],
        ),
        (
            "let mut slot = Slot(owned(&t, \"old\"));",
            "slot.0 = bad_rhs(&t);",
            vec!["rhs", "panic", "after", "old"],
            vec!["after", "old"],
        ),
        (
            "",
            "bad_place(&t).0 = rhs(&t);",
            vec!["rhs", "place", "new", "panic", "after"],
            vec!["after"],
        ),
        (
            "let mut items = [owned(&t, \"old\")];",
            "items[bad_key(&t)] = rhs(&t);",
            vec!["rhs", "key", "new", "panic", "after", "old"],
            vec!["after", "old"],
        ),
        (
            "let mut items = [owned(&t, \"old\")];",
            "items[far_key(&t)] = rhs(&t);",
            vec!["rhs", "key", "new", "panic", "after", "old"],
            vec!["after", "old"],
        ),
        (
            "let mut slot = Slot(Owned { t:t.clone(), name:\"old\", fail:true });",
            "slot.0 = rhs(&t);",
            vec!["rhs", "old", "panic", "after", "new"],
            vec!["after", "old", "scope_panic"],
        ),
        (
            "let mut slot = Slot(owned(&t, \"old\")); let r = &mut slot;",
            "r.0 = rhs(&t);",
            vec!["rhs", "old", "after", "new"],
            vec!["after", "old"],
        ),
        (
            "let mut slot = Slot(owned(&t, \"old\"));",
            "slot.0 = { let _temporary = owned(&t, \"rhs_temporary\"); rhs(&t) };",
            vec!["rhs", "rhs_temporary", "old", "after", "new"],
            vec!["after", "old"],
        ),
    ] {
        let source = format!(
            r#"
#![allow(unused)]
use std::{{cell::RefCell, rc::Rc, ops::{{Deref,DerefMut,Index,IndexMut}}, panic::{{catch_unwind,AssertUnwindSafe}}}};
type Trace=Rc<RefCell<Vec<&'static str>>>;
struct Owned {{ t:Trace, name:&'static str, fail:bool }}
impl Drop for Owned {{ fn drop(&mut self) {{ self.t.borrow_mut().push(self.name); if self.fail {{ panic!("drop"); }} }} }}
struct Slot(Owned); struct Wrapped(Slot,Trace); struct Named {{value:Owned}}
struct DerefNamed<'a>(&'a mut Named,Trace);
impl Deref for DerefNamed<'_> {{type Target=Named; fn deref(&self)->&Named {{self.0}}}}
impl DerefMut for DerefNamed<'_> {{fn deref_mut(&mut self)->&mut Named {{self.1.borrow_mut().push("deref_mut");self.0}}}}
struct Indexed {{ value:Owned,t:Trace }}
impl Index<usize> for Indexed {{ type Output=Owned; fn index(&self,_:usize)->&Owned {{ &self.value }} }}
impl IndexMut<usize> for Indexed {{ fn index_mut(&mut self,_:usize)->&mut Owned {{ self.t.borrow_mut().push("index_mut"); &mut self.value }} }}
fn owned(t:&Trace,name:&'static str)->Owned {{ Owned{{t:t.clone(),name,fail:false}} }}
fn rhs(t:&Trace)->Owned {{t.borrow_mut().push("rhs"); owned(t,"new")}}
fn key(t:&Trace)->usize {{t.borrow_mut().push("key");0}}
fn far_key(t:&Trace)->usize {{t.borrow_mut().push("key");9}}
fn bad_key(t:&Trace)->usize {{t.borrow_mut().push("key");panic!("key")}}
fn bad_rhs(t:&Trace)->Owned {{t.borrow_mut().push("rhs");panic!("rhs")}}
fn bad_place(t:&Trace)->Slot {{t.borrow_mut().push("place");panic!("place")}}
fn place<'a>(s:&'a mut Slot,t:&Trace)->&'a mut Slot {{t.borrow_mut().push("place");s}}
fn temporary(t:&Trace)->Slot {{t.borrow_mut().push("place");Slot(owned(t,"old"))}}
fn main() {{
 std::panic::set_hook(Box::new(|_|{{}})); let t:Trace=Rc::new(RefCell::new(Vec::new()));
 if catch_unwind(AssertUnwindSafe(|| {{
  {setup}
  if catch_unwind(AssertUnwindSafe(|| {{ {statement} }})).is_err() {{ t.borrow_mut().push("panic"); }}
  (|| t.borrow_mut().push("after"))();
 }})).is_err() {{t.borrow_mut().push("scope_panic");}}
 assert_eq!(*t.borrow(), if cfg!(mutated) {{vec!{mutated:?}}} else {{vec!{original:?}}});
}}
"#
        );
        let (root, found) = scan(&source)?;
        let m = found
            .iter()
            .find(|m| m.original() == statement)
            .context("missing assignment")?;
        for changed in [false, true] {
            if changed {
                m.apply(root.path())?;
            }
            let mut args = vec!["--edition=2024", "src/main.rs", "-o", "probe"];
            if changed {
                args.extend(["--cfg", "mutated"]);
            }
            for (program, args) in [("rustc", args), ("./probe", vec![])] {
                let out = execute(root.path(), program, &args).await?;
                ensure!(
                    out.status.success(),
                    "{statement} ({changed}): {}",
                    out.stderr.text()?
                );
            }
        }
    }
    Ok(())
}

#[tokio::test]
#[ignore = "native compile-failure controls for partial moves, inference and denied warnings"]
async fn native_assignment_compile_failures_are_not_repaired() -> Result<()> {
    for (source, statement, message) in [
        (
            "struct S{value:String} fn main(){let mut s=S{value:String::new()};let _moved=s.value;s.value=String::new();drop(s);}",
            "s.value=String::new();",
            "partially moved",
        ),
        (
            "fn main(){let mut a=[Default::default()];a[0]=7u8;let _n=a.len();}",
            "a[0]=7u8;",
            "error[E0790]",
        ),
        (
            "#![deny(warnings)]\nfn main(){let mut a=[0u8];a[0]=7;assert_eq!(a.len(),1);}",
            "a[0]=7;",
            "does not need to be mutable",
        ),
    ] {
        let (root, found) = scan(source)?;
        let args = ["--edition=2024", "src/main.rs", "-o", "probe"];
        let before = execute(root.path(), "rustc", &args).await?;
        ensure!(before.status.success(), "{}", before.stderr.text()?);
        found
            .iter()
            .find(|m| m.original() == statement)
            .unwrap()
            .apply(root.path())?;
        let after = execute(root.path(), "rustc", &args).await?;
        ensure!(
            !after.status.success() && after.stderr.text()?.contains(message),
            "{}",
            after.stderr.text()?
        );
    }
    Ok(())
}
