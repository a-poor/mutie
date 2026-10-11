//! File-backed command output with optional head/tail retention for artifacts.
use std::{
    fs::File,
    io::{self, Read, Seek, SeekFrom, Write},
    num::NonZeroUsize,
    path::Path,
};

use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Retention {
    pub total_bytes: u64,
    pub head_bytes: u64,
    pub tail_bytes: u64,
    pub omitted_bytes: u64,
}

impl Retention {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.head_bytes
                .checked_add(self.tail_bytes)
                .and_then(|n| n.checked_add(self.omitted_bytes))
                == Some(self.total_bytes)
                && (self.omitted_bytes > 0 || self.tail_bytes == 0),
            "invalid log retention byte counts"
        );
        Ok(())
    }

    pub fn notice(&self) -> String {
        format!(
            "[Log truncated: {} bytes omitted; retained {} head + {} tail of {} bytes]",
            self.omitted_bytes, self.head_bytes, self.tail_bytes, self.total_bytes
        )
    }
}

pub struct Capture {
    file: tempfile::NamedTempFile,
    total_bytes: u64,
    limit: Option<NonZeroUsize>,
    utf8: bool,
}

impl Capture {
    pub fn new(file: tempfile::NamedTempFile) -> io::Result<Self> {
        let total_bytes = file.as_file().metadata()?.len();
        Ok(Self {
            file,
            total_bytes,
            limit: None,
            utf8: false,
        })
    }

    /// The caller has rendered valid UTF-8; keep cuts on character boundaries.
    pub fn utf8(file: tempfile::NamedTempFile) -> io::Result<Self> {
        let mut capture = Self::new(file)?;
        capture.utf8 = true;
        Ok(capture)
    }

    pub fn set_limit(&mut self, limit: Option<NonZeroUsize>) {
        self.limit = limit;
    }

    /// Always reads the complete capture, independently of the artifact limit.
    pub fn reader(&self) -> io::Result<io::Take<File>> {
        // Freeze the endpoint after process-group cleanup. A surviving external
        // writer cannot extend parsing indefinitely or change retention counts.
        Ok(self.file.reopen()?.take(self.total_bytes))
    }

    pub fn retention(&self) -> Result<Retention> {
        let total_bytes = self.total_bytes;
        let keep = self
            .limit
            .map(|n| n.get() as u64)
            .unwrap_or(total_bytes)
            .min(total_bytes);
        let mut tail_bytes = if total_bytes > keep { keep / 2 } else { 0 };
        let mut head_bytes = keep - tail_bytes;
        if self.utf8 && total_bytes > keep {
            let mut reader = self.file.reopen()?;
            let mut continuation = |offset| -> io::Result<bool> {
                reader.seek(SeekFrom::Start(offset))?;
                let mut byte = [0];
                Ok(reader.read(&mut byte)? == 1 && byte[0] & 0xc0 == 0x80)
            };
            while head_bytes > 0 && continuation(head_bytes)? {
                head_bytes -= 1;
            }
            while tail_bytes > 0 && continuation(total_bytes - tail_bytes)? {
                tail_bytes -= 1;
            }
        }
        Ok(Retention {
            total_bytes,
            head_bytes,
            tail_bytes,
            omitted_bytes: total_bytes - head_bytes - tail_bytes,
        })
    }

    fn write_retained(&self, writer: &mut impl Write) -> Result<()> {
        let retention = self.retention()?;
        let mut reader = self.file.reopen()?;
        io::copy(&mut (&mut reader).take(retention.head_bytes), writer)?;
        if retention.omitted_bytes > 0 {
            writeln!(writer, "\n{}", retention.notice())?;
            reader.seek(SeekFrom::Start(
                retention.total_bytes - retention.tail_bytes,
            ))?;
            io::copy(&mut reader.take(retention.tail_bytes), writer)?;
        }
        Ok(())
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        let mut file =
            tempfile::NamedTempFile::new_in(path.parent().context("missing log parent")?)?;
        self.write_retained(&mut file)?;
        file.as_file().sync_all()?;
        file.persist(path)
            .with_context(|| format!("saving {}", path.display()))?;
        Ok(())
    }

    pub fn text(&self) -> Result<String> {
        let mut bytes = Vec::new();
        self.write_retained(&mut bytes)?;
        Ok(String::from_utf8_lossy(&bytes).into_owned())
    }

