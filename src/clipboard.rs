//! Write-only terminal clipboard requests, emitted only for explicit yanks.

use std::io::Write;

use anyhow::{Result, ensure};
use base64::{Engine, engine::general_purpose::STANDARD};

/// Send at most 64 KiB using OSC 52 without reading the clipboard.
///
/// A successful write does not acknowledge terminal clipboard permission.
///
/// # Errors
/// Returns empty/oversized payload or writer/flush failures without truncation.
pub fn copy(writer: &mut impl Write, text: &str) -> Result<()> {
    ensure!(!text.is_empty(), "Select text before yanking");
    ensure!(text.len() <= 65_536, "Yank selection exceeds 64 KiB");
    let encoded = STANDARD.encode(text);
    write!(writer, "\x1b]52;c;{encoded}\x1b\\")?;
    writer.flush()?;
    Ok(())
}
