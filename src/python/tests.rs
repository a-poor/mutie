//! Direct observer probes deliberately bypass static admission to test defenses.
//! Passing a probe does not admit its imports into the supported project scope.
use super::*;
use crate::report::Outcome;
use std::{ffi::OsStr, fs};

#[tokio::test]
#[ignore = "requires MUTIE_PYTHON; shared discovery, mutation writes and classified execution"]
async fn shared_mutation_execution_contract() -> Result<()> {
    let root = tempfile::tempdir()?;
    for name in [
        "pyproject.toml",
        "sample/__init__.py",
        "tests/test_sample.py",
    ] {
        let target = root.path().join(name);
        fs::create_dir_all(target.parent().unwrap())?;
        fs::copy(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/python")
                .join(name),
            target,
        )?;
    }
    let executable = std::path::PathBuf::from(std::env::var("MUTIE_PYTHON")?);
    let cancellation = Cancellation::default();
    let project = project::Project::load(root.path(), &executable, &cancellation).await?;
    let mutations = project.discover(&crate::mutation::Scope::new(&[], &[])?)?;
    assert_eq!(mutations.len(), 2);
    let source = fs::read(root.path().join("sample/__init__.py"))?;
    for (mutation, expected) in mutations.iter().zip([Outcome::Killed, Outcome::Missed]) {
        mutation.apply(root.path())?;
        let modified = fs::read(root.path().join("sample/__init__.py"))?;
        let result = execute(
            root.path(),
            &executable,
            Duration::from_secs(15),
            NonZeroUsize::new(4194304).unwrap(),
            &cancellation,
        )
        .await?;
        assert_eq!(result.command.classify()?, expected);
        assert_eq!(result.nodes.len(), 2);
        assert_eq!(fs::read(root.path().join("sample/__init__.py"))?, modified);
        assert!(!root.path().join(".mutie").exists());
        fs::write(root.path().join("sample/__init__.py"), &source)?;
    }
    // A genuine runtime error and an import/collection assertion cannot become kills.
    for test in [
        "def test_error(): raise RuntimeError('runtime')\n",
        "assert False\ndef test_body(): pass\n",
    ] {
        fs::write(root.path().join("tests/test_sample.py"), test)?;
        let result = execute(
            root.path(),
            &executable,
            Duration::from_secs(15),
            NonZeroUsize::new(4194304).unwrap(),
            &cancellation,
        )
        .await?;
        assert!(result.command.classify().is_err());
        assert!(!result.command.text.text()?.is_empty());
    }
    Ok(())
}

