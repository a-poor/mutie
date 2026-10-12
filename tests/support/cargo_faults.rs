//! Faults injected at the Cargo boundary after real fixture commands complete.
use super::*;
use std::{ffi::OsStr, os::unix::fs::symlink};

pub fn inject(
    real: &OsStr,
    args: &[OsString],
    state: &Path,
    label: &str,
    phase: &str,
    kind: &str,
) -> ! {
    let output = Command::new(real).args(args).output().unwrap();
    assert!(
        output.status.success(),
        "fault prerequisite failed: {output:?}"
    );
    if label == "c" {
        record(state, "fault", "ready", "ready to fail");
        while !state.join("release-fault").exists() {
            thread::sleep(Duration::from_millis(10));
        }
    }
    record(
        state,
        label,
        "workspace",
        env::current_dir().unwrap().display(),
    );
    record(
        state,
        label,
        "target",
        env::var("CARGO_TARGET_DIR").unwrap(),
    );
    record(state, label, "tmpdir", env::temp_dir().display());
    fs::write(env::temp_dir().join("fault-artifact"), "scratch").unwrap();

    let stdout = String::from_utf8(output.stdout).unwrap();
    let filtered: String = stdout
        .lines()
        .filter(|line| match kind {
            "missing-build" | "malformed-build" => !line.contains("\"reason\":\"build-finished\""),
            "missing-tests" => !line.starts_with("test result:"),
            "malformed-metadata" => false,
            _ => true,
        })
        .map(|line| format!("{line}\n"))
        .collect();
    print!("{filtered}");
    std::io::stderr().write_all(&output.stderr).unwrap();
    eprintln!("injected {phase} fault: {kind}");
    match kind {
        "malformed-build" => println!("{{\"reason\":\"build-finished\",\"success\":\"yes\"}}"),
        "malformed-metadata" => println!("{{broken metadata"),
        "exit" => {
            // A failed test line must not turn an unexpected tool exit into a kill.
            println!("test result: FAILED. 1 failed");
            std::io::stdout().flush().unwrap();
            std::process::exit(17);
        }
        "signal" => {
            std::io::stdout().flush().unwrap();
            // SAFETY: raise SIGTERM only in this test-owned proxy process.
            unsafe extern "C" {
                fn raise(signal: i32) -> i32;
            }
            unsafe {
                raise(15);
            }
            unreachable!("SIGTERM should terminate the proxy");
        }
        "spawn" => {
            // The fixture removes fallback Cargo binaries from PATH. Atomic
            // replacement leaves the running proxy/rustc hard link intact.
            let cargo = env::current_exe().unwrap();
            let replacement = cargo.with_file_name("broken-cargo");
            symlink("cargo", &replacement).unwrap();
            fs::rename(replacement, cargo).unwrap();
        }
        "overflow-out" | "overflow-err" | "overflow-mixed" => {
            if phase == "build" {
                println!(
                    "{{\"reason\":\"compiler-message\",\"message\":{{\"level\":\"error\",\"rendered\":\"injected compiler error\\n\"}}}}"
                );
                println!("{{\"reason\":\"build-finished\",\"success\":false}}");
            } else if phase == "tests" {
                println!("test result: FAILED. 1 failed");
            }
            std::io::stdout().flush().unwrap();
            let _descendant = Command::new("sleep").arg("300").spawn().unwrap();
            record(state, label, "descendant.pid", _descendant.id());
            let noise = [b'x'; 4096];
            loop {
                if kind != "overflow-err" {
                    let _ = std::io::stdout().write_all(&noise);
                }
                if kind != "overflow-out" {
                    let _ = std::io::stderr().write_all(&noise);
                }
            }
        }
        _ => {}
    }
    std::io::stdout().flush().unwrap();
    std::process::exit(0);
}
