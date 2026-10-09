#![cfg(unix)]

//! Exercise the standalone browser with a real terminal and no Go on PATH.
use serde_json::json;
use std::{
    fs::{self, File},
    io::{self, Read, Write},
    mem::MaybeUninit,
    os::{
        fd::{AsRawFd, FromRawFd},
        unix::process::CommandExt,
    },
    path::{Path, PathBuf},
    process::{Child, Command, ExitStatus, Stdio},
    thread,
    time::{Duration, Instant},
};

fn fixture(empty: bool) -> tempfile::TempDir {
    let temp = tempfile::tempdir_in(std::env::temp_dir().canonicalize().unwrap()).unwrap();
    let root = temp.path().join("saved run");
    fs::create_dir(&root).unwrap();
    // Neither browser startup nor command generation may consult this config.
    fs::write(temp.path().join("mutie.toml"), "invalid = [").unwrap();
    let status = if empty { "failed" } else { "completed" };
    let mut mutations = Vec::new();
    let mut executions = Vec::new();
    for id in 1..=if empty { 0 } else { 2 } {
        mutations.push(json!({
            "id": id, "file": format!("file{id}.go"), "operator": "boolean_flip",
            "source_fingerprint": "sha256:fixture", "byte_start": 0, "byte_end": 4,
            "line": 1, "column": 1, "end_line": 1, "end_column": 5,
            "original": "true", "replacement": "false", "diff": format!("{id}.diff")
        }));
        executions.push(json!({
            "id": id, "state": "completed", "outcome": if id == 1 { "missed" } else { "killed" },
            "duration_seconds": 0.01, "logs": {"text": format!("{id}.log")}
        }));
        fs::write(
            root.join(format!("{id}.diff")),
            "--- a/file.go\n+++ b/file.go\n-true\n+false\n",
        )
        .unwrap();
        fs::write(
            root.join(format!("{id}.log")),
            format!("diagnostic {id}\n\x1b[2Jtext controls sanitized\n"),
        )
        .unwrap();
    }
    let run = json!({
        "schema_version": 1, "operator_catalog_version": 1, "checkpoint": 3, "status": status,
        "finished_at_unix_seconds": 10, "planned_mutations": if empty { None } else { Some(2) },
        "module_root": "/missing/a project's checkout", "phase": if empty { "baseline" } else { "finished" },
        "error": if empty { Some("baseline failed") } else { None }
    });
    let inventory = json!({"schema_version": 1, "operator_catalog_version": 1, "complete": !empty, "mutations": mutations});
    let outcomes = json!({
        "schema_version": 1, "checkpoint": 3, "status": status, "mutations": executions,
        "baseline": {"outcome": if empty { "failed" } else { "passed" }, "logs": {"text": "baseline.log"}}
    });
    for (name, value) in [
        ("run.json", run),
        ("mutants.json", inventory),
        ("outcomes.json", outcomes),
    ] {
        fs::write(root.join(name), serde_json::to_vec(&value).unwrap()).unwrap();
    }
    fs::write(root.join("baseline.log"), "baseline diagnostic\n").unwrap();
    temp
}

fn files(root: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    let mut result = fs::read_dir(root)
        .unwrap()
        .map(|entry| {
            let path = entry.unwrap().path();
            let bytes = fs::read(&path).unwrap();
            (path, bytes)
        })
        .collect::<Vec<_>>();
    result.sort();
    result
}

struct Session {
    temp: tempfile::TempDir,
    original: Vec<(PathBuf, Vec<u8>)>,
    master: File,
    _slave: File,
    child: Child,
    output: Vec<u8>,
    screen: vt100::Parser,
    termios: libc::termios,
}

impl Session {
    fn start(empty: bool, module: Option<&str>) -> Self {
        Self::start_fixture(fixture(empty), module)
    }

