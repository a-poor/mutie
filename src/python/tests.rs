//! Direct observer probes deliberately bypass static admission to test defenses.
//! Passing a probe does not admit its imports into the supported project scope.
use super::*;
use crate::report::Outcome;
use std::{ffi::OsStr, fs, path::PathBuf};

#[tokio::test]
#[ignore = "requires MUTIE_PYTHON_DEPENDENCY; audited webencodings environment"]
async fn declared_dependency_preserves_body_and_lifecycle_attribution() -> Result<()> {
    let root = tempfile::tempdir()?;
    fs::create_dir(root.path().join("sample"))?;
    fs::create_dir(root.path().join("tests"))?;
    fs::write(
        root.path().join("pyproject.toml"),
        "[project]\nname='sample'\ndependencies=['webencodings>=0.4']\n",
    )?;
    let source = root.path().join("sample/__init__.py");
    fs::write(
        &source,
        "from webencodings import decode, UTF8\ndef value():\n return decode(b'hello', UTF8)[0]\n",
    )?;
    let tests = root.path().join("tests/test_sample.py");
    fs::write(
        &tests,
        "from sample import value\ndef test_value(): assert value() == 'hello'\n",
    )?;
    let executable = PathBuf::from(std::env::var("MUTIE_PYTHON_DEPENDENCY")?);
    let timeout = Duration::from_secs(15);
    let limit = NonZeroUsize::new(4194304).unwrap();
    let (identity, collection) = execution_scope(
        root.path(),
        &executable,
        timeout,
        limit,
        &Cancellation::default(),
    )
    .await?;
    assert_eq!(collection.classify()?, Outcome::Missed);
    let check = |identity: ExecutionScope, root: PathBuf| async move {
        test(
            &root,
            timeout,
            None,
            Some(limit),
            &identity,
            &Cancellation::default(),
        )
        .await
    };
    assert_eq!(
        check(identity.clone(), root.path().into())
            .await?
            .classify()?,
        Outcome::Missed
    );
    fs::write(
        &source,
        "from webencodings import decode, UTF8\ndef value():\n return decode(b'other', UTF8)[0]\n",
    )?;
    assert_eq!(
        check(identity.clone(), root.path().into())
            .await?
            .classify()?,
        Outcome::Killed
    );
    fs::write(
        &source,
        "from webencodings import decode, UTF8\ndef value():\n return decode(bytes([255]), UTF8, errors='strict')[0]\n",
    )?;
    assert!(
        check(identity.clone(), root.path().into())
            .await?
            .classify()
            .is_err()
    );
    fs::write(
        &source,
        "from webencodings import decode, UTF8\ndef value():\n return decode(b'hello', UTF8)[0]\n",
    )?;
    fs::write(
        &tests,
        "import pytest\nfrom sample import value\n@pytest.fixture(autouse=True)\ndef cleanup():\n yield\n assert False\ndef test_value(): assert value() == 'hello'\n",
    )?;
    assert!(
        check(identity.clone(), root.path().into())
            .await?
            .classify()
            .is_err()
    );
    // An installed audited package is insufficient without an explicit declaration.
    fs::write(
        root.path().join("pyproject.toml"),
        "[project]\nname='sample'\n",
    )?;
    assert!(
        execution_scope(
            root.path(),
            &executable,
            timeout,
            limit,
            &Cancellation::default()
        )
        .await
        .is_err()
    );
    Ok(())
}

