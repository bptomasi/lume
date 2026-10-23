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
    /// Optional remote reader servers, never required for a local library.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub servers: Vec<Server>,
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
    /// Explicit cache and article lifetimes; unlimited and inactive by default.
    #[serde(default, skip_serializing_if = "retention_default")]
    pub retention: super::retention::Retention,
}

impl Default for Configuration {
    fn default() -> Self {
        Self {
            schema_version: 1,
            ui: Ui::default(),
            feeds: Vec::new(),
            servers: Vec::new(),
            folders: Vec::new(),
            smart_lists: Vec::new(),
            note_collections: Vec::new(),
            recommendations: Recommendations::default(),
            workspaces: crate::model::LayoutHistory::new(&Workspace::default())
                .profiles,
            retention: super::retention::Retention::default(),
        }
    }
}

fn retention_default(value: &super::retention::Retention) -> bool {
    *value == super::retention::Retention::default()
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

/// Optional Miniflux server with one explicit credential source.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Server {
    /// Stable identifier, independent of the server URL.
    pub id: String,
    /// Absolute HTTP(S) server root without a query or fragment.
    pub url: String,
    /// Environment variable holding the per-application API token.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub token_env: String,
    /// Optional inline API token; mutually exclusive with `token_env`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token: Option<crate::model::ApiToken>,
    /// Whether collection through this server is enabled.
    #[serde(default = "enabled")]
    pub enabled: bool,
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
    /// Opt into SVG typesetting; unsupported terminals retain readable text.
    pub latex_graphics: bool,
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
            latex_graphics: workspace.latex_graphics,
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
        workspace.latex_graphics = self.latex_graphics;
        workspace.keybindings = self.keybindings.clone();
    }
}

impl Configuration {
    /// Copy portable definitions with inline credentials hidden for display.
    pub fn redacted(&self) -> Self {
        let mut copy = self.clone();
        for server in &mut copy.servers {
            if server.token.is_some() {
                server.token = Some("[redacted]".into());
            }
        }
        copy
    }

