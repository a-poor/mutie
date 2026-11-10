//! Native Node controls with deterministic prerequisite programs. Actual ESLint,
//! documentation and markdown tools are covered by the external project audit.
use super::*;
use crate::report::Outcome;

fn fixture() -> Result<tempfile::TempDir> {
    let temp = tempfile::tempdir()?;
    let root = temp.path();
    for directory in ["rules", "test/unit", "scripts"] {
        fs::create_dir_all(root.join(directory))?;
    }
    let scripts = recipe::SCRIPTS
        .iter()
        .map(|(name, text)| ((*name).to_owned(), serde_json::Value::from(*text)))
        .collect::<serde_json::Map<_, _>>();
    let mut dependencies = serde_json::Map::new();
    for (name, version, executable) in recipe::TOOLS {
        dependencies.insert((*name).into(), (*version).into());
        let directory = root.join("node_modules").join(name);
        fs::create_dir_all(directory.join(executable).parent().unwrap())?;
        fs::write(
            directory.join("package.json"),
            serde_json::to_vec(
                &serde_json::json!({"name":name,"version":version,"type":"module"}),
            )?,
        )?;
        fs::write(
            directory.join(executable),
            "process.stdout.write('prerequisite completed\\n');",
        )?;
    }
    fs::write(
        root.join("package.json"),
        serde_json::to_vec(&serde_json::json!({
            "name":"composed-node-control", "type":"module", "exports":{"default":"./index.js"},
            "scripts":scripts,"devDependencies":dependencies
        }))?,
    )?;
    for name in [
        "eslint.config.js",
        "eslint.dogfooding.config.js",
        ".eslint-doc-generatorrc.js",
    ] {
        fs::write(root.join(name), "export default {};")?;
    }
    for name in [".markdownlint.json", ".npmpackagejsonlintrc.json"] {
        fs::write(root.join(name), "{}")?;
    }
    fs::write(root.join(".markdownlintignore"), "node_modules\n")?;
    fs::write(
        root.join("index.js"),
        "import meta from './package.json' with {type:'json'}; export {enabled} from './rules/flags.js'; export const name=meta.name;",
    )?;
    fs::write(root.join("rules/flags.js"), "export const enabled=true;")?;
    fs::write(
        root.join("test/body.js"),
        "import {test} from 'node:test'; import assert from 'node:assert/strict'; import {enabled,name} from '../index.js'; test('enabled',()=>{assert.equal(enabled,true);assert.equal(name,'composed-node-control');});",
    )?;
    fs::write(
        root.join("test/unit/second.js"),
        "import {test} from 'node:test'; test('second directory',()=>{});",
    )?;
    Ok(temp)
}

