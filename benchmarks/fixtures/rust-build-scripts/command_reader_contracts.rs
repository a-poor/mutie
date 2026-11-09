//! Separate-copy tests for public grep-cli command-reader behavior (Unix).
#![cfg(unix)]
use std::{io::Read, process::Command};
use grep_cli::CommandReader;

fn reader(script: &str) -> CommandReader {
    CommandReader::new(Command::new("/bin/sh").args(["-c", script])).unwrap()
}

#[test]
fn failure_diagnostics_frame_trimmed_stderr() {
    let mut command = reader("printf '  useful diagnostic  \\n' >&2; exit 1");
    let error = command.read_to_end(&mut Vec::new()).unwrap_err();
    let border = "-".repeat(79);
    assert_eq!(error.to_string(), format!("\n{border}\nuseful diagnostic\n{border}"));
}

#[test]
fn closing_after_a_partial_read_is_idempotent_and_finishes_the_stream() {
    let mut command = reader("printf payload; exit 1");
    let mut byte = [0];
    command.read_exact(&mut byte).unwrap();
    assert_eq!(byte, [b'p']);
    command.close().unwrap();
    command.close().unwrap();
    assert_eq!(command.read(&mut byte).unwrap(), 0);
}

#[test]
fn a_nonzero_exit_with_empty_stderr_still_fails_at_eof() {
    let mut command = reader("exit 1");
    let error = command.read_to_end(&mut Vec::new()).unwrap_err();
    assert_eq!(error.to_string(), "<stderr is empty>");
}

#[test]
fn the_last_byte_precedes_eof_and_eof_never_adds_bytes() {
    let mut command = reader("printf x; printf failure >&2; exit 1");
    let mut byte = [0];
    assert_eq!(command.read(&mut byte).unwrap(), 1);
    assert_eq!(byte, [b'x']);
    assert!(command.read(&mut byte).is_err());
    let mut empty = reader("exit 0");
    assert_eq!(empty.read(&mut byte).unwrap(), 0);
    assert_eq!(empty.read(&mut byte).unwrap(), 0);
}
