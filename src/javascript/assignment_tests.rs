use super::*;

fn find(source: &str, syntax: &str) -> Result<(tempfile::TempDir, Vec<Mutation>)> {
    let root = tempfile::tempdir()?;
    fs::create_dir(root.path().join("src"))?;
    fs::write(root.path().join(format!("src/assignment.{syntax}")), source)?;
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
            .filter(|m| m.operator == Operator::AssignmentRemove)
            .collect(),
    ))
}

#[test]
fn assignments_preserve_exact_statements_and_saved_operator_recipes() -> Result<()> {
    for syntax in ["js", "ts"] {
        let source = "// 🦀\r\nexport function f(a) { if(a) (a.value /* target */ =\r\n 3); else a['item'] = 4;\n a.nested.value = 5\n return a; }";
        let (root, mutations) = find(source, syntax)?;
        assert_eq!(mutations.len(), 3);
        assert_eq!(mutations[0].original(), "(a.value /* target */ =\r\n 3);");
        assert_eq!(mutations[0].line, 2);
        assert_eq!(mutations[0].end_line, 3);
        for mutation in mutations {
            assert!(valid_replacement(
                mutation.operator,
                mutation.original(),
                ";",
                syntax == "ts"
            ));
            assert!(!valid_replacement(
                Operator::CallRemove,
                mutation.original(),
                ";",
                syntax == "ts"
            ));
            for changed in ["", "{}", "pass", "void 0", "; injected();"] {
                assert!(!valid_replacement(
                    mutation.operator,
                    mutation.original(),
                    changed,
                    syntax == "ts"
                ));
            }
            mutation.apply(root.path())?;
            let path = root.path().join(format!("src/assignment.{syntax}"));
            assert_eq!(
                fs::read_to_string(&path)?,
                format!(
                    "{};{}",
                    &source[..mutation.range.start],
                    &source[mutation.range.end..]
                )
            );
            assert!(mutation.apply(root.path()).is_err());
            fs::write(path, source)?;
        }
        for original in [
            "x = 1;",
            "a();",
            "a.x += 1;",
            "a.x ||= 1;",
            "a.x++;",
            "a.x = b.x = 1;",
            "a.x = i++;",
            "a.x = (i = 1);",
            "[a.x] = [1];",
            "({x:a.x} = {x:1});",
            "const x = (a.x=1);",
            "return a.x=1;",
            "a.x=1; b.x=2;",
            "a?.x = 1;",
            "super.x=1;",
            "a.#x = 1;",
        ] {
            assert!(
                !valid_replacement(Operator::AssignmentRemove, original, ";", syntax == "ts"),
                "{syntax}: {original}"
            );
        }
    }
    let (_, mutations) = find(
        "export function f(a: any, x: unknown) { (a as any).value = x as number; a!.value = x!; a['x'] = (x satisfies unknown); } type T = { value: true };",
        "ts",
    )?;
    assert_eq!(mutations.len(), 3);
    for m in mutations {
        assert!(valid_replacement(m.operator, m.original(), ";", true));
    }
    Ok(())
}

#[test]
fn assignments_exclude_binding_suspension_value_and_class_initialization_contexts() -> Result<()> {
    for syntax in ["js", "ts"] {
        let source = r#"
let x; x = 1; const y = (a.x = 1); const arrow=()=>a.x=1;
if(a.x=1) {} while(a.x=1) break; for(a.x=1; x; a.x=2) break;
function f(a) { return a.x = 1; }
async function g(a) { a.x = await a(); a[await a()] = 1; a.x = async()=>await a(); }
function* h(a) { a.x = yield 1; a[yield 1] = 2; }
class C extends B {
    constructor() { super(); this.x=1; const f=()=>{this.x=2;}; }
    static { this.x=1; const f=()=>{this.x=2;}; }
    field = (()=>{this.x=1;})();
    set value(v) { this.x=v; }
    method() { this.x=2; }
}
class Quoted { 'constructor'() { this.x=1; } }
class Escaped { constr\u0075ctor() { this.x=1; } }
"#;
        let (_, mutations) = find(source, syntax)?;
        assert_eq!(
            mutations.iter().map(Mutation::original).collect::<Vec<_>>(),
            ["this.x=v;", "this.x=2;"]
        );
    }
    Ok(())
}

