use super::*;

fn find(source: &str, syntax: &str) -> Result<(tempfile::TempDir, Vec<Mutation>)> {
    let root = tempfile::tempdir()?;
    fs::create_dir(root.path().join("src"))?;
    fs::write(root.path().join(format!("src/logic.{syntax}")), source)?;
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
            .filter(|m| matches!(m.operator, Operator::LogicalSwap | Operator::NegationRemove))
            .collect(),
    ))
}

#[test]
fn logical_grouping_trivia_and_saved_recipes_are_exact() -> Result<()> {
    for syntax in ["js", "ts"] {
        let source = "// 🦀\r\nconst x = a /*left*/ && //right\n b && c; const y = ! /*note*/ value; const z = !!value;";
        let (root, mutations) = find(source, syntax)?;
        assert_eq!(mutations.len(), 5);
        assert_eq!(mutations[0].original(), "a /*left*/ && //right\n b");
        assert_eq!(mutations[0].replacement, "((a) /*left*/ || //right\n (b))");
        assert_eq!(
            mutations[1].replacement,
            "((a /*left*/ && //right\n b) || (c))"
        );
        assert_eq!(mutations[2].replacement, "( /*note*/ value)");
        assert_eq!(mutations[3].replacement, "(!value)");
        assert_eq!(mutations[4].replacement, "(value)");
        for m in mutations {
            assert!(valid_replacement(
                m.operator,
                m.original(),
                &m.replacement,
                syntax == "ts"
            ));
            assert!(!valid_replacement(
                m.operator,
                m.original(),
                "arbitrary()",
                syntax == "ts"
            ));
            m.apply(root.path())?;
            let edited = fs::read_to_string(root.path().join(format!("src/logic.{syntax}")))?;
            let mut parser = Parser::new();
            parser.set_language(
                &if syntax == "ts" {
                    tree_sitter_typescript::LANGUAGE_TYPESCRIPT
                } else {
                    tree_sitter_javascript::LANGUAGE
                }
                .into(),
            )?;
            parse(&mut parser, &edited)?;
            fs::write(root.path().join(format!("src/logic.{syntax}")), source)?;
        }
        assert!(!valid_replacement(
            Operator::LogicalSwap,
            "a && b && c",
            "a || b && c",
            syntax == "ts"
        ));
        assert!(!valid_replacement(
            Operator::NegationRemove,
            "!value",
            "value",
            syntax == "ts"
        ));
    }
    Ok(())
}

#[test]
fn runtime_not_is_distinct_from_typescript_assertions_and_other_operators() -> Result<()> {
    let source = "type T = true | false; interface X { value: T } class C { field!: T; } const a = value!; const b = value! as T; const c = a ?? b; let d = 1; d &&= 2; d ||= 3; d ??= 4; const e = a & b | ~c; const r = /!&&/; const s = '! ||'; const real = !value!;";
    let (_, found) = find(source, "ts")?;
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].original(), "!value");
    assert_eq!(found[0].replacement, "(value)");
    assert!(valid_replacement(
        found[0].operator,
        found[0].original(),
        &found[0].replacement,
        true
    ));
    for source in [
        "async function f(){ return !(await work()); }",
        "function* f(){ return !(yield work()); }",
        "const a = !({ value: 1 });",
        "const a = `text ${!value}`;",
    ] {
        let (_, found) = find(source, "js")?;
        assert_eq!(found.len(), 1);
        assert!(
            valid_replacement(
                found[0].operator,
                found[0].original(),
                &found[0].replacement,
                false
            ),
            "{source}"
        );
    }
    Ok(())
}