#[tokio::test]
#[ignore = "requires MUTIE_PYTHON; copied-input reads and confined temporary file output"]
async fn native_project_file_access_boundaries() -> Result<()> {
    let outside = tempfile::tempdir()?;
    let sentinel = outside.path().join("sentinel");
    fs::write(&sentinel, b"unchanged")?;
    let outside_path = format!("{:?}", sentinel);
    let outside_dir = format!("{:?}", outside.path());
    for (name, body, passing) in [
        ("read_input", "assert Path(__file__).read_bytes()".to_owned(), true),
        ("temporary_output", "p = tmp_path / 'value'\n with open(p, 'wb') as f: f.write(b'ok')\n assert p.read_bytes() == b'ok'".to_owned(), true),
        ("input_write", "with pytest.raises(PermissionError): Path(__file__).write_text('changed')".to_owned(), false),
        ("input_remove", "with pytest.raises(PermissionError): Path(__file__).unlink()".to_owned(), false),
        ("external_read", format!("with pytest.raises(PermissionError): Path({outside_path}).read_bytes()"), false),
        ("external_write", format!("with pytest.raises(PermissionError): Path({outside_path}).write_bytes(b'changed')"), false),
        ("external_scan", format!("with pytest.raises(PermissionError): list(Path({outside_dir}).iterdir())"), false),
        ("descriptor", "with open(1, 'wb', closefd=False) as stream: stream.write(b'ok')".to_owned(), true),
        ("owned_directory_descriptor", "import os\n descriptor = os.open(tmp_path, os.O_RDONLY)\n try:\n  assert list(os.scandir(descriptor)) == []\n  os.mkdir('nested', dir_fd=descriptor)\n  os.rmdir('nested', dir_fd=descriptor)\n finally: os.close(descriptor)".to_owned(), true),
        ("relative_symlink", "import os\n (tmp_path / 'value').write_text('ok')\n os.symlink('value', tmp_path / 'link')\n assert (tmp_path / 'link').read_text() == 'ok'".to_owned(), true),
        ("changed_home", format!("import os\n os.environ['HOME'] = {outside_dir}\n with pytest.raises(PermissionError): Path({outside_path}).read_bytes()"), false),
        ("openat_owned", "import os\n directory = os.open(tmp_path, os.O_RDONLY)\n try:\n  descriptor = os.open('created', os.O_WRONLY | os.O_CREAT, dir_fd=directory)\n  os.write(descriptor, b'ok')\n  os.close(descriptor)\n  assert (tmp_path / 'created').read_bytes() == b'ok'\n finally: os.close(directory)".to_owned(), true),
        ("openat_source_write", "import os\n directory = os.open(Path(__file__).parent, os.O_RDONLY)\n os.chdir(tmp_path)\n try:\n  with pytest.raises(PermissionError):\n   descriptor = os.open('test_probe.py', os.O_WRONLY, dir_fd=directory)\n   os.close(descriptor)\n finally: os.close(directory)".to_owned(), false),
        ("symlink_escape", format!("with pytest.raises(PermissionError): (tmp_path / 'link').symlink_to({outside_path})"), false),
    ] {
        let source = format!("from pathlib import Path\nimport pytest\ndef test_body(tmp_path):\n {body}\n");
        let (_, outcome, owned) = probe(&source, Duration::from_secs(15), 4194304, &Cancellation::default()).await?;
        if passing {
            assert_eq!(outcome?, Outcome::Missed, "{name}");
        } else {
            assert!(outcome.is_err(), "{name}");
        }
        assert_eq!(fs::read_to_string(owned.path().join("repo/tests/test_probe.py"))?, source, "{name}");
        assert_eq!(fs::read(&sentinel)?, b"unchanged", "{name}");
    }
    Ok(())
}

#[tokio::test]
#[ignore = "requires MUTIE_PYTHON; independent thread and unraisable attribution"]
async fn native_thread_lifetime_attribution() -> Result<()> {
    for (name, source, expected) in [
        (
            "joined",
            "def test_body():\n t=threading.Thread(target=lambda: None)\n t.start()\n t.join()",
            Some(Outcome::Missed),
        ),
        (
            "joined_body_failure",
            "def test_body():\n t=threading.Thread(target=lambda: None)\n t.start()\n t.join()\n assert False",
            Some(Outcome::Killed),
        ),
        (
            "start_twice",
            "def test_body():\n t=threading.Thread(target=lambda: None)\n t.start()\n t.join()\n with pytest.raises(RuntimeError): t.start()",
            Some(Outcome::Missed),
        ),
        (
            "thread_assertion",
            "def test_body():\n def fail(): assert False\n t=threading.Thread(target=fail)\n t.start()\n t.join()",
            None,
        ),
        (
            "suppressed_thread",
            "def test_body():\n previous=threading.excepthook\n threading.excepthook=lambda args: None\n try:\n  def fail(): assert False\n  t=threading.Thread(target=fail)\n  t.start()\n  t.join()\n finally: threading.excepthook=previous",
            None,
        ),
        (
            "mixed",
            "def test_body():\n def fail(): raise ValueError('thread failure')\n t=threading.Thread(target=fail)\n t.start()\n t.join()\n assert False",
            None,
        ),
        (
            "late_thread",
            "def test_body():\n threading.Thread(target=lambda: time.sleep(1), daemon=True).start()",
            None,
        ),
        (
            "raw_thread",
            "def test_body():\n import _thread\n _thread.start_new_thread(lambda: None, ())\n time.sleep(.02)",
            None,
        ),
        (
            "suppressed_destructor",
            "def test_body():\n previous=sys.unraisablehook\n sys.unraisablehook=lambda args: None\n try:\n  class Broken:\n   def __del__(self): raise ValueError('cleanup')\n  Broken()\n finally: sys.unraisablehook=previous",
            None,
        ),
        (
            "finalizer_thread",
            "@pytest.fixture\ndef cleanup():\n yield\n def fail(): assert False\n t=threading.Thread(target=fail)\n t.start()\n t.join()\ndef test_body(cleanup): assert False",
            None,
        ),
    ] {
        let source = format!("import pytest, threading, time, sys\n{source}\n");
        let (_, outcome, _) = probe(
            &source,
            Duration::from_secs(15),
            4194304,
            &Cancellation::default(),
        )
        .await?;
        match expected {
            Some(expected) => assert_eq!(outcome?, expected, "{name}"),
            None => assert!(outcome.is_err(), "{name}"),
        }
    }
    Ok(())
}