async fn execute(runtime: &str, root: &Path, file: &str) -> Result<()> {
    let output = crate::process::run(
        runtime,
        &[file],
        root,
        std::time::Duration::from_secs(10),
        std::num::NonZeroUsize::new(65536),
        &Cancellation::default(),
    )
    .await?;
    ensure!(
        output.status.success() && !output.timed_out && output.capture_error.is_none(),
        "{runtime}: {}",
        output.stderr.text()?
    );
    Ok(())
}

async fn native_contract(runtime: &str) -> Result<()> {
    for syntax in ["js", "ts"] {
        for (statement, trace) in [
            (
                "s.target.value = s.rhs();",
                "['target','rhs','store:value:7']",
            ),
            (
                "s.receiver()[s.key()] = s.rhs();",
                "['receiver','key','rhs','coerce','store:value:7']",
            ),
            (
                "if(s.flag) s.target[0]=s.rhs(); else return null;",
                "['target','rhs','store:0:7']",
            ),
            (
                "label: (s.target.value = s.rhs());",
                "['target','rhs','store:value:7']",
            ),
            (
                "s.target.value = s.rhs()\n",
                "['target','rhs','store:value:7']",
            ),
            (
                "for(let i=0;i<2;i++) s.target.value = s.rhs();",
                "['target','rhs','store:value:7','target','rhs','store:value:7']",
            ),
            (
                "try { s.target.value = s.rhs(); } finally { s.trace.push('finally'); }",
                "['target','rhs','store:value:7','finally']",
            ),
        ] {
            let source = format!("export function run(s) {{ {statement}\n return s.marker; }}");
            let (root, mutations) = find(&source, syntax)?;
            assert_eq!(mutations.len(), 1);
            fs::write(root.path().join("package.json"), r#"{"type":"module"}"#)?;
            let check = |expected: &str| {
                format!(
                    r#"
import assert from 'node:assert/strict'; import {{run}} from './src/assignment.{syntax}';
const trace=[], marker={{}};
const target=new Proxy({{}},{{set(obj,key,value){{trace.push(`store:${{key}}:${{value}}`); return Reflect.set(obj,key,value);}}}});
const s={{trace,marker,flag:true,get target(){{trace.push('target');return target;}},receiver(){{trace.push('receiver');return target;}},key(){{trace.push('key');return {{[Symbol.toPrimitive](){{trace.push('coerce');return 'value';}}}};}},rhs(){{trace.push('rhs');return 7;}}}};
assert.equal(run(s),marker); assert.deepEqual(trace,{expected});
"#
                )
            };
            let file = format!("check.{syntax}");
            fs::write(root.path().join(&file), check(trace))?;
            execute(runtime, root.path(), &file).await?;
            mutations[0].apply(root.path())?;
            fs::write(
                root.path().join(&file),
                check(if statement.contains("finally") {
                    "['finally']"
                } else {
                    "[]"
                }),
            )?;
            execute(runtime, root.path(), &file).await?;
        }
        // Preserve a non-directive string after the removed statement. Using a
        // Function body keeps this control outside ESM's automatic strict mode.
        let source = "function run(s) { s.value=1; 'use strict'; return this; }";
        let (root, mutations) = find(source, syntax)?;
        mutations[0].apply(root.path())?;
        let changed = fs::read_to_string(root.path().join(format!("src/assignment.{syntax}")))?;
        fs::write(
            root.path().join("check.cjs"),
            format!(
                "const assert=require('node:assert/strict'); for(const source of {}) {{const f=new Function(source+'; return run')(); assert.equal(f.call(null, {{}}),globalThis);}}",
                serde_json::to_string(&[source, &changed])?
            ),
        )?;
        execute(runtime, root.path(), "check.cjs").await?;
    }
    Ok(())
}

#[tokio::test]
#[ignore = "requires pinned Node; paired property/index effects and statement boundaries"]
async fn native_node_assignments() -> Result<()> {
    native_contract("node").await
}

#[tokio::test]
#[ignore = "requires pinned Bun; paired property/index effects and statement boundaries"]
async fn native_bun_assignments() -> Result<()> {
    native_contract("bun").await
}