    #[cfg(test)]
    pub fn from_bytes(bytes: &[u8]) -> Self {
        let mut file = tempfile::NamedTempFile::new().unwrap();
        file.write_all(bytes).unwrap();
        Self::new(file).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn readers_and_counts_ignore_writes_after_capture_completion() -> Result<()> {
        let mut capture = Capture::from_bytes(b"first");
        capture.set_limit(NonZeroUsize::new(3));
        let mut late_writer = capture.file.reopen()?;
        late_writer.seek(SeekFrom::End(0))?;
        late_writer.write_all(b"later output")?;
        assert_eq!(capture.retention()?.total_bytes, 5);
        let mut complete = String::new();
        capture.reader()?.read_to_string(&mut complete)?;
        assert_eq!(complete, "first");
        assert!(capture.text()?.ends_with('t'));
        Ok(())
    }

    #[test]
    fn readable_cuts_never_split_utf8_and_still_respect_byte_budget() -> Result<()> {
        let original = "界é☀\r\nhello\n☀é界";
        for limit in 1..=original.len() + 1 {
            let mut file = tempfile::NamedTempFile::new()?;
            file.write_all(original.as_bytes())?;
            let mut capture = Capture::utf8(file)?;
            capture.set_limit(NonZeroUsize::new(limit));
            let mut saved = Vec::new();
            capture.write_retained(&mut saved)?;
            assert!(std::str::from_utf8(&saved).is_ok(), "limit {limit}");
            let retention = capture.retention()?;
            retention.validate()?;
            assert!(retention.head_bytes + retention.tail_bytes <= limit as u64);
            assert!(original.is_char_boundary(retention.head_bytes as usize));
            assert!(original.is_char_boundary(original.len() - retention.tail_bytes as usize));
        }
        Ok(())
    }

    #[test]
    fn head_tail_counts_and_atomic_artifacts_cover_tiny_exact_and_unlimited_limits() -> Result<()> {
        for bytes in [
            b"".as_slice(),
            b"0123456789",
            "界\r\né\n".as_bytes(),
            b"\xff\xfe\n",
        ] {
            for limit in [
                None,
                NonZeroUsize::new(1),
                NonZeroUsize::new(5),
                NonZeroUsize::new(bytes.len()),
                NonZeroUsize::new(100),
            ] {
                let mut capture = Capture::from_bytes(bytes);
                capture.set_limit(limit);
                let retention = capture.retention()?;
                retention.validate()?;
                let keep = limit
                    .map(|l| l.get())
                    .unwrap_or(bytes.len())
                    .min(bytes.len());
                assert_eq!(retention.head_bytes + retention.tail_bytes, keep as u64);
                assert_eq!(retention.omitted_bytes, (bytes.len() - keep) as u64);
                let root = tempfile::tempdir()?;
                let path = root.path().join("output.log");
                capture.save(&path)?;
                let saved = std::fs::read(&path)?;
                assert_eq!(capture.text()?, String::from_utf8_lossy(&saved));
                if retention.omitted_bytes == 0 {
                    assert_eq!(saved, bytes);
                } else {
                    assert!(saved.starts_with(&bytes[..retention.head_bytes as usize]));
                    assert!(saved.ends_with(&bytes[bytes.len() - retention.tail_bytes as usize..]));
                    assert_eq!(saved.len(), keep + retention.notice().len() + 2);
                }
                assert_eq!(std::fs::read_dir(root.path())?.count(), 1);
                let mut complete = Vec::new();
                capture.reader()?.read_to_end(&mut complete)?;
                assert_eq!(complete, bytes);
            }
        }
        Ok(())
    }

    #[test]
    fn retention_rejects_overflow_and_inconsistent_counts() {
        for r in [
            Retention {
                total_bytes: 9,
                head_bytes: 2,
                tail_bytes: 2,
                omitted_bytes: 6,
            },
            Retention {
                total_bytes: u64::MAX,
                head_bytes: u64::MAX,
                tail_bytes: 1,
                omitted_bytes: 0,
            },
            Retention {
                total_bytes: 4,
                head_bytes: 2,
                tail_bytes: 2,
                omitted_bytes: 0,
            },
        ] {
            assert!(r.validate().is_err());
        }
    }
}
