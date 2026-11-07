#[path = "support/invocation.rs"]
mod invocation;
#[allow(unused_imports)]
use invocation::{Child, Command};

use std::{fs, path::Path, process::Output};

fn command(root: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_mutie"))
        .current_dir(root)
        .args(args)
        .env("PATH", root.join("no-tools"))
        .output()
        .unwrap()
}

fn success(output: Output) -> String {
    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

#[test]
fn commands_work_without_go_checkout_or_config_and_require_explicit_rules() {
    let root = tempfile::tempdir().unwrap();
    fs::write(root.path().join("mutie.toml"), "invalid config [").unwrap();
    assert!(success(command(root.path(), &["runs"])).contains("No saved runs found"));
    assert!(
        success(command(root.path(), &["prune", "--keep-last", "0"]))
            .contains("Preview only; nothing changed")
    );
    assert!(
        success(command(
            root.path(),
            &["prune", "--older-than", "7", "--apply"]
        ))
        .contains("Deleted 0 of 0")
    );
    assert!(!root.path().join(".mutie").exists());
    for args in [
        vec!["prune"],
        vec!["prune", "--apply"],
        vec!["prune", "--older-than", "0"],
        vec!["prune", "--older-than", "18446744073709551615"],
        vec!["prune", "--keep-last", "-1"],
        vec!["prune", "--keep-last", "1", "--list"],
        vec!["runs", "--apply"],
        vec!["--list", "runs"],
    ] {
        let output = command(root.path(), &args);
        assert!(!output.status.success(), "{args:?}");
    }
}

#[cfg(unix)]
#[test]
fn live_run_is_preserved_then_finalized_report_can_be_previewed_and_deleted() {
    use std::{
        os::unix::fs::PermissionsExt,
        process::Stdio,
        thread,
        time::{Duration, Instant},
    };

    // Always release the fake Go command and reap Mutie, including on assertion failure.
    struct Running {
        child: Option<Child>,
        release: std::path::PathBuf,
    }
    impl Drop for Running {
        fn drop(&mut self) {
            let _ = fs::write(&self.release, "release");
            if let Some(mut child) = self.child.take() {
                let _ = child.wait();
            }
        }
    }

    let root = tempfile::tempdir().unwrap();
    let module = root.path().join("module");
    let bin = root.path().join("bin");
    fs::create_dir(&module).unwrap();
    fs::create_dir(&bin).unwrap();
    fs::write(module.join("go.mod"), "module example.com/retention\n").unwrap();
    fs::write(module.join("a.go"), "package p\nvar a = true\n").unwrap();
    let go = bin.join("go");
    fs::write(
        &go,
        r#"#!/bin/sh
if [ "$1" = list ]; then
    printf '{"Dir":"%s","GoFiles":["a.go"]}\n' "$PWD"
else
    printf 'ready' > "$MUTIE_RETENTION_READY"
    while [ ! -f "$MUTIE_RETENTION_RELEASE" ]; do /bin/sleep 0.02; done
    printf '{"Action":"pass","Package":"p"}\n'
fi
"#,
    )
    .unwrap();
    fs::set_permissions(&go, fs::Permissions::from_mode(0o755)).unwrap();
    let ready = root.path().join("ready");
    let release = root.path().join("release");
    let child = Command::new(env!("CARGO_BIN_EXE_mutie"))
        .arg(&module)
        .args(["--ui", "plain", "--baseline-timeout", "10"])
        .env("PATH", &bin)
        .env("MUTIE_RETENTION_READY", &ready)
        .env("MUTIE_RETENTION_RELEASE", &release)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut running = Running {
        child: Some(child),
        release: release.clone(),
    };
    let deadline = Instant::now() + Duration::from_secs(10);
    while !ready.exists() {
        assert!(Instant::now() < deadline, "fake Go never started");
        thread::sleep(Duration::from_millis(10));
    }
    let parent = module.join(".mutie");
    let saved = fs::read_dir(&parent)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let active = success(command(&module, &["prune", "--keep-last", "0", "--apply"]));
    assert!(
        active.contains("protected") && active.contains("locked"),
        "{active}"
    );
    assert!(active.contains("Deleted 0 of 0"));
    assert!(saved.exists());
    fs::write(&release, "release").unwrap();
    success(running.child.take().unwrap().wait_with_output().unwrap());
    assert!(saved.join(".mutie.lock").is_file());
    // Listing and retention work even after Go and the original source are gone.
    fs::remove_file(module.join("go.mod")).unwrap();
    fs::remove_file(module.join("a.go")).unwrap();
    fs::write(module.join("mutie.toml"), "invalid config [").unwrap();
    let metadata = fs::read(saved.join("run.json")).unwrap();
    let log = fs::read(saved.join("logs/000001.log")).unwrap();
    let list = success(command(&module, &["runs"]));
    assert!(list.contains("completed"), "{list}");
    let preview = success(command(
        root.path(),
        &["prune", parent.to_str().unwrap(), "--keep-last", "0"],
    ));
    assert!(
        preview.contains("Selected runs: 1") && preview.contains("Preview only"),
        "{preview}"
    );
    assert_eq!(fs::read(saved.join("run.json")).unwrap(), metadata);
    assert_eq!(fs::read(saved.join("logs/000001.log")).unwrap(), log);
    fs::write(parent.join("keep.txt"), "unrelated").unwrap();
    let output = success(command(&module, &["prune", "--keep-last", "0", "--apply"]));
    assert!(output.contains("Deleted 1 of 1"), "{output}");
    assert!(!saved.exists());
    assert_eq!(
        fs::read_to_string(parent.join("keep.txt")).unwrap(),
        "unrelated"
    );
}