    fn start_fixture(temp: tempfile::TempDir, module: Option<&str>) -> Self {
        let root = temp.path().join("saved run");
        let original = files(&root);
        let (mut master, mut slave) = (-1, -1);
        let mut size = libc::winsize {
            ws_row: 24,
            ws_col: 110,
            ws_xpixel: 0,
            ws_ypixel: 0,
        };
        // SAFETY: openpty initializes owned descriptors through live pointers.
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
            0
        );
        // SAFETY: each new descriptor is transferred to exactly one File.
        let (master, mut slave) = unsafe { (File::from_raw_fd(master), File::from_raw_fd(slave)) };
        let termios = termios(&slave);
        // SAFETY: flags affect only the owned master descriptor.
        let flags = unsafe { libc::fcntl(master.as_raw_fd(), libc::F_GETFL) };
        assert_eq!(
            unsafe { libc::fcntl(master.as_raw_fd(), libc::F_SETFL, flags | libc::O_NONBLOCK) },
            0
        );
        slave
            .write_all(b"earlier output\n$ mutie report\n")
            .unwrap();
        let mut command = Command::new(env!("CARGO_BIN_EXE_mutie"));
        command
            .arg("report")
            .arg(&root)
            .current_dir(temp.path())
            .env("PATH", "")
            .env("TERM", "xterm-256color")
            .stdin(Stdio::from(slave.try_clone().unwrap()))
            .stdout(Stdio::from(slave.try_clone().unwrap()))
            .stderr(Stdio::from(slave.try_clone().unwrap()));
        if let Some(module) = module {
            command.args(["--module", module]);
        }
        // SAFETY: only async-signal-safe terminal/session syscalls run after fork.
        unsafe {
            command.pre_exec(|| {
                if libc::setsid() == -1 || libc::ioctl(0, libc::TIOCSCTTY as _, 0) == -1 {
                    return Err(io::Error::last_os_error());
                }
                Ok(())
            });
        }
        let child = command.spawn().unwrap();
        let mut session = Self {
            temp,
            original,
            master,
            _slave: slave,
            child,
            output: Vec::new(),
            screen: vt100::Parser::new(24, 110, 1000),
            termios,
        };
        session.expect("mutie report");
        session.until("initial browser frame", |s| {
            s.screen.screen().alternate_screen()
                && s.screen.screen().contents().contains("Mutations")
        });
        session
    }

    fn pump(&mut self) {
        let mut buffer = [0; 8192];
        loop {
            match self.master.read(&mut buffer) {
                Ok(0) => break,
                Ok(n) => {
                    self.output.extend_from_slice(&buffer[..n]);
                    self.screen.process(&buffer[..n]);
                }
                Err(error)
                    if error.kind() == io::ErrorKind::WouldBlock
                        || error.raw_os_error() == Some(libc::EIO) =>
                {
                    break;
                }
                Err(error) => panic!("PTY read: {error}"),
            }
        }
    }

    fn until(&mut self, expected: &str, ready: impl Fn(&Self) -> bool) {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            self.pump();
            if ready(self) {
                return;
            }
            assert!(
                Instant::now() < deadline,
                "PTY timed out waiting for {expected:?}:\n{}\n{}",
                self.screen.screen().contents(),
                String::from_utf8_lossy(&self.output)
            );
            thread::sleep(Duration::from_millis(10));
        }
    }

    fn expect(&mut self, text: &str) {
        self.until(text, |s| s.screen.screen().contents().contains(text));
    }
    fn send(&mut self, bytes: &[u8]) {
        self.master.write_all(bytes).unwrap();
    }

    fn resize(&mut self, rows: u16, cols: u16) {
        self.pump();
        let start = self.output.len();
        self.screen.screen_mut().set_size(rows, cols);
        let size = libc::winsize {
            ws_row: rows,
            ws_col: cols,
            ws_xpixel: 0,
            ws_ypixel: 0,
        };
        // SAFETY: ioctl receives a live winsize for our terminal.
        assert_eq!(
            unsafe { libc::ioctl(self.master.as_raw_fd(), libc::TIOCSWINSZ, &size) },
            0
        );
        // Old labels can survive the emulator's resize. Wait for Ratatui's
        // clear and completed redraw before sending the next keyboard input.
        self.until("completed resize frame", |s| {
            let frame = &s.output[start..];
            frame.windows(4).any(|bytes| bytes == b"\x1b[2J") && frame.ends_with(b"\x1b[?25l")
        });
    }

    fn finish(&mut self) -> ExitStatus {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            self.pump();
            if let Some(status) = self.child.try_wait().unwrap() {
                self.pump();
                return status;
            }
            assert!(Instant::now() < deadline, "browser did not exit");
            thread::sleep(Duration::from_millis(10));
        }
    }

    fn assert_restored(&self) {
        let now = termios(&self.master);
        assert_eq!(now.c_lflag, self.termios.c_lflag);
        assert_eq!(now.c_iflag, self.termios.c_iflag);
        assert_eq!(now.c_oflag, self.termios.c_oflag);
        assert!(!self.screen.screen().alternate_screen());
        assert!(!self.screen.screen().hide_cursor());
        let output = String::from_utf8_lossy(&self.output);
        for sequence in [
            "\x1b[?1049h",
            "\x1b[?1049l",
            "\x1b[?2004h",
            "\x1b[?2004l",
            "\x1b[?25h",
        ] {
            assert!(output.contains(sequence), "missing {sequence:?}");
        }
        assert!(self.screen.screen().contents().contains("earlier output"));
        assert!(self.screen.screen().contents().contains("$ mutie report"));
        assert_eq!(files(&self.temp.path().join("saved run")), self.original);
        assert!(!self.temp.path().join(".mutie").exists());
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        if self.child.try_wait().ok().flatten().is_none() {
            let _ = self.child.kill();
            // Drain output while reaping; a full PTY must not block teardown.
            let deadline = Instant::now() + Duration::from_secs(2);
            while Instant::now() < deadline && self.child.try_wait().ok().flatten().is_none() {
                let mut bytes = [0; 8192];
                let _ = self.master.read(&mut bytes);
                thread::sleep(Duration::from_millis(10));
            }
        }
    }
}

