use super::*;

fn find(source: &str, syntax: &str) -> Result<(tempfile::TempDir, Vec<Mutation>)> {
    let root = tempfile::tempdir()?;
    fs::create_dir(root.path().join("src"))?;
    fs::write(root.path().join(format!("src/branch.{syntax}")), source)?;
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
fn branches_preserve_condition_bytes_and_validate_exact_recipes() -> Result<()> {
    for syntax in ["js", "ts"] {
        let source = "// 🦀\r\nexport function choose(a,b) { if (a /*comment*/ && // line\n b) if (b) return 1; else if ((a ?? b)) return 2; return 3; }";
        let (root, found) = find(source, syntax)?;
        assert_eq!(found.len(), 6);
        assert_eq!(found[0].original(), "(a /*comment*/ && // line\n b)");
        assert_eq!(
            found[0].replacement,
            "(true || (a /*comment*/ && // line\n b))"
        );
        assert_eq!(
            found[1].replacement,
            "(false && (a /*comment*/ && // line\n b))"
        );
        for m in &found {
            assert!(valid_replacement(
                m.operator,
                m.original(),
                &m.replacement,
                syntax == "ts"
            ));
            for bad in [
                "true",
                "(true)",
                "(false)",
                "(value, true)",
                "(true || arbitrary())",
            ] {
                assert!(!valid_replacement(
                    m.operator,
                    m.original(),
                    bad,
                    syntax == "ts"
                ));
            }
            m.apply(root.path())?;
            let changed = fs::read_to_string(root.path().join(format!("src/branch.{syntax}")))?;
            assert_eq!(
                changed,
                format!(
                    "{}{}{}",
                    &source[..m.range.start],
                    m.replacement,
                    &source[m.range.end..]
                )
            );
            let mut parser = Parser::new();
            parser.set_language(
                &if syntax == "ts" {
                    tree_sitter_typescript::LANGUAGE_TYPESCRIPT
                } else {
                    tree_sitter_javascript::LANGUAGE
                }
                .into(),
            )?;
            parse(&mut parser, &changed)?;
            // Retain source identity checks after applying the mutation.
            assert!(m.apply(root.path()).is_err());
            fs::write(root.path().join(format!("src/branch.{syntax}")), source)?;
        }
        for original in [
            "true",
            "(true)",
            "((/*note*/false))",
            "(x) {} if (y)",
            "(x); injected();",
        ] {
            assert!(!valid_replacement(
                Operator::BranchForceTrue,
                original,
                &format!("(true || {original})"),
                syntax == "ts"
            ));
        }
    }
    Ok(())
}

#[test]
fn branches_exclude_literal_conditions_loops_ternaries_and_types() -> Result<()> {
    let source = "type T = true | false; function f(x: T): void {} const text='if(x)'; const re=/if(x)/; if (true) f(false); if (((/*note*/ false))) f(true); while (x) break; for (;x;) break; const t = x ? 1 : 2; if (x as T) f(x); if (x!) f(x);";
    let (_, found) = find(source, "ts")?;
    assert_eq!(found.len(), 4);
    assert_eq!(found[0].original(), "(x as T)");
    assert_eq!(found[2].original(), "(x!)");
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
#[ignore = "requires pinned Node and Bun; native JS/TS branch side-effect and control-flow controls"]
async fn native_branches_skip_conditions_and_keep_selected_bodies() -> Result<()> {
    for runtime in ["node", "bun"] {
        for syntax in ["js", "ts"] {
            let annotation = if syntax == "ts" { ": any" } else { "" };
            for condition in [
                "call()",
                "(state.value = call())",
                "state.value && call()",
                "state.value ?? call()",
                "(call(), state.value)",
                "state.value?.missing",
                "state.value",
            ] {
                let source = format!(
                    "export function run(call{annotation},state{annotation}) {{ if ({condition}) {{ state.trace.push('yes'); return state.yes; }} else {{ state.trace.push('no'); return state.no; }} }}"
                );
                let (root, found) = find(&source, syntax)?;
                assert_eq!(found.len(), 2);
                fs::write(root.path().join("package.json"), r#"{"type":"module"}"#)?;
                for m in found {
                    m.apply(root.path())?;
                    let yes = m.operator == Operator::BranchForceTrue;
                    let branch = if yes { "yes" } else { "no" };
                    let script = format!(
                        r#"import assert from 'node:assert/strict';
import {{run}} from './src/branch.{syntax}';
const sentinel = new Error('condition must be skipped');
const marker = {{[Symbol.toPrimitive](){{throw sentinel}}, valueOf(){{throw sentinel}}}};
for (const value of [null,undefined,false,true,0,1,0n,1n,'', 'x',Symbol('x'),marker]) {{
 const trace=[]; let reads=0, writes=0;
 const state={{get value(){{reads++;throw sentinel}},set value(v){{writes++;throw sentinel}},trace,yes:{{value}},no:{{value}}}};
 const result=run(()=>{{throw sentinel}},state);
 assert.equal(result,state.{branch}); assert.deepEqual(trace,['{branch}']); assert.equal(reads,0); assert.equal(writes,0);
}}
const bodyError=new Error('selected body');
const state={{trace:{{push(){{throw bodyError}}}}}};
assert.throws(()=>run(()=>{{throw sentinel}},state), e=>e===bodyError);
"#
                    );
                    fs::write(root.path().join(format!("check.{syntax}")), script)?;
                    execute(runtime, root.path(), syntax).await?;
                    fs::write(root.path().join(format!("src/branch.{syntax}")), &source)?;
                }
            }
            for (source, check) in [
                (
                    "export async function run(call) { if (await call()) return 'yes'; else return 'no'; }",
                    "assert.equal(await run(()=>{throw new Error('skipped await')}),EXPECTED);",
                ),
                (
                    "export function* run(call) { if (yield call()) return 'yes'; else return 'no'; }",
                    "assert.deepEqual(run(()=>{throw new Error('skipped yield')}).next(),{value:EXPECTED,done:true});",
                ),
                (
                    "export function run(call) { if (call()) if (false) return 'inner'; else return 'yes'; else return 'no'; }",
                    "assert.equal(run(()=>{throw new Error('skipped condition')}),EXPECTED);",
                ),
            ] {
                let (root, found) = find(source, syntax)?;
                assert_eq!(found.len(), 2);
                fs::write(root.path().join("package.json"), r#"{"type":"module"}"#)?;
                for m in found {
                    let expected = if m.operator == Operator::BranchForceTrue {
                        "'yes'"
                    } else {
                        "'no'"
                    };
                    m.apply(root.path())?;
                    fs::write(
                        root.path().join(format!("check.{syntax}")),
                        format!(
                            "import assert from 'node:assert/strict'; import {{run}} from './src/branch.{syntax}'; {}",
                            check.replace("EXPECTED", expected)
                        ),
                    )?;
                    execute(runtime, root.path(), syntax).await?;
                    fs::write(root.path().join(format!("src/branch.{syntax}")), source)?;
                }
            }
        }
    }
    Ok(())
}

async fn execute(runtime: &str, root: &Path, syntax: &str) -> Result<()> {
    let output = crate::process::run(
        runtime,
        &[&format!("check.{syntax}")],
        root,
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
    Ok(())
}
