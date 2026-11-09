use super::*;
use crate::{
    cancellation::Cancellation,
    mutation::{Mutation, Operator, Scope, discover},
};
use anyhow::Result;
use std::{fs, path::Path, time::Duration};

fn find(root: &Path, source: &str) -> Result<Vec<Mutation>> {
    fs::write(root.join("main.go"), source)?;
    Ok(
        discover(root, vec!["main.go".into()], &Scope::new(&[], &[])?)?
            .into_iter()
            .filter(|m| m.operator == Operator::AssignmentRemove)
            .collect(),
    )
}

#[test]
fn selects_only_single_selector_index_stores_in_statement_lists() -> Result<()> {
    let root = tempfile::tempdir()?;
    let source = r#"package p
func f(a *S, xs []int, m map[string]int, ch chan int, v any) {
    a.x = 1
    ( /* lhs */ a.x ) = 2
    xs[0] = 3
    m["x"] = 4
    (*a).x = 5
    get().x = rhs()
    xs[key()] = rhs()
    a.x = <-ch
    if a.x = 0; ok() { a.x = 6 }
    for a.x = 0; ok(); a.x = 1 { a.x = 7 }
    switch a.x = 0; v { case 0: a.x = 8; default: a.x = 9 }
    switch v.(type) { case int: a.x = 10 }
    select { case a.x = <-ch: a.x = 11; default: a.x = 12 }
    func() { a.x = 13 }()
label: a.x = 14
    a.x, xs[0] = 1, 2
    a.x, ok = m["x"]
    a.x += 1
    xs[0]++
    *a = S{}
    x := 1
    x = 2
    _ = x
    var y = 3
    for _, a.x = range xs { }
}
"#;
    let found = find(root.path(), source)?;
    assert_eq!(
        found.iter().map(Mutation::original).collect::<Vec<_>>(),
        [
            "a.x = 1",
            "( /* lhs */ a.x ) = 2",
            "xs[0] = 3",
            "m[\"x\"] = 4",
            "(*a).x = 5",
            "get().x = rhs()",
            "xs[key()] = rhs()",
            "a.x = <-ch",
            "a.x = 6",
            "a.x = 7",
            "a.x = 8",
            "a.x = 9",
            "a.x = 10",
            "a.x = 11",
            "a.x = 12",
            "a.x = 13"
        ]
    );
    for mutation in found {
        assert!(
            valid_replacement(mutation.original(), ""),
            "{}",
            mutation.original()
        );
        crate::identity::MutationIdentity::from(&mutation).validate()?;
        mutation.apply(root.path())?;
        let actual = fs::read_to_string(root.path().join("main.go"))?;
        assert_eq!(
            actual,
            format!(
                "{}{}",
                &source[..mutation.range.start],
                &source[mutation.range.end..]
            )
        );
        assert!(mutation.apply(root.path()).is_err());
        let mut parser = Parser::new();
        parser.set_language(&tree_sitter_go::LANGUAGE.into())?;
        assert!(!parser.parse(&actual, None).unwrap().root_node().has_error());
        fs::write(root.path().join("main.go"), source)?;
    }
    Ok(())
}

#[test]
fn preserves_multiline_bytes_and_rejects_wrong_saved_recipes() -> Result<()> {
    let root = tempfile::tempdir()?;
    let source = "package p\r\nfunc f(a *S) {\r\n\t// é\r\n\ta.x /* target */ =\r\n\t\trhs(\"é\") // after\r\n}";
    let found = find(root.path(), source)?;
    assert_eq!(found.len(), 1);
    let m = &found[0];
    assert_eq!((m.line, m.column, m.end_line, m.end_column), (4, 2, 5, 12));
    assert_eq!(m.original(), "a.x /* target */ =\r\n\t\trhs(\"é\")");
    assert!(m.diff().contains("+\t // after\r\n"));
    for original in [
        "x = 1",
        "x := 1",
        "a.x += 1",
        "a.x++",
        "a.x, b.x = 1, 2",
        "*a = 1",
        "f()",
        "a.x = 1; b.x = 2",
        "label: a.x = 1",
        "a.x=1;",
        " a.x=1",
        "a.x=1 // comment",
    ] {
        assert!(!valid_replacement(original, ""), "{original}");
    }
    for replacement in [";", "{}", "pass", "f()", "if false {a.x=1}"] {
        assert!(!valid_replacement("a.x=1", replacement));
    }
    Ok(())
}

