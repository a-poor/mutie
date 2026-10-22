use super::*;

fn find(source: &str, syntax: &str) -> Result<(tempfile::TempDir, Vec<Mutation>)> {
    let root = tempfile::tempdir()?;
    fs::create_dir(root.path().join("src"))?;
    fs::write(root.path().join(format!("src/math.{syntax}")), source)?;
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
            .filter(|m| m.operator == Operator::ArithmeticReplace)
            .collect(),
    ))
}

#[test]
fn arithmetic_spans_grouping_trivia_and_exact_saved_recipes() -> Result<()> {
    for syntax in ["js", "ts"] {
        let source = "// 🦀\r\nconst x = a /*left*/ + //right\n b * c; const y = d / e - f % g;";
        let (root, found) = find(source, syntax)?;
        assert_eq!(found.len(), 5);
        for m in &found {
            assert!(valid_replacement(
                m.operator,
                m.original(),
                &m.replacement,
                syntax == "ts"
            ));
            assert!(!valid_replacement(
                m.operator,
                m.original(),
                "a + b",
                syntax == "ts"
            ));
            m.apply(root.path())?;
            let edited = fs::read_to_string(root.path().join(format!("src/math.{syntax}")))?;
            assert_eq!(
                edited,
                format!(
                    "{}{}{}",
                    &source[..m.range.start],
                    m.replacement,
                    &source[m.range.end..]
                )
            );
            fs::write(root.path().join(format!("src/math.{syntax}")), source)?;
        }
        assert_eq!(found[0].replacement, "((a) /*left*/ - //right\n (b * c))");
        assert_eq!(found[1].replacement, "((b) / (c))");
        assert!(
            found
                .iter()
                .any(|m| m.original() == "f % g" && m.replacement == "((f) * (g))")
        );
        assert!(!valid_replacement(
            Operator::ArithmeticReplace,
            "a ** b",
            "((a) * (b))",
            syntax == "ts"
        ));
    }
    Ok(())
}

#[test]
fn arithmetic_excludes_unary_updates_assignments_powers_types_and_text() -> Result<()> {
    let source = "type T = -1 | 2; type U<X> = {[K in keyof X]-?: X[K]}; let x = 2; const a = -x; const b = +x; x++; --x; x += 1; x -= 1; x *= 1; x /= 1; x %= 1; const power = x ** 2; const shift = x >> 1; const text = 'a+b'; const r = /a+b/; const runtime = `${x + 1}`;";
    let (_, found) = find(source, "ts")?;
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].original(), "x + 1");
    Ok(())
}

#[tokio::test]
#[ignore = "requires pinned Node and Bun; bounded native JS/TS arithmetic/coercion/error controls"]
async fn native_arithmetic_values_coercion_and_exceptions() -> Result<()> {
    for runtime in ["node", "bun"] {
        for syntax in ["js", "ts"] {
            for (op, expected, bigint, strings, zero) in [
                ("+", "5", "5n", "5", "1"),
                ("-", "9", "9n", "'72'", "1"),
                ("*", "3.5", "3n", "3.5", "Infinity"),
                ("/", "14", "14n", "14", "0"),
                ("%", "14", "14n", "14", "0"),
            ] {
                let params = if syntax == "ts" {
                    "a: any,b: any"
                } else {
                    "a,b"
                };
                let source = format!("export function run({params}) {{ return a() {op} b(); }}");
                let (root, found) = find(&source, syntax)?;
                assert_eq!(found.len(), 1);
                found[0].apply(root.path())?;
                fs::write(root.path().join("package.json"), r#"{"type":"module"}"#)?;
                let hint = if op == "-" { "default" } else { "number" };
                let signed_zero = match op {
                    "-" => "0",
                    "*" => "NaN",
                    _ => "-0",
                };
                let zero_zero = if op == "*" { "NaN" } else { "0" };
                let negative_bigint = match op {
                    "+" => "-9n",
                    "-" => "-5n",
                    "*" => "-3n",
                    _ => "-14n",
                };
                let bigint_zero = if op == "*" {
                    "assert.throws(()=>run(()=>1n,()=>0n), RangeError);"
                } else if op == "+" || op == "-" {
                    "assert.equal(run(()=>1n,()=>0n),1n);"
                } else {
                    "assert.equal(run(()=>1n,()=>0n),0n);"
                };
                let script = format!(
                    r#"import assert from 'node:assert/strict';
import {{run}} from './src/math.{syntax}';
assert.equal(run(()=>7,()=>2),{expected}); assert.equal(run(()=>7n,()=>2n),{bigint});
assert.equal(run(()=>'7',()=>'2'),{strings}); assert.ok(Object.is(run(()=>1,()=>0),{zero}));
assert.ok(Object.is(run(()=>-0,()=>0),{signed_zero})); assert.ok(Object.is(run(()=>0,()=>0),{zero_zero}));
assert.equal(run(()=>-7n,()=>2n),{negative_bigint});
assert.ok(Number.isNaN(run(()=>NaN,()=>1)));
assert.throws(()=>run(()=>7n,()=>2),TypeError); assert.throws(()=>run(()=>Symbol('x'),()=>1),TypeError);
{bigint_zero}
const trace=[];
function operand(name) {{trace.push('eval:'+name); return {{[Symbol.toPrimitive](hint) {{trace.push('coerce:'+name+':'+hint);return name==='L'?7:2}}}}}}
assert.equal(run(()=>operand('L'),()=>operand('R')),{expected});
assert.deepEqual(trace,['eval:L','eval:R','coerce:L:{hint}','coerce:R:{hint}']);
trace.length=0; const sentinel=new Error('conversion');
const bad={{[Symbol.toPrimitive](){{trace.push('coerce:L');throw sentinel}}}};
assert.throws(()=>run(()=>{{trace.push('eval:L');return bad}},()=>{{trace.push('eval:R');return 2}}), e=>e===sentinel);
assert.deepEqual(trace,['eval:L','eval:R','coerce:L']);
let calls=0; assert.throws(()=>run(()=>{{throw sentinel}},()=>{{calls++;return 2}}),e=>e===sentinel); assert.equal(calls,0);
"#
                );
                fs::write(root.path().join(format!("check.{syntax}")), script)?;
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
                    "{runtime}/{syntax}/{op}: {}",
                    output.stderr.text()?
                );
            }
        }
    }
    Ok(())
}
