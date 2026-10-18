//! The real Crossterm commands qualify balanced capture and partial I/O failures.

use std::io::{self, Write};

use anyhow::Result;

use super::MouseCapture;

#[test]
fn capture_is_not_duplicated_and_suspension_can_be_resumed() -> Result<()> {
    let mut capture = MouseCapture::default();
    let mut wire = Vec::new();
    capture.sync_to(&mut wire, true)?;
    let enabled = wire.clone();

    capture.sync_to(&mut wire, true)?;
    assert_eq!(wire, enabled);
    capture.suspend_to(&mut wire)?;
    assert!(wire.ends_with(b"\x1b[?1000l"));
    capture.sync_to(&mut wire, true)?;
    capture.suspend_to(&mut wire)?;

    // These are deliberately exact terminal wire fixtures, not UI text.
    let sgr_enable = b"\x1b[?1006h";
    assert_eq!(
        wire.windows(sgr_enable.len())
            .filter(|part| *part == sgr_enable)
            .count(),
        2
    );
    assert!(wire.ends_with(b"\x1b[?1000l"));
    assert!(!capture.enabled);
    Ok(())
}

#[test]
fn partial_enable_failure_waits_for_a_new_request_and_still_disables_capture()
-> Result<()> {
    let mut capture = MouseCapture::default();
    let mut broken = PartialWrite { remaining: 2 };

    assert!(capture.sync_to(&mut broken, true).is_err());
    let mut cleanup = Vec::new();
    capture.sync_to(&mut cleanup, true)?;
    assert!(cleanup.is_empty());
    capture.suspend_to(&mut cleanup)?;

    assert!(cleanup.ends_with(b"\x1b[?1000l"));
    assert!(!capture.enabled);
    Ok(())
}

#[test]
fn failed_disable_retains_cleanup_authority_for_another_attempt() -> Result<()>
{
    let mut capture = MouseCapture::default();
    let mut wire = Vec::new();
    capture.sync_to(&mut wire, true)?;

    assert!(
        capture
            .suspend_to(&mut PartialWrite { remaining: 0 })
            .is_err()
    );
    capture.suspend_to(&mut wire)?;

    assert!(wire.ends_with(b"\x1b[?1000l"));
    assert!(!capture.enabled);
    Ok(())
}

struct PartialWrite {
    remaining: usize,
}

impl Write for PartialWrite {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        if self.remaining == 0 {
            return Err(io::Error::other("Controlled terminal I/O failure"));
        }
        let count = self.remaining.min(buffer.len());
        self.remaining -= count;
        Ok(count)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