#[tokio::test]
#[ignore = "requires MUTIE_PYTHON; pristine expected-failure identity and lifecycle controls"]
async fn expected_failures_bind_to_pristine_bodies() -> Result<()> {
    let root = tempfile::tempdir()?;
    fs::create_dir(root.path().join("sample"))?;
    fs::create_dir(root.path().join("tests"))?;
    fs::write(
        root.path().join("pyproject.toml"),
        "[project]\nname='sample'\n",
    )?;
    let source = root.path().join("sample/__init__.py");
    fs::write(&source, "expected = True\nvalue = True\n")?;
    let tests = root.path().join("tests/test_sample.py");
    let original_tests = "import pytest\nimport sample\ndef test_expected():\n if sample.expected: pytest.xfail('known input')\ndef test_value():\n assert sample.value\n";
    fs::write(&tests, original_tests)?;
    let executable = PathBuf::from(std::env::var("MUTIE_PYTHON")?);
    let cancellation = Cancellation::default();
    let timeout = Duration::from_secs(15);
    let limit = NonZeroUsize::new(4194304).unwrap();
    let (identity, baseline) =
        execution_scope(root.path(), &executable, timeout, limit, &cancellation).await?;
    assert_eq!(baseline.classify()?, Outcome::Missed);
    let check = |root: PathBuf, identity: ExecutionScope| async move {
        test(
            &root,
            timeout,
            None,
            Some(limit),
            &identity,
            &Cancellation::default(),
        )
        .await
    };
    assert_eq!(
        check(root.path().into(), identity.clone())
            .await?
            .classify()?,
        Outcome::Missed
    );
    fs::write(&source, "expected = True\nvalue = False\n")?;
    assert_eq!(
        check(root.path().into(), identity.clone())
            .await?
            .classify()?,
        Outcome::Killed
    );
    fs::write(&source, "expected = False\nvalue = True\n")?;
    assert!(
        check(root.path().into(), identity.clone())
            .await?
            .classify()
            .is_err()
    );
    fs::write(&source, "expected = True\nvalue = True\n")?;
    fs::write(
        &tests,
        original_tests.replace("assert sample.value", "pytest.xfail('new skip')"),
    )?;
    assert!(
        check(root.path().into(), identity.clone())
            .await?
            .classify()
            .is_err()
    );
    fs::write(
        &tests,
        format!(
            "{original_tests}\n@pytest.fixture(autouse=True)\ndef cleanup():\n yield\n assert False\n"
        ),
    )?;
    assert!(
        check(root.path().into(), identity.clone())
            .await?
            .classify()
            .is_err()
    );
    Ok(())
}

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

