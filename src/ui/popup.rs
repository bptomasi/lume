//! Modal footprints are shared by drawing and native image occlusion.

use ratatui::layout::Rect;

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
