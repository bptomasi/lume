//! Durable filtering, annotation and layout contracts.

use serde::{Deserialize, Serialize};

use super::{Article, Comment, ContentSource, Pane};

/// A destination queried across the entire library.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum View {
    /// Unread articles.
    Inbox,
    /// All cached articles.
    All,
    /// Cached, diverse local recommendations.
    Recommended,
    /// Read articles, derived from the durable read flag.
    Read,
    /// Recoverable deleted articles, excluded from other destinations.
    Trash,
    /// Personal favorites.
    Starred,
    /// Notebook of annotated articles.
    Annotated,
    /// Explicit next-reading queue.
    Queue,
    /// Personal list.
    Collection(i64),
    /// Subscription.
    Feed(i64),
    /// Saved smart list.
    Smart(i64),
    /// Articles with a saved reading position.
    Continue,
    /// Entries collected after a captured previous-session boundary.
    SinceLastVisit(i64),
    /// One representative per canonical URL; occurrences remain accessible.
    Grouped,
    /// Collapsed navigation section; not an article destination.
    Group(String),
}

impl View {
    /// Compare persistent destination identity, ignoring a session's time boundary.
    pub fn same_destination(&self, other: &Self) -> bool {
        matches!(
            (self, other),
            (Self::SinceLastVisit(_), Self::SinceLastVisit(_))
        ) || self == other
    }
}

/// Smart-list criteria with AND semantics.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SmartFilter {
    /// Full-text terms, including bodies and annotations.
    #[serde(default)]
    pub text: String,
    /// Restrict to unread articles.
    #[serde(default)]
    pub unread: bool,
    /// Restrict to favorites.
    #[serde(default)]
    pub starred: bool,
    /// Restrict to a subscription, including an archived subscription.
    #[serde(default)]
    pub feed: Option<i64>,
    /// Match any subscription in this bounded set; empty means unrestricted.
    #[serde(default)]
    pub feeds: Vec<i64>,
    /// Restrict to articles published within this many days.
    #[serde(default)]
    pub days: Option<u16>,
}

/// A named, durable smart list.
#[derive(Clone, Debug)]
pub struct SmartList {
    /// Durable identity.
    pub id: i64,
    /// Display name.
    pub name: String,
    /// Query criteria.
    pub filter: SmartFilter,
}

/// Conditional saved-search rename that retains its identity and criteria.
pub struct SmartListRename {
    /// Existing saved-search identity.
    pub id: i64,
    /// Exact name observed while planning the rename.
    pub expected_name: String,
    /// Validated replacement name.
    pub name: String,
}

/// Bounded query without a historical cutoff.
#[derive(Clone, Debug)]
pub struct ArticleQuery {
    /// Destination.
    pub view: View,
    /// Additional full-text terms.
    pub text: String,
    /// Matching rows to skip.
    pub offset: usize,
    /// Returned rows, clamped to 500.
    pub limit: usize,
    /// Open inbox article retained during personal edits.
    pub retained: Option<i64>,
}

/// A metadata page and its complete match count.
pub struct ArticlePage {
    /// Bounded page, newest first.
    pub articles: Vec<Article>,
    /// Matches across the entire database.
    pub total: usize,
}

/// Annotation with its original source for notebook/export views.
pub struct NotebookEntry {
    /// Source metadata.
    pub article: Article,
    /// Note and optional captured quote.
    pub comment: Comment,
}

/// Restorable layout, independent of terminal fonts and pixels.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct Workspace {
    /// Preferred article body; Full falls back to Feed until a cache exists.
    pub content_source: ContentSource,
    /// Fill the terminal with the article, preserving the underlying layout.
    pub fullscreen: bool,
    /// Reader-only mode.
    pub zen: bool,
    /// Show library sidebar.
    pub show_library: bool,
    /// Show headlines beside the reader when space permits.
    pub show_articles: bool,
    /// Show the reader independently of other panes.
    pub show_reader: bool,
    /// Show the article's position-ordered annotations.
    pub show_annotations: bool,
    /// Stable left-to-right order, retaining collapsed pane positions.
    pub panel_order: [Pane; 4],
    /// Optional binary topology; old workspaces retain fixed-width columns.
    pub layout: Option<super::PaneLayout>,
    /// Independent Notebook arrangement, preserving the reading workspace.
    pub notebook_layout: Option<super::PanelLayout>,
    /// Request standard terminal mouse reporting; keyboard remains available.
    pub mouse: bool,
    /// Portable image loading and protocol preferences.
    pub images: ImageSettings,
    /// Expand prose word gaps visually, preserving copied text and quotations.
    pub justify: bool,
    /// Maximum prose width in terminal columns.
    pub reading_width: u16,
    /// Centered prose width in fullscreen, independent of the panel layout.
    pub fullscreen_width: u16,
    /// Sidebar width in terminal columns.
    pub library_width: u16,
    /// Headlines width in terminal columns.
    pub article_width: u16,
    /// Annotation pane width in terminal columns.
    pub annotation_width: u16,
    /// Compact headlines.
    pub compact: bool,
    /// Last selected article, independent of the previous page.
    pub article: Option<i64>,
    /// Last metadata page offset within the destination.
    pub page_offset: usize,
    /// Last destination.
    pub view: View,
    /// Collapsed sidebar sections, using stable English labels.
    pub collapsed_groups: Vec<String>,
    /// Pinned destinations in personal order.
    pub pins: Vec<View>,
    /// Personal destination ordering; absent entries retain catalog order.
    pub navigation_order: Vec<View>,
    /// Previous completed terminal session, Unix seconds.
    pub last_visit: i64,
    /// Selected dark palette.
    pub theme: String,
    /// Normal-mode key names mapped to discoverable ex commands.
    pub keybindings: std::collections::BTreeMap<String, String>,
}

