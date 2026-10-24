//! Native images outlive text-cell diffs; clear only on inspector boundaries.
use ratatui::{Frame, Terminal, backend::Backend};
use std::io;

#[derive(Default)]
pub(super) struct Presentation {
    native_inspector: bool,
}

impl Presentation {
    pub(super) fn draw<B: Backend>(
        &mut self,
        terminal: &mut Terminal<B>,
        native_inspector: bool,
        render: impl FnOnce(&mut Frame<'_>),
    ) -> io::Result<()> {
        if native_inspector != self.native_inspector {
            terminal.clear()?;
        }
        terminal.draw(render)?;
        // A failed clear or draw must retry the transition on the next frame.
        self.native_inspector = native_inspector;
        Ok(())
    }
}

#[cfg(test)]
mod tests;
