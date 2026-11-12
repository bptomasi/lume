//! Shared ex-command vocabulary, aliases and discoverable metadata.
//!
//! Parsing and configuration validation use this registry; application state
//! and terminal input handling remain outside this owner.

/// Syntax and intent of one available ex command.
#[derive(Clone, Copy, Debug)]
pub struct CommandSpec {
    /// Typed identity shared by aliases and application policy.
    pub command: Command,
    /// Canonical verb accepted by the dispatcher.
    pub name: &'static str,
    /// Complete syntax, including a required argument when applicable.
    pub usage: &'static str,
    /// Human-readable effect or required context.
    pub description: &'static str,
    /// Accepted alternative verbs, preserving older command spellings.
    pub aliases: &'static [&'static str],
    /// Quit aliases whose editor destination is the whole application.
    pub(crate) application_aliases: &'static [&'static str],
}

/// Match the command verb literally, including aliases; arguments stay intact.
///
/// The fixed registry bounds matching independently of the article count.
pub fn command_suggestions(text: &str) -> Vec<&'static CommandSpec> {
    let verb = text.split_whitespace().next().unwrap_or("");
    let exact = text
        .trim_start()
        .contains(char::is_whitespace)
        .then(|| resolve(verb))
        .flatten();
    COMMANDS
        .iter()
        .filter(|command| {
            if let Some(exact) = exact {
                return command.command == exact.command;
            }
            command.name.starts_with(verb)
                || command.aliases.iter().any(|alias| alias.starts_with(verb))
        })
        .collect()
}

pub(crate) fn resolve(verb: &str) -> Option<&'static CommandSpec> {
    COMMANDS
        .iter()
        .find(|command| command.name == verb || command.aliases.contains(&verb))
}

