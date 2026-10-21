//! Sprite contracts and terminal-size behavior, independent of library state.

use anyhow::Result;
use ratatui::{Terminal, backend::TestBackend};

use super::*;

#[test]
fn authored_pixels_have_fixed_rows_and_a_known_palette() {
    let rows = LIGHTHOUSE.split(|byte| *byte == b'\n').collect::<Vec<_>>();
    assert_eq!(rows.len(), usize::from(HEIGHT) + 1);
    assert!(
        rows[..usize::from(HEIGHT)]
            .iter()
            .all(|row| row.len() == usize::from(WIDTH))
    );
    assert!(rows[usize::from(HEIGHT)].is_empty());
    assert!(
        LIGHTHOUSE
            .iter()
            .all(|byte| b".abcdewsmlo\n".contains(byte))
    );
}

#[test]
fn full_scene_is_centered_with_a_bright_lantern_and_dim_beam() -> Result<()> {
    let mut terminal = Terminal::new(TestBackend::new(120, 40))?;
    terminal.draw(|frame| draw(frame, "lume"))?;
    let buffer = terminal.backend().buffer();
    assert_eq!(art_area(buffer.area), Rect::new(20, 5, 80, 23));
    assert_eq!(buffer[(58, 10)].fg, theme::TEXT);
    assert_eq!(buffer[(80, 11)].fg, Color::Rgb(51, 44, 35));
    assert_eq!(row_text(buffer, 29).trim(), "l u m e");
    assert_eq!(
        row_text(buffer, 30).trim(),
        "a quiet place for curious minds"
    );
    assert_eq!(row_text(buffer, 32).trim(), "Press any key to begin");
    assert_eq!(buffer[(0, 0)].bg, theme::PANEL);
    Ok(())
}

#[test]
fn resizing_and_themes_keep_the_entry_hint_visible() -> Result<()> {
    for name in ["lume", "nord", "gruvbox"] {
        for (width, height) in [(20, 6), (40, 18), (80, 28), (240, 80)] {
            let mut terminal = Terminal::new(TestBackend::new(width, height))?;
            terminal.draw(|frame| draw(frame, name))?;
            let buffer = terminal.backend().buffer();
            let text = buffer
                .content
                .iter()
                .map(|cell| cell.symbol())
                .collect::<String>();
            assert!(text.contains("Press any key"));
            assert!(text.contains("l u m e"));
            let [red, green, blue] = theme::image_background(name);
            let background = Color::Rgb(red, green, blue);
            assert_eq!(buffer[(0, 0)].bg, background);
            let art = art_area(buffer.area);
            if art.height > 0 {
                assert_eq!(buffer[(art.x, art.y)].bg, background);
            }
        }
    }
    Ok(())
}

#[test]
fn even_minimal_terminals_draw_without_panicking() -> Result<()> {
    for (width, height) in [(1, 1), (8, 2), (20, 3)] {
        let mut terminal = Terminal::new(TestBackend::new(width, height))?;
        terminal.draw(|frame| draw(frame, "lume"))?;
        assert_eq!(terminal.backend().buffer().area.width, width);
    }
    Ok(())
}

fn row_text(buffer: &Buffer, y: u16) -> String {
    (0..buffer.area.width)
        .map(|x| buffer[(x, y)].symbol())
        .collect()
}