#[tokio::test]
#[ignore = "requires MUTIE_PYTHON; unittest body and lifecycle attribution"]
async fn native_unittest_lifecycle_attribution() -> Result<()> {
    for (name, members, expected) in [
        (
            "pass",
            "def test_body(self): self.assertEqual(1, 1)",
            Some(Outcome::Missed),
        ),
        (
            "assert",
            "def test_body(self): self.assertEqual(1, 2)",
            Some(Outcome::Killed),
        ),
        (
            "runtime",
            "def test_body(self): raise ValueError('body')",
            None,
        ),
        (
            "setup",
            "def setUp(self): self.assertTrue(False)\ndef test_body(self): pass",
            None,
        ),
        (
            "teardown",
            "def tearDown(self): self.assertTrue(False)\ndef test_body(self): pass",
            None,
        ),
        (
            "mixed_teardown",
            "def tearDown(self): self.assertTrue(False)\ndef test_body(self): self.fail('body')",
            None,
        ),
        (
            "cleanup",
            "def test_body(self): self.addCleanup(self.fail, 'cleanup')",
            None,
        ),
        (
            "mixed_cleanup",
            "def test_body(self):\n self.addCleanup(self.fail, 'cleanup')\n self.fail('body')",
            None,
        ),
        (
            "multiple_cleanup",
            "def test_body(self):\n self.addCleanup(self.fail, 'one')\n self.addCleanup(self.fail, 'two')\n self.fail('body')",
            None,
        ),
        (
            "passing_cleanup",
            "def test_body(self):\n self.addCleanup(self.assertTrue, True)\n self.fail('body')",
            Some(Outcome::Killed),
        ),
        (
            "class_setup",
            "@classmethod\ndef setUpClass(cls): assert False\ndef test_body(self): pass",
            None,
        ),
        (
            "class_teardown",
            "@classmethod\ndef tearDownClass(cls): assert False\ndef test_body(self): self.fail('body')",
            None,
        ),
        (
            "class_cleanup",
            "@classmethod\ndef setUpClass(cls): cls.addClassCleanup(lambda: 1/0)\ndef test_body(self): self.fail('body')",
            None,
        ),
        (
            "class_cleanup_pass",
            "@classmethod\ndef setUpClass(cls): cls.addClassCleanup(lambda: None)\ndef test_body(self): self.fail('body')",
            Some(Outcome::Killed),
        ),
        (
            "setup_and_cleanup",
            "def setUp(self):\n self.addCleanup(self.fail, 'cleanup')\n self.fail('setup')\ndef test_body(self): pass",
            None,
        ),
        (
            "suppressed_cleanup",
            "def test_body(self):\n self.addCleanup(lambda: None)\n self.addCleanup(self.fail, 'cleanup')\n self.fail('body')",
            None,
        ),
        (
            "fake_exception",
            "def test_body(self):\n class AssertionError(Exception): pass\n raise AssertionError('fake')",
            None,
        ),
        ("skip", "def test_body(self): self.skipTest('skip')", None),
        (
            "expected_failure",
            "@unittest.expectedFailure\ndef test_body(self): self.fail('expected')",
            None,
        ),
        (
            "subtest",
            "def test_body(self):\n with self.subTest(x=1): self.fail('subtest')",
            None,
        ),
        (
            "override",
            "def run(self, result=None): result.addSuccess(self)\ndef test_body(self): pass",
            None,
        ),
        ("return_value", "def test_body(self): return 1", None),
        (
            "reinitialize",
            "def test_body(self):\n self.addCleanup(self.fail, 'cleanup')\n super().__init__()\n self.fail('body')",
            None,
        ),
    ] {
        let source = format!(
            "import unittest\nclass TestBody(unittest.TestCase):\n {}\n",
            members.replace('\n', "\n ")
        );
        let (raw, result, _) = probe(
            &source,
            Duration::from_secs(15),
            4194304,
            &Cancellation::default(),
        )
        .await?;
        match expected {
            Some(expected) => assert_eq!(
                result.with_context(|| format!(
                    "{name}: {}",
                    raw.stdout.text().unwrap_or_default()
                ))?,
                expected,
                "{name}"
            ),
            None => assert!(
                result.is_err(),
                "{name}: {result:?}\n{}",
                raw.stdout.text()?
            ),
        }
    }
    for prefix in [
        "def setup_module(): assert False\n",
        "def teardown_module(): assert False\n",
        "def setUpModule(): assert False\n",
        "def tearDownModule(): assert False\n",
        "import atexit\natexit.register(lambda: 1/0)\n",
    ] {
        let source = format!(
            "{prefix}import unittest\nclass TestBody(unittest.TestCase):\n def test_body(self): self.fail('body')\n"
        );
        let (_, result, _) = probe(
            &source,
            Duration::from_secs(15),
            4194304,
            &Cancellation::default(),
        )
        .await?;
        assert!(result.is_err(), "{prefix}: {result:?}");
    }
    Ok(())
}

