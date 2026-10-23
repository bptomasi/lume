//! Small data contracts shared by storage, collection and presentation.

mod credentials;
pub use credentials::ApiToken;

mod document;
mod offline;
mod reading_session;
pub use offline::*;
pub use reading_session::{ReadingSession, SessionItem};
mod extraction;
mod layout;
mod layout_history;
mod notebook;
pub use layout_history::LayoutHistory;
pub use notebook::*;
mod recommendation;
pub use recommendation::*;
mod organization;
mod pane;
mod study;
mod workspace;
pub use layout::{Dock, PaneLayout, PanelLayout};

pub use document::*;
pub use extraction::*;
pub use organization::*;
pub use pane::Pane;
pub use study::*;
pub use workspace::*;

/// Smallest cache scope affected by a committed undo transaction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UndoEffect {
    /// The undo stack was empty.
    Unchanged,
    /// Only annotations on this captured article changed.
    Notes(i64),
    /// Article/library state changed and needs its ordinary refresh path.
    Library,
}

/// Article metadata; bodies are loaded only when an article is selected.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Article {
    /// Local, durable SQLite identity.
    pub id: i64,
    /// Subscription that supplied the article.
    pub feed_id: i64,
    /// Human-readable headline.
    pub title: String,
    /// Safe HTTP(S) URL, or an empty string when absent.
    pub url: String,
    /// Source title as displayed in the library.
    pub feed: String,
    /// Author as supplied by the publisher.
    pub author: String,
    /// Publication time in Unix seconds; zero when unknown.
    pub published: i64,
    /// Whether the user has yet to mark the article read.
    pub unread: bool,
    /// Personal favorite, independent of list membership.
    pub starred: bool,
    /// Number of private comments.
    pub comments: usize,
    /// Whether this article is in the explicit next-reading queue.
    pub queued: bool,
}

/// Downloaded content ready for transactional ingestion.
#[derive(Debug)]
pub struct IncomingArticle {
    /// Feed-scoped publisher identity, with a URL fallback.
    pub external_id: String,
    /// Headline.
    pub title: String,
    /// Canonical HTTP(S) article URL.
    pub url: String,
    /// Author display name.
    pub author: String,
    /// Unix publication time.
    pub published: i64,
    /// Terminal-safe Markdown normalized from feed content.
    pub body: String,
    /// Whether the feed supplied full content, a summary, or no usable text.
    pub body_origin: FeedOrigin,
}

/// A subscription and its conditional HTTP cache validators.
#[derive(Clone, Debug)]
pub struct Feed {
    /// Local subscription identity.
    pub id: i64,
    /// Display title.
    pub title: String,
    /// Normalized absolute HTTP(S) feed URL.
    pub url: String,
    /// Last successful ETag, if any.
    pub etag: String,
    /// Last successful Last-Modified header, if any.
    pub modified: String,
    /// Last collection failure, empty after success.
    pub error: String,
    /// Remote collection state when the feed is owned by an optional server.
    pub server: Option<ServerFeed>,
}

/// Configuration projection for one optional remote reader server.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Server {
    /// Local server identity.
    pub id: i64,
    /// Normalized server root without a trailing slash.
    pub url: String,
    /// Environment variable holding the API token.
    pub token_env: String,
    /// Inline credential when the environment variable name is empty.
    pub token: Option<ApiToken>,
    /// Whether collection through this server is enabled.
    pub enabled: bool,
    /// Last server-level failure, empty after success or before the first try.
    pub error: String,
}

/// Local feed mapped to one remote subscription.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServerFeed {
    /// Local server identity.
    pub server_id: i64,
    /// Normalized server root.
    pub server_url: String,
    /// Environment variable holding the API token.
    pub token_env: String,
    /// Inline credential when the environment variable name is empty.
    pub token: Option<ApiToken>,
    /// Whether the server definition is enabled.
    pub enabled: bool,
    /// Remote feed identity.
    pub remote_id: i64,
    /// Whether a first successful entry page has been observed.
    pub initialized: bool,
    /// Latest observed remote `changed_at` second, zero before first sync.
    pub last_changed: i64,
    /// Last remote ID while a bounded incremental pass is incomplete.
    pub sync_after_id: i64,
    /// Highest change timestamp observed during the current bounded pass.
    pub sync_latest: i64,
    /// Whether the subscription still exists on the server.
    pub present: bool,
}

/// Initial unread and starred state for one newly observed remote entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RemoteArticleState {
    /// Whether the entry is unread on the server.
    pub unread: bool,
    /// Whether the entry is starred on the server.
    pub starred: bool,
}

/// Remote collection result for one server feed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RemoteFeedUpdate {
    /// Local server identity.
    pub server_id: i64,
    /// Remote feed identity.
    pub remote_id: i64,
    /// Whether this update completes the first observed page set.
    pub initialized: bool,
    /// Latest remote `changed_at` second in this update.
    pub last_changed: i64,
    /// Last remote ID while a bounded incremental pass is incomplete.
    pub sync_after_id: i64,
    /// Highest change timestamp observed during the current bounded pass.
    pub sync_latest: i64,
    /// Initial state keyed by the article's stable external identity.
    pub states: std::collections::BTreeMap<String, RemoteArticleState>,
}

/// One remote subscription discovered on a configured server.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RemoteFeedCandidate {
    /// Remote feed identity.
    pub remote_id: i64,
    /// Remote display title.
    pub title: String,
    /// Original publisher feed URL reported by the server.
    pub remote_url: String,
    /// Local API identity used as the read-only feed URL.
    pub local_url: String,
}

/// An explicitly named reading collection.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Collection {
    /// Local list identity.
    pub id: i64,
    /// Unique display name, compared case-insensitively by storage.
    pub name: String,
    /// Member identities in full snapshots; empty in navigation catalogs.
    pub articles: std::collections::BTreeSet<i64>,
}

/// A private, editable comment attached to a cached article.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Comment {
    /// Durable identity, used for editing and deletion.
    pub id: i64,
    /// Plain text, never interpreted as terminal control sequences.
    pub body: String,
    /// Creation time in Unix seconds.
    pub created: i64,
    /// Immutable passage captured when the annotation was created.
    pub quote: String,
    /// Original UTF-8 byte offset; a hint after publisher edits.
    pub anchor: usize,
    /// Origin of the captured quote; legacy annotations belong to the feed.
    #[serde(default)]
    pub source: ContentSource,
    /// Full-cache revision at capture; zero for legacy/feed annotations.
    #[serde(default)]
    pub source_revision: i64,
}

/// Navigation catalog plus a bounded metadata page; bodies remain separate.
#[derive(Default)]
pub struct Library {
    /// Current metadata page, newest first with a stable ID tie-break.
    pub articles: Vec<Article>,
    /// Active subscriptions.
    pub feeds: Vec<Feed>,
    /// Configured remote reader connections, including paused servers.
    pub servers: Vec<Server>,
    /// Personal lists, including empty lists.
    pub collections: Vec<Collection>,
    /// Saved typed search criteria.
    pub smart_lists: Vec<SmartList>,
}