    pub(crate) fn normalize(&mut self) {
        self.retention.unprotected_smart_lists.sort();
        self.retention
            .sources
            .sort_by(|left, right| left.id.cmp(&right.id));
        self.feeds.sort_by(|left, right| left.id.cmp(&right.id));
        self.servers.sort_by(|left, right| left.id.cmp(&right.id));
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
        self.validate_servers()?;
        self.validate_queries()?;
        self.retention.validate()?;
        self.validate_retention_sources()?;
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

    fn validate_retention_sources(&self) -> Result<()> {
        for key in &self.retention.unprotected_smart_lists {
            ensure!(
                self.smart_lists.iter().any(|list| list.id == *key),
                "Unknown unprotected smart list"
            );
        }
        for source in &self.retention.sources {
            if let Some((server, _)) = server_feed_reference(&source.id)? {
                ensure!(
                    self.servers.iter().any(|item| item.id == server),
                    "Unknown server in retention source"
                );
            } else {
                ensure!(
                    self.feeds.iter().any(|item| item.id == source.id),
                    "Unknown feed in retention source"
                );
            }
        }
        Ok(())
    }

    fn validate_servers(&self) -> Result<()> {
        ensure!(self.servers.len() <= 16, "At most 16 servers");
        let mut ids = BTreeSet::new();
        let mut urls = BTreeSet::new();
        for server in &self.servers {
            validate_id(&server.id)?;
            ensure!(
                ids.insert(&server.id),
                "Duplicate server ID: {}",
                server.id
            );
            let url = crate::source::server_url(&server.url)?;
            ensure!(urls.insert(url.clone()), "Duplicate server URL: {url}");
            ensure!(
                server.token.is_some() != !server.token_env.is_empty(),
                "Server requires exactly one of token or token_env"
            );
            if let Some(token) = &server.token {
                token.validate()?;
            } else {
                validate_env_name(&server.token_env)?;
            }
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
                if let Some((server, _)) = server_feed_reference(id)? {
                    ensure!(
                        self.servers.iter().any(|item| item.id == server),
                        "Unknown server in feed reference: {id}"
                    );
                } else {
                    ensure!(feeds.contains(id), "Unknown feed reference: {id}");
                }
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

/// Remote scopes use the stable connection key and publisher feed identity.
pub(crate) fn server_feed_reference(key: &str) -> Result<Option<(&str, i64)>> {
    let Some(reference) = key.strip_prefix("server:") else {
        return Ok(None);
    };
    let (server, remote) = reference.split_once(':').ok_or_else(|| {
        anyhow::anyhow!("Invalid server feed reference: {key}")
    })?;
    validate_id(server)?;
    let id = remote.parse::<i64>()?;
    ensure!(id > 0, "Remote feed needs a positive ID");
    Ok(Some((server, id)))
}

pub(crate) fn validate_env_name(name: &str) -> Result<()> {
    let mut bytes = name.bytes();
    ensure!(
        name.len() <= 128
            && bytes
                .next()
                .is_some_and(|byte| byte.is_ascii_alphabetic() || byte == b'_')
            && bytes.all(|byte| byte.is_ascii_alphanumeric() || byte == b'_'),
        "Token environment variable requires 1–128 ASCII letters, digits or underscores and cannot start with a digit"
    );
    Ok(())
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

#[cfg(test)]
mod tests {
    use anyhow::Result;

    #[test]
    fn inline_credentials_round_trip_but_debug_and_diagnostics_hide_them()
    -> Result<()> {
        let text = r#"schema_version = 1
[[servers]]
id = "home"
url = "https://reader.example"
token = "private-test-key"
"#;
        let config = crate::config::decode(text)?;
        assert!(config.servers[0].token_env.is_empty());
        assert!(!format!("{config:?}").contains("private-test-key"));
        assert!(crate::config::encode(&config)?.contains("private-test-key"));
        let redacted = crate::config::redacted_document(text)?;
        assert!(!redacted.contains("private-test-key"));
        assert!(redacted.contains("[redacted]"));
        assert!(crate::config::decode(&redacted).is_ok());
        let invalid = format!("{text}enabled = 42\n");
        let error = crate::config::decode(&invalid)
            .err()
            .ok_or_else(|| anyhow::anyhow!("Invalid field was accepted"))?;
        assert!(!format!("{error:#}").contains("private-test-key"));
        assert!(
            crate::config::decode(&format!("{text}token_env = \"TOKEN\"\n"))
                .is_err()
        );
        Ok(())
    }

    #[test]
    fn retention_rejects_invalid_bounds_ages_and_unknown_sources() {
        for value in ["0", "-1", "365001", "true", "1.5", "\"forever\""] {
            let text = format!("[retention.defaults]\nread_days = {value}\n");
            assert!(crate::config::decode(&text).is_err());
        }
        for text in [
            "[retention]\nbatch_size = 0",
            "[retention]\ninterval_hours = 0",
            "[retention]\nmax_images_per_run = 0",
            "[retention]\nmax_articles_per_run = 0",
            "[[retention.sources]]\nid = 'unknown'",
            "[retention]\nunprotected_smart_lists = ['unknown']",
        ] {
            assert!(crate::config::decode(text).is_err());
        }
    }

    #[test]
    fn retention_age_overrides_preserve_inheritance_and_explicit_unlimited()
    -> Result<()> {
        let text = r#"
[[feeds]]
id = "journal"
url = "https://journal.example/feed"
[retention.defaults]
read_days = 90
[[retention.sources]]
id = "journal"
read_days = "unlimited"
full_text_days = 30
"#;
        let config = crate::config::decode(text)?;
        let resolved =
            config.retention.sources[0].resolve(&config.retention.defaults);
        assert_eq!(resolved.read_days.0, None);
        assert_eq!(resolved.full_text_days.0, Some(30));
        assert_eq!(
            crate::config::decode(&crate::config::encode(&config)?)?,
            config
        );
        Ok(())
    }

    #[test]
    fn server_definition_round_trips_with_a_normalized_root() -> Result<()> {
        let config = crate::config::decode(
            r#"
            schema_version = 1

            [[servers]]
            id = "home"
            url = "https://reader.example/base/"
            token_env = "LUME_MINIFLUX_TOKEN"
            "#,
        )?;
        assert_eq!(config.servers.len(), 1);
        assert_eq!(config.servers[0].url, "https://reader.example/base");
        assert!(config.servers[0].enabled);
        let text = crate::config::encode(&config)?;
        assert!(text.contains("[[servers]]"));
        assert!(text.contains("token_env = \"LUME_MINIFLUX_TOKEN\""));
        Ok(())
    }

    #[test]
    fn server_validation_rejects_ambiguous_or_unsafe_roots() {
        for (field, value) in [
            ("url", "\"https://reader.example/?token=secret\""),
            ("url", "\"https://user:secret@reader.example\""),
            ("url", "\"ftp://reader.example\""),
            ("token_env", "\"1INVALID\""),
            ("token_env", "\"\""),
        ] {
            let text = format!(
                "schema_version = 1\n[[servers]]\nid = \"home\"\n\
                 url = \"https://reader.example\"\n\
                 token_env = \"LUME_MINIFLUX_TOKEN\"\n{field} = {value}\n"
            );
            assert!(
                crate::config::decode(&text).is_err(),
                "accepted {field} = {value}"
            );
        }
        let duplicate = r#"
            schema_version = 1

            [[servers]]
            id = "home"
            url = "https://reader.example"
            token_env = "TOKEN_A"

            [[servers]]
            id = "home"
            url = "https://other.example"
            token_env = "TOKEN_B"
            "#;
        assert!(crate::config::decode(duplicate).is_err());
    }
}
