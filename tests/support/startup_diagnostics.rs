//! Best-effort post-deadline diagnostics for an owned, unreaped CLI child.
//! Sampling never turns a failed readiness check into a pass or extends its bound.

#[cfg(target_os = "macos")]
pub fn sample(child: &std::process::Child) -> String {
    use std::{
        fs::File,
        io::Read,
        process::{Command, Stdio},
        time::{Duration, Instant},
    };

    let capture = match tempfile::NamedTempFile::new() {
        Ok(file) => file,
        Err(error) => return format!("startup sample unavailable: {error}"),
    };
    let errors = match tempfile::NamedTempFile::new() {
        Ok(file) => file,
        Err(error) => return format!("startup sample unavailable: {error}"),
    };
    let error_output = match errors.reopen() {
        Ok(file) => file,
        Err(error) => return format!("startup sample unavailable: {error}"),
    };
    let mut sampler = match Command::new("/usr/bin/sample")
        .arg(child.id().to_string())
        .args(["1", "-file"])
        .arg(capture.path())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(error_output)
        .spawn()
    {
        Ok(sampler) => sampler,
        Err(error) => return format!("startup sample unavailable: {error}"),
    };
    let deadline = Instant::now() + Duration::from_secs(3);
    let status = loop {
        match sampler.try_wait() {
            Ok(Some(status)) => break format!("{status}"),
            Ok(None) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(10));
            }
            result => {
                let _ = sampler.kill();
                let _ = sampler.wait();
                break format!("sample deadline or wait error: {result:?}");
            }
        }
    };
    let mut bytes = Vec::new();
    if let Ok(file) = File::open(capture.path()) {
        // Even diagnostic failures must not flood the test harness output.
        let _ = file.take(64 * 1024).read_to_end(&mut bytes);
    }
    let mut error_bytes = Vec::new();
    if let Ok(file) = File::open(errors.path()) {
        let _ = file.take(4 * 1024).read_to_end(&mut error_bytes);
    }
    format!(
        "post-deadline CLI sample ({status}; at most 64 KiB plus 4 KiB stderr):\n{}\nsampler stderr: {}",
        String::from_utf8_lossy(&bytes),
        String::from_utf8_lossy(&error_bytes)
    )
}

#[cfg(not(target_os = "macos"))]
pub fn sample(_child: &std::process::Child) -> String {
    "automatic startup stack sampling is only available on macOS".into()
}
