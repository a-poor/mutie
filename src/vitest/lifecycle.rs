//! Worker failure-phase evidence for the pinned runner's unreported lifecycle APIs.
//! Only the disposable dependency copy is instrumented. Unknown bytes fail closed.
use anyhow::{Context, Result, ensure};
use sha2::{Digest, Sha256};
use std::{fs, path::Path};

const RUNNER_SHA256: &str = "c10a7a80fca65fb42e27eabf7655c5ce5ecf36e8aaeb1811499efd063df62556";
// 4.1.5 differs only in concurrent scheduling. The failure and fixture dispatch
// sites below are identical; both versions must pass the native lifecycle suite.
const RUNNER_415_SHA256: &str = "da3f73e5ec0b717015581d52727b49c67cbbe9cc64ebe34bfa0dc0ded1d133e7";

// Each replacement is checked against the exact audited source and expected count.
const REPLACEMENTS: &[(&str, &str, usize)] = &[
    (
        "\tconst suite = test.suite || test.file;\n",
        "\ttest.meta.mutieLifecycleEvidence = true;\n\ttest.meta.mutieBodyEntered = false;\n\ttest.meta.mutieBodyFailed = false;\n\ttest.meta.mutieLifecycleFailed = false;\n\tconst suite = test.suite || test.file;\n",
        1,
    ),
    (
        "withFixtures(handler, { context })",
        "withFixtures(handler, { context, mutieBody: true })",
        1,
    ),
    (
        "return fn(context);",
        "if (options?.mutieBody) context.task.meta.mutieBodyEntered = !(context.task.promises?.length); return fn(context);",
        5,
    ),
    // Assertion-count checks can be configured by hooks; keep their failures
    // conservative rather than attributing them to the body by timing alone.
    (
        "\t\t\t\t\tawait runner.onAfterTryTask?.(test, {",
        "\t\t\t\t\ttest.meta.mutieBodyEntered = false;\n\t\t\t\t\tawait runner.onAfterTryTask?.(test, {",
        1,
    ),
    // Initially mark every task failure site as lifecycle; replace the single
    // callback/fixture-setup catch below with the body-entry distinction.
    (
        "failTask(test.result, e, runner.config.diffOptions);",
        "test.meta.mutieLifecycleFailed = true; failTask(test.result, e, runner.config.diffOptions);",
        6,
    ),
    (
        "failTask(test.result, error, runner.config.diffOptions);",
        "test.meta.mutieLifecycleFailed = true; failTask(test.result, error, runner.config.diffOptions);",
        1,
    ),
    (
        "test.meta.mutieLifecycleFailed = true; failTask(test.result, e, runner.config.diffOptions);\n\t\t\t\t}\n\t\t\t\ttry {\n\t\t\t\t\tawait runner.onTaskFinished?.(test);",
        "if (test.meta.mutieBodyEntered) test.meta.mutieBodyFailed = true; else test.meta.mutieLifecycleFailed = true;\n\t\t\t\t\tfailTask(test.result, e, runner.config.diffOptions);\n\t\t\t\t}\n\t\t\t\ttry {\n\t\t\t\t\tawait runner.onTaskFinished?.(test);",
        1,
    ),
];

pub fn instrument(root: &Path) -> Result<()> {
    let vitest = root
        .join("node_modules/vitest")
        .canonicalize()
        .context("locating installed Vitest")?;
    let runner = super::project::resolve_dependency(root, &vitest, "@vitest/runner")?;
    let resolved = runner
        .join("dist/chunk-artifact.js")
        .canonicalize()
        .context("locating pinned Vitest worker runner")?;
    ensure!(
        resolved.starts_with(root.join("node_modules")),
        "Vitest worker runner escapes isolated project"
    );
    let source = fs::read(&resolved).context("reading pinned Vitest worker runner")?;
    let patched = patch(&source)?;
    // Replace the inode, avoiding changes through any dependency-store hard links.
    let mut replacement = tempfile::NamedTempFile::new_in(resolved.parent().unwrap())?;
    use std::io::Write;
    replacement.write_all(patched.as_bytes())?;
    replacement
        .persist(&resolved)
        .context("installing isolated Vitest lifecycle evidence")?;
    Ok(())
}

fn patch(source: &[u8]) -> Result<String> {
    ensure!(
        [RUNNER_SHA256, RUNNER_415_SHA256]
            .contains(&format!("{:x}", Sha256::digest(source)).as_str()),
        "unsupported Vitest worker runner bytes; lifecycle evidence cannot be established"
    );
    let mut source = String::from_utf8(source.to_vec())?;
    for (anchor, replacement, count) in REPLACEMENTS {
        ensure!(
            source.matches(anchor).count() == *count,
            "ambiguous Vitest lifecycle instrumentation anchor"
        );
        source = source.replace(anchor, replacement);
    }
    Ok(source)
}

#[cfg(test)]
mod tests {
    #[test]
    fn unknown_runner_bytes_are_rejected() {
        assert!(
            super::patch(b"export default {};")
                .unwrap_err()
                .to_string()
                .contains("unsupported Vitest worker runner bytes")
        );
    }
}
