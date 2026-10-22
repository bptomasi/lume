//! Portable identities reference other definitions, never local SQLite row IDs.

use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

use crate::model::{Interest, PanelLayout, RecommendationProfile, Workspace};

/// Versioned, user-authored definitions. Omitted collections are empty.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Configuration {
    /// Supported document contract, independent of the application version.
    pub schema_version: u16,
    /// Portable appearance and key bindings; excludes navigation and progress.
    pub ui: Ui,
    /// Explicit subscriptions, including optionally disabled sources.
    pub feeds: Vec<Feed>,
    /// User-defined article folders; memberships remain in SQLite.
    pub folders: Vec<Folder>,
    /// Named article queries.
    pub smart_lists: Vec<SmartList>,
    /// Named annotation queries, using the Notebook search grammar.
    pub note_collections: Vec<Folder>,
    /// Explicit local recommendation preferences; empty by default.
    pub recommendations: Recommendations,
    /// User-named layout profiles; excludes layout undo history.
    pub workspaces: BTreeMap<String, PanelLayout>,
}

impl Default for Configuration {
    fn default() -> Self {
        Self {
            schema_version: 1,
            ui: Ui::default(),
            feeds: Vec::new(),
            folders: Vec::new(),
            smart_lists: Vec::new(),
            note_collections: Vec::new(),
            recommendations: Recommendations::default(),
            workspaces: crate::model::LayoutHistory::new(&Workspace::default())
                .profiles,
        }
    }
}

/// Subscription identity and extraction policy; publisher titles remain cached.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Feed {
    /// Stable identifier referenced by smart lists, independent of display names.
    pub id: String,
    /// Absolute HTTP(S) subscription URL.
    pub url: String,
    /// Whether collection is enabled; disabling retains articles and annotations.
    #[serde(default = "enabled")]
    pub enabled: bool,
    /// Optional CSS override; empty selects automatic full-article extraction.
    #[serde(default)]
    pub selector: String,
    /// Automatic extraction on opening: missing, always or manual.
    #[serde(default)]
    pub fetch_policy: crate::model::FetchPolicy,
    /// Recommendation source preference, minus ten through ten.
    #[serde(default)]
    pub priority: i8,
}

fn enabled() -> bool {
    true
}

/// A named definition with a stable ID; folders leave the query empty.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Folder {
    /// Stable portable identity.
    pub id: String,
    /// Editable display name.
    pub name: String,
    /// Annotation search, omitted for ordinary article folders.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub query: String,
}

/// A portable smart list, combining its explicit conditions with AND.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SmartList {
    /// Stable portable identity.
    pub id: String,
    /// Editable display name.
    pub name: String,
    /// Boolean article search, including OR, NOT and exact phrases.
    #[serde(default)]
    pub query: String,
    /// Restrict to unread articles.
    #[serde(default)]
    pub unread: bool,
    /// Restrict to favorites.
    #[serde(default)]
    pub starred: bool,
    /// Single subscription restriction, using a feed's portable ID.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub feed: Option<String>,
    /// Any-of subscription group, using portable IDs.
    #[serde(default)]
    pub feeds: Vec<String>,
    /// Maximum publication age in days.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub days: Option<u16>,
}

/// User-authored relevance preferences; feedback and affinities remain private.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Recommendations {
    /// At most 24 explicit interests; no interests means no recommendations.
    pub interests: Vec<Interest>,
    /// Boolean exclusion query.
    pub exclude: String,
    /// Optional maximum estimated reading duration.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub minutes: Option<u16>,
    /// Include already read articles in the shortlist.
    pub include_read: bool,
}

/// Explicit appearance; source content, selection and session history stay local.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Ui {
    /// Built-in palette name.
    pub theme: String,
    /// Request standard terminal mouse reporting.
    pub mouse: bool,
    /// Image downloads and protocol override.
    pub images: crate::model::ImageSettings,
    /// Visually justify eligible prose without altering readable text.
    pub justify: bool,
    /// Render supported LaTeX as selectable Unicode instead of raw source.
    pub latex_formulas: bool,
    /// Normal-mode key names mapped to ex commands.
    pub keybindings: BTreeMap<String, String>,
    /// Pane arrangement and prose widths, without article identities.
    pub layout: PanelLayout,
}

impl Default for Ui {
    fn default() -> Self {
        Self::capture(&Workspace::default())
    }
}

impl Ui {
    pub(crate) fn capture(workspace: &Workspace) -> Self {
        Self {
            theme: workspace.theme.clone(),
            mouse: workspace.mouse,
            images: workspace.images.clone(),
            justify: workspace.justify,
            latex_formulas: workspace.latex_formulas,
            keybindings: workspace.keybindings.clone(),
            layout: PanelLayout::capture(workspace),
        }
    }

