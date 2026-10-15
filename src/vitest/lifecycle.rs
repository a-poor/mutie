//! Conservative worker evidence for the pinned runner's unreported lifecycle APIs.
//! Only the disposable dependency copy is instrumented. Unknown bytes fail closed.
use anyhow::{Context, Result, ensure};
use sha2::{Digest, Sha256};
use std::{fs, path::Path};

const RUNNER_SHA256: &str = "c10a7a80fca65fb42e27eabf7655c5ce5ecf36e8aaeb1811499efd063df62556";

const INSERTIONS: &[(&str, &str)] = &[
    (
        "\tconst suite = test.suite || test.file;\n",
        "\ttest.meta.mutieLifecycleEvidence = true;\n\ttest.meta.mutieLifecycle = getAroundEachHooks(suite).length > 0;\n",
    ),
    (
        "\t\tconst pendingFixtures = resolveDeps(usedFixtures, registrations);\n",
        "\t\tif (pendingFixtures.length && context.task) context.task.meta.mutieLifecycle = true;\n",
    ),
    (
        "async function callTestHooks(runner, test, hooks, sequence) {\n",
        "\tif (hooks.length) test.meta.mutieLifecycle = true;\n",
    ),
];

pub fn instrument(root: &Path) -> Result<()> {
    // pnpm's pinned virtual-store path; Project::load validates self-contained deps.
    let path = root.join("node_modules/.pnpm/@vitest+runner@4.1.9/node_modules/@vitest/runner/dist/chunk-artifact.js");
    let resolved = path
        .canonicalize()
        .context("locating pinned Vitest worker runner")?;
    ensure!(
        resolved.starts_with(root),
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
        format!("{:x}", Sha256::digest(source)) == RUNNER_SHA256,
        "unsupported Vitest worker runner bytes; lifecycle evidence cannot be established"
    );
    let mut source = String::from_utf8(source.to_vec())?;
    for (anchor, insertion) in INSERTIONS {
        ensure!(
            source.matches(anchor).count() == 1,
            "ambiguous Vitest lifecycle instrumentation anchor"
        );
        source = source.replacen(anchor, &format!("{anchor}{insertion}"), 1);
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