async fn native(root: &Path) -> Result<String> {
    let out = crate::process::run(
        "go",
        &["run", "main.go"],
        root,
        Duration::from_secs(20),
        std::num::NonZeroUsize::new(262144),
        &Cancellation::default(),
    )
    .await?;
    assert!(!out.timed_out && out.capture_error.is_none());
    assert!(out.status.success(), "{}", out.stderr.text()?);
    out.stdout.text()
}

#[tokio::test]
#[ignore = "requires Go 1.24+ and a writable build cache; original/mutant semantic pairs"]
async fn native_assignment_effects() -> Result<()> {
    // Calls on receiver/key/RHS establish order without assuming the order of
    // ordinary variable loads, which the Go specification does not guarantee.
    let cases = [
        (
            "var a S; func obj() *S { fmt.Print(\"receiver;\"); return &a }",
            "obj().x = rhs()",
            "a.x",
            "receiver;rhs;end:7;",
            "end:0;",
        ),
        (
            "var a = []int{0}; func obj() []int { fmt.Print(\"receiver;\"); return a }; func key() int { fmt.Print(\"key;\"); return 0 }",
            "obj()[key()] = rhs()",
            "a[0]",
            "receiver;key;rhs;end:7;",
            "end:0;",
        ),
        (
            "var a = map[int]int{}; func obj() map[int]int { fmt.Print(\"receiver;\"); return a }; func key() int { fmt.Print(\"key;\"); return 2 }",
            "obj()[key()] = rhs()",
            "len(a)",
            "receiver;key;rhs;end:1;",
            "end:0;",
        ),
        (
            "var a = [1]int{}",
            "a[0] = rhs()",
            "a[0]",
            "rhs;end:7;",
            "end:0;",
        ),
        (
            "var a = [1]int{}",
            "(&a)[0] = rhs()",
            "a[0]",
            "rhs;end:7;",
            "end:0;",
        ),
        (
            "var a S; var b = &a",
            "(*b).x = rhs()",
            "a.x",
            "rhs;end:7;",
            "end:0;",
        ),
        ("var a *S", "a.x = rhs()", "0", "rhs;panic;end:0;", "end:0;"),
        (
            "var a map[int]int",
            "a[0] = rhs()",
            "len(a)",
            "rhs;panic;end:0;",
            "end:0;",
        ),
        (
            "var a []int",
            "a[0] = rhs()",
            "len(a)",
            "rhs;panic;end:0;",
            "end:0;",
        ),
        (
            "var a = map[any]int{}; var key any = []int{1}",
            "a[key] = rhs()",
            "len(a)",
            "rhs;panic;end:0;",
            "end:0;",
        ),
        (
            "var a S; func bad() int { fmt.Print(\"rhs;\"); panic(\"bad\") }",
            "a.x = bad()",
            "a.x",
            "rhs;panic;end:0;",
            "end:0;",
        ),
        (
            "var a S; func bad() *S { fmt.Print(\"receiver;\"); panic(\"bad\") }",
            "bad().x = rhs()",
            "a.x",
            "receiver;panic;end:0;",
            "end:0;",
        ),
        (
            "var a = []int{0}; func bad() int { fmt.Print(\"key;\"); panic(\"bad\") }",
            "a[bad()] = rhs()",
            "a[0]",
            "key;panic;end:0;",
            "end:0;",
        ),
        (
            "var a S; var ch = make(chan int,1); func init() { ch <- 7 }",
            "a.x = <-ch",
            "len(ch)",
            "end:0;",
            "end:1;",
        ),
        (
            "var a S",
            "a.x = func() int { defer fmt.Print(\"cleanup;\"); return rhs() }()",
            "a.x",
            "rhs;cleanup;end:7;",
            "end:0;",
        ),
        (
            "type Box[T any] struct { value T }; var a Box[int]",
            "a.value = rhs()",
            "a.value",
            "rhs;end:7;",
            "end:0;",
        ),
    ];
    let root = tempfile::tempdir()?;
    for (setup, statement, result, before, after) in cases {
        let source = format!(
            "package main\nimport \"fmt\"\ntype S struct {{ x int }}\nfunc rhs() int {{ fmt.Print(\"rhs;\"); return 7 }}\n{setup}\nfunc probe() {{ defer func() {{ if recover()!=nil {{ fmt.Print(\"panic;\") }} }}(); {statement} }}\nfunc main() {{ probe(); fmt.Printf(\"end:%v;\", {result}) }}\n"
        );
        let found = find(root.path(), &source)?;
        let mutation = found.iter().find(|m| m.original() == statement).unwrap();
        assert_eq!(native(root.path()).await?, before, "original: {statement}");
        mutation.apply(root.path())?;
        assert_eq!(native(root.path()).await?, after, "mutant: {statement}");
    }
    Ok(())
}