#[tokio::test]
#[ignore = "requires pinned Node and Bun; bounded native JS/TS grouping, values and evaluation controls"]
async fn native_logical_and_negation_semantics() -> Result<()> {
    // Select by original expression, including overlapping nested mutations.
    // Expected values/traces are explicit rather than computed with our recipe.
    let cases = [
        (
            "a() && b() && c()",
            "a() && b()",
            "''",
            "'B'",
            "'C'",
            "'C'",
            "['a','b','c']",
        ),
        (
            "a() || b() || c()",
            "a() || b() || c()",
            "'A'",
            "'B'",
            "'C'",
            "'C'",
            "['a','c']",
        ),
        (
            "a() && b()",
            "a() && b()",
            "'A'",
            "'B'",
            "0",
            "'A'",
            "['a']",
        ),
        ("a() || b()", "a() || b()", "0", "'B'", "0", "0", "['a']"),
        (
            "(a() ?? b()) && c()",
            "(a() ?? b()) && c()",
            "null",
            "'B'",
            "'C'",
            "'B'",
            "['a','b']",
        ),
        ("!a()", "!a()", "'text'", "0", "0", "'text'", "['a']"),
        ("!!a()", "!!a()", "0", "0", "0", "true", "['a']"),
        ("!!a()", "!a()", "0", "0", "0", "true", "['a']"),
        ("!a()", "!a()", "0n", "0", "0", "0n", "['a']"),
        ("!a()", "!a()", "NaN", "0", "0", "NaN", "['a']"),
        ("!a()", "!a()", "undefined", "0", "0", "undefined", "['a']"),
        ("!a()", "!a()", "null", "0", "0", "null", "['a']"),
        ("!a()", "!a()", "opaque", "0", "0", "opaque", "['a']"),
        (
            "a() && b()",
            "a() && b()",
            "opaque",
            "0",
            "0",
            "opaque",
            "['a']",
        ),
        (
            "a() ?? (b() && c())",
            "b() && c()",
            "null",
            "'B'",
            "'C'",
            "'B'",
            "['a','b']",
        ),
        (
            "typeof !a()",
            "!a()",
            "'text'",
            "0",
            "0",
            "'string'",
            "['a']",
        ),
        ("-!a()", "!a()", "2", "0", "0", "-2", "['a']"),
        ("!a()", "!a()", "symbol", "0", "0", "symbol", "['a']"),
    ];
    for runtime in ["node", "bun"] {
        for syntax in ["js", "ts"] {
            for (expression, selected, a, b, c, expected, trace) in cases {
                let source = if syntax == "ts" {
                    format!("export function run(a: any,b: any,c: any) {{ return {expression}; }}")
                } else {
                    format!("export function run(a,b,c) {{ return {expression}; }}")
                };
                let (root, mutations) = find(&source, syntax)?;
                let m = mutations.iter().find(|m| m.original() == selected).unwrap();
                assert!(valid_replacement(
                    m.operator,
                    m.original(),
                    &m.replacement,
                    syntax == "ts"
                ));
                m.apply(root.path())?;
                fs::write(root.path().join("package.json"), r#"{"type":"module"}"#)?;
                let newly_evaluated = match expression {
                    "a() && b()" => {
                        "assert.throws(()=>run(()=>0,()=>{throw sentinel},()=>0), e=>e===sentinel);"
                    }
                    "a() || b()" => {
                        "assert.throws(()=>run(()=>1,()=>{throw sentinel},()=>0), e=>e===sentinel);"
                    }
                    _ => "",
                };
                let harness = format!(
                    r#"import assert from 'node:assert/strict';
import {{run}} from './src/logic.{syntax}';
const trace=[];
const opaque={{[Symbol.toPrimitive](){{throw new Error('unexpected coercion')}}}};
const symbol=Symbol('identity');
const a=()=>{{trace.push('a');return {a}}}; const b=()=>{{trace.push('b');return {b}}}; const c=()=>{{trace.push('c');return {c}}};
assert.ok(Object.is(run(a,b,c), {expected})); assert.deepEqual(trace,{trace});
const sentinel=new Error('left'); let rightCalls=0;
assert.throws(()=>run(()=>{{throw sentinel}},()=>{{rightCalls++;return 1}},()=>{{rightCalls++;return 2}}), e=>e===sentinel);
assert.equal(rightCalls,0);
{newly_evaluated}
"#
                );
                fs::write(root.path().join(format!("check.{syntax}")), harness)?;
                let output = crate::process::run(
                    runtime,
                    &[&format!("check.{syntax}")],
                    root.path(),
                    std::time::Duration::from_secs(10),
                    std::num::NonZeroUsize::new(65536),
                    &Cancellation::default(),
                )
                .await?;
                ensure!(
                    output.status.success() && !output.timed_out && output.capture_error.is_none(),
                    "{runtime}/{syntax}/{expression}/{selected}: {}",
                    output.stderr.text()?
                );
            }
        }
    }
    Ok(())
}
