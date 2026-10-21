//! Whole welcome frames with stable captions, palette and artwork assertions.

use anyhow::{Result, ensure};
use lume::ui;
use ratatui::{Terminal, backend::TestBackend, style::Color};

pub fn run(iterations: usize, stress: bool) -> Result<()> {
    let (width, height) = if stress { (300, 100) } else { (120, 40) };
    let mut terminal = Terminal::new(TestBackend::new(width, height))?;
    let label = format!("native welcome frame ({width}x{height})");
    super::measure(&label, iterations * 25, || {
        terminal.draw(|frame| ui::draw_welcome(frame, "lume"))?;
        let buffer = terminal.backend().buffer();
        let text = buffer
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        ensure!(text.contains("l u m e"), "Wordmark missing");
        ensure!(
            text.contains("Press any key to begin"),
            "Entry hint missing"
        );
        ensure!(
            buffer.content.iter().any(|cell| cell.symbol() == "▀"),
            "Native pixel art missing"
        );
        ensure!(
            buffer[(0, 0)].bg == Color::Rgb(23, 23, 26),
            "Welcome background differs from the panels"
        );
        Ok(())
    })
}