#[tokio::test]
#[ignore = "requires MUTIE_PYTHON; conftest setup, yield and finalizer attribution"]
async fn native_conftest_lifecycle_attribution() -> Result<()> {
    for scope in ["function", "class", "module", "session"] {
        for (name, body, test, expected) in [
            (
                "pass",
                "yield 1",
                "def test_body(value): assert value == 1",
                Some(Outcome::Missed),
            ),
            (
                "body",
                "yield 1",
                "def test_body(value): assert value == 2",
                Some(Outcome::Killed),
            ),
            (
                "setup",
                "assert False\nyield 1",
                "def test_body(value): pass",
                None,
            ),
            (
                "teardown",
                "yield 1\nassert False",
                "def test_body(value): pass",
                None,
            ),
            (
                "mixed",
                "yield 1\nassert False",
                "def test_body(value): assert False",
                None,
            ),
            (
                "finalizer",
                "def fail(): assert False\nrequest.addfinalizer(fail)\nreturn 1",
                "def test_body(value): assert False",
                None,
            ),
            (
                "multiple",
                "def fail(): assert False\nrequest.addfinalizer(fail)\nrequest.addfinalizer(fail)\nyield 1\nassert False",
                "def test_body(value): assert False",
                None,
            ),
            (
                "setup_cleanup",
                "def fail(): assert False\nrequest.addfinalizer(fail)\nassert False\nyield 1",
                "def test_body(value): pass",
                None,
            ),
            (
                "empty_yield",
                "if False: yield 1",
                "def test_body(value): pass",
                None,
            ),
            (
                "double_yield",
                "yield 1\nyield 2",
                "def test_body(value): assert False",
                None,
            ),
            (
                "dynamic",
                "assert False\nyield 1",
                "def test_body(request): request.getfixturevalue('value')",
                None,
            ),
            (
                "caught_dynamic",
                "assert False\nyield 1",
                "def test_body(request):\n try: request.getfixturevalue('value')\n except AssertionError: pass\n assert False",
                None,
            ),
        ] {
            let conftest = format!(
                "import pytest\n@pytest.fixture(scope='{scope}')\ndef value(request):\n {}\n",
                body.replace('\n', "\n ")
            );
            let (raw, result, _) = probe_with_conftest(
                test,
                Some(&conftest),
                Duration::from_secs(15),
                4194304,
                &Cancellation::default(),
            )
            .await?;
            match expected {
                Some(expected) => assert_eq!(
                    result.with_context(|| format!(
                        "{scope}/{name}: {}",
                        raw.stdout.text().unwrap_or_default()
                    ))?,
                    expected
                ),
                None => assert!(result.is_err(), "{scope}/{name}: {result:?}"),
            }
        }
    }
    Ok(())
}

#[tokio::test]
#[ignore = "requires MUTIE_PYTHON; shared parametrized fixtures and unittest autouse"]
async fn native_conftest_shared_lifetimes() -> Result<()> {
    for scope in ["function", "class", "module", "session"] {
        let conftest = format!(
            "import pytest\n@pytest.fixture(scope='{scope}', params=[1, 2])\ndef value(request):\n yield request.param\n@pytest.fixture\ndef dependency(value, request):\n request.addfinalizer(lambda: None)\n yield value\n@pytest.fixture(autouse=True)\ndef automatic(dependency):\n yield\n"
        );
        let (raw, result, _) = probe_with_conftest(
            "class TestShared:\n def test_first(self, dependency): assert dependency in (1, 2)\n def test_second(self, dependency): assert dependency in (1, 2)\ndef test_third(dependency): assert dependency in (1, 2)\n",
            Some(&conftest), Duration::from_secs(15), 4194304, &Cancellation::default(),
        ).await?;
        assert_eq!(
            result
                .with_context(|| format!("{scope}: {}", raw.stdout.text().unwrap_or_default()))?,
            Outcome::Missed
        );
    }
    for (body, expected) in [
        ("yield", Some(Outcome::Missed)),
        ("assert False\nyield", None),
        ("yield\nassert False", None),
    ] {
        let conftest = format!(
            "import pytest\n@pytest.fixture(autouse=True)\ndef automatic():\n {}\n",
            body.replace('\n', "\n ")
        );
        let (_, result, _) = probe_with_conftest("import unittest\nclass TestCase(unittest.TestCase):\n def test_body(self): self.assertTrue(True)\n", Some(&conftest), Duration::from_secs(15), 4194304, &Cancellation::default()).await?;
        match expected {
            Some(outcome) => assert_eq!(result?, outcome),
            None => assert!(result.is_err()),
        }
    }
    Ok(())
}

async fn probe(
    source: &str,
    timeout: Duration,
    budget: usize,
    cancellation: &Cancellation,
) -> Result<(process::CommandOutput, Result<Outcome>, tempfile::TempDir)> {
    probe_with_conftest(source, None, timeout, budget, cancellation).await
}