async fn probe(
    source: &str,
    timeout: Duration,
    budget: usize,
    cancellation: &Cancellation,
) -> Result<(process::CommandOutput, Result<Outcome>, tempfile::TempDir)> {
    let directory = tempfile::tempdir()?;
    let owned = directory.path().canonicalize()?;
    let root = owned.join("repo");
    fs::create_dir_all(root.join("tests"))?;
    fs::create_dir_all(root.join("sample"))?;
    fs::write(root.join("sample/__init__.py"), "")?;
    fs::write(root.join("tests/test_probe.py"), source)?;
    let driver = owned.join("driver.py");
    fs::write(&driver, DRIVER)?;
    let config = owned.join("pytest.ini");
    fs::write(&config, "[pytest]\n")?;
    let evidence = owned.join("evidence.jsonl");
    let cache = format!("pycache_prefix={}", owned.join("cache").display());
    let python =
        std::env::var("MUTIE_PYTHON").context("set MUTIE_PYTHON to pinned venv launcher")?;
    let raw = process::run_with_file_limit(
        &python,
        &[
            "-I",
            "-B",
            "-X",
            &cache,
            driver.to_str().unwrap(),
            root.to_str().unwrap(),
            evidence.to_str().unwrap(),
            config.to_str().unwrap(),
            "probe-token",
            "",
            "sample",
            "tests",
        ],
        &owned,
        timeout,
        NonZeroUsize::new(budget).unwrap(),
        cancellation,
        &[
            ("PYTEST_DISABLE_PLUGIN_AUTOLOAD", OsStr::new("1")),
            ("PYTEST_ADDOPTS", OsStr::new("")),
            ("PYTEST_PLUGINS", OsStr::new("")),
            ("HOME", owned.as_os_str()),
            ("TMPDIR", owned.as_os_str()),
        ],
        4 * 1024 * 1024,
    )
    .await?;
    let outcome = read_evidence(
        &raw,
        &evidence,
        "probe-token",
        root.to_str().unwrap(),
        false,
    )
    .map(|(_, outcome)| outcome);
    Ok((raw, outcome, directory))
}

#[tokio::test]
#[ignore = "requires MUTIE_PYTHON; real production observer attribution controls"]
async fn native_body_and_lifetime_attribution() -> Result<()> {
    for (name, source, expected) in [
        (
            "pass",
            "def test_body(): assert True\n",
            Some(Outcome::Missed),
        ),
        (
            "assertion",
            "def test_body(): assert False\n",
            Some(Outcome::Killed),
        ),
        (
            "pytest_fail",
            "import pytest\ndef test_body(): pytest.fail('body')\n",
            Some(Outcome::Killed),
        ),
        (
            "runtime",
            "def test_body(): raise ValueError('body')\n",
            None,
        ),
        (
            "fake_assertion",
            "class AssertionError(Exception): pass\ndef test_body(): raise AssertionError('fake')\n",
            None,
        ),
        (
            "setup",
            "def setup_function(): assert False\ndef test_body(): pass\n",
            None,
        ),
        (
            "mixed_teardown",
            "def teardown_function(): assert False\ndef test_body(): assert False\n",
            None,
        ),
        (
            "late_cleanup",
            "import atexit\ndef fail(): raise ValueError('cleanup')\natexit.register(fail)\ndef test_body(): assert False\n",
            None,
        ),
        (
            "late_stderr",
            "import atexit, sys\natexit.register(lambda: sys.stderr.write('late diagnostic'))\ndef test_body(): pass\n",
            None,
        ),
        (
            "unittest",
            "import unittest\nclass TestBody(unittest.TestCase):\n def setUp(self): assert False\n def test_body(self): pass\n",
            None,
        ),
        (
            "thread",
            "import threading\ndef test_body():\n t=threading.Thread(target=lambda: None)\n t.start()\n t.join()\n",
            None,
        ),
        (
            "escaped_application_import",
            "import sample\nsample.__file__ = '/outside/sample.py'\ndef test_body(): assert False\n",
            None,
        ),
        (
            "warning",
            "import warnings\ndef test_body(): warnings.warn('ambiguous')\n",
            None,
        ),
    ] {
        let (raw, result, _) = probe(
            source,
            Duration::from_secs(15),
            4194304,
            &Cancellation::default(),
        )
        .await?;
        match expected {
            Some(expected) => assert_eq!(result?, expected, "{name}"),
            None => assert!(
                result.is_err(),
                "{name}: {result:?}\n{}",
                raw.stdout.text()?
            ),
        }
    }
    Ok(())
}