#[test]
fn saved_dismissals_filter_and_export_without_a_checkout_or_go() {
    let temp = fixture(false);
    let path = temp.path().join("saved run/mutants.json");
    let mut inventory: serde_json::Value =
        serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    inventory["mutations"][0]["source_fingerprint"] = json!(format!("sha256:{}", "a".repeat(64)));
    let mut identity = inventory["mutations"][0].clone();
    identity.as_object_mut().unwrap().remove("id");
    identity.as_object_mut().unwrap().remove("diff");
    inventory["dismissals"] = json!({"matched": [{"mutation_id": 1, "dismissal": {
        "id": 5, "reason": "accepted test gap", "note": "Reviewed boundary", "mutation": identity
    }}], "needs_review": []});
    fs::write(&path, serde_json::to_vec(&inventory).unwrap()).unwrap();
    // Neither repository review data nor configuration is consulted by the browser.
    fs::write(temp.path().join("mutie.dismissals.toml"), "[broken").unwrap();
    let mut session = Session::start_fixture(temp, None);
    session.expect("1 dismissed | 0 need review");
    session.expect("accepted test gap");
    session.send(b" s");
    session.expect("Review: undismissed");
    session.expect("Marked: 1 (1 hidden)");
    session.send(b"s");
    session.expect("Review: dismissed");
    session.expect("[D]");
    session.resize(24, 70);
    session.expect("accepted test gap");
    session.send(b"v");
    session.expect("Saved dismissal review");
    session.send(b"e");
    assert!(session.finish().success());
    session.assert_restored();
    let output = String::from_utf8_lossy(&session.output);
    assert!(output.contains("--id 1"));
}

fn termios(file: &File) -> libc::termios {
    let mut value = MaybeUninit::uninit();
    // SAFETY: successful tcgetattr initializes the entire value.
    assert_eq!(
        unsafe { libc::tcgetattr(file.as_raw_fd(), value.as_mut_ptr()) },
        0
    );
    unsafe { value.assume_init() }
}

#[test]
fn browser_filters_inspects_and_exports_preserving_the_shell_and_report() {
    let mut session = Session::start(false, None);
    session.expect("+false");
    session.send(b" j o"); // mark IDs 1 and 2, then hide killed rows
    session.expect("Marked: 2 (1 hidden)");
    session.send(b"l");
    session.expect("diagnostic 1");
    session.expect("text controls");
    assert!(String::from_utf8_lossy(&session.output).contains("�[2Jtext controls"));
    session.resize(18, 70);
    session.expect("Mutation log");
    session.send(b"/\x1b[200~nonexistent\x1b[201~\r");
    session.expect("No matches");
    session.resize(5, 28);
    session.expect("resize to");
    session.resize(24, 110);
    session.expect("No matches");
    session.send(b"r");
    session.expect("Rerun command");
    session.send(b"e");
    assert!(session.finish().success());
    session.assert_restored();
    let output = String::from_utf8_lossy(&session.output);
    let restored = output.rfind("\x1b[?1049l").unwrap();
    let printed = &output[restored..];
    assert!(printed.contains("mutie '/missing/a project'\\''s checkout' --rerun '"));
    assert!(printed.contains("--id 1,2"));
}

#[test]
fn failed_run_without_inventory_is_inspectable_and_quits_cleanly() {
    let mut session = Session::start(true, None);
    session.expect("No mutations recorded");
    session.expect("baseline diagnostic");
    session.send(b"e");
    session.expect("no complete mutation inventory");
    session.send(b"q");
    assert!(session.finish().success());
    session.assert_restored();
}

#[test]
fn current_row_command_uses_the_module_override() {
    let mut session = Session::start(false, Some("/moved/$checkout"));
    session.send(b"je");
    assert!(session.finish().success());
    session.assert_restored();
    let output = String::from_utf8_lossy(&session.output);
    assert!(output.contains("mutie '/moved/$checkout' --rerun '"));
    assert!(output.contains("--id 2"));
}

#[test]
fn ctrl_c_and_sigterm_restore_the_terminal() {
    for signal in [false, true] {
        let mut session = Session::start(false, None);
        if signal {
            // SAFETY: signal only this test's live child.
            assert_eq!(
                unsafe { libc::kill(session.child.id() as i32, libc::SIGTERM) },
                0
            );
        } else {
            session.send(b"\x03");
        }
        assert!(!session.finish().success());
        session.assert_restored();
        let output = String::from_utf8_lossy(&session.output);
        assert!(output.rfind("interrupted").unwrap() > output.rfind("\x1b[?1049l").unwrap());
    }
}

#[test]
fn invalid_reports_and_nonterminals_fail_before_terminal_setup() {
    let temp = fixture(false);
    for path in [temp.path().join("missing"), temp.path().join("saved run")] {
        let output = Command::new(env!("CARGO_BIN_EXE_mutie"))
            .arg("report")
            .arg(&path)
            .current_dir(temp.path())
            .env("PATH", "")
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert!(!output.stdout.contains(&b'\x1b'));
        let error = String::from_utf8_lossy(&output.stderr);
        assert!(
            if path.ends_with("missing") {
                error.contains("opening report directory")
            } else {
                error.contains("interactive terminal")
            },
            "{error}"
        );
        assert!(!error.contains("go.mod"));
        assert!(!error.contains("mutie.toml"));
    }
}
