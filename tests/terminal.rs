#![cfg(unix)]

//! A small PTY harness checks behavior that the in-memory renderer cannot:
//! screen/scrollback preservation, cursor queries, redraws during a blocked
//! command, real Ctrl-C and termios.
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
    sync::Mutex,
    thread,
    time::{Duration, Instant},
};

// openpty has no atomic close-on-exec option. Keep another test from spawning
// between descriptor allocation and setting FD_CLOEXEC; sessions still run in
// parallel once their children have started.
static SPAWN_LOCK: Mutex<()> = Mutex::new(());
// This is a whole-workflow watchdog, not a command timeout. Under competing
// Cargo builds a successful baseline/discovery/mutation run can exceed 10s.
const WAIT_LIMIT: Duration = Duration::from_secs(30);

struct Session {
    _temp: tempfile::TempDir,
    description: String,
    started: Instant,
    last_pump: Instant,
    longest_pump_gap: Duration,
    cursor_queries: usize,
    root: PathBuf,
    state: PathBuf,
    workspaces: PathBuf,
    master: File,
    _slave: File,
    child: Child,
    output: Vec<u8>,
    screen: vt100::Parser,
    query_tail: Vec<u8>,
    respond_to_queries: bool,
    termios: libc::termios,
}

impl Session {
    fn start(mode: &str, ui: &str, ci: bool) -> Self {
        Self::start_with_terminal(mode, ui, ci, (24, 100), 2, PREAMBLE)
    }

    fn start_with_terminal(
        mode: &str,
        ui: &str,
        ci: bool,
        (rows, columns): (u16, u16),
        workers: usize,
        preamble: &str,
    ) -> Self {
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
# Tool processes receive null stdin and captured stdout/stderr. They must not
# inherit this session's PTY handles or handles belonging to concurrent tests.
for fd in /dev/fd/*; do
    fd=${fd##*/}
    if [ -t "$fd" ]; then
        printf 'inherited terminal descriptor %s\n' "$fd" >&2
        exit 1
    fi
done
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

