#![cfg(unix)]

//! A small PTY harness checks behavior that the in-memory renderer cannot:
//! cursor queries, redraws during a blocked command, real Ctrl-C and termios.
use std::{
    fs::{self, File},
    io::{self, Read, Write},
    mem::MaybeUninit,
    os::{
        fd::{AsRawFd, FromRawFd},
        unix::{fs::PermissionsExt, process::CommandExt},
    },
    path::PathBuf,
    process::{Child, Command, ExitStatus, Stdio},
    thread,
    time::{Duration, Instant},
};

struct Session {
    _temp: tempfile::TempDir,
    root: PathBuf,
    state: PathBuf,
    workspaces: PathBuf,
    master: File,
    _slave: File,
    child: Child,
    output: Vec<u8>,
    query_tail: Vec<u8>,
    respond_to_queries: bool,
    termios: libc::termios,
}

impl Session {
    fn start(mode: &str, ui: &str, ci: bool) -> Self {
        let temp = tempfile::tempdir_in(std::env::temp_dir().canonicalize().unwrap()).unwrap();
        let root = temp.path().join("module");
        let bin = temp.path().join("bin");
        let state = temp.path().join("state");
        let workspaces = temp.path().join("workspaces");
        for dir in [&root, &bin, &state, &workspaces] {
            fs::create_dir(dir).unwrap();
        }
        fs::write(root.join("go.mod"), "module example.com/terminal\n").unwrap();
        fs::write(
            root.join("code.go"),
            "package p\nvar a = true\nvar b = true\nvar c = true\nvar d = true\n",
        )
        .unwrap();
        let go = bin.join("go");
        fs::write(
            &go,
            r#"#!/bin/sh
set -eu
if [ "$1" = list ]; then
    if [ "$MODE" = empty ]; then
        printf '{"Dir":"%s","GoFiles":[]}\n' "$PWD"
    else
        printf '{"Dir":"%s","GoFiles":["code.go"]}\n' "$PWD"
    fi
    exit 0
fi
if grep -q false code.go; then
    printf '{"Action":"output","Output":"worker diagnostic\\n"}\n'
    touch "$STATE/worker.ready"
    if [ "$MODE" = phases ]; then
        printf '{"Action":"build-output","ImportPath":"p","Output":"compiler diagnostic\\n"}\n'
        touch "$STATE/building.ready"
        while [ ! -f "$STATE/building.release" ]; do sleep 0.02; done
        printf '{"Action":"start","Package":"p"}\n'
        touch "$STATE/testing.ready"
        while [ ! -f "$STATE/testing.release" ]; do sleep 0.02; done
        if grep -q 'b = false' code.go; then
            while [ ! -f "$STATE/last.release" ]; do sleep 0.02; done
        fi
        printf '{"Action":"pass","Package":"p"}\n'
    fi
    if [ "$MODE" = cancel ]; then sleep 30; else sleep 0.15; fi
else
    printf '{"Action":"output","Output":"baseline diagnostic\\n"}\n'
    touch "$STATE/baseline.ready"
    if [ "$MODE" = live ] || [ "$MODE" = cancel ]; then
        while [ ! -f "$STATE/release" ]; do sleep 0.02; done
    fi
    if [ "$MODE" = fail ]; then exit 1; fi
fi
"#,
        )
        .unwrap();
        fs::set_permissions(go, fs::Permissions::from_mode(0o755)).unwrap();

        let mut master = -1;
        let mut slave = -1;
        let mut size = libc::winsize {
            ws_row: 24,
            ws_col: 100,
            ws_xpixel: 0,
            ws_ypixel: 0,
        };
        // SAFETY: openpty initializes the two descriptors; pointers refer to live values.
        assert_eq!(
            unsafe {
                libc::openpty(
                    &mut master,
                    &mut slave,
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    &mut size,
                )
            },
            0,
            "{}",
            io::Error::last_os_error()
        );
        // SAFETY: these descriptors are new and become uniquely owned by File.
        let (master, slave) = unsafe { (File::from_raw_fd(master), File::from_raw_fd(slave)) };
        let termios = termios(&slave);
        // SAFETY: operate only on the owned master descriptor.
        let flags = unsafe { libc::fcntl(master.as_raw_fd(), libc::F_GETFL) };
        assert_eq!(
            unsafe { libc::fcntl(master.as_raw_fd(), libc::F_SETFL, flags | libc::O_NONBLOCK) },
            0
        );
        let mut paths = vec![bin];
        paths.extend(std::env::split_paths(
            &std::env::var_os("PATH").unwrap_or_default(),
        ));
        let mut command = Command::new(env!("CARGO_BIN_EXE_mutie"));
        command
            .arg(&root)
            .args(["--ui", ui, "--jobs", "2", "--timeout", "10", "--verbose"])
            .env("PATH", std::env::join_paths(paths).unwrap())
            .env("STATE", &state)
            .env("MODE", mode)
            .env("TMPDIR", &workspaces)
            .env("TERM", "xterm-256color")
            .env_remove("CI")
            .stdin(Stdio::from(slave.try_clone().unwrap()))
            .stdout(Stdio::from(slave.try_clone().unwrap()))
            .stderr(Stdio::from(slave.try_clone().unwrap()));
        if ci {
            command.env("CI", "true");
        }
        // SAFETY: pre_exec performs only async-signal-safe session/terminal syscalls.
        unsafe {
            command.pre_exec(|| {
                if libc::setsid() == -1 || libc::ioctl(0, libc::TIOCSCTTY as _, 0) == -1 {
                    return Err(io::Error::last_os_error());
                }
                Ok(())
            });
        }
        let child = command.spawn().unwrap();
        Self {
            _temp: temp,
            root,
            state,
            workspaces,
            master,
            _slave: slave,
            child,
            output: Vec::new(),
            query_tail: Vec::new(),
            respond_to_queries: mode != "no_cursor",
            termios,
        }
    }