impl Default for Workspace {
    fn default() -> Self {
        Self {
            content_source: ContentSource::Full,
            fullscreen: false,
            zen: false,
            show_library: true,
            show_articles: true,
            show_reader: true,
            show_annotations: false,
            panel_order: Pane::ALL,
            layout: None,
            notebook_layout: None,
            mouse: true,
            images: ImageSettings::default(),
            justify: false,
            reading_width: 82,
            fullscreen_width: 80,
            library_width: 23,
            article_width: 36,
            annotation_width: 32,
            compact: false,
            article: None,
            page_offset: 0,
            view: View::Inbox,
            collapsed_groups: Vec::new(),
            pins: Vec::new(),
            navigation_order: Vec::new(),
            last_visit: 0,
            theme: "lume".into(),
            keybindings: std::collections::BTreeMap::new(),
        }
    }
}

impl Workspace {
    /// Whether a pane is expanded, independently of responsive suppression.
    pub fn pane_visible(&self, pane: Pane) -> bool {
        match pane {
            Pane::Library => self.show_library,
            Pane::Articles => self.show_articles,
            Pane::Reader => self.show_reader,
            Pane::Annotations => self.show_annotations,
        }
    }

    /// Set expansion without changing the saved order or dimensions.
    pub fn set_pane_visible(&mut self, pane: Pane, visible: bool) {
        match pane {
            Pane::Library => self.show_library = visible,
            Pane::Articles => self.show_articles = visible,
            Pane::Reader => self.show_reader = visible,
            Pane::Annotations => self.show_annotations = visible,
        }
    }

    /// Fixed-width preferences; the reader consumes the remaining columns.
    pub fn pane_width(&self, pane: Pane) -> Option<u16> {
        match pane {
            Pane::Library => Some(self.library_width),
            Pane::Articles => Some(self.article_width),
            Pane::Annotations => Some(self.annotation_width),
            Pane::Reader => None,
        }
    }

    /// Update a fixed pane width within the same bounds as keyboard settings.
    pub fn resize_pane(&mut self, pane: Pane, delta: i32) {
        let (width, minimum, maximum) = match pane {
            Pane::Library => (&mut self.library_width, 16, 48),
            Pane::Articles => (&mut self.article_width, 24, 80),
            Pane::Annotations => (&mut self.annotation_width, 24, 80),
            Pane::Reader => return,
        };
        *width = (i32::from(*width) + delta).clamp(minimum, maximum) as u16;
    }

    /// Every pane must occur once so hit testing and rendering stay aligned.
    pub fn valid_panel_order(&self) -> bool {
        Pane::ALL.iter().all(|pane| {
            self.panel_order.iter().filter(|item| *item == pane).count() == 1
        })
    }
}

/// Portable image rendering policy; terminal detection remains runtime state.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ImageSettings {
    /// Permit image downloads and preparation after opening an article.
    pub enabled: bool,
    /// Explicit protocol override, or conservative terminal detection.
    pub protocol: ImagePreference,
}

impl Default for ImageSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            protocol: ImagePreference::Auto,
        }
    }
}

/// Persisted override, separate from the protocol detected in this terminal.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize,
)]
#[serde(rename_all = "lowercase")]
pub enum ImagePreference {
    /// Detect supported terminals; use character cells otherwise.
    #[default]
    Auto,
    /// Character-cell preview, requiring no graphics extension.
    Blocks,
    /// Sixel graphics, qualified on Foot.
    Sixel,
    /// iTerm2 inline graphics.
    Iterm2,
}
