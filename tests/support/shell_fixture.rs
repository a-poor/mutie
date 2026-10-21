//! Go test/list fixtures use the existing system shell as their PATH executable.
//! Loading a fresh executable can stall before entry on macOS. The shell reads
//! ordinary `test`/`list` input files from Mutie's copied workspace instead; no
//! production launch path, PID/group, environment or deadline changes are needed.
use std::{fs, os::unix::fs::symlink, path::Path};

pub fn install(root: &Path, program: &Path, script: &str) {
    // `go test ...` becomes `/bin/sh test ...`; restore the original first
    // argument for the fixture's command dispatch before evaluating its body.
    let script = format!("set -- \"$0\" \"$@\"\n{script}");
    for name in ["test", "list"] {
        fs::write(root.join(name), &script).unwrap();
    }
    symlink("/bin/sh", program).unwrap();
}
