//! Clipboard writes use fixed independent wire fixtures and explicit failures.

use std::io::{self, Write};

use anyhow::Result;
use lume::clipboard;

#[test]
fn clipboard_requests_encode_exact_utf8_without_exposing_control_bytes()
-> Result<()> {
    for (text, expected) in [
        ("hello", "\x1b]52;c;aGVsbG8=\x1b\\"),
        ("café\n", "\x1b]52;c;Y2Fmw6kK\x1b\\"),
        ("\x1b]52;c;evil\x07", "\x1b]52;c;G101MjtjO2V2aWwH\x1b\\"),
    ] {
        let mut bytes = Vec::new();
        clipboard::copy(&mut bytes, text)?;
        assert_eq!(bytes, expected.as_bytes());
    }
    Ok(())
}

struct Broken;

impl Write for Broken {
    fn write(&mut self, _bytes: &[u8]) -> io::Result<usize> {
        Err(io::Error::new(io::ErrorKind::BrokenPipe, "fixture closed"))
    }

    fn flush(&mut self) -> io::Result<()> {
        Err(io::Error::other("fixture flush failed"))
    }
}

#[test]
fn clipboard_rejects_invalid_sizes_before_writing_and_reports_io_failures() {
    let mut bytes = Vec::new();
    assert!(clipboard::copy(&mut bytes, "").is_err());
    assert!(clipboard::copy(&mut bytes, &"x".repeat(65_537)).is_err());
    assert!(bytes.is_empty());
    assert!(clipboard::copy(&mut Broken, "valid").is_err());
}

struct FlushFailure;

impl Write for FlushFailure {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Err(io::Error::other("fixture flush failed"))
    }
}

#[test]
fn clipboard_accepts_the_exact_limit_and_reports_flush_failure() -> Result<()> {
    let mut bytes = Vec::new();
    clipboard::copy(&mut bytes, &"x".repeat(65_536))?;
    assert_eq!(bytes.len(), 87_393);
    assert!(bytes.ends_with(b"eA==\x1b\\"));
    assert!(clipboard::copy(&mut FlushFailure, "valid").is_err());
    Ok(())
}
