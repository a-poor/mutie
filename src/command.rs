//! Captures and outcome evidence shared by language adapters.
use anyhow::{Result, bail};

use crate::{capture::Capture, process, report::Outcome};

pub struct CommandOutput {
    pub(crate) raw: process::CommandOutput,
    pub text: Capture,
    pub(crate) evidence: OutcomeEvidence,
    pub(crate) tool: &'static str,
}

/// Exit-status adapters retain their existing failure evidence. Structured
/// runners must validate completion even when the process exits successfully.
#[derive(Debug)]
pub enum OutcomeEvidence {
    ExitStatus(Failure),
    Validated(Result<Outcome, String>),
}

#[derive(Clone, Copy, Debug)]
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
    pub fn classify(&self) -> Result<Outcome> {
        classify(self)
    }

    #[cfg(test)]
    pub fn mark_interrupted(&mut self) {
        self.raw.interrupted = true;
    }
}

pub fn classify(output: &CommandOutput) -> Result<Outcome> {
    if output.interrupted {
        return Err(crate::cancellation::Interrupted.into());
    }
    if let Some(error) = &output.capture_error {
        bail!("{} test output capture failed: {error}", output.tool);
    }
    if output.timed_out {
        return Ok(Outcome::Timeout);
    }
    let failure = match &output.evidence {
        OutcomeEvidence::Validated(result) => {
            return result
                .as_ref()
                .copied()
                .map_err(|error| anyhow::anyhow!("{error}"));
        }
        OutcomeEvidence::ExitStatus(failure) => failure,
    };
    if output.status.success() {
        return Ok(Outcome::Missed);
    }
    match failure {
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

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::{os::unix::process::ExitStatusExt, time::Duration};

    fn structured(result: Result<Outcome, String>, code: i32) -> CommandOutput {
        CommandOutput {
            raw: process::CommandOutput {
                status: std::process::ExitStatus::from_raw(code << 8),
                timed_out: false,
                interrupted: false,
                capture_error: None,
                elapsed: Duration::ZERO,
                stdout: Capture::from_bytes(b"raw diagnostics"),
                stderr: Capture::from_bytes(b""),
            },
            text: Capture::from_bytes(b"retained diagnostics"),
            evidence: OutcomeEvidence::Validated(result),
            tool: "structured test runner",
        }
    }

    #[test]
    fn completion_evidence_is_required_even_after_successful_exit() -> Result<()> {
        let output = structured(Err("missing completion".into()), 0);
        assert!(
            classify(&output)
                .unwrap_err()
                .to_string()
                .contains("missing completion")
        );
        assert_eq!(text(&output)?, "retained diagnostics");
        for (outcome, code) in [
            (Outcome::Missed, 0),
            (Outcome::Killed, 1),
            (Outcome::BuildFailed, 1),
        ] {
            assert_eq!(classify(&structured(Ok(outcome), code))?, outcome);
        }
        Ok(())
    }

    #[test]
    fn process_failures_take_precedence_over_completion() -> Result<()> {
        for evidence in [Ok(Outcome::Missed), Err("missing completion".into())] {
            let mut output = structured(evidence, 0);
            output.raw.timed_out = true;
            assert_eq!(classify(&output)?, Outcome::Timeout);
            output.raw.capture_error = Some("budget exceeded".into());
            assert!(
                classify(&output)
                    .unwrap_err()
                    .to_string()
                    .contains("budget exceeded")
            );
            output.raw.interrupted = true;
            assert!(
                classify(&output)
                    .unwrap_err()
                    .is::<crate::cancellation::Interrupted>()
            );
        }
        Ok(())
    }
}
