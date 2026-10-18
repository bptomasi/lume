//! External editor adapter with private temporary files and no shell evaluation.

use std::{
    io::{Read, Write},
    process::Command,
};

use anyhow::{Context, Result, ensure};

/// Run a configured editor on a private draft and return its bounded UTF-8 text.
///
/// # Errors
/// Rejects invalid commands, unsuccessful exits, oversized output or I/O errors.
/// The caller owns terminal suspension and retains its draft on any failure.
pub fn edit(draft: &str, command: &str) -> Result<String> {
    let arguments = shlex::split(command).context("Invalid editor quoting")?;
    let program = arguments.first().context("Set VISUAL or EDITOR")?;
    let mut file = tempfile::Builder::new()
        .prefix("lume-note-")
        .suffix(".md")
        .tempfile()?;
    file.write_all(draft.as_bytes())?;
    file.flush()?;
    let status = Command::new(program)
        .args(&arguments[1..])
        .arg(file.path())
        .status()
        .context("Could not start external editor")?;
    ensure!(status.success(), "Editor exited unsuccessfully: {status}");
    let mut text = String::new();
    std::fs::File::open(file.path())?
        .take(16_385)
        .read_to_string(&mut text)?;
    ensure!(text.len() <= 16_384, "Edited note exceeds 16 KiB");
    ensure!(
        text.chars()
            .all(|c| !c.is_control() || matches!(c, '\n' | '\t')),
        "Edited note contains terminal control characters"
    );
    Ok(text)
}