#[tokio::test]
#[ignore = "requires MUTIE_PYTHON; real output and per-file limits"]
async fn native_capture_and_file_limits() -> Result<()> {
    for (source, budget, capture_failure) in [
        (
            "def test_body(capsys):\n with capsys.disabled(): print('x'*100000)\n",
            4096,
            true,
        ),
        (
            "def test_body():\n with open('large.bin', 'wb') as f: f.write(b'x'*(5*1024*1024))\n",
            4194304,
            false,
        ),
        (
            "import pytest\n@pytest.mark.parametrize('value',[1], ids=['x'*(5*1024*1024)])\ndef test_body(value): pass\n",
            4194304,
            false,
        ),
    ] {
        let (raw, outcome, directory) = probe(
            source,
            Duration::from_secs(15),
            budget,
            &Cancellation::default(),
        )
        .await?;
        assert!(outcome.is_err());
        if capture_failure {
            assert!(raw.capture_error.is_some(), "{}", raw.stdout.text()?);
        }
        for path in [
            directory.path().join("repo/large.bin"),
            directory.path().join("evidence.jsonl"),
        ] {
            if path.exists() {
                assert!(fs::metadata(path)?.len() <= 4 * 1024 * 1024);
            }
        }
    }
    Ok(())
}

#[tokio::test]
#[ignore = "requires MUTIE_PYTHON; real process-group cleanup on exit, timeout and cancellation"]
async fn native_descendant_shutdown() -> Result<()> {
    for mode in ["exit", "timeout", "cancel"] {
        let markers = tempfile::tempdir()?;
        let ready = markers.path().join("ready");
        let leaked = markers.path().join("leaked");
        let child = format!(
            "import time; time.sleep(5); open({}, 'w').write('leaked')",
            serde_json::to_string(leaked.to_str().unwrap())?
        );
        let source = format!(
            "import subprocess, sys\nfrom pathlib import Path\ndef test_body():\n subprocess.Popen([sys.executable, '-c', {}], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)\n Path({}).write_text('ready')\n {}",
            serde_json::to_string(&child)?,
            serde_json::to_string(ready.to_str().unwrap())?,
            if mode == "exit" {
                "pass\n"
            } else {
                "while True: pass\n"
            }
        );
        let cancellation = Cancellation::default();
        let stopping = cancellation.clone();
        let waiting = ready.clone();
        let cancel = mode == "cancel";
        let stop = tokio::spawn(async move {
            if cancel {
                let deadline = tokio::time::Instant::now() + Duration::from_secs(20);
                while !waiting.exists() {
                    anyhow::ensure!(tokio::time::Instant::now() < deadline, "test did not start");
                    tokio::time::sleep(Duration::from_millis(20)).await;
                }
                stopping.cancel();
            }
            Ok::<_, anyhow::Error>(())
        });
        let result = probe(
            &source,
            if mode == "timeout" {
                Duration::from_secs(3)
            } else {
                Duration::from_secs(20)
            },
            4194304,
            &cancellation,
        )
        .await;
        stop.await??;
        let (raw, outcome, _) = result?;
        assert!(
            ready.exists(),
            "probe did not reach test body: {}",
            raw.stderr.text()?
        );
        assert!(outcome.is_err());
        match mode {
            "exit" => {
                assert!(raw.status.success());
                assert!(raw.group_remaining);
            }
            "timeout" => assert!(raw.timed_out),
            "cancel" => assert!(raw.interrupted),
            _ => unreachable!(),
        }
        tokio::time::sleep(Duration::from_secs(5)).await;
        assert!(!leaked.exists(), "descendant survived {mode}");
    }
    Ok(())
}