async fn execute(root: &Path, limit: u64, capture: usize) -> Result<command::CommandOutput> {
    let cancellation = Cancellation::default();
    let expected = execution_scope(root, &cancellation).await?;
    // Spawn onto an ordinary Tokio worker: this catches nested debug futures
    // exhausting its default stack, rather than hiding it on the test thread.
    let path = root.to_owned();
    tokio::spawn(async move {
        test(
            &path,
            Duration::from_secs(limit),
            NonZeroUsize::new(1024),
            NonZeroUsize::new(capture),
            &expected,
            &cancellation,
        )
        .await
    })
    .await
    .context("composed Node worker panicked")?
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires audited Node 24.22.0 on PATH; native prerequisite/body/lifecycle controls"]
async fn native_composed_outcomes_and_rerun_identity() -> Result<()> {
    let fixture = fixture()?;
    let root = fixture.path();
    let original = fs::read(root.join("rules/flags.js"))?;
    let expected = execution_scope(root, &Cancellation::default()).await?;
    assert_eq!(execute(root, 10, 65536).await?.classify()?, Outcome::Missed);
    fs::write(root.join("rules/flags.js"), "export const enabled=false;")?;
    assert_eq!(execute(root, 10, 65536).await?.classify()?, Outcome::Killed);
    fs::write(
        root.join("test/unit/second.js"),
        "import {test,afterEach} from 'node:test'; afterEach(()=>{throw new Error('cleanup');}); test('second',()=>{});",
    )?;
    assert!(execute(root, 10, 65536).await?.classify().is_err());
    fs::write(
        root.join("test/unit/second.js"),
        "import {test} from 'node:test'; test('second directory',()=>{});",
    )?;
    let lint = root.join("node_modules/eslint/bin/eslint.js");
    fs::write(
        &lint,
        "process.stdout.write('lint failed\\n'); process.exitCode=1;",
    )?;
    let mixed = execute(root, 10, 65536).await?;
    assert!(mixed.classify().is_err());
    assert!(
        mixed.text.text()?.contains("enabled"),
        "runtime tests must still execute after ordinary lint failures"
    );
    fs::write(root.join("rules/flags.js"), &original)?;
    assert!(execute(root, 10, 65536).await?.classify().is_err());
    fs::write(&lint, "process.stdout.write('prerequisite completed\\n');")?;
    let current = execution_scope(root, &Cancellation::default()).await?;
    expected.validate_rerun(&current)?;
    fs::write(
        root.join("test/unit/extra.js"),
        "import {test} from 'node:test'; test('stronger test',()=>{});",
    )?;
    expected.validate_rerun(&execution_scope(root, &Cancellation::default()).await?)?;
    fs::write(
        root.join("eslint.config.js"),
        "export default {changed:true};",
    )?;
    assert!(
        expected
            .validate_rerun(&execution_scope(root, &Cancellation::default()).await?)
            .is_err()
    );
    assert_eq!(fs::read(root.join("rules/flags.js"))?, original);
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires audited Node 24.22.0 on PATH; shared time/output and process cleanup controls"]
async fn native_composed_shared_budgets() -> Result<()> {
    let fixture = fixture()?;
    let root = fixture.path();
    let lint = root.join("node_modules/eslint/bin/eslint.js");
    let docs = root.join("node_modules/eslint-doc-generator/dist/bin/eslint-doc-generator.js");
    fs::write(&lint, "process.stdout.write('x'.repeat(3000));")?;
    fs::write(&docs, "process.stdout.write('x'.repeat(3000));")?;
    let noisy = execute(root, 10, 4096).await?;
    assert!(noisy.raw.capture_error.is_some());
    assert!(noisy.classify().is_err());
    fs::write(
        &lint,
        "await new Promise(resolve=>setTimeout(resolve,1500));",
    )?;
    fs::write(
        &docs,
        "await new Promise(resolve=>setTimeout(resolve,1500));",
    )?;
    assert_eq!(execute(root, 2, 65536).await?.classify()?, Outcome::Timeout);
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires audited Node 24.22.0; helper locations, repeated worker IDs and lifecycle mixtures"]
async fn native_imported_helper_ownership() -> Result<()> {
    let fixture = fixture()?;
    let root = fixture.path();
    fs::create_dir(root.join("test/helpers"))?;
    fs::write(
        root.join("test/helpers/shared.js"),
        "import {test} from 'node:test'; import assert from 'node:assert/strict'; export function register(actual){test('same name',()=>assert.equal(actual,true));}",
    )?;
    for (first, second) in [(false, true), (true, false), (false, false), (true, true)] {
        fs::write(
            root.join("test/body.js"),
            format!("import {{register}} from './helpers/shared.js'; register({first});"),
        )?;
        fs::write(
            root.join("test/unit/second.js"),
            format!("import {{register}} from '../helpers/shared.js'; register({second});"),
        )?;
        assert_eq!(
            execute(root, 10, 65536).await?.classify()?,
            if first && second {
                Outcome::Missed
            } else {
                Outcome::Killed
            }
        );
    }
    fs::write(
        root.join("test/body.js"),
        "import {register} from './helpers/shared.js'; register(false);",
    )?;
    fs::write(
        root.join("test/unit/second.js"),
        "import {afterEach} from 'node:test'; import {register} from '../helpers/shared.js'; afterEach(()=>{throw new Error('cleanup');}); register(false);",
    )?;
    assert!(execute(root, 10, 65536).await?.classify().is_err());
    fs::write(
        root.join("test/unit/second.js"),
        "import {test} from 'node:test'; test('plan',t=>{t.plan(1);});",
    )?;
    assert!(execute(root, 10, 65536).await?.classify().is_err());
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires audited Node 24.22.0; deferred dependency loading must not supply body kills"]
async fn native_dependency_loading_failures_remain_errors() -> Result<()> {
    let fixture = fixture()?;
    let root = fixture.path();
    let manifest = root.join("package.json");
    let mut value: serde_json::Value = serde_json::from_slice(&fs::read(&manifest)?)?;
    value["devDependencies"]["lazy"] = "1.0.0".into();
    fs::write(manifest, serde_json::to_vec(&value)?)?;
    let package = root.join("node_modules/lazy");
    fs::create_dir(&package)?;
    fs::write(
        package.join("package.json"),
        r#"{"name":"lazy","version":"1.0.0","type":"module","exports":"./index.js"}"#,
    )?;
    fs::write(
        package.join("throw.cjs"),
        "throw new Error('CJS evaluation');",
    )?;
    fs::write(
        package.join("throw.js"),
        "throw new Error('ESM evaluation');",
    )?;
    fs::write(package.join("pending.js"), "await new Promise(()=>{});")?;
    fs::write(package.join("value.cjs"), "module.exports=false;")?;
    fs::write(package.join("value.js"), "export default false;")?;
    fs::write(
        root.join("test/body.js"),
        "import {test} from 'node:test'; import assert from 'node:assert/strict'; import {run} from 'lazy'; test('lazy dependency',async()=>assert.equal(await run(),true));",
    )?;
    for (name, body, killed) in [
        (
            "ordinary dependency body exception",
            "throw new Error('ordinary body');",
            true,
        ),
        ("missing CJS module", "return load('./missing.cjs');", false),
        ("CJS evaluation", "return load('./throw.cjs');", false),
        ("ESM evaluation", "return import('./throw.js');", false),
        (
            "caught loader error",
            "try {load('./missing.cjs');} catch {} return true;",
            false,
        ),
        (
            "unsettled import",
            "void import('./pending.js'); return true;",
            false,
        ),
        ("successful CJS load", "return load('./value.cjs');", true),
        (
            "caught exit-time loader error",
            "process.on('exit',()=>{try {load('./missing.cjs');} catch {}}); throw new Error('body');",
            false,
        ),
        (
            "loader rejection after exit listeners",
            "process.on('exit',()=>{void Promise.resolve().then(()=>import('./throw.js')).catch(()=>{});}); throw new Error('body');",
            false,
        ),
        (
            "successful ESM load",
            "return import('./value.js').then(module=>module.default);",
            true,
        ),
    ] {
        fs::write(
            package.join("index.js"),
            format!(
                "import {{createRequire}} from 'node:module'; const load=createRequire(import.meta.url); export function run(){{{body}}}"
            ),
        )?;
        let result = execute(root, 10, 65536).await?.classify();
        if killed {
            assert_eq!(result?, Outcome::Killed, "{name}");
        } else {
            assert!(result.is_err(), "{name}: {result:?}");
        }
    }
    Ok(())
}
