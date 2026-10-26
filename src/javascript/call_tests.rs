use super::*;

fn find(source: &str, syntax: &str) -> Result<(tempfile::TempDir, Vec<Mutation>)> {
    let root = tempfile::tempdir()?;
    fs::create_dir(root.path().join("src"))?;
    fs::write(root.path().join(format!("src/call.{syntax}")), source)?;
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
            .filter(|m| m.operator == Operator::CallRemove)
            .collect(),
    ))
}

#[test]
fn standalone_calls_preserve_spans_and_reject_other_contexts() -> Result<()> {
    for syntax in ["js", "ts"] {
        let source = "// 🦀\r\nexport function f(a) { if(a) (a.m(/* note */ a.x())); else a?.m();\n a[/*key*/'m'](\n 1\n )\n return a(); }";
        let (root, found) = find(source, syntax)?;
        assert_eq!(found.len(), 3);
        for mutation in found {
            assert!(valid_replacement(
                Operator::CallRemove,
                mutation.original(),
                ";",
                syntax == "ts"
            ));
            for bad in ["", "void 0", "{}", "false", "; injected();"] {
                assert!(!valid_replacement(
                    Operator::CallRemove,
                    mutation.original(),
                    bad,
                    syntax == "ts"
                ));
            }
            mutation.apply(root.path())?;
            let actual = fs::read_to_string(root.path().join(format!("src/call.{syntax}")))?;
            assert_eq!(
                actual,
                format!(
                    "{};{}",
                    &source[..mutation.range.start],
                    &source[mutation.range.end..]
                )
            );
            assert!(mutation.apply(root.path()).is_err());
            fs::write(root.path().join(format!("src/call.{syntax}")), source)?;
        }
        let (_, found) = find(
            "export async function f(a) { const x=a(); return a(); } export async function g(a) { await a(); a(await a()); a(async()=>await a()); } export function* h(a) { yield a(); a(yield a()); } const x=()=>a(); new C(); tag`x`; import('x'); class C extends B { constructor(){super();} }",
            syntax,
        )?;
        assert!(found.is_empty(), "{found:?}");
        for invalid in [
            "",
            "a();b();",
            "return a();",
            "a() + b()",
            "new C();",
            "tag`x`;",
            "import('x');",
            "super();",
            "await a();",
        ] {
            assert!(
                !valid_replacement(Operator::CallRemove, invalid, ";", syntax == "ts"),
                "{invalid}"
            );
        }
    }
    let (_, found) = find(
        "export function f(a: any) { a.m<number>(1); (a as any)(); a!(); } type T = () => void;",
        "ts",
    )?;
    assert_eq!(found.len(), 3);
    Ok(())
}

#[tokio::test]
#[ignore = "requires pinned Node/Bun; receiver/argument and statement-boundary controls"]
async fn native_calls_skip_all_evaluation_and_preserve_statement_boundaries() -> Result<()> {
    for runtime in ["node", "bun"] {
        for syntax in ["js", "ts"] {
            for source in [
                "export function run(s) { s.method(s.arg()); return s.marker; }",
                "export function run(s) { s.receiver()[s.key()](...s.args); return s.marker; }",
                "export function run(s) { if (s.flag) s.method(); else return null; return s.marker; }",
                "export function run(s) { outer: s.method(); return s.marker; }",
                "export function run(s) { (s?.method?.(s.arg())); return s.marker; }",
                "export function run(s) { s.method()\n return s.marker; }",
                "export function run(s) { try { s.method(); } finally { s.trace.push('finally'); } return s.marker; }",
                "export function run(s) { for(let i=0;i<2;i++) s.method(); return s.marker; }",
            ] {
                let (root, found) = find(source, syntax)?;
                // The finally callback has its own independent candidate.
                assert!(!found.is_empty());
                let mutation = &found[0];
                assert!(valid_replacement(
                    Operator::CallRemove,
                    mutation.original(),
                    ";",
                    syntax == "ts"
                ));
                fs::write(root.path().join("package.json"), r#"{"type":"module"}"#)?;
                mutation.apply(root.path())?;
                let trace = if source.contains("finally") {
                    "['finally']"
                } else {
                    "[]"
                };
                fs::write(
                    root.path().join(format!("check.{syntax}")),
                    format!(
                        r#"
import assert from 'node:assert/strict'; import {{run}} from './src/call.{syntax}';
const marker={{}}; const trace=[]; const fail=()=>{{throw new Error('must skip')}};
const s={{marker,trace,flag:true,receiver:fail,key:fail,arg:fail,get method(){{return fail()}},get args(){{return fail()}}}};
assert.equal(run(s),marker); assert.deepEqual(trace,{trace});
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
