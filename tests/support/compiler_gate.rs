//! Test-only PATH proxies. Real Cargo/rustc do the work; checkpoints control
//! when compilation or execution can finish. Compiled directly by the fixture.
use std::{
    env,
    ffi::OsString,
    fs,
    io::Write,
    os::unix::process::CommandExt,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::Duration,
};

mod cargo_faults;

unsafe extern "C" {
    fn getpgrp() -> i32;
}

fn record(state: &Path, label: &str, kind: &str, value: impl ToString) {
    fs::write(state.join(format!("{label}.{kind}")), value.to_string()).unwrap();
}

fn label() -> &'static str {
    let source = fs::read_to_string("src/flags.rs").unwrap();
    for (function, original, label) in [
        ("unit_flag", "true", "a"),
        ("cli_flag", "false", "b"),
        ("untested_flag", "true", "c"),
        ("pending_flag", "true", "d"),
    ] {
        if let Some(body) = source
            .split(&format!("pub fn {function}() -> bool {{"))
            .nth(1)
            && !body.trim_start().starts_with(original)
        {
            return label;
        }
    }
    "baseline"
}

fn main() {
    let program = env::current_exe().unwrap();
    let cargo = program.file_name().unwrap() == "cargo";
    let mut args: Vec<OsString> = env::args_os().skip(1).collect();
    let real = env::var_os(if cargo {
        "MUTIE_TEST_REAL_CARGO"
    } else {
        "MUTIE_TEST_REAL_RUSTC"
    })
    .unwrap();
    let state = PathBuf::from(env::var_os("MUTIE_RUST_TEST_STATE").unwrap());
    let case = env::var("MUTIE_TEST_COMPILE_CASE").unwrap();

    if cargo
        && args
            .first()
            .is_some_and(|arg| arg == "test" || arg == "metadata")
    {
        let label = label();
        let building = args.iter().any(|arg| arg == "--no-run");
        let phase = if args[0] == "metadata" {
            "metadata"
        } else if building {
            "build"
        } else {
            "tests"
        };
        record(&state, label, &format!("{phase}.pid"), std::process::id());
        // SAFETY: reads only this process's group.
        record(&state, label, &format!("{phase}.group"), unsafe {
            getpgrp()
        });
        record(&state, label, &format!("{phase}.started"), "started");
        if phase != "metadata" {
            println!("Cargo {phase} checkpoint {label}");
            std::io::stdout().flush().unwrap();
        }
        if let Some(fault) = case.strip_prefix("fault:") {
            let parts: Vec<_> = fault.split(':').collect();
            if parts[0] == label && parts[1] == phase {
                cargo_faults::inject(&real, &args, &state, label, phase, parts[2]);
            }
        }
        if case == "deadline" && label == "b" && building {
            let status = Command::new(real).args(args).status().unwrap();
            if !status.success() {
                std::process::exit(status.code().unwrap_or(1));
            }
            record(&state, label, "ready", "compiled");
            while !state.join("release-build").exists() {
                thread::sleep(Duration::from_millis(10));
            }
            return;
        }
    } else if !cargo
        && args.iter().any(|arg| arg == "--test")
        && args
            .windows(2)
            .any(|pair| pair[0] == "--crate-name" && pair[1] == "mutie_rust_fixture")
    {
        let label = label();
        let block = (case == "baseline" && label == "baseline")
            || (case == "mutations" && matches!(label, "b" | "c"))
            || (case.starts_with("fault:c:") && label == "b");
        if block {
            // A real rustc waits for source on stdin. Keeping the write end
            // open gives us a live compiler without machine-speed assumptions.
            let input = args
                .iter()
                .position(|arg| Path::new(arg).ends_with("src/main.rs"))
                .unwrap();
            args[input] = "-".into();
            let mut compiler = Command::new(real)
                .args(args)
                .stdin(Stdio::piped())
                .spawn()
                .unwrap();
            let mut descendant = Command::new("sleep").arg("300").spawn().unwrap();
            record(&state, label, "proxy.pid", std::process::id());
            record(&state, label, "compiler.pid", compiler.id());
            record(&state, label, "descendant.pid", descendant.id());
            record(
                &state,
                label,
                "workspace",
                env::current_dir().unwrap().display(),
            );
            record(
                &state,
                label,
                "target",
                env::var("CARGO_TARGET_DIR").unwrap(),
            );
            let temporary = env::temp_dir();
            fs::write(temporary.join("compiler-artifact"), "owned scratch file").unwrap();
            record(&state, label, "tmpdir", temporary.display());
            eprintln!("compiler checkpoint {label}");
            record(&state, label, "ready", "compiler waiting for input");
            loop {
                assert!(
                    compiler.try_wait().unwrap().is_none(),
                    "compiler exited before cancellation"
                );
                assert!(
                    descendant.try_wait().unwrap().is_none(),
                    "helper exited before cancellation"
                );
                thread::sleep(Duration::from_millis(20));
            }
        }
    }
    panic!("exec failed: {}", Command::new(real).args(args).exec());
}
