//! Popup geometry and rounded frames share drawing and image-occlusion bounds.

use ratatui::{Frame, layout::Rect, widgets::Clear};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

use crate::app::{InputKind, Mode};

#[derive(Clone, Copy)]
pub(super) enum Popup {
    Help,
    Command,
    Picker,
    Smart,
    Document,
    Settings,
    Leader,
    Editor(bool),
}

impl Popup {
    pub(super) fn from_mode(mode: &Mode) -> Option<Self> {
        match mode {
            Mode::Input(input)
                if matches!(
                    input.kind,
                    InputKind::Search | InputKind::LibrarySearch
                ) =>
            {
                None
            }
            Mode::Help => Some(Self::Help),
            Mode::Input(input) if matches!(input.kind, InputKind::Command) => {
                Some(Self::Command)
            }
            Mode::Input(input) => Some(Self::Editor(multiline(&input.kind))),
            Mode::Picker(_) => Some(Self::Picker),
            Mode::Smart(_) => Some(Self::Smart),
            Mode::Document(_) => Some(Self::Document),
            Mode::Settings(_) => Some(Self::Settings),
            Mode::Leader => Some(Self::Leader),
            Mode::Normal | Mode::Image(_) => None,
        }
    }

    pub(super) fn area(self, screen: Rect) -> Rect {
        let (width, height) = match self {
            Self::Help => (88, 40),
            Self::Command => (100, 25),
            Self::Picker => (110, 28),
            Self::Smart => (82, 22),
            Self::Document => (u16::MAX, u16::MAX),
            Self::Settings => (96, 15),
            Self::Leader => (72, 15),
            Self::Editor(multiline) => (84, if multiline { 24 } else { 9 }),
        };
        super::overlays::centered(screen, width, height)
    }
}

pub(super) fn multiline(kind: &InputKind) -> bool {
    matches!(
        kind,
        InputKind::Comment { .. }
            | InputKind::Highlight { .. }
            | InputKind::StudyCard { .. }
    )
}

pub(super) fn draw_frame(
    frame: &mut Frame<'_>,
    title: &str,
    area: Rect,
) -> Rect {
    let title = frame_title(title, area.width);
    let block = super::theme::panel(title, true);
    let inner = block.inner(area);
    frame.render_widget(Clear, area);
    frame.render_widget(block, area);
    inner
}

fn frame_title(title: &str, width: u16) -> String {
    if width < 10 {
        return String::new();
    }
    let mut remaining = usize::from(width - 8);
    let safe = crate::content::label(title.trim(), 256);
    let mut text = String::new();
    for grapheme in safe.graphemes(true) {
        let width = grapheme.width();
        if width > remaining {
            break;
        }
        remaining -= width;
        text.push_str(grapheme);
    }
    // Keep a connected horizontal segment beside both corner arcs. Long
    // titles must not consume the outline, including at narrow terminal sizes.
    format!("─ {} ", text.trim_end())
}

#[cfg(test)]
mod tests {
    use ratatui::{Terminal, backend::TestBackend};

    use super::*;

    #[test]
    fn every_popup_preserves_four_arcs_and_connected_title_margins()
    -> anyhow::Result<()> {
        let kinds = [
            Popup::Help,
            Popup::Command,
            Popup::Picker,
            Popup::Smart,
            Popup::Document,
            Popup::Settings,
            Popup::Leader,
            Popup::Editor(false),
            Popup::Editor(true),
        ];
        for (width, height) in [(24, 10), (80, 28), (160, 50)] {
            for kind in kinds {
                let screen = Rect::new(0, 0, width, height);
                let area = kind.area(screen);
                let mut terminal =
                    Terminal::new(TestBackend::new(width, height))?;
                terminal.draw(|frame| {
                    draw_frame(frame, &"Wide 界 title 🦀 ".repeat(20), area);
                })?;
                let buffer = terminal.backend().buffer();
                for (x, y, glyph) in [
                    (area.x, area.y, "╭"),
                    (area.right() - 1, area.y, "╮"),
                    (area.x, area.bottom() - 1, "╰"),
                    (area.right() - 1, area.bottom() - 1, "╯"),
                    (area.x + 1, area.y, "─"),
                    (area.right() - 2, area.y, "─"),
                ] {
                    assert_eq!(buffer[(x, y)].symbol(), glyph);
                }
            }
        }
        Ok(())
    }
}