async fn probe_with_conftest(
    source: &str,
    conftest: Option<&str>,
    timeout: Duration,
    budget: usize,
    cancellation: &Cancellation,
) -> Result<(process::CommandOutput, Result<Outcome>, tempfile::TempDir)> {
    probe_with_temporary(source, conftest, timeout, budget, cancellation, None).await
}

async fn probe_with_temporary(
    source: &str,
    conftest: Option<&str>,
    timeout: Duration,
    budget: usize,
    cancellation: &Cancellation,
    temporary: Option<&Path>,
) -> Result<(process::CommandOutput, Result<Outcome>, tempfile::TempDir)> {
    let directory = tempfile::tempdir()?;
    let owned = directory.path().canonicalize()?;
    let root = owned.join("repo");
    fs::create_dir_all(root.join("tests"))?;
    fs::create_dir_all(root.join("sample"))?;
    fs::write(root.join("sample/__init__.py"), "")?;
    fs::write(root.join("tests/test_probe.py"), source)?;
    if let Some(conftest) = conftest {
        fs::write(root.join("tests/conftest.py"), conftest)?;
    }
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
            ("TMPDIR", temporary.unwrap_or(&owned).as_os_str()),
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
        &if conftest.is_some() {
            vec!["tests/conftest.py".into()]
        } else {
            vec![]
        },
        None,
    )
    .and_then(|(_, outcome, _, _)| match outcome {
        protocol::CandidateOutcome::Scored(outcome) => Ok(outcome),
        protocol::CandidateOutcome::BodyError => {
            anyhow::bail!("Python run contains an unsupported phase failure")
        }
    });
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
            Some(Outcome::Missed),
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
            "def test_body(tmp_path):\n with open(tmp_path / 'large.bin', 'wb') as f: f.write(b'x'*(5*1024*1024))\n",
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
        if source.contains("large.bin") {
            let mut files = Vec::new();
            let mut pending = vec![directory.path().to_owned()];
            while let Some(path) = pending.pop() {
                for entry in fs::read_dir(path)? {
                    let entry = entry?;
                    if entry.file_type()?.is_dir() {
                        pending.push(entry.path());
                    } else if entry.file_name() == "large.bin" {
                        files.push(entry.path());
                    }
                }
            }
            assert_eq!(files.len(), 1, "file-limit probe did not write its output");
            assert_eq!(fs::metadata(&files[0])?.len(), 4 * 1024 * 1024);
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
        let result = probe_with_temporary(
            &source,
            None,
            if mode == "timeout" {
                Duration::from_secs(3)
            } else {
                Duration::from_secs(20)
            },
            4194304,
            &cancellation,
            Some(markers.path()),
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

#[tokio::test]
#[ignore = "requires MUTIE_PYTHON; actual exception types bound continuation eligibility"]
async fn native_continuation_exception_types() -> Result<()> {
    for (source, expected) in [
        (
            "def test_body(): raise ValueError('body')\n",
            Some(protocol::CandidateOutcome::BodyError),
        ),
        (
            "class DomainError(ValueError): pass\ndef test_body(): raise DomainError('body')\n",
            Some(protocol::CandidateOutcome::BodyError),
        ),
        (
            "class ValueError(Exception): pass\ndef test_body(): raise ValueError('fake')\n",
            None,
        ),
        ("def test_body(): raise OSError('resource')\n", None),
        ("def test_body(): raise MemoryError('resource')\n", None),
        (
            "class ResourceError(OSError, ValueError): pass\ndef test_body(): raise ResourceError('resource')\n",
            None,
        ),
        (
            "class BodyAssertion(AssertionError, ValueError): pass\ndef test_body(): raise BodyAssertion('assertion')\n",
            Some(protocol::CandidateOutcome::Scored(Outcome::Killed)),
        ),
    ] {
        let (raw, _, directory) = probe(
            source,
            Duration::from_secs(15),
            4194304,
            &Cancellation::default(),
        )
        .await?;
        let root = directory.path().canonicalize()?.join("repo");
        let actual = read_evidence(
            &raw,
            &directory.path().join("evidence.jsonl"),
            "probe-token",
            root.to_str().unwrap(),
            false,
            &[],
            None,
        );
        match expected {
            Some(expected) => assert_eq!(actual?.1, expected, "{source}"),
            None => assert!(actual.is_err(), "{source}"),
        }
    }
    Ok(())
}
