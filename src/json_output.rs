//! One flushed JSON record per execution event. Stdout is a data stream in this mode.
use std::{
    io::{self, Write},
    time::Instant,
};

use anyhow::{Context, Result};
use serde_json::Value;

pub struct JsonOutput {
    started: Instant,
    failed: bool,
}

impl JsonOutput {
    pub fn new(started: Instant) -> Self {
        Self {
            started,
            failed: false,
        }
    }

    pub fn emit(&mut self, mut event: Value) -> Result<()> {
        // A disconnected consumer must not prevent draining workers or saving
        // the failed run. The original write error already stops scheduling.
        if self.failed {
            return Ok(());
        }
        event["elapsed_seconds"] = self.started.elapsed().as_secs_f64().into();
        let result = write_event(&mut io::stdout().lock(), &event);
        self.failed = result.is_err();
        result
    }
}

fn write_event(writer: &mut impl Write, event: &Value) -> Result<()> {
    let mut bytes = serde_json::to_vec(event).context("encoding JSON event")?;
    bytes.push(b'\n');
    writer.write_all(&bytes).context("writing JSON event")?;
    writer.flush().context("flushing JSON event")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn records_escape_embedded_newlines_and_flush_without_waiting_for_completion() -> Result<()> {
        #[derive(Default)]
        struct Writer {
            bytes: Vec<u8>,
            flushes: usize,
        }
        impl Write for Writer {
            fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
                self.bytes.write(bytes)
            }
            fn flush(&mut self) -> io::Result<()> {
                self.flushes += 1;
                Ok(())
            }
        }
        let mut writer = Writer::default();
        let event =
            serde_json::json!({"event":"command_finished", "output":"line one\nline two\u{1b}"});
        write_event(&mut writer, &event)?;
        assert_eq!(writer.flushes, 1);
        assert_eq!(writer.bytes.iter().filter(|&&b| b == b'\n').count(), 1);
        assert_eq!(serde_json::from_slice::<Value>(&writer.bytes)?, event);
        Ok(())
    }
}
