//! Preview is read-only and does not require a real JavaScript runtime in default tests.
#![cfg(unix)]
use std::{
    fs,
    os::unix::fs::symlink,
    path::{Path, PathBuf},
    process::Command,
    time::{Duration, Instant},
};

struct Fixture {
    _temp: tempfile::TempDir,
    root: PathBuf,
    tools: PathBuf,
    mock: PathBuf,
    tcl_library: Option<PathBuf>,
}
impl Fixture {
    fn configure(&self, command: &mut Command) {
        if let Some(library) = &self.tcl_library {
            command.env("TCL_LIBRARY", library).env_remove("TCLLIBPATH");
        }
    }
    fn path(&self) -> &Path {
        &self.root
    }
    fn node(&self, program: &Path, version: &str, executable: &Path, calls: Option<&Path>) {
        if !program.exists() {
            symlink(&self.mock, program).unwrap();
        }
        let identity = serde_json::json!({"version":version, "executable":executable});
        fs::write(
            program.with_extension("response"),
            format!(
                "{}\n{identity}",
                calls.map(|p| p.to_str().unwrap()).unwrap_or("")
            ),
        )
        .unwrap();
    }
}

fn write(root: &Path, name: &str, data: &[u8]) {
    let path = root.join(name);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, data).unwrap();
}
fn fixture() -> Fixture {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("project");
    let tools = temp.path().join("tools");
    fs::create_dir(&root).unwrap();
    fs::create_dir(&tools).unwrap();
    // On macOS, fresh Mach-O images can stall before main in the loader.
    // Use the installed interpreter with private, non-executable script data.
    // Other Unix platforms retain the native stand-in and its argument checks.
    #[cfg(target_os = "macos")]
    let (mock, tcl_library) = {
        let library = temp.path().join("mock-tcl");
        fs::create_dir(&library).unwrap();
        fs::write(
            library.join("init.tcl"),
            include_bytes!("support/mock_node.tcl"),
        )
        .unwrap();
        let program = PathBuf::from("/usr/bin/tclsh");
        assert!(
            program.is_file(),
            "macOS fixture requires its system Tcl interpreter"
        );
        (program, Some(library))
    };
    #[cfg(not(target_os = "macos"))]
    let (mock, tcl_library) = {
        let mock = temp.path().join("mock-node");
        let compile = Command::new("rustc")
            .args([
                "--edition=2024",
                "--crate-name",
                "mock_node",
                "-D",
                "warnings",
            ])
            .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/support/mock_node.rs"))
            .arg("-o")
            .arg(&mock)
            .output()
            .unwrap();
        assert!(compile.status.success(), "{compile:?}");
        (mock, None)
    };
    let dir = Fixture {
        _temp: temp,
        root,
        tools,
        mock,
        tcl_library,
    };
    // Preparing the fixture includes confirming it responds to readiness;
    // no identity response is consumed and no launcher counter is touched.
    let mut command = Command::new(&dir.mock);
    command.arg("--fixture-ready");
    dir.configure(&mut command);
    let mut ready = command.spawn().unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(status) = ready.try_wait().unwrap() {
            assert!(status.success(), "Node fixture failed to initialize");
            break;
        }
        if Instant::now() >= deadline {
            let _ = ready.kill();
            let _ = ready.wait();
            panic!("Node fixture did not initialize within ten seconds");
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/vitest");
    for name in [
        "package.json",
        "pnpm-lock.yaml",
        "src/flags.js",
        "src/flags.ts",
    ] {
        write(dir.path(), name, &fs::read(source.join(name)).unwrap());
    }
    write(
        dir.path(),
        "node_modules/.pnpm/lock.yaml",
        &fs::read(source.join("pnpm-lock.yaml")).unwrap(),
    );
    write(
        dir.path(),
        "node_modules/.modules.yaml",
        b"packageManager: pnpm@11.10.0\nnodeLinker: isolated\nvirtualStoreDir: .pnpm\n",
    );
    write(
        dir.path(),
        "node_modules/vitest/package.json",
        br#"{"name":"vitest","version":"4.1.9"}"#,
    );
    write(
        dir.path(),
        "node_modules/vitest/vitest.mjs",
        b"throw new Error('preview must not execute Vitest');",
    );
    write(
        dir.path(),
        "node_modules/vite/package.json",
        br#"{"name":"vite","version":"8.3.4"}"#,
    );
    dir.node(
        &dir.tools.join("node"),
        "v24.22.0",
        Path::new("/fake/node"),
        None,
    );
    dir
}
fn preview(fixture: &Fixture, args: &[&str]) -> std::process::Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_mutie"));
    fixture.configure(&mut command);
    command
        .arg("vitest-list")
        .arg(fixture.path())
        .args(args)
        .env("PATH", &fixture.tools)
        .output()
        .unwrap()
}

