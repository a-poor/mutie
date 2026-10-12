//! Captures and outcome evidence shared by language adapters.
use anyhow::{Result, bail, ensure};

use crate::{capture::Capture, process, report::Outcome};

pub struct CommandOutput {
    pub(crate) raw: process::CommandOutput,
    pub text: Capture,
    pub(crate) failure: Failure,
    pub(crate) tool: &'static str,
}

#[derive(Clone, Copy)]
pub enum Failure {
    Build,
    Test,
    Unknown,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TestPhase {
    #[default]
    Running,
    Building,
    Testing,
}

impl std::ops::Deref for CommandOutput {
    type Target = process::CommandOutput;

    fn deref(&self) -> &Self::Target {
        &self.raw
    }
}

impl CommandOutput {
    #[cfg(test)]
    pub fn mark_interrupted(&mut self) {
        self.raw.interrupted = true;
    }
}

pub fn classify(output: &CommandOutput) -> Result<Outcome> {
    ensure!(
        !output.interrupted,
        "cannot classify an interrupted command"
    );
    if let Some(error) = &output.capture_error {
        bail!("{} test output capture failed: {error}", output.tool);
    }
    if output.timed_out {
        return Ok(Outcome::Timeout);
    }
    if output.status.success() {
        return Ok(Outcome::Missed);
    }
    match output.failure {
        Failure::Build => Ok(Outcome::BuildFailed),
        Failure::Test => Ok(Outcome::Killed),
        Failure::Unknown => bail!(
            "{} command failed without a test or build failure ({}):\n{}",
            output.tool,
            output.status,
            text(output)?
        ),
    }
}

pub fn text(output: &CommandOutput) -> Result<String> {
    output.text.text()
}

/// Decode stderr incrementally, retaining partial UTF-8 across chunk boundaries.
pub(crate) fn write_lossy(
    reader: &mut impl std::io::Read,
    writer: &mut impl std::io::Write,
) -> std::io::Result<()> {
    let mut bytes = [0; 16 * 1024];
    let mut pending = Vec::new();
    loop {
        let count = reader.read(&mut bytes)?;
        pending.extend_from_slice(&bytes[..count]);
        let mut offset = 0;
        while offset < pending.len() {
            match std::str::from_utf8(&pending[offset..]) {
                Ok(text) => {
                    writer.write_all(text.as_bytes())?;
                    offset = pending.len();
                }
                Err(error) => {
                    let end = offset + error.valid_up_to();
                    writer.write_all(&pending[offset..end])?;
                    offset = end;
                    match error.error_len() {
                        Some(len) => {
                            writer.write_all("�".as_bytes())?;
                            offset += len;
                        }
                        None if count == 0 => {
                            writer.write_all("�".as_bytes())?;
                            offset = pending.len();
                        }
                        None => break,
                    }
                }
            }
        }
        pending.drain(..offset);
        if count == 0 {
            return Ok(());
        }
    }
}