    /// Update appearance without changing article, destination or session state.
    pub fn apply(&self, workspace: &mut Workspace) {
        self.layout.apply(workspace);
        workspace.theme = self.theme.clone();
        workspace.mouse = self.mouse;
        workspace.images = self.images.clone();
        workspace.justify = self.justify;
        workspace.latex_formulas = self.latex_formulas;
        workspace.keybindings = self.keybindings.clone();
    }
}

impl Configuration {
    pub(crate) fn normalize(&mut self) {
        self.feeds.sort_by(|left, right| left.id.cmp(&right.id));
        self.folders.sort_by(|left, right| left.id.cmp(&right.id));
        self.smart_lists
            .sort_by(|left, right| left.id.cmp(&right.id));
        self.note_collections
            .sort_by(|left, right| left.id.cmp(&right.id));
        self.recommendations
            .interests
            .sort_by(|left, right| left.name.cmp(&right.name));
    }

    pub(crate) fn validate(&self) -> Result<()> {
        ensure!(self.schema_version == 1, "Unsupported configuration schema");
        ensure!(self.feeds.len() <= 5000, "At most 5,000 feeds");
        validate_names(self.folders.iter().map(|item| (&item.id, &item.name)))?;
        validate_names(
            self.smart_lists.iter().map(|item| (&item.id, &item.name)),
        )?;
        validate_names(
            self.note_collections
                .iter()
                .map(|item| (&item.id, &item.name)),
        )?;
        self.validate_feeds()?;
        self.validate_queries()?;
        let mut workspace = Workspace::default();
        self.ui.apply(&mut workspace);
        crate::store::validate_workspace_dimensions(&workspace)?;
        for (key, command) in &self.ui.keybindings {
            crate::app::validate_binding(key, command)?;
        }
        ensure!(self.workspaces.len() <= 32, "At most 32 workspaces");
        for (name, layout) in &self.workspaces {
            ensure!(
                crate::store::valid_workspace_name(name),
                "Invalid workspace name"
            );
            layout.apply(&mut workspace);
            crate::store::validate_workspace_dimensions(&workspace)?;
        }
        crate::recommendation::validate_profile(&RecommendationProfile {
            interests: self.recommendations.interests.clone(),
            exclude: self.recommendations.exclude.clone(),
            minutes: self.recommendations.minutes,
            include_read: self.recommendations.include_read,
            sources: BTreeMap::new(),
        })
    }

    fn validate_feeds(&self) -> Result<()> {
        ensure!(
            self.feeds.iter().filter(|feed| feed.priority != 0).count() <= 128,
            "At most 128 source preferences"
        );
        let mut ids = BTreeSet::new();
        let mut urls = BTreeSet::new();
        for feed in &self.feeds {
            validate_id(&feed.id)?;
            ensure!(ids.insert(&feed.id), "Duplicate feed ID: {}", feed.id);
            let url = crate::source::feed_url(&feed.url)?;
            ensure!(urls.insert(url.to_string()), "Duplicate feed URL: {url}");
            ensure!(
                (-10..=10).contains(&feed.priority),
                "Invalid source priority"
            );
            crate::source::article::rules::validate_selector(&feed.selector)?;
        }
        Ok(())
    }

    fn validate_queries(&self) -> Result<()> {
        let feeds = self
            .feeds
            .iter()
            .map(|feed| &feed.id)
            .collect::<BTreeSet<_>>();
        for list in &self.smart_lists {
            crate::search::parse(&list.query)?;
            ensure!(list.feeds.len() <= 128, "Source group exceeds 128 feeds");
            ensure!(
                list.days.is_none_or(|days| days > 0),
                "Days must be positive"
            );
            for id in list.feed.iter().chain(&list.feeds) {
                ensure!(feeds.contains(id), "Unknown feed reference: {id}");
            }
        }
        ensure!(
            self.note_collections.len() <= 100,
            "At most 100 note collections"
        );
        for list in &self.note_collections {
            crate::search::notes::validate(&list.query)?;
        }
        ensure!(
            self.folders.iter().all(|folder| folder.query.is_empty()),
            "Article folders cannot define a query"
        );
        Ok(())
    }
}

fn validate_id(id: &str) -> Result<()> {
    ensure!(
        !id.is_empty()
            && id.len() <= 64
            && id
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric()
                    || b"_-".contains(&byte)),
        "IDs require 1–64 ASCII letters, digits, underscores or hyphens"
    );
    Ok(())
}

fn validate_names<'a>(
    items: impl Iterator<Item = (&'a String, &'a String)>,
) -> Result<()> {
    let mut ids = BTreeSet::new();
    let mut names = BTreeSet::new();
    for (id, name) in items {
        ensure!(ids.len() < 1000, "At most 1,000 named definitions");
        validate_id(id)?;
        crate::store::validate_collection_name(name)?;
        ensure!(ids.insert(id), "Duplicate definition ID: {id}");
        ensure!(
            names.insert(name.to_lowercase()),
            "Duplicate definition name: {name}"
        );
    }
    Ok(())
}
