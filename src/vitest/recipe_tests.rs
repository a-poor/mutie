use super::{NodeTool, execution, project::Project};
use crate::{cancellation::Cancellation, report::Outcome, workspace::Workspace};
use anyhow::{Context, Result};
use std::{fs, num::NonZeroUsize, path::Path, time::Duration};

async fn run(
    root: &Path,
    node: &NodeTool,
    seconds: u64,
    bytes: usize,
) -> Result<crate::command::CommandOutput> {
    Ok(execution::run(
        &Project::load(root, &Cancellation::default())?,
        node,
        None,
        Duration::from_secs(seconds),
        NonZeroUsize::new(bytes),
        &Cancellation::default(),
    )
    .await?
    .into_command(NonZeroUsize::new(64 * 1024)))
}

#[tokio::test]
#[ignore = "requires MUTIE_VITEST_RECIPE_FIXTURE with the pinned UFO development tools"]
async fn real_typecheck_completion_and_failure_boundaries() -> Result<()> {
    let input = std::env::var_os("MUTIE_VITEST_RECIPE_FIXTURE")
        .context("set MUTIE_VITEST_RECIPE_FIXTURE")?;
    let workspace = Workspace::create(Path::new(&input), &[], &Cancellation::default())?;
    let root = workspace.path();
    // This is a generated control fixture, not a reduced upstream acceptance run.
    for directory in ["src", "test"] {
        fs::remove_dir_all(root.join(directory))?;
        fs::create_dir(root.join(directory))?;
    }
    let mut manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join("package.json"))?)?;
    manifest["scripts"]["test"] = "vitest run --typecheck".into();
    fs::write(
        root.join("package.json"),
        serde_json::to_vec_pretty(&manifest)?,
    )?;
    let source = "export const ready = (): boolean => true;\n";
    let body = "import { test, expect } from 'vitest'; import { ready } from '../src/ready'; test('body', () => expect(ready()).toBe(true));\n";
    let types = "import { test, expectTypeOf } from 'vitest'; import { ready } from '../src/ready'; test('type', () => expectTypeOf(ready()).toEqualTypeOf<boolean>());\n";
    fs::write(root.join("src/ready.ts"), source)?;
    fs::write(root.join("test/ready.test.ts"), body)?;
    fs::write(root.join("test/types.test-d.ts"), types)?;
    // A copied manager launcher must never run or refer back to its input tree.
    fs::remove_file(root.join("node_modules/.bin/tsc"))?;
    fs::write(
        root.join("node_modules/.bin/tsc"),
        "#!/bin/sh\necho MUTIE_WRONG_LAUNCHER >&2\nexit 92\n",
    )?;
    let node = super::resolve_node(&root, &Cancellation::default()).await?;
    assert_eq!(
        run(&root, &node, 30, 4 * 1024 * 1024).await?.classify()?,
        Outcome::Missed
    );
    for (label, source_text, body_text, type_text, expected) in [
        (
            "body",
            source.replace("true", "false"),
            body.to_owned(),
            types.to_owned(),
            "killed",
        ),
        (
            "source-type-error",
            source.replace("true", "'wrong'"),
            body.to_owned(),
            types.to_owned(),
            "error",
        ),
        (
            "type-assertion",
            source.to_owned(),
            body.to_owned(),
            types.replace("<boolean>", "<number>"),
            "error",
        ),
        (
            "body-and-type",
            source.replace("true", "false"),
            body.to_owned(),
            types.replace("<boolean>", "<number>"),
            "error",
        ),
        (
            "body-and-hook",
            source.replace("true", "false"),
            format!(
                "import {{afterEach}} from 'vitest'; afterEach(()=>{{throw new Error('cleanup')}}); {body}"
            ),
            types.to_owned(),
            "error",
        ),
    ] {
        fs::write(root.join("src/ready.ts"), source_text)?;
        fs::write(root.join("test/ready.test.ts"), body_text)?;
        fs::write(root.join("test/types.test-d.ts"), type_text)?;
        let output = run(&root, &node, 30, 4 * 1024 * 1024).await?;
        match expected {
            "killed" => assert_eq!(output.classify()?, Outcome::Killed, "{label}"),
            _ => assert!(
                output.classify().is_err(),
                "{label}: {:?}",
                output.classify()
            ),
        }
    }
    fs::write(root.join("src/ready.ts"), source)?;
    fs::write(root.join("test/ready.test.ts"), body)?;
    fs::write(root.join("test/types.test-d.ts"), types)?;
    let compiler = super::recipe::tool(&root, "typescript", "6.0.3", "bin/tsc")?;
    let original = fs::read(&compiler)?;
    for (label, script, seconds, bytes, expected) in [
        (
            "compiler-exit",
            "process.exit(2);",
            30,
            4 * 1024 * 1024,
            "error",
        ),
        (
            "compiler-signal",
            "process.kill(process.pid, 'SIGTERM');",
            30,
            4 * 1024 * 1024,
            "error",
        ),
        (
            "compiler-timeout",
            "setInterval(()=>{}, 1000);",
            2,
            4 * 1024 * 1024,
            "timeout",
        ),
        (
            "compiler-capture",
            "console.log('x'.repeat(100000)); process.exit(2);",
            30,
            4096,
            "error",
        ),
    ] {
        // Replace the inode, preserving any hard-linked dependency store inputs.
        fs::remove_file(&compiler)?;
        fs::write(&compiler, script)?;
        let output = run(&root, &node, seconds, bytes).await?;
        if expected == "timeout" {
            assert_eq!(output.classify()?, Outcome::Timeout, "{label}");
        } else {
            assert!(output.classify().is_err(), "{label}");
        }
    }
    fs::remove_file(&compiler)?;
    fs::write(&compiler, original)?;
    assert_eq!(
        run(&root, &node, 30, 4 * 1024 * 1024).await?.classify()?,
        Outcome::Missed
    );
    Ok(())
}

