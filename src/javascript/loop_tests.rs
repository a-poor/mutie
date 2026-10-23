use super::*;

fn find(source: &str, syntax: &str) -> Result<(tempfile::TempDir, Vec<Mutation>)> {
    let root = tempfile::tempdir()?;
    fs::create_dir(root.path().join("src"))?;
    fs::write(root.path().join(format!("src/loop.{syntax}")), source)?;
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
            .filter(|m| m.operator == Operator::LoopSkip)
            .collect(),
    ))
}

#[test]
fn loops_preserve_spans_and_reject_wrong_recipes() -> Result<()> {
    for syntax in ["js", "ts"] {
        let source = "// 🦀\r\nexport function run(a,b) { while (a /*comment*/ && // line\n b) break; for (let i=0; a,b; i++) { while (b) break; } }";
        let (root, found) = find(source, syntax)?;
        assert_eq!(found.len(), 3);
        assert_eq!(found[0].original(), "(a /*comment*/ && // line\n b)");
        assert_eq!(found[1].original(), "a,b");
        for m in found {
            assert!(valid_replacement(
                m.operator,
                m.original(),
                &m.replacement,
                syntax == "ts"
            ));
            for bad in ["false", "(false)", "(false && unrelated)", "(true || (a))"] {
                assert!(!valid_replacement(
                    m.operator,
                    m.original(),
                    bad,
                    syntax == "ts"
                ));
            }
            m.apply(root.path())?;
            let changed = fs::read_to_string(root.path().join(format!("src/loop.{syntax}")))?;
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
            fs::write(root.path().join(format!("src/loop.{syntax}")), source)?;
        }
        for original in [
            "",
            ";",
            "true",
            "((/*note*/false))",
            "x;){};injected();for(;y",
        ] {
            assert!(!valid_replacement(
                Operator::LoopSkip,
                original,
                &format!("(false && ({original}))"),
                syntax == "ts"
            ));
        }
    }
    Ok(())
}

#[test]
fn loops_exclude_literals_iterators_do_while_and_types() -> Result<()> {
    let source = "type T = true | false; const text='while(x)'; const re=/while(x)/; while (true) break; while (((false))) break; for (;;) break; for (; /*empty*/ ;) break; for(;true;) break; for(;((false));) {} for (const x of xs) {} for (const x in xs) {} async function iterate(){for await(const x of xs){}} do {} while (x); if(x){} const t=x?1:2; for(;x as T;) break; while(x!) break;";
    let (_, found) = find(source, "ts")?;
    assert_eq!(found.len(), 2);
    assert_eq!(found[0].original(), "x as T");
    assert_eq!(found[1].original(), "(x!)");
    for m in found {
        assert!(valid_replacement(
            m.operator,
            m.original(),
            &m.replacement,
            true
        ));
    }
    Ok(())
}

#[tokio::test]
#[ignore = "requires pinned Node and Bun; loop initializer/effect/lexical controls"]
async fn native_loops_skip_conditions_bodies_updates_and_preserve_initializers() -> Result<()> {
    for runtime in ["node", "bun"] {
        for syntax in ["js", "ts"] {
            for (source, check) in [
                (
                    "export function run(s) { while (s.condition()) { s.body(); } return s.marker; }",
                    "assert.equal(run(s),s.marker); assert.deepEqual(trace,[]);",
                ),
                (
                    "export function run(s) { for(let i=s.init(); s.condition(); s.update()) { s.body(); } return s.marker; }",
                    "assert.equal(run(s),s.marker); assert.deepEqual(trace,['init']);",
                ),
                (
                    "export function run(s) { for(var i=s.init(); (s.value=s.condition(),s.value); s.update()) { s.body(); } return i; }",
                    "assert.equal(run(s),7); assert.deepEqual(trace,['init']);",
                ),
                (
                    "export function run(s) { let i=8; outer: for(let i=s.init(); s.value; s.update()) { s.body(); continue outer; } return i; }",
                    "assert.equal(run(s),8); assert.deepEqual(trace,['init']);",
                ),
                (
                    "export function run(s) { for(let i=s.init(); s.condition(); s.update()) {} }",
                    "s.init=()=>{trace.push('init');throw sentinel}; assert.throws(()=>run(s),e=>e===sentinel); assert.deepEqual(trace,['init']);",
                ),
                (
                    "export async function run(s) { while(await s.condition()) s.body(); return s.marker; }",
                    "assert.equal(await run(s),s.marker); assert.deepEqual(trace,[]);",
                ),
                (
                    "export function* run(s) { while(yield s.condition()) s.body(); return s.marker; }",
                    "assert.deepEqual(run(s).next(),{value:s.marker,done:true}); assert.deepEqual(trace,[]);",
                ),
                (
                    "export function run(s) { for(let i=s.init(); i < 1; i++) {} return s.marker; }",
                    "assert.equal(run(s),s.marker); assert.deepEqual(trace,['init']);",
                ),
            ] {
                let (root, found) = find(source, syntax)?;
                assert_eq!(found.len(), 1);
                assert!(valid_replacement(
                    Operator::LoopSkip,
                    found[0].original(),
                    &found[0].replacement,
                    syntax == "ts"
                ));
                fs::write(root.path().join("package.json"), r#"{"type":"module"}"#)?;
                found[0].apply(root.path())?;
                fs::write(
                    root.path().join(format!("check.{syntax}")),
                    format!(
                        r#"
import assert from 'node:assert/strict'; import {{run}} from './src/loop.{syntax}';
const sentinel=new Error('skipped'); const trace=[];
const s={{marker:{{}},init(){{trace.push('init');return 7}},condition(){{throw sentinel}},body(){{throw sentinel}},update(){{throw sentinel}},get value(){{throw sentinel}},set value(v){{throw sentinel}}}};
{check}
"#
                    ),
                )?;
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
                    "{runtime}/{syntax}: {}",
                    output.stderr.text()?
                );
            }
        }
    }
    Ok(())
}