#[test]
fn previews_comparison_operator_names_without_executing_vitest() {
    let project = fixture();
    write(
        project.path(),
        "src/comparisons.ts",
        b"export const same = 1 === 1; export const less = 1 < 2;",
    );
    let output = preview(&project, &["--include", "src/comparisons.ts"]);
    assert!(output.status.success(), "{output:?}");
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(stdout.matches(": comparison_invert ").count(), 2);
    assert_eq!(stdout.matches(": comparison_boundary ").count(), 1);
    assert!(!project.path().join(".mutie").exists());
}

#[test]
fn identity_fixture_rejects_other_commands_without_consuming_response() {
    let project = fixture();
    let program = project.tools.join("identity node");
    let calls = project.tools.join("identity calls");
    project.node(&program, "v24.22.0", &program, Some(&calls));
    let invoke = |args: &[&str]| {
        let mut command = Command::new(&program);
        project.configure(&mut command);
        command.args(args).output().unwrap()
    };
    let ready = invoke(&["--fixture-ready"]);
    assert!(ready.status.success(), "{ready:?}");
    assert!(ready.stdout.is_empty());
    assert!(!calls.exists());
    let expression =
        "console.log(JSON.stringify({version:process.version,executable:process.execPath}))";
    for args in [
        vec!["--eval", "throw new Error('unexpected execution')"],
        vec!["run", "vitest"],
        vec!["--eval", expression, "extra"],
    ] {
        let result = invoke(&args);
        assert!(!result.status.success(), "{result:?}");
        assert!(result.stdout.is_empty(), "{result:?}");
        assert!(!calls.exists());
    }
    let identity = invoke(&["--eval", expression]);
    assert!(identity.status.success(), "{identity:?}");
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&identity.stdout).unwrap(),
        serde_json::json!({"version":"v24.22.0", "executable":program})
    );
    assert_eq!(fs::read_to_string(&calls).unwrap(), "called\n");
    fs::remove_file(program.with_extension("response")).unwrap();
    let missing = invoke(&["--eval", expression]);
    assert!(!missing.status.success(), "{missing:?}");
    assert!(missing.stdout.is_empty());
    assert_eq!(fs::read_to_string(calls).unwrap(), "called\n");
}
#[test]
fn lists_filters_and_preserves_source_without_executing_vitest() {
    let dir = fixture();
    let original = fs::read(dir.path().join("src/flags.ts")).unwrap();
    let result = preview(&dir, &[]);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let stdout = String::from_utf8(result.stdout).unwrap();
    assert!(stdout.contains("preview only"));
    assert!(stdout.contains("6 mutations"));
    assert_eq!(
        stdout,
        String::from_utf8(preview(&dir, &[]).stdout).unwrap()
    );
    let filtered = preview(&dir, &["--include", "src/*.ts"]);
    assert!(filtered.status.success());
    assert!(String::from_utf8_lossy(&filtered.stdout).contains("3 mutations"));
    assert_eq!(fs::read(dir.path().join("src/flags.ts")).unwrap(), original);
    assert!(!dir.path().join(".mutie").exists());
}
#[test]
fn rejects_configuration_without_executing_it_and_rejects_runtime_mismatch() {
    let dir = fixture();
    write(
        dir.path(),
        "vite.config.ts",
        b"throw new Error('must not execute');",
    );
    let result = preview(&dir, &[]);
    assert!(!result.status.success());
    assert!(
        String::from_utf8_lossy(&result.stderr)
            .contains("unsupported Vitest project configuration")
    );
    assert!(result.stdout.is_empty());
    fs::remove_file(dir.path().join("vite.config.ts")).unwrap();
    dir.node(
        &dir.tools.join("node"),
        "v20.5.1",
        Path::new("/fake/node"),
        None,
    );
    let result = preview(&dir, &[]);
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("requires Node 24.22.0"));
}

#[test]
fn scored_baseline_rechecks_pinned_node_without_repeating_launcher() {
    for replacement in [Some("v24.22.0"), Some("v20.0.0"), None] {
        let project = fixture();
        let tools = tempfile::tempdir().unwrap();
        let pinned = tools.path().join("pinned node");
        let calls = tools.path().join("launcher-calls");
        project.node(
            &tools.path().join("node"),
            "v24.22.0",
            &pinned,
            Some(&calls),
        );
        if let Some(version) = replacement {
            project.node(&pinned, version, Path::new("/different/node"), None);
        }
        let reports = tempfile::tempdir().unwrap();
        let mut command = Command::new(env!("CARGO_BIN_EXE_mutie"));
        project.configure(&mut command);
        let result = command
            .arg(project.path())
            .args(["--lang", "vitest", "--ui", "plain", "--output-dir"])
            .arg(reports.path())
            .env("PATH", tools.path())
            .output()
            .unwrap();
        assert!(!result.status.success());
        let stderr = String::from_utf8_lossy(&result.stderr);
        let expected = match replacement {
            Some("v24.22.0") => "identity changed during the run",
            Some(_) => "requires Node 24.22.0",
            None => "could not start",
        };
        assert!(stderr.contains(expected), "{stderr}");
        assert_eq!(fs::read_to_string(calls).unwrap(), "called\n");
    }
}