    fn pump(&mut self) {
        let mut buf = [0; 8192];
        loop {
            match self.master.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    self.output.extend_from_slice(&buf[..n]);
                    self.query_tail.extend_from_slice(&buf[..n]);
                    while let Some(index) = self.query_tail.windows(4).position(|w| w == b"\x1b[6n")
                    {
                        // Minimal terminal emulation: answer cursor-position requests.
                        if self.respond_to_queries {
                            self.master.write_all(b"\x1b[2;1R").unwrap();
                        }
                        self.query_tail.drain(..index + 4);
                    }
                    if self.query_tail.len() > 3 {
                        self.query_tail.drain(..self.query_tail.len() - 3);
                    }
                }
                Err(error)
                    if error.kind() == io::ErrorKind::WouldBlock
                        || error.raw_os_error() == Some(libc::EIO) =>
                {
                    break;
                }
                Err(error) => panic!("PTY read failed: {error}"),
            }
        }
    }

    fn until(&mut self, condition: impl Fn(&Self) -> bool) {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            self.pump();
            if condition(self) {
                return;
            }
            assert!(
                Instant::now() < deadline,
                "PTY wait timed out: {}",
                String::from_utf8_lossy(&self.output)
            );
            thread::sleep(Duration::from_millis(10));
        }
    }

    fn resize(&mut self, rows: u16, columns: u16) {
        let size = libc::winsize {
            ws_row: rows,
            ws_col: columns,
            ws_xpixel: 0,
            ws_ypixel: 0,
        };
        // SAFETY: resize the owned terminal with a valid winsize pointer.
        assert_eq!(
            unsafe { libc::ioctl(self.master.as_raw_fd(), libc::TIOCSWINSZ, &size) },
            0
        );
    }

    fn wait_for_full_frame(&mut self, expected: &str) {
        let deadline = Instant::now() + Duration::from_secs(10);
        let mut width = 100;
        loop {
            // Resizing requests a complete frame, allowing text assertions
            // without interpreting Ratatui's incremental cursor movements.
            width = if width == 100 { 110 } else { 100 };
            self.resize(24, width);
            let start = self.output.len();
            let time = Instant::now();
            self.until(|_| time.elapsed() >= Duration::from_millis(150));
            if String::from_utf8_lossy(&self.output[start..]).contains(expected) {
                return;
            }
            assert!(
                Instant::now() < deadline,
                "missing {expected:?}: {}",
                String::from_utf8_lossy(&self.output)
            );
        }
    }

    fn finish(&mut self) -> ExitStatus {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            self.pump();
            if let Some(status) = self.child.try_wait().unwrap() {
                self.pump();
                return status;
            }
            assert!(
                Instant::now() < deadline,
                "CLI did not finish: {}",
                String::from_utf8_lossy(&self.output)
            );
            thread::sleep(Duration::from_millis(10));
        }
    }

    fn report(&self, name: &str) -> serde_json::Value {
        let dir = fs::read_dir(self.root.join(".mutie"))
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        serde_json::from_slice(&fs::read(dir.join(name)).unwrap()).unwrap()
    }

    fn assert_restored(&self, interactive: bool) {
        // macOS revokes the slave when its session leader exits; the master
        // still exposes the terminal settings for the restoration assertion.
        let now = termios(&self.master);
        assert_eq!(now.c_lflag, self.termios.c_lflag);
        assert_eq!(now.c_iflag, self.termios.c_iflag);
        assert_eq!(now.c_oflag, self.termios.c_oflag);
        let output = String::from_utf8_lossy(&self.output);
        assert!(!output.contains("\x1b[?1049h"), "must stay inline");
        if interactive {
            let restored = output.rfind("\x1b[?25h").expect("cursor must be restored");
            let summary = output.find("Run ").expect("final summary");
            assert!(
                restored < summary,
                "summary must follow terminal restoration"
            );
        } else {
            assert!(!output.contains('\x1b'));
        }
        assert!(output.contains("Report saved:"));
        assert!(fs::read_dir(&self.workspaces).unwrap().next().is_none());
        assert!(
            !fs::read_to_string(self.root.join("code.go"))
                .unwrap()
                .contains("false")
        );
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        if self.child.try_wait().ok().flatten().is_none() {
            // SAFETY: request cleanup from the child owned by this session.
            unsafe {
                libc::kill(self.child.id() as i32, libc::SIGTERM);
            }
            for _ in 0..100 {
                if self.child.try_wait().ok().flatten().is_some() {
                    return;
                }
                thread::sleep(Duration::from_millis(10));
            }
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

fn termios(file: &File) -> libc::termios {
    let mut value = MaybeUninit::uninit();
    // SAFETY: tcgetattr initializes the termios value on success.
    assert_eq!(
        unsafe { libc::tcgetattr(file.as_raw_fd(), value.as_mut_ptr()) },
        0
    );
    unsafe { value.assume_init() }
}

#[test]
fn live_view_redraws_while_baseline_waits_and_restores_after_resize_and_success() {
    let mut session = Session::start("live", "auto", false);
    session.until(|s| s.state.join("baseline.ready").exists());
    let start = session.output.len();
    let time = Instant::now();
    session.until(|_| time.elapsed() > Duration::from_millis(350));
    assert!(
        session.output.len() > start + 20,
        "status must refresh while baseline is blocked"
    );
    assert_eq!(
        session.report("outcomes.json")["baseline"],
        serde_json::Value::Null
    );
    assert!(!String::from_utf8_lossy(&session.output).contains("baseline diagnostic"));
    for (rows, cols) in [(5, 28), (24, 100)] {
        let size = libc::winsize {
            ws_row: rows,
            ws_col: cols,
            ws_xpixel: 0,
            ws_ypixel: 0,
        };
        // SAFETY: resize the owned terminal with a valid winsize pointer.
        assert_eq!(
            unsafe { libc::ioctl(session.master.as_raw_fd(), libc::TIOCSWINSZ, &size) },
            0
        );
        let time = Instant::now();
        session.until(|_| time.elapsed() > Duration::from_millis(150));
    }
    fs::write(session.state.join("release"), "").unwrap();
    assert!(session.finish().success());
    session.assert_restored(true);
    assert_eq!(session.report("outcomes.json")["summary"]["missed"], 4);
}

#[test]
fn live_worker_phases_and_idle_rows_arrive_before_completion() {
    let mut session = Session::start("phases", "interactive", false);
    session.until(|s| s.state.join("building.ready").exists());
    session.wait_for_full_frame("building");
    assert_eq!(session.report("outcomes.json")["summary"]["missed"], 0);
    fs::write(session.state.join("building.release"), "").unwrap();
    session.until(|s| s.state.join("testing.ready").exists());
    session.wait_for_full_frame("testing");
    assert_eq!(session.report("outcomes.json")["summary"]["missed"], 0);
    fs::write(session.state.join("testing.release"), "").unwrap();
    session.until(|s| s.report("outcomes.json")["summary"]["missed"] == 3);
    session.wait_for_full_frame("worker 1 idle");
    assert_eq!(
        session.report("outcomes.json")["mutations"][1]["state"],
        "running"
    );
    assert!(!String::from_utf8_lossy(&session.output).contains("compiler diagnostic"));
    fs::write(session.state.join("last.release"), "").unwrap();
    assert!(session.finish().success());
    session.assert_restored(true);
    let directory = fs::read_dir(session.root.join(".mutie"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    assert!(
        fs::read_to_string(directory.join("logs/000002.stdout.jsonl"))
            .unwrap()
            .contains("compiler diagnostic")
    );
    assert_eq!(session.report("outcomes.json")["summary"]["missed"], 4);
}

#[test]
fn ctrl_c_drains_active_workers_saves_partial_results_and_restores_terminal() {
    let mut session = Session::start("cancel", "interactive", true);
    fs::write(session.state.join("release"), "").unwrap();
    session.until(|s| s.state.join("worker.ready").exists());
    session.master.write_all(&[3]).unwrap();
    assert!(!session.finish().success());
    session.assert_restored(true);
    let report = session.report("outcomes.json");
    assert_eq!(report["status"], "interrupted");
    assert_eq!(report["mutations"][0]["state"], "interrupted");
    assert_eq!(report["mutations"][3]["state"], "pending");
    assert!(report["mutations"][0]["logs"]["text"].is_string());
}

#[test]
fn empty_and_failed_runs_restore_terminal_and_ci_uses_plain_output() {
    for (mode, ui, ci, interactive, success) in [
        ("empty", "interactive", false, true, true),
        ("fail", "interactive", false, true, false),
        ("success", "auto", true, false, true),
        ("success", "plain", false, false, true),
    ] {
        let mut session = Session::start(mode, ui, ci);
        assert_eq!(session.finish().success(), success);
        session.assert_restored(interactive);
        assert_eq!(
            session.report("run.json")["status"],
            if success { "completed" } else { "failed" }
        );
    }
}

#[test]
fn failed_terminal_setup_restores_input_and_auto_falls_back_to_plain() {
    for ui in ["interactive", "auto"] {
        let mut session = Session::start("no_cursor", ui, false);
        assert_eq!(session.finish().success(), ui == "auto");
        session.assert_restored(true);
        let output = String::from_utf8_lossy(&session.output);
        if ui == "auto" {
            assert!(output.contains("using plain output"));
            assert!(output.contains("Running baseline"));
        } else {
            assert!(!session.state.join("baseline.ready").exists());
            assert_eq!(session.report("run.json")["status"], "failed");
        }
    }
}
