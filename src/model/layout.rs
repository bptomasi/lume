//! Bounded pane topology shared by commands, pointer docking and persistence.

use serde::{Deserialize, Serialize};

use super::{Pane, Workspace};

/// Direction in which a pane is docked relative to an existing pane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Dock {
    /// Before the target in a row.
    Left,
    /// After the target in a row.
    Right,
    /// Above the target in a column.
    Above,
    /// Below the target in a column.
    Below,
}

impl Dock {
    /// Parse the public docking command vocabulary.
    pub fn named(value: &str) -> Option<Self> {
        match value {
            "left" => Some(Self::Left),
            "right" => Some(Self::Right),
            "above" | "up" => Some(Self::Above),
            "below" | "down" => Some(Self::Below),
            _ => None,
        }
    }

    /// Whether the split allocates terminal columns instead of rows.
    pub fn horizontal(self) -> bool {
        matches!(self, Self::Left | Self::Right)
    }
}

/// A binary division with exactly one leaf for each of the four pane identities.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PaneLayout {
    /// An independently collapsible pane.
    Pane(Pane),
    /// Allocate a percentage to the first child; the other gets the remainder.
    Split {
        /// Divide columns when true, otherwise divide rows.
        horizontal: bool,
        /// First child's share, constrained to 15 through 85 percent.
        percent: u16,
        /// Leading child.
        first: Box<Self>,
        /// Trailing child.
        second: Box<Self>,
    },
}

impl PaneLayout {
    /// Construct equal-width columns while retaining collapsed identities.
    pub fn columns(order: &[Pane]) -> Self {
        let Some((pane, rest)) = order.split_first() else {
            return Self::Pane(Pane::Reader);
        };
        if rest.is_empty() {
            return Self::Pane(*pane);
        }
        Self::Split {
            horizontal: true,
            percent: (100 / order.len()) as u16,
            first: Box::new(Self::Pane(*pane)),
            second: Box::new(Self::columns(rest)),
        }
    }

    /// Validate bounded topology before geometry or persistence traverses it.
    pub fn valid(&self) -> bool {
        let mut leaves = Vec::new();
        self.collect(&mut leaves, 0)
            && leaves.len() == Pane::ALL.len()
            && Pane::ALL.iter().all(|pane| {
                leaves.iter().filter(|leaf| *leaf == pane).count() == 1
            })
    }

    fn collect(&self, leaves: &mut Vec<Pane>, depth: usize) -> bool {
        if depth > 3 {
            return false;
        }
        match self {
            Self::Pane(pane) => leaves.push(*pane),
            Self::Split {
                horizontal: _,
                percent,
                first,
                second,
            } => {
                return (15..=85).contains(percent)
                    && first.collect(leaves, depth + 1)
                    && second.collect(leaves, depth + 1);
            }
        }
        true
    }

    /// Move an existing identity beside a target without duplicating leaves.
    pub fn dock(&self, pane: Pane, target: Pane, edge: Dock) -> Option<Self> {
        if pane == target || !self.valid() {
            return None;
        }
        let remainder = self.without(pane)?;
        let result = remainder.insert(pane, target, edge);
        result.valid().then_some(result)
    }

    fn without(&self, removed: Pane) -> Option<Self> {
        match self {
            Self::Pane(pane) => (*pane != removed).then(|| self.clone()),
            Self::Split {
                horizontal,
                percent,
                first,
                second,
            } => match (first.without(removed), second.without(removed)) {
                (Some(first), Some(second)) => Some(Self::Split {
                    horizontal: *horizontal,
                    percent: *percent,
                    first: Box::new(first),
                    second: Box::new(second),
                }),
                (first, second) => first.or(second),
            },
        }
    }

    fn insert(&self, pane: Pane, target: Pane, edge: Dock) -> Self {
        match self {
            Self::Pane(id) if *id == target => {
                let before = matches!(edge, Dock::Left | Dock::Above);
                let (first, second) = if before {
                    (pane, target)
                } else {
                    (target, pane)
                };
                Self::Split {
                    horizontal: edge.horizontal(),
                    percent: 50,
                    first: Box::new(Self::Pane(first)),
                    second: Box::new(Self::Pane(second)),
                }
            }
            Self::Split {
                horizontal,
                percent,
                first,
                second,
            } => Self::Split {
                horizontal: *horizontal,
                percent: *percent,
                first: Box::new(first.insert(pane, target, edge)),
                second: Box::new(second.insert(pane, target, edge)),
            },
            _ => self.clone(),
        }
    }

    /// Resize a validated split; return whether its saved share changed.
    pub fn resize_path(&mut self, path: &[bool], percent: u16) -> bool {
        let Some(share) = self.split_share(path) else {
            return false;
        };
        let next = percent.clamp(15, 85);
        if *share == next {
            return false;
        }
        *share = next;
        true
    }

    fn split_share(&mut self, path: &[bool]) -> Option<&mut u16> {
        let Self::Split {
            first,
            second,
            percent,
            ..
        } = self
        else {
            return None;
        };
        let Some((trailing, rest)) = path.split_first() else {
            return Some(percent);
        };
        let child = if *trailing { second } else { first };
        child.split_share(rest)
    }
}

/// Layout-only state: switching a workspace never changes article or keybindings.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PanelLayout {
    /// Custom topology; None retains legacy fixed-width columns.
    pub tree: Option<PaneLayout>,
    /// Stable order of pane controls and collapsed identities.
    pub order: [Pane; 4],
    /// Library, articles, reader and annotations expansion flags.
    pub visible: [bool; 4],
    /// Library, articles and annotations preferred column counts.
    pub widths: [u16; 3],
    /// Prose width preference.
    pub reading_width: u16,
    /// Fullscreen prose width, defaulting for profiles saved before this field.
    #[serde(default = "default_fullscreen_width")]
    pub fullscreen_width: u16,
    /// Compact headline preference.
    pub compact: bool,
    /// Reader-only display.
    pub zen: bool,
    /// Fullscreen article display.
    pub fullscreen: bool,
}

impl PanelLayout {
    /// Capture layout without navigation, content provenance or personal data.
    pub fn capture(workspace: &Workspace) -> Self {
        Self {
            tree: workspace.layout.clone(),
            order: workspace.panel_order,
            visible: Pane::ALL.map(|pane| workspace.pane_visible(pane)),
            widths: [
                workspace.library_width,
                workspace.article_width,
                workspace.annotation_width,
            ],
            reading_width: workspace.reading_width,
            fullscreen_width: workspace.fullscreen_width,
            compact: workspace.compact,
            zen: workspace.zen,
            fullscreen: workspace.fullscreen,
        }
    }

    /// Restore only captured layout preferences.
    pub fn apply(&self, workspace: &mut Workspace) {
        workspace.layout = self.tree.clone();
        workspace.panel_order = self.order;
        for (pane, visible) in Pane::ALL.into_iter().zip(self.visible) {
            workspace.set_pane_visible(pane, visible);
        }
        [
            workspace.library_width,
            workspace.article_width,
            workspace.annotation_width,
        ] = self.widths;
        workspace.reading_width = self.reading_width;
        workspace.fullscreen_width = self.fullscreen_width;
        workspace.compact = self.compact;
        workspace.zen = self.zen;
        workspace.fullscreen = self.fullscreen;
    }
}

fn default_fullscreen_width() -> u16 {
    super::Workspace::default().fullscreen_width
}