#[tokio::test]
#[ignore = "requires MUTIE_VITEST_RECIPE_FIXTURE with the pinned UFO development tools"]
async fn real_lint_prerequisites_fail_closed_and_bind_reruns() -> Result<()> {
    let input = std::env::var_os("MUTIE_VITEST_RECIPE_FIXTURE")
        .context("set MUTIE_VITEST_RECIPE_FIXTURE")?;
    let workspace = Workspace::create(Path::new(&input), &[], &Cancellation::default())?;
    let root = workspace.path();
    let node = super::resolve_node(&root, &Cancellation::default()).await?;
    let identity = super::execution_scope(&root, &Cancellation::default()).await?;
    assert_eq!(identity.recipe, super::recipe::Recipe::LintTypecheck);
    assert_eq!(
        run(&root, &node, 30, 4 * 1024 * 1024).await?.classify()?,
        Outcome::Missed
    );
    let control = root.join("src/lint-control.ts");
    for code in [
        "export const invalid = ;\n",
        "export const formatProbe=1;\n",
    ] {
        fs::write(&control, code)?;
        let output = run(&root, &node, 30, 4 * 1024 * 1024).await?;
        assert!(output.classify().is_err());
        assert!(format!("{:#}", output.classify().unwrap_err()).contains("prerequisite"));
    }
    fs::remove_file(control)?;
    // Formatting configuration is execution scope, not a source mutation.
    let config = root.join(".editorconfig");
    let text = fs::read_to_string(&config)?;
    fs::write(&config, format!("{text}\n# changed\n"))?;
    let changed = super::execution_scope(&root, &Cancellation::default()).await?;
    assert!(identity.validate_rerun(&changed).is_err());
    fs::write(config, text)?;
    let config = root.join("eslint.config.mjs");
    let text = fs::read_to_string(&config)?;
    fs::write(
        config,
        format!("{text}\nthrow new Error('must not execute');\n"),
    )?;
    assert!(Project::load(&root, &Cancellation::default()).is_err());
    Ok(())
}

#[tokio::test]
#[ignore = "requires MUTIE_VITEST_RECIPE_FIXTURE; composed command resource budgets"]
async fn real_composed_recipe_shares_time_and_output_limits() -> Result<()> {
    let input = std::env::var_os("MUTIE_VITEST_RECIPE_FIXTURE")
        .context("set MUTIE_VITEST_RECIPE_FIXTURE")?;
    let workspace = Workspace::create(Path::new(&input), &[], &Cancellation::default())?;
    let root = workspace.path();
    let node = super::resolve_node(&root, &Cancellation::default()).await?;
    let eslint = super::recipe::tool(&root, "eslint", "10.2.1", "bin/eslint.js")?;
    let prettier = super::recipe::tool(&root, "prettier", "3.8.3", "bin/prettier.cjs")?;
    for path in [&eslint, &prettier] {
        fs::remove_file(path)?;
        fs::write(path, "process.stdout.write('x'.repeat(3000));\n")?;
    }
    let output = run(&root, &node, 30, 4096).await?;
    assert!(
        output.capture_error.is_some(),
        "each prerequisite individually fits, but their combined output must not"
    );
    assert!(output.classify().is_err());
    assert!(output.stdout.retention()?.total_bytes <= 4096);
    for path in [&eslint, &prettier] {
        fs::remove_file(path)?;
        fs::write(path, "setTimeout(() => {}, 1500);\n")?;
    }
    let output = run(&root, &node, 2, 4096).await?;
    assert_eq!(
        output.classify()?,
        Outcome::Timeout,
        "prerequisites must share a deadline"
    );
    Ok(())
}