// The registry generates both identities and metadata. Each variant takes its
// documentation from the command description; adding a command makes the
// application routing match non-exhaustive until an owner is selected.
macro_rules! command_registry {
    ($($variant:ident {
        name: $name:literal,
        usage: $usage:literal,
        description: $description:literal,
        aliases: [$($alias:literal),* $(,)?],
        $(application_aliases: [$($application_alias:literal),* $(,)?],)?
    }),* $(,)?) => {
        /// A registered ex command, independent of its spelling or aliases.
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub enum Command {
            $(#[doc = $description] $variant,)*
        }

        const COMMANDS: &[CommandSpec] = &[
            $(CommandSpec {
                command: Command::$variant,
                name: $name,
                usage: $usage,
                description: $description,
                aliases: &[$($alias,)* $($($application_alias),*)?],
                application_aliases: &[$($($application_alias),*)?],
            },)*
        ];
    };
}

command_registry! {
    Fullscreen {
        name: "fullscreen",
        usage: "fullscreen",
        description: "Expand article to the entire terminal · F11 / Z toggle",
        aliases: ["fs"],
    },
    MarkRead {
        name: "mark-read",
        usage: "mark-read",
        description: "Mark the selected article as read; show it in Read",
        aliases: [],
    },
    MarkUnread {
        name: "mark-unread",
        usage: "mark-unread",
        description: "Return the selected article to Inbox; remove from Read",
        aliases: [],
    },
    Read {
        name: "read",
        usage: "read",
        description: "Open the automatic folder of read articles",
        aliases: [],
    },
    DeleteArticle {
        name: "delete-article",
        usage: "delete-article",
        description: "Move the selected article to Trash; u undoes",
        aliases: ["delete-post", "delete"],
    },
    RestoreArticle {
        name: "restore-article",
        usage: "restore-article",
        description: "Restore the selected article from Trash; u undoes",
        aliases: ["restore-post", "restore"],
    },
    Trash {
        name: "trash",
        usage: "trash [clean]",
        description: "Browse Trash; clean permanently deletes all its articles",
        aliases: [],
    },
    Folder {
        name: "folder",
        usage: "folder NAME",
        description: "Create a personal article folder",
        aliases: ["list"],
    },
    Fetch {
        name: "fetch",
        usage: "fetch",
        description: "Fetch and cache the full article in the background",
        aliases: [],
    },
    Content {
        name: "content",
        usage: "content",
        description: "Switch Feed / Full article; accepts feed or full",
        aliases: [],
    },
    FetchPolicy {
        name: "fetch-policy",
        usage: "fetch-policy missing|always|manual",
        description: "Set automatic article fetching for this subscription",
        aliases: [],
    },
    Folders {
        name: "folders",
        usage: "folders",
        description: "Browse personal folders, or create one",
        aliases: [],
    },
    Save {
        name: "save",
        usage: "save NAME",
        description: "Save the selected article to a folder; create if missing",
        aliases: [],
    },
    Organize {
        name: "organize",
        usage: "organize",
        description: "Choose several folders for the selected article",
        aliases: [],
    },
    Rename {
        name: "rename",
        usage: "rename NAME",
        description: "Rename the selected personal folder",
        aliases: [],
    },
    DeleteFolder {
        name: "delete-folder",
        usage: "delete-folder",
        description: "Remove the selected folder; keep articles; u undoes",
        aliases: ["delete-list"],
    },
    Favorites {
        name: "favorites",
        usage: "favorites",
        description: "Open your favorite articles",
        aliases: [],
    },
    Favorite {
        name: "favorite",
        usage: "favorite",
        description: "Toggle the selected article as a favorite",
        aliases: [],
    },
    Help {
        name: "help",
        usage: "help [layout|vim|lists|sources|study|commands]",
        description: "Read the guide or jump to a named section",
        aliases: [],
    },
    Refresh {
        name: "refresh",
        usage: "refresh",
        description: "Collect all enabled feeds in the background",
        aliases: ["r"],
    },
    Feed {
        name: "feed",
        usage: "feed URL",
        description: "Subscribe directly to an RSS, Atom or JSON Feed URL",
        aliases: [],
    },
    Server {
        name: "server",
        usage: "server URL TOKEN_ENV|--token",
        description: "Connect via an environment variable or masked TOML token entry",
        aliases: [],
    },
    Servers {
        name: "servers",
        usage: "servers",
        description: "Inspect server connections, errors and cached articles",
        aliases: [],
    },
    ServerHistory {
        name: "server-history",
        usage: "server-history",
        description: "Restart configured history import; cached personal data stays intact",
        aliases: [],
    },
    Unserver {
        name: "unserver",
        usage: "unserver",
        description: "Remove the server and unsaved articles; keep personal saves",
        aliases: [],
    },
    ServerEnable {
        name: "server-enable",
        usage: "server-enable",
        description: "Resume collection from the selected server",
        aliases: [],
    },
    ServerDisable {
        name: "server-disable",
        usage: "server-disable",
        description: "Pause the selected server; keep cached articles readable",
        aliases: [],
    },
    Discover {
        name: "discover",
        usage: "discover URL",
        description: "Find feeds advertised by a website",
        aliases: [],
    },
    Unfeed {
        name: "unfeed",
        usage: "unfeed",
        description: "Remove the feed and unsaved articles; keep personal saves",
        aliases: [],
    },
    Import {
        name: "import",
        usage: "import PATH",
        description: "Import subscriptions from an OPML file",
        aliases: [],
    },
    Export {
        name: "export",
        usage: "export PATH",
        description: "Export the selected article and notes to Markdown",
        aliases: [],
    },
    ExportNotebook {
        name: "export-notebook",
        usage: "export-notebook PATH",
        description: "Export all annotations to a new Markdown file",
        aliases: [],
    },
    ExportOpml {
        name: "export-opml",
        usage: "export-opml PATH",
        description: "Export subscriptions to a new OPML file",
        aliases: [],
    },
    Palette {
        name: "palette",
        usage: "palette",
        description: "Search actions and library destinations",
        aliases: [],
    },
    Notebook {
        name: "notebook",
        usage: "notebook",
        description: "Read articles with private annotations",
        aliases: [],
    },
    Queue {
        name: "queue",
        usage: "queue",
        description: "Open the read-next queue",
        aliases: [],
    },
    Smart {
        name: "smart",
        usage: "smart",
        description: "Create or edit a saved search",
        aliases: [],
    },
    DeleteSmart {
        name: "delete-smart",
        usage: "delete-smart",
        description: "Remove the selected smart list; keep every matching article",
        aliases: [],
    },
    Undo {
        name: "undo",
        usage: "undo",
        description: "Reverse the last personal edit",
        aliases: [],
    },
    Links {
        name: "links",
        usage: "links",
        description: "Choose a link from the selected article",
        aliases: [],
    },
    Editor {
        name: "editor",
        usage: "editor",
        description: "Write a private comment using your external editor",
        aliases: [],
    },
    DeleteComment {
        name: "delete-comment",
        usage: "delete-comment",
        description: "Remove the selected private comment",
        aliases: [],
    },
    Zen {
        name: "zen",
        usage: "zen",
        description: "Toggle centered reading",
        aliases: [],
    },
    Sidebar {
        name: "sidebar",
        usage: "sidebar",
        description: "Toggle the library sidebar",
        aliases: [],
    },
    Articles {
        name: "articles",
        usage: "articles",
        description: "Toggle the headlines panel",
        aliases: [],
    },
    Compact {
        name: "compact",
        usage: "compact",
        description: "Toggle compact headlines",
        aliases: [],
    },
    Width {
        name: "width",
        usage: "width COLUMNS",
        description: "Set prose width in terminal columns",
        aliases: [],
    },
    FullscreenWidth {
        name: "fullscreen-width",
        usage: "fullscreen-width COLUMNS",
        description: "Set centered fullscreen prose width (40–140 columns)",
        aliases: [],
    },
    SidebarWidth {
        name: "sidebar-width",
        usage: "sidebar-width COLUMNS",
        description: "Set library width in terminal columns",
        aliases: [],
    },
    ArticlesWidth {
        name: "articles-width",
        usage: "articles-width COLUMNS",
        description: "Set headline panel width in terminal columns",
        aliases: [],
    },
    Write {
        name: "write",
        usage: "write",
        description: "Write the session; in note editors :w saves and stays",
        aliases: ["w", "w!", "write!", "wa", "wa!", "wall", "wall!"],
    },
    Wq {
        name: "wq",
        usage: "wq",
        description: "Write and quit; in note editors save and return to reader",
        aliases: ["wq!", "x", "x!", "xit", "xit!", "exit", "exit!"],
    },
    Wqa {
        name: "wqa",
        usage: "wqa",
        description: "Write the open note and session, then quit Lume",
        aliases: ["wqa!", "wqall", "wqall!", "xa", "xa!", "xall", "xall!"],
    },
    Quit {
        name: "quit",
        usage: "quit",
        description: "Save your session and leave Lume",
        aliases: ["q", "q!", "quit!"],
        application_aliases: ["qa", "qa!", "qall", "qall!"],
    },
    Find {
        name: "find",
        usage: "find TEXT",
        description: "Find exact text within this article",
        aliases: [],
    },
    FindNext {
        name: "find-next",
        usage: "find-next",
        description: "Jump to the next reader search match",
        aliases: [],
    },
    FindPrev {
        name: "find-prev",
        usage: "find-prev",
        description: "Jump to the previous reader search match",
        aliases: [],
    },
    Toc {
        name: "toc",
        usage: "toc",
        description: "Navigate headings in the current article",
        aliases: ["outline"],
    },
    Highlights {
        name: "highlights",
        usage: "highlights",
        description: "Browse passage highlights; accepts a text filter",
        aliases: [],
    },
    JumpBack {
        name: "jump-back",
        usage: "jump-back",
        description: "Return to the previous reader passage",
        aliases: [],
    },
    JumpForward {
        name: "jump-forward",
        usage: "jump-forward",
        description: "Revisit a reader jump",
        aliases: [],
    },
    CopyCode {
        name: "copy-code",
        usage: "copy-code",
        description: "Copy the complete code block under the cursor",
        aliases: [],
    },
    Images {
        name: "images",
        usage: "images [on|off|retry]",
        description: "Load or pause article images; retry failures",
        aliases: [],
    },
    Image {
        name: "image",
        usage: "image",
        description: "Inspect image under cursor; zoom and pan from its source",
        aliases: [],
    },
    ImageProtocol {
        name: "image-protocol",
        usage: "image-protocol [auto|blocks|sixel|iterm2]",
        description: "Select native image pixels or portable character blocks",
        aliases: [],
    },
    ImageOpen {
        name: "image-open",
        usage: "image-open",
        description: "Open the image under the Reader cursor in a browser",
        aliases: [],
    },
    Tables {
        name: "tables",
        usage: "tables",
        description: "Inspect full-width tables with horizontal scrolling",
        aliases: [],
    },
    ExtractPreview {
        name: "extract-preview",
        usage: "extract-preview SELECTOR|auto",
        description: "Preview one CSS container; confirm before saving the source rule",
        aliases: [],
    },
    SourceHealth {
        name: "source-health",
        usage: "source-health",
        description: "Inspect collection errors, timestamps, retries and extraction rules",
        aliases: ["health"],
    },
    ExplainSearch {
        name: "explain-search",
        usage: "explain-search",
        description: "Explain the current search; accepts an explicit query",
        aliases: [],
    },
    SelectArticle {
        name: "select-article",
        usage: "select-article",
        description: "Toggle the captured article in the bulk selection",
        aliases: [],
    },
    SelectPage {
        name: "select-page",
        usage: "select-page",
        description: "Capture this page of articles for a batch operation",
        aliases: [],
    },
    ClearSelection {
        name: "clear-selection",
        usage: "clear-selection",
        description: "Discard the captured bulk selection",
        aliases: [],
    },
    Bulk {
        name: "bulk",
        usage: "bulk ACTION",
        description: "Apply read/unread/favorite/queue/tag/folder to the captured selection",
        aliases: [],
    },
    Tags {
        name: "tags",
        usage: "tags",
        description: "Inspect or replace article tags; accepts names or clear",
        aliases: [],
    },
    HighlightTags {
        name: "highlight-tags",
        usage: "highlight-tags",
        description: "Inspect or replace the selected annotation tags",
        aliases: [],
    },
    Related {
        name: "related",
        usage: "related",
        description: "Browse occurrences of this article across feeds",
        aliases: [],
    },
    Pin {
        name: "pin",
        usage: "pin",
        description: "Pin or unpin the current destination",
        aliases: [],
    },
    Collapse {
        name: "collapse",
        usage: "collapse",
        description: "Fold a sidebar section; accepts feeds/servers/lists/folders/pinned",
        aliases: [],
    },
    MoveUp {
        name: "move-up",
        usage: "move-up",
        description: "Move the current destination up in personal order",
        aliases: [],
    },
    MoveDown {
        name: "move-down",
        usage: "move-down",
        description: "Move the current destination down in personal order",
        aliases: [],
    },
    Continue {
        name: "continue",
        usage: "continue",
        description: "Resume articles with a saved reading position",
        aliases: [],
    },
    SinceLastVisit {
        name: "since-last-visit",
        usage: "since-last-visit",
        description: "Browse entries first collected since the previous session",
        aliases: [],
    },
    Grouped {
        name: "grouped",
        usage: "grouped",
        description: "Browse one representative per exact canonical article URL",
        aliases: [],
    },
    Review {
        name: "review",
        usage: "review [NOTE_QUERY]",
        description: "Review due passage highlights",
        aliases: [],
    },
    Cards {
        name: "cards",
        usage: "cards [NOTE_QUERY]",
        description: "Recall due user-authored questions before revealing answers",
        aliases: [],
    },
    Card {
        name: "card",
        usage: "card",
        description: "Write a card on the selected annotation: Question | Answer",
        aliases: [],
    },
    ReviewIn {
        name: "review-in",
        usage: "review-in DAYS",
        description: "Schedule the selected passage after an explicit review",
        aliases: [],
    },
    LinkNote {
        name: "link-note",
        usage: "link-note ARTICLE_ID",
        description: "Link the selected annotation to a related article",
        aliases: [],
    },
    NoteLinks {
        name: "note-links",
        usage: "note-links",
        description: "Browse the selected note's related articles",
        aliases: [],
    },
    ExportNotes {
        name: "export-notes",
        usage: "export-notes DIRECTORY",
        description: "Reconcile Markdown notes without replacing local edits",
        aliases: [],
    },
    Config {
        name: "config",
        usage: "config",
        description: "Inspect the current declarative TOML configuration",
        aliases: [],
    },
    ConfigReload {
        name: "config-reload",
        usage: "config-reload",
        description: "Reconcile file and pending definition edits",
        aliases: [],
    },
    Settings {
        name: "settings",
        usage: "settings",
        description: "Preview preferences; Enter edits, Ctrl-s saves, Esc discards",
        aliases: [],
    },
    LatexFormulas {
        name: "latex-formulas",
        usage: "latex-formulas on|off|toggle|text|graphics",
        description: "Select text, vector formulas or original source; persistent",
        aliases: [],
    },
    Justify {
        name: "justify",
        usage: "justify on|off|toggle",
        description: "Justify prose visually; preserve copied text and code",
        aliases: [],
    },
    SaveStatus {
        name: "save-status",
        usage: "save-status",
        description: "Inspect session saving and the latest failure; :w retries",
        aliases: [],
    },
    Theme {
        name: "theme",
        usage: "theme lume|nord|gruvbox",
        description: "Select a persistent dark palette",
        aliases: [],
    },
    Bind {
        name: "bind",
        usage: "bind KEY COMMAND",
        description: "Bind a normal-mode key to an available command",
        aliases: [],
    },
    Unbind {
        name: "unbind",
        usage: "unbind KEY",
        description: "Remove a personal normal-mode key binding",
        aliases: [],
    },
    Note {
        name: "note",
        usage: "note",
        description: "Write a new private comment",
        aliases: [],
    },
    Annotate {
        name: "annotate",
        usage: "annotate",
        description: "Write a note on the selected passage or current row",
        aliases: [],
    },
    EditNote {
        name: "edit-note",
        usage: "edit-note",
        description: "Edit the selected note or highlight annotation",
        aliases: [],
    },
    Annotations {
        name: "annotations",
        usage: "annotations",
        description: "Show or collapse article notes in source order",
        aliases: [],
    },
    Reader {
        name: "reader",
        usage: "reader",
        description: "Show or collapse the article reader",
        aliases: [],
    },
    Panel {
        name: "panel",
        usage: "panel library|articles|reader|annotations",
        description: "Toggle any pane, including the reader",
        aliases: [],
    },
    PanelClose {
        name: "panel-close",
        usage: "panel-close",
        description: "Collapse the focused pane",
        aliases: [],
    },
    PanelFocus {
        name: "panel-focus",
        usage: "panel-focus PANE",
        description: "Expand and focus a pane",
        aliases: [],
    },
    PanelMove {
        name: "panel-move",
        usage: "panel-move left|right",
        description: "Move the focused pane through the layout",
        aliases: [],
    },
    PanelResize {
        name: "panel-resize",
        usage: "panel-resize +/-COLUMNS",
        description: "Resize the focused pane by terminal columns",
        aliases: [],
    },
    Mouse {
        name: "mouse",
        usage: "mouse on|off|status",
        description: "Configure mouse capture or inspect received events",
        aliases: [],
    },
    Workspace {
        name: "workspace",
        usage: "workspace list|save NAME|load NAME|delete NAME",
        description: "Named layouts: Reading, Study, Browse and your own",
        aliases: [],
    },
    PanelDock {
        name: "panel-dock",
        usage: "panel-dock PANE TARGET left|right|above|below",
        description: "Dock one pane beside or above/below another",
        aliases: [],
    },
    PanelSplit {
        name: "panel-split",
        usage: "panel-split right|below",
        description: "Split the focused pane and open another · Ctrl-w v/s",
        aliases: [],
    },
    LayoutReset {
        name: "layout-reset",
        usage: "layout-reset",
        description: "Restore standard pane order and sizes · Ctrl-w =",
        aliases: [],
    },
    LayoutColumns {
        name: "layout-columns",
        usage: "layout-columns",
        description: "Flatten splits into columns, keeping pane order and sizes",
        aliases: [],
    },
    LayoutUndo {
        name: "layout-undo",
        usage: "layout-undo",
        description: "Undo a layout change · Ctrl-w u",
        aliases: [],
    },
    LayoutRedo {
        name: "layout-redo",
        usage: "layout-redo",
        description: "Redo a layout change · Ctrl-w r",
        aliases: [],
    },
    Notes {
        name: "notes",
        usage: "notes [QUERY]",
        description: "Open independent note search with source context",
        aliases: [],
    },
    NoteSearch {
        name: "note-search",
        usage: "note-search [QUERY]",
        description: "Search note bodies and quotations · Space /",
        aliases: [],
    },
    NoteType {
        name: "note-type",
        usage: "note-type insight|question|definition|example|todo|synthesis",
        description: "Set the selected note type · Notebook T opens choices",
        aliases: [],
    },
    NoteStatus {
        name: "note-status",
        usage: "note-status open|resolved",
        description: "Resolve or reopen a question/action · Notebook r toggles",
        aliases: [],
    },
    NotePin {
        name: "note-pin",
        usage: "note-pin",
        description: "Pin or unpin the selected note · Notebook p",
        aliases: [],
    },
    NoteTags {
        name: "note-tags",
        usage: "note-tags [TAGS|clear]",
        description: "Edit independent note tags · Notebook t",
        aliases: [],
    },
    NoteCollection {
        name: "note-collection",
        usage: "note-collection [NAME]",
        description: "Save the active note query as a dynamic collection",
        aliases: [],
    },
    NoteCollectionDelete {
        name: "note-collection-delete",
        usage: "note-collection-delete NAME",
        description: "Delete a saved query while retaining its notes",
        aliases: [],
    },
    NoteRecent {
        name: "note-recent",
        usage: "note-recent",
        description: "Choose from the latest 30 explicit note searches",
        aliases: [],
    },
    NoteBacklinks {
        name: "note-backlinks",
        usage: "note-backlinks",
        description: "Navigate incoming and outgoing note references · Notebook b",
        aliases: [],
    },
    NoteHistory {
        name: "note-history",
        usage: "note-history",
        description: "Preview and restore one of the latest 100 note versions",
        aliases: [],
    },
    NoteReanchor {
        name: "note-reanchor",
        usage: "note-reanchor",
        description: "Preview the current Reader selection as a replacement note anchor",
        aliases: [],
    },
    NoteSynthesis {
        name: "note-synthesis",
        usage: "note-synthesis",
        description: "Compose evidence from notes marked with v · Notebook s",
        aliases: [],
    },
    Drafts {
        name: "drafts",
        usage: "drafts",
        description: "Recover interrupted note/card buffers without deleting checkpoints",
        aliases: [],
    },
    ExportNoteFiles {
        name: "export-note-files",
        usage: "export-note-files DIRECTORY",
        description: "Incrementally export Note-ID.md files; preserve local edits",
        aliases: [],
    },
    Recommendations {
        name: "recommendations",
        usage: "recommendations",
        description: "Open the cached For you shortlist",
        aliases: ["for-you"],
    },
    RecommendProfile {
        name: "recommend-profile",
        usage: "recommend-profile",
        description: "Show local interests and preferences",
        aliases: [],
    },
    Interest {
        name: "interest",
        usage: "interest NAME | PRIORITY | QUERY",
        description: "Add or replace an interest; priority 1–10",
        aliases: [],
    },
    InterestRemove {
        name: "interest-remove",
        usage: "interest-remove NAME",
        description: "Remove one interest",
        aliases: [],
    },
    RecommendExclude {
        name: "recommend-exclude",
        usage: "recommend-exclude [QUERY]",
        description: "Exclude matches; empty clears exclusions",
        aliases: [],
    },
    RecommendSettings {
        name: "recommend-settings",
        usage: "recommend-settings minutes N/off | read on/off | source ID WEIGHT | reset",
        description: "Configure local recommendations",
        aliases: [],
    },
    RecommendFeedback {
        name: "recommend-feedback",
        usage: "recommend-feedback useful/not-interested/too-basic/too-advanced/clear",
        description: "Rate the selected article explicitly",
        aliases: [],
    },
    RecommendWhy {
        name: "recommend-why",
        usage: "recommend-why",
        description: "Explain cached matching evidence",
        aliases: ["why"],
    },
    Similar {
        name: "similar",
        usage: "similar",
        description: "Find articles sharing current interests",
        aliases: [],
    },
    Offline {
        name: "offline",
        usage: "offline prepare [MiB]|queue [MiB]|resume|cancel|release|status",
        description: "Prepare a folder or Read next, including full text and images",
        aliases: [],
    },
    AutoRead {
        name: "auto-read",
        usage: "auto-read [on|off|toggle|status]",
        description: "Mark read after configured active Reader time and displayed coverage",
        aliases: [],
    },
    Backup {
        name: "backup",
        usage: "backup [status|now|cancel]",
        description: "Inspect or create a local snapshot with configured rotation",
        aliases: [],
    },
    Storage {
        name: "storage",
        usage: "storage [auto|MiB|GiB|bytes|details [UNIT]|status|clean images|clean articles|clean all]",
        description: "Inspect storage; preview explicit recoverable-cache cleanup",
        aliases: [],
    },
    Retention {
        name: "retention",
        usage: "retention [status|explain [ID]|preset [NAME]|preview|apply|cancel]",
        description: "Preview or apply opt-in retention with personal protections",
        aliases: [],
    },
    Session {
        name: "session",
        usage: "session MINUTES|status|next|end",
        description: "Preview a diverse reading plan within a total time budget",
        aliases: [],
    },
    OpenLink {
        name: "open-link",
        usage: "open-link",
        description: "Open the reader cursor's link as a temporary reading",
        aliases: [],
    },
    OpenPage {
        name: "open-page",
        usage: "open-page URL",
        description: "Read a linked page inside Lume; Ctrl-o returns",
        aliases: [],
    },
    SavePage {
        name: "save-page",
        usage: "save-page",
        description: "Save the current temporary page in Saved pages",
        aliases: [],
    },
    Performance {
        name: "performance",
        usage: "performance [on|off|reset|status]",
        description: "Inspect bounded processing percentiles and pending work",
        aliases: [],
    },
}

impl Command {
    pub(crate) fn allowed_on_linked_page(self) -> bool {
        matches!(
            self,
            Command::Help
                | Command::Performance
                | Command::Image
                | Command::ImageOpen
                | Command::Images
                | Command::ImageProtocol
                | Command::LatexFormulas
                | Command::OpenLink
                | Command::OpenPage
                | Command::SavePage
        )
    }

    /// Article mutations/navigation require the source page to be visible.
    pub(crate) fn requires_visible_article(self) -> bool {
        matches!(
            self,
            Command::Favorite
                | Command::MarkRead
                | Command::MarkUnread
                | Command::Save
                | Command::Organize
                | Command::DeleteArticle
                | Command::RestoreArticle
                | Command::Bulk
                | Command::Tags
                | Command::SelectArticle
                | Command::SelectPage
                | Command::ClearSelection
                | Command::Rename
                | Command::DeleteFolder
                | Command::Unfeed
                | Command::DeleteSmart
                | Command::Pin
                | Command::MoveUp
                | Command::MoveDown
                | Command::Collapse
                | Command::Fetch
                | Command::Content
                | Command::ExtractPreview
                | Command::Links
                | Command::Toc
                | Command::Tables
                | Command::CopyCode
                | Command::RecommendFeedback
                | Command::RecommendWhy
                | Command::Similar
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn registry_spellings_are_unique_and_aliases_keep_identity() {
        let mut spellings = BTreeSet::new();
        let mut identities = BTreeSet::new();
        for spec in COMMANDS {
            assert!(
                identities.insert(spec.name),
                "Duplicate command: {}",
                spec.name
            );
            for spelling in
                std::iter::once(spec.name).chain(spec.aliases.iter().copied())
            {
                assert!(
                    spellings.insert(spelling),
                    "Duplicate spelling: {spelling}"
                );
                assert_eq!(
                    resolve(spelling).map(|found| found.command),
                    Some(spec.command)
                );
            }
        }
    }

    #[test]
    fn help_has_one_section_aware_entry_at_its_original_position() {
        let commands = command_suggestions("");
        let help = command_suggestions("help");
        assert_eq!(help.len(), 1);
        assert_eq!(commands[18].command, Command::Help);
        assert_eq!(commands[19].command, Command::Refresh);
        assert_eq!(
            help[0].usage,
            "help [layout|vim|lists|sources|study|commands]"
        );
        assert_eq!(
            command_suggestions("list Ciência")[0].command,
            Command::Folder
        );
        assert_eq!(command_suggestions("fs ")[0].command, Command::Fullscreen);
    }
}