        let spawning = SPAWN_LOCK.lock().unwrap_or_else(|error| error.into_inner());
        let mut master = -1;
        let mut slave = -1;
        let mut size = libc::winsize {
            ws_row: rows,
            ws_col: columns,
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
        let (master, mut slave) = unsafe { (File::from_raw_fd(master), File::from_raw_fd(slave)) };
        for file in [&master, &slave] {
            // SAFETY: operate only on the two newly owned descriptors. Without
            // CLOEXEC every CLI and tool child can keep other sessions alive.
            let flags = unsafe { libc::fcntl(file.as_raw_fd(), libc::F_GETFD) };
            assert_ne!(flags, -1, "{}", io::Error::last_os_error());
            assert_eq!(
                unsafe { libc::fcntl(file.as_raw_fd(), libc::F_SETFD, flags | libc::FD_CLOEXEC) },
                0,
                "{}",
                io::Error::last_os_error()
            );
        }
        let termios = termios(&slave);
        // SAFETY: operate only on the owned master descriptor.
        let flags = unsafe { libc::fcntl(master.as_raw_fd(), libc::F_GETFL) };
        assert_eq!(
            unsafe { libc::fcntl(master.as_raw_fd(), libc::F_SETFL, flags | libc::O_NONBLOCK) },
            0
        );
        // Seed the terminal before the CLI starts, just as a shell would.
        slave.write_all(preamble.as_bytes()).unwrap();
        let mut paths = vec![bin];
        paths.extend(std::env::split_paths(
            &std::env::var_os("PATH").unwrap_or_default(),
        ));
        let mut command = Command::new(env!("CARGO_BIN_EXE_mutie"));
        command
            .arg(&root)
            .args([
                "--ui",
                ui,
                "--jobs",
                &workers.to_string(),
                "--timeout",
                "10",
                "--verbose",
            ])
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
        if mode == "policy" {
            command.arg("--fail-on-missed");
        } else if mode == "score_policy" {
            command.args(["--min-score", "1"]);
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
        drop(spawning);
        Self {
            _temp: temp,
            description: format!(
                "mode={mode}, ui={ui}, ci={ci}, size={rows}x{columns}, workers={workers}"
            ),
            started: Instant::now(),
            last_pump: Instant::now(),
            longest_pump_gap: Duration::ZERO,
            cursor_queries: 0,
            root,
            state,
            workspaces,
            master,
            _slave: slave,
            child,
            output: Vec::new(),
            screen: vt100::Parser::new(rows, columns, 10_000),
            query_tail: Vec::new(),
            respond_to_queries: mode != "no_cursor",
            termios,
        }
    }

    fn pump(&mut self) {
        let now = Instant::now();
        self.longest_pump_gap = self.longest_pump_gap.max(now - self.last_pump);
        self.last_pump = now;
        let mut buf = [0; 8192];
        loop {
            match self.master.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    self.consume(&buf[..n]);
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

    fn consume(&mut self, bytes: &[u8]) {
        self.output.extend_from_slice(bytes);
        for byte in bytes {
            self.screen.process(std::slice::from_ref(byte));
            self.query_tail.push(*byte);
            if self.query_tail.ends_with(b"\x1b[6n") {
                self.cursor_queries += 1;
                if self.respond_to_queries {
                    let (row, col) = self.screen.screen().cursor_position();
                    let (_, columns) = self.screen.screen().size();
                    let col = col.min(columns - 1); // pending wrap at the right edge
                    write!(self.master, "\x1b[{};{}R", row + 1, col + 1).unwrap();
                }
            }
            if self.query_tail.len() == 4 {
                self.query_tail.remove(0);
            }
        }
    }

    fn resize_with_queued_output(&mut self, rows: u16, columns: u16) {
        // Hold bytes produced at the old size until after the emulator resizes.
        // This forces the race without relying on a resize landing on a tick.
        let deadline = Instant::now() + WAIT_LIMIT;
        let mut bytes = [0; 8192];
        loop {
            match self.master.read(&mut bytes) {
                Ok(n) if n > 0 => {
                    self.resize_screen(rows, columns);
                    self.consume(&bytes[..n]);
                    return;
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {}
                other => panic!("waiting for queued frame: {other:?}"),
            }
            assert!(Instant::now() < deadline, "no frame arrived");
            thread::sleep(Duration::from_millis(10));
        }
    }

    fn until(&mut self, condition: impl Fn(&Self) -> bool) {
        let deadline = Instant::now() + WAIT_LIMIT;
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
        self.pump();
        self.resize_screen(rows, columns);
    }

    fn resize_screen(&mut self, rows: u16, columns: u16) {
        // vt100 truncates rows on resize. Model a terminal that scrolls enough
        // to keep its cursor visible on height shrink, preserving those rows
        // in scrollback. Width changes still do not reflow; markers fit the
        // narrowest width used in these tests.
        let (row, col) = self.screen.screen().cursor_position();
        let (old_rows, old_columns) = self.screen.screen().size();
        if row >= rows {
            self.screen
                .process(format!("\x1b[{}S\x1b[{};{}H", row + 1 - rows, rows, col + 1).as_bytes());
        } else if rows > old_rows {
            // Model pulling scrollback back onto the screen when growing.
            // Rebuild the text grid so pulled rows leave scrollback and the
            // cursor moves with them. Color assertions use renderer tests.
            let mut saved = self.screen.screen().clone();
            saved.set_scrollback(usize::MAX);
            let backlog = saved.scrollback();
            let pulled = backlog.min(usize::from(rows - old_rows));
            let mut lines = Vec::new();
            for offset in (1..=backlog).rev() {
                saved.set_scrollback(offset);
                lines.push(saved.rows(0, old_columns).next().unwrap());
            }
            saved.set_scrollback(0);
            lines.extend(saved.rows(0, old_columns));
            lines.extend(std::iter::repeat_n(
                String::new(),
                usize::from(rows - old_rows) - pulled,
            ));
            self.screen = vt100::Parser::new(rows, old_columns, 10_000);
            self.screen.process(lines.join("\r\n").as_bytes());
            self.screen
                .process(format!("\x1b[{};{}H", usize::from(row) + pulled + 1, col + 1).as_bytes());
        }
        self.screen.screen_mut().set_size(rows, columns);
        self.assert_history();
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
        let deadline = Instant::now() + WAIT_LIMIT;
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
        let deadline = Instant::now() + WAIT_LIMIT;
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
            assert!(
                !output.contains("\x1b[2J"),
                "must not erase the whole screen"
            );
            assert!(!output.contains("\x1b[3J"), "must not erase scrollback");
            assert!(
                !self.screen.screen().contents().contains("Ctrl-C to stop"),
                "the live status area must be cleared before the final summary"
            );
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
        self.assert_history();
        assert!(fs::read_dir(&self.workspaces).unwrap().next().is_none());
        assert!(
            !fs::read_to_string(self.root.join("code.go"))
                .unwrap()
                .contains("false")
        );
    }

    fn assert_history(&self) {
        let mut screen = self.screen.screen().clone();
        screen.set_scrollback(usize::MAX);
        let scrollback = screen.scrollback();
        for marker in ["earlier output", "$ mutie"] {
            assert!(
                (0..=scrollback).any(|offset| {
                    screen.set_scrollback(offset);
                    screen.contents().contains(marker)
                }),
                "lost {marker:?} from screen and {scrollback} scrollback rows; screen:\n{}",
                self.screen.screen().contents()
            );
        }
    }

    fn dump_diagnostics(&mut self) {
        eprintln!(
            "PTY session: {}; pid={}, elapsed={:?}, exit after cleanup={:?}\n\
             cursor queries={}, responses enabled={}, longest pump gap={:?}\nscreen:\n{}",
            self.description,
            self.child.id(),
            self.started.elapsed(),
            self.child.try_wait(),
            self.cursor_queries,
            self.respond_to_queries,
            self.longest_pump_gap,
            self.screen.screen().contents(),
        );
        let start = self.output.len().saturating_sub(32 * 1024);
        eprintln!(
            "PTY output (last {} of {} bytes, escaped):\n{:?}",
            self.output.len() - start,
            self.output.len(),
            String::from_utf8_lossy(&self.output[start..]),
        );
        // Emit saved errors and command diagnostics before TempDir removes them.
        // All reads are best effort so a missing/partial report cannot mask the
        // original assertion failure. Bound each file even for a noisy child.
        let Ok(runs) = fs::read_dir(self.root.join(".mutie")) else {
            return;
        };
        for run in runs.flatten() {
            let directory = run.path();
            let mut files = vec![directory.join("run.json"), directory.join("outcomes.json")];
            if let Ok(logs) = fs::read_dir(directory.join("logs")) {
                files.extend(
                    logs.flatten().map(|entry| entry.path()).filter(|path| {
                        path.extension().is_some_and(|extension| extension == "log")
                    }),
                );
            }
            files.sort();
            for path in files {
                let Ok(file) = File::open(&path) else {
                    continue;
                };
                let mut bytes = Vec::new();
                if file.take(16 * 1024).read_to_end(&mut bytes).is_ok() {
                    eprintln!(
                        "{} (up to 16 KiB):\n{}",
                        path.display(),
                        String::from_utf8_lossy(&bytes)
                    );
                }
            }
        }
    }
}

const PREAMBLE: &str = "earlier output\n$ mutie\n";

impl Drop for Session {
    fn drop(&mut self) {
        if self.child.try_wait().ok().flatten().is_none() {
            // SAFETY: request cleanup from the child owned by this session.
            unsafe {
                libc::kill(self.child.id() as i32, libc::SIGTERM);
            }
            // Keep draining even on assertion failure: macOS session-leader
            // exit can wait for queued PTY output before becoming waitable.
            for attempt in 0..200 {
                let mut buf = [0; 8192];
                while let Ok(n) = self.master.read(&mut buf) {
                    if n == 0 {
                        break;
                    }
                    self.output.extend_from_slice(&buf[..n]);
                }
                if self.child.try_wait().ok().flatten().is_some() {
                    break;
                }
                if attempt == 100 {
                    let _ = self.child.kill();
                }
                thread::sleep(Duration::from_millis(10));
            }
        }
        if thread::panicking() {
            self.dump_diagnostics();
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
        session.resize(rows, cols);
        let time = Instant::now();
        session.until(|_| time.elapsed() > Duration::from_millis(150));
    }
    fs::write(session.state.join("release"), "").unwrap();
    assert!(session.finish().success());
    session.assert_restored(true);
    assert_eq!(session.report("outcomes.json")["summary"]["missed"], 4);
}

#[test]
fn width_shrink_preserves_preceding_output_and_prompt() {
    let mut session = Session::start("live", "interactive", false);
    session.until(|s| s.screen.screen().contents().contains("baseline"));
    session.assert_history();
    session.resize(24, 80);
    let time = Instant::now();
    session.until(|_| time.elapsed() >= Duration::from_millis(250));
    session.assert_history();
    fs::write(session.state.join("release"), "").unwrap();
    assert!(session.finish().success());
    session.assert_restored(true);
}

#[test]
fn short_and_oversized_views_preserve_history_on_completion_and_cancellation() {
    // vt100 cannot wrap on a single-row grid; cover narrow and one-row
    // terminals separately, keeping the one-row case wide enough for logs.
    for size in [(24, 100), (5, 28), (3, 18), (2, 14), (1, 512)] {
        for cancel in [false, true] {
            let mut session =
                Session::start_with_terminal("live", "interactive", false, size, 64, PREAMBLE);
            session.until(|s| s.state.join("baseline.ready").exists());
            let time = Instant::now();
            session.until(|_| time.elapsed() >= Duration::from_millis(150));
            session.assert_history();
            let mut screen = session.screen.screen().clone();
            screen.set_scrollback(usize::MAX);
            let first_query = session
                .output
                .windows(4)
                .position(|w| w == b"\x1b[6n")
                .unwrap();
            let mut before_ui = vt100::Parser::new(size.0, size.1, 10_000);
            before_ui.process(&session.output[..first_query]);
            let (row, _) = before_ui.screen().cursor_position();
            before_ui.screen_mut().set_scrollback(usize::MAX);
            assert_eq!(
                screen.scrollback(),
                before_ui.screen().scrollback() + usize::from(row),
                "oversized view must scroll only the pre-existing content at {size:?}"
            );
            if cancel {
                session.master.write_all(&[3]).unwrap();
            } else {
                fs::write(session.state.join("release"), "").unwrap();
            }
            assert_eq!(session.finish().success(), !cancel, "size {size:?}");
            session.assert_restored(true);
        }
    }
}

#[test]
fn resizing_preserves_scrollback_and_prompt_near_the_bottom() {
    for cancel in [false, true] {
        let preamble = format!("{}{PREAMBLE}", "\n".repeat(21));
        let mut session =
            Session::start_with_terminal("live", "interactive", false, (24, 100), 2, &preamble);
        session.until(|s| s.screen.screen().contents().contains("baseline"));
        session.assert_history();
        for size in [(24, 80), (5, 28), (3, 18), (24, 100), (40, 120)] {
            session.resize(size.0, size.1);
            let time = Instant::now();
            session.until(|_| time.elapsed() >= Duration::from_millis(150));
            session.assert_history();
        }
        if cancel {
            session.master.write_all(&[3]).unwrap();
        } else {
            fs::write(session.state.join("release"), "").unwrap();
        }
        assert_eq!(session.finish().success(), !cancel);
        session.assert_restored(true);
        let visible = session.screen.screen().contents();
        assert!(
            visible.contains("earlier output") && visible.contains("$ mutie"),
            "{visible}"
        );
    }
}

#[test]
fn queued_frames_cannot_erase_history_after_resize() {
    for cancel in [false, true] {
        let preamble = format!("{}{PREAMBLE}", "\n".repeat(21));
        let mut session =
            Session::start_with_terminal("live", "interactive", false, (24, 100), 2, &preamble);
        session.until(|s| s.screen.screen().contents().contains("baseline"));
        for (rows, columns) in [(40, 120), (5, 28), (24, 100)] {
            session.resize_with_queued_output(rows, columns);
            let start = Instant::now();
            session.until(|_| start.elapsed() >= Duration::from_millis(150));
            session.assert_history();
        }
        if cancel {
            session.master.write_all(&[3]).unwrap();
        } else {
            fs::write(session.state.join("release"), "").unwrap();
        }
        assert_eq!(session.finish().success(), !cancel);
        session.assert_restored(true);
    }
}

#[test]
fn startup_preserves_a_partial_prompt_line() {
    let mut session = Session::start_with_terminal(
        "success",
        "interactive",
        false,
        (24, 100),
        2,
        PREAMBLE.trim_end(),
    );
    assert!(session.finish().success());
    session.assert_restored(true);
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
fn findings_failure_restores_terminal_and_preserves_completed_report() {
    for (ui, ci, interactive) in [("interactive", false, true), ("auto", true, false)] {
        let mut session = Session::start("policy", ui, ci);
        assert_eq!(session.finish().code(), Some(3));
        session.assert_restored(interactive);
        let report = session.report("run.json");
        assert_eq!(report["status"], "completed");
        assert_eq!(report["policy"]["status"], "failed");
        assert_eq!(report["policy"]["undismissed_missed"], 4);
        let output = String::from_utf8_lossy(&session.output);
        assert!(output.contains("Findings policy: failed"));
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

#[test]
fn score_failure_restores_terminal_and_preserves_completed_report() {
    for (ui, ci, interactive) in [("interactive", false, true), ("auto", true, false)] {
        let mut session = Session::start("score_policy", ui, ci);
        assert_eq!(session.finish().code(), Some(3));
        session.assert_restored(interactive);
        let report = session.report("run.json");
        assert_eq!(report["status"], "completed");
        assert_eq!(report["policy"]["status"], "disabled");
        assert_eq!(report["score_policy"]["status"], "failed");
        assert_eq!(report["score"]["percentage"], 0.0);
        let output = String::from_utf8_lossy(&session.output);
        assert!(output.contains("Minimum-score policy: failed"));
        assert!(output.contains("Mutation score: 0.00%"));
    }
}
