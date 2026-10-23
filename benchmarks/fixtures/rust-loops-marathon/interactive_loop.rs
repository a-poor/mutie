#![cfg(unix)]
use std::{
    ffi::CString,
    fs::File,
    io::{Read, Write},
    os::{
        fd::{AsRawFd, FromRawFd},
        unix::ffi::OsStrExt,
    },
    thread,
    time::{Duration, Instant},
};

struct OwnedChild {
    pid: libc::pid_t,
    status: Option<i32>,
    master: Option<File>,
}
impl OwnedChild {
    fn status(&mut self) -> Option<i32> {
        if self.status.is_none() {
            let mut status = 0;
            let result = unsafe { libc::waitpid(self.pid, &mut status, libc::WNOHANG) };
            assert!(result >= 0, "waitpid failed");
            if result == self.pid {
                self.status = Some(status);
            }
        }
        self.status
    }
}
impl Drop for OwnedChild {
    fn drop(&mut self) {
        // Close the PTY before reaping, including after a failed readiness check.
        drop(self.master.take());
        if self.status.is_none() {
            unsafe {
                // The PID fallback covers cancellation before the child setsid.
                libc::kill(-self.pid, libc::SIGKILL);
                libc::kill(self.pid, libc::SIGKILL);
                let mut status = 0;
                while libc::waitpid(self.pid, &mut status, 0) < 0 {
                    if std::io::Error::last_os_error().raw_os_error() != Some(libc::EINTR) {
                        break;
                    }
                }
            }
        }
    }
}

#[test]
fn interactive_viewer_draws_before_accepting_quit() {
    let temp = tempfile::tempdir().unwrap();
    let book = temp.path().join("viewer.md");
    std::fs::write(&book, "# LOOP-READY-73591\n\nA read-only document.\n").unwrap();
    // Prepare all allocation before fork. The child uses only native syscalls
    // and execve, never the allocator or locks inherited from the test process.
    let args = [
        CString::new(env!("CARGO_BIN_EXE_marathon")).unwrap(),
        CString::new("run").unwrap(),
        CString::new(book.as_os_str().as_bytes()).unwrap(),
    ];
    let mut argv: Vec<_> = args.iter().map(|s| s.as_ptr()).collect();
    argv.push(std::ptr::null());
    let mut env: Vec<_> = std::env::vars_os()
        .filter(|(k, _)| k != "TERM")
        .map(|(k, v)| {
            let mut value = k.as_bytes().to_vec();
            value.push(b'=');
            value.extend_from_slice(v.as_bytes());
            CString::new(value).unwrap()
        })
        .collect();
    env.push(CString::new("TERM=xterm-256color").unwrap());
    let mut envp: Vec<_> = env.iter().map(|s| s.as_ptr()).collect();
    envp.push(std::ptr::null());
    let (master, slave) = unsafe {
        let (mut master, mut slave) = (-1, -1);
        let mut size = libc::winsize {
            ws_row: 20,
            ws_col: 80,
            ws_xpixel: 0,
            ws_ypixel: 0,
        };
        assert_eq!(
            libc::openpty(
                &mut master,
                &mut slave,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                &mut size
            ),
            0
        );
        (File::from_raw_fd(master), File::from_raw_fd(slave))
    };
    let deadline = Instant::now() + Duration::from_secs(10);
    let pid = unsafe { libc::fork() };
    assert!(pid >= 0, "fork failed");
    if pid == 0 {
        unsafe {
            if libc::setsid() < 0 || libc::ioctl(slave.as_raw_fd(), libc::TIOCSCTTY as _, 0) < 0 {
                libc::_exit(126);
            }
            for fd in 0..3 {
                if libc::dup2(slave.as_raw_fd(), fd) < 0 {
                    libc::_exit(126);
                }
            }
            libc::close(master.as_raw_fd());
            if slave.as_raw_fd() > 2 {
                libc::close(slave.as_raw_fd());
            }
            libc::execve(args[0].as_ptr(), argv.as_ptr(), envp.as_ptr());
            libc::_exit(127);
        }
    }
    drop(slave);
    unsafe {
        let flags = libc::fcntl(master.as_raw_fd(), libc::F_GETFL);
        assert!(flags >= 0);
        assert_eq!(
            libc::fcntl(master.as_raw_fd(), libc::F_SETFL, flags | libc::O_NONBLOCK),
            0
        );
    }
    let mut child = OwnedChild {
        pid,
        status: None,
        master: Some(master),
    };
    let mut output = Vec::new();
    loop {
        let mut chunk = [0; 4096];
        match child.master.as_mut().unwrap().read(&mut chunk) {
            Ok(n) => output.extend_from_slice(&chunk[..n]),
            Err(e)
                if e.kind() == std::io::ErrorKind::WouldBlock
                    || e.raw_os_error() == Some(libc::EIO) => {}
            Err(e) => panic!("terminal read: {e}"),
        }
        assert!(output.len() <= 65536, "terminal capture budget exceeded");
        if String::from_utf8_lossy(&output).contains("LOOP-READY-73591") {
            break;
        }
        assert!(
            child.status().is_none(),
            "viewer exited without rendering its document: {}",
            String::from_utf8_lossy(&output)
        );
        assert!(
            Instant::now() < deadline,
            "viewer did not render within ten seconds: {}",
            String::from_utf8_lossy(&output)
        );
        thread::sleep(Duration::from_millis(10));
    }
    assert!(child.status().is_none(), "viewer must wait for input");
    child.master.as_mut().unwrap().write_all(b"q").unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        // Keep draining restoration output while waiting for terminal shutdown.
        let mut chunk = [0; 4096];
        match child.master.as_mut().unwrap().read(&mut chunk) {
            Ok(n) => output.extend_from_slice(&chunk[..n]),
            Err(e)
                if e.kind() == std::io::ErrorKind::WouldBlock
                    || e.raw_os_error() == Some(libc::EIO) => {}
            Err(e) => panic!("terminal shutdown read: {e}"),
        }
        assert!(output.len() <= 65536, "terminal capture budget exceeded");
        if let Some(status) = child.status() {
            assert!(libc::WIFEXITED(status));
            assert_eq!(libc::WEXITSTATUS(status), 0);
            break;
        }
        assert!(
            Instant::now() < deadline,
            "viewer did not honor quit: {}",
            String::from_utf8_lossy(&output)
        );
        thread::sleep(Duration::from_millis(10));
    }
}
