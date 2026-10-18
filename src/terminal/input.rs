//! Crossterm owns protocol detection, parsing and platform compatibility.
//!
//! Read at most 256 available events before application work. This bounds
//! each drain and lets the application skip intermediate navigation loads.

use std::{
    io::{self, Write},
    time::Duration,
};

use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture,
        KeyboardEnhancementFlags, PopKeyboardEnhancementFlags,
        PushKeyboardEnhancementFlags,
    },
    execute,
};
use lume::app::InputBatch;

pub(super) fn read_batch() -> io::Result<InputBatch> {
    let mut events = vec![event::read()?];
    while events.len() < 256 && event::poll(Duration::ZERO)? {
        events.push(event::read()?);
    }
    Ok(InputBatch::new(events))
}

pub(super) fn enable_keyboard() -> io::Result<()> {
    execute!(
        io::stdout(),
        PushKeyboardEnhancementFlags(
            KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
                | KeyboardEnhancementFlags::REPORT_ALTERNATE_KEYS
                | KeyboardEnhancementFlags::REPORT_EVENT_TYPES
                | KeyboardEnhancementFlags::REPORT_ALL_KEYS_AS_ESCAPE_CODES
        )
    )
}

pub(super) fn disable_keyboard() -> io::Result<()> {
    execute!(io::stdout(), PopKeyboardEnhancementFlags)
}

/// Standard capture commands are balanced across suspension and shutdown.
#[derive(Default)]
pub(super) struct MouseCapture {
    enabled: bool,
    requested: Option<bool>,
}

impl MouseCapture {
    pub(super) fn sync(&mut self, requested: bool) -> io::Result<()> {
        self.sync_to(&mut io::stdout().lock(), requested)
    }

    fn sync_to(
        &mut self,
        output: &mut impl Write,
        requested: bool,
    ) -> io::Result<()> {
        if self.requested == Some(requested) {
            return Ok(());
        }
        self.requested = Some(requested);
        if requested {
            // Track partial writes too, so cleanup retries a matching disable.
            self.enabled = true;
            return execute!(output, EnableMouseCapture);
        }
        self.suspend_to(output)?;
        self.requested = Some(false);
        Ok(())
    }

    pub(super) fn suspend(&mut self) -> io::Result<()> {
        self.suspend_to(&mut io::stdout().lock())
    }

    fn suspend_to(&mut self, output: &mut impl Write) -> io::Result<()> {
        self.requested = None;
        if self.enabled {
            execute!(output, DisableMouseCapture)?;
            self.enabled = false;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;

impl Drop for MouseCapture {
    fn drop(&mut self) {
        let _ = self.suspend();
    }
}
