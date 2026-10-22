//! Layout history has separate authority from personal article edit history.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::{Dock, Pane, PaneLayout, PanelLayout, Workspace};

/// At most 32 named profiles and 32 undo/redo entries, independent of articles.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LayoutHistory {
    /// User-named layout profiles.
    pub profiles: BTreeMap<String, PanelLayout>,
    /// Most recently committed layout.
    pub current: PanelLayout,
    /// Earlier layouts, oldest first.
    pub past: Vec<PanelLayout>,
    /// Undone layouts, next restoration last.
    pub future: Vec<PanelLayout>,
}

impl LayoutHistory {
    /// Seed useful profiles without overwriting any persisted user profile.
    pub fn new(workspace: &Workspace) -> Self {
        let reading = Workspace {
            show_articles: false,
            reading_width: workspace.reading_width,
            fullscreen_width: workspace.fullscreen_width,
            ..Workspace::default()
        };
        let mut study = reading.clone();
        study.show_annotations = true;
        study.layout = PaneLayout::columns(&study.panel_order).dock(
            Pane::Annotations,
            Pane::Reader,
            Dock::Below,
        );
        let mut browse = reading.clone();
        browse.show_articles = true;
        Self {
            profiles: BTreeMap::from([
                ("Reading".into(), PanelLayout::capture(&reading)),
                ("Study".into(), PanelLayout::capture(&study)),
                ("Browse".into(), PanelLayout::capture(&browse)),
            ]),
            current: PanelLayout::capture(workspace),
            past: Vec::new(),
            future: Vec::new(),
        }
    }

    /// Add a committed change; a new change invalidates the redo branch.
    pub fn observe(&mut self, layout: PanelLayout) {
        if self.current == layout {
            return;
        }
        self.past.push(std::mem::replace(&mut self.current, layout));
        if self.past.len() > 32 {
            self.past.remove(0);
        }
        self.future.clear();
    }

    /// Restore the previous committed layout, retaining the redo branch.
    pub fn undo(&mut self) -> Option<PanelLayout> {
        let previous = self.past.pop()?;
        self.future
            .push(std::mem::replace(&mut self.current, previous));
        Some(self.current.clone())
    }

    /// Reapply the most recently undone layout.
    pub fn redo(&mut self) -> Option<PanelLayout> {
        let next = self.future.pop()?;
        self.past.push(std::mem::replace(&mut self.current, next));
        Some(self.current.clone())
    }
}
