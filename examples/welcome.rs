//! Export the native welcome screen for visual review, without a database.

mod support;

use std::path::PathBuf;

use anyhow::Result;
use ratatui::{Terminal, backend::TestBackend};

fn main() -> Result<()> {
    let mut arguments = std::env::args().skip(1);
    let output = arguments
        .next()
        .map_or_else(|| PathBuf::from("/tmp/lume-welcome.json"), PathBuf::from);
    let width = arguments
        .next()
        .map(|value| value.parse())
        .transpose()?
        .unwrap_or(120);
    let height = arguments
        .next()
        .map(|value| value.parse())
        .transpose()?
        .unwrap_or(40);
    let theme = arguments.next().unwrap_or_else(|| "lume".into());
    let mut terminal = Terminal::new(TestBackend::new(width, height))?;
    terminal.draw(|frame| lume::ui::draw_welcome(frame, &theme))?;
    support::export(terminal.backend().buffer(), &output)
}
