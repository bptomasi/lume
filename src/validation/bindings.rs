//! Bounded normal-mode bindings use the shared ex-command vocabulary.

use anyhow::{Result, ensure};

fn valid_key_name(name: &str) -> bool {
    let base = name.strip_prefix("ctrl-").unwrap_or(name);
    let base = base.strip_prefix("alt-").unwrap_or(base);
    if matches!(
        base,
        "space"
            | "enter"
            | "tab"
            | "backtab"
            | "esc"
            | "up"
            | "down"
            | "left"
            | "right"
    ) {
        return true;
    }
    if let Some(number) = base
        .strip_prefix('f')
        .and_then(|number| number.parse::<u8>().ok())
    {
        return (1..=12).contains(&number);
    }
    base.chars().count() == 1 && !base.chars().any(char::is_control)
}

pub(crate) fn validate_binding(key: &str, command: &str) -> Result<()> {
    ensure!(
        valid_key_name(key),
        "Use a character, arrow, f1–f12 or named key with optional ctrl-/alt- prefixes"
    );
    ensure!(
        key != "ctrl-c" && !key.is_empty() && key.len() <= 32,
        "Ctrl-c is reserved for exit; key names use at most 32 bytes"
    );
    ensure!(
        command.len() <= 4096 && !command.chars().any(char::is_control),
        "Invalid bound command"
    );
    let verb = command.split_whitespace().next().unwrap_or("");
    ensure!(
        crate::command::resolve(verb).is_some(),
        "Bind an available : command"
    );
    Ok(())
}
