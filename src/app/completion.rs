//! Discoverable ex commands with literal prefix matching and bounded work.
//!
//! Suggestions describe the existing dispatch contracts; aliases resolve to
//! their canonical verb before dispatch. Tab only edits the draft. Enter on
//! an incomplete verb fills it first, so browsing never executes an action.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::{App, InputKind, Mode};

/// Syntax and intent of one available ex command.
#[derive(Clone, Copy, Debug)]
pub struct CommandSpec {
    /// Canonical verb accepted by the dispatcher.
    pub name: &'static str,
    /// Complete syntax, including a required argument when applicable.
    pub usage: &'static str,
    /// Human-readable effect or required context.
    pub description: &'static str,
    /// Accepted alternative verbs, preserving older command spellings.
    pub aliases: &'static [&'static str],
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
                return command.name == exact.name;
            }
            command.name.starts_with(verb)
                || command.aliases.iter().any(|alias| alias.starts_with(verb))
        })
        .collect()
}

pub(super) fn resolve(verb: &str) -> Option<&'static CommandSpec> {
    COMMANDS
        .iter()
        .find(|command| command.name == verb || command.aliases.contains(&verb))
}

impl App {
    pub(super) fn completion_key(&mut self, key: KeyEvent) -> bool {
        let Mode::Input(input) = &mut self.mode else {
            return false;
        };
        if matches!(input.kind, InputKind::NoteSearch)
            && key.code == KeyCode::Tab
        {
            if let Some(text) = crate::search::notes::complete(&input.text) {
                input.text = text;
                input.editor = super::editor::textarea(&input.text);
            }
            return true;
        }
        if matches!(input.kind, InputKind::Search) && key.code == KeyCode::Tab {
            return complete_search(input);
        }
        if !matches!(input.kind, InputKind::Command) {
            return false;
        }
        let control = key.modifiers.contains(KeyModifiers::CONTROL);
        let forward = match key.code {
            KeyCode::Tab => {
                complete(input);
                return true;
            }
            KeyCode::Down => true,
            KeyCode::Up | KeyCode::BackTab => false,
            KeyCode::Char('n') if control => true,
            KeyCode::Char('p') if control => false,
            _ => return false,
        };
        let count = command_suggestions(&input.text).len();
        if count > 0 {
            input.completion = if forward {
                (input.completion + 1) % count
            } else {
                (input.completion + count - 1) % count
            };
        }
        true
    }

    pub(super) fn prepare_command(&mut self) -> bool {
        let Mode::Input(input) = &mut self.mode else {
            return true;
        };
        if !matches!(input.kind, InputKind::Command) {
            return true;
        }
        let verb = input.text.split_whitespace().next().unwrap_or("");
        let Some(command) = resolve(verb) else {
            return !complete(input);
        };
        let argument = input.text.trim_start().get(verb.len()..).unwrap_or("");
        let required = command
            .usage
            .split_whitespace()
            .nth(1)
            .is_some_and(|argument| !argument.starts_with('['));
        if required && argument.trim().is_empty() {
            self.status = format!("Usage: :{}", command.usage);
            return false;
        }
        true
    }
}

fn complete_search(input: &mut super::Input) -> bool {
    let Some(completed) = crate::search::complete(&input.text) else {
        return false;
    };
    input.text = completed;
    input.editor = super::editor::textarea(&input.text);
    true
}

fn complete(input: &mut super::Input) -> bool {
    let choices = command_suggestions(&input.text);
    let Some(command) = choices.get(input.completion) else {
        return false;
    };
    let verb = input.text.split_whitespace().next().unwrap_or("");
    let argument = input.text.trim_start().get(verb.len()..).unwrap_or("");
    let argument = if argument.is_empty() && command.usage != command.name {
        " "
    } else {
        argument
    };
    let completed = format!("{}{argument}", command.name);
    if completed.len() > 16_384 {
        return false;
    }
    input.text = completed;
    input.editor = super::editor::textarea(&input.text);
    input.completion = 0;
    true
}

const COMMANDS: &[CommandSpec] = &[
    CommandSpec {
        name: "fullscreen",
        usage: "fullscreen",
        description: "Expand article to the entire terminal · F11 / Z toggle",
        aliases: &["fs"],
    },
    CommandSpec {
        name: "mark-read",
        usage: "mark-read",
        description: "Mark the selected article as read; show it in Read",
        aliases: &[],
    },
    CommandSpec {
        name: "mark-unread",
        usage: "mark-unread",
        description: "Return the selected article to Inbox; remove from Read",
        aliases: &[],
    },
    CommandSpec {
        name: "read",
        usage: "read",
        description: "Open the automatic folder of read articles",
        aliases: &[],
    },
    CommandSpec {
        name: "delete-article",
        usage: "delete-article",
        description: "Move the selected article to Trash; u undoes",
        aliases: &["delete-post", "delete"],
    },
    CommandSpec {
        name: "restore-article",
        usage: "restore-article",
        description: "Restore the selected article from Trash; u undoes",
        aliases: &["restore-post", "restore"],
    },
    CommandSpec {
        name: "trash",
        usage: "trash",
        description: "Browse deleted articles and restore them",
        aliases: &[],
    },
    CommandSpec {
        name: "folder",
        usage: "folder NAME",
        description: "Create a personal article folder",
        aliases: &["list"],
    },
    CommandSpec {
        name: "fetch",
        usage: "fetch",
        description: "Fetch and cache the full article in the background",
        aliases: &[],
    },
    CommandSpec {
        name: "content",
        usage: "content",
        description: "Switch Feed / Full article; accepts feed or full",
        aliases: &[],
    },
    CommandSpec {
        name: "fetch-policy",
        usage: "fetch-policy missing|always|manual",
        description: "Set automatic article fetching for this subscription",
        aliases: &[],
    },
    CommandSpec {
        name: "folders",
        usage: "folders",
        description: "Browse personal folders, or create one",
        aliases: &[],
    },
    CommandSpec {
        name: "save",
        usage: "save NAME",
        description: "Save the selected article to a folder; create if missing",
        aliases: &[],
    },
    CommandSpec {
        name: "organize",
        usage: "organize",
        description: "Choose several folders for the selected article",
        aliases: &[],
    },
    CommandSpec {
        name: "rename",
        usage: "rename NAME",
        description: "Rename the selected personal folder",
        aliases: &[],
    },
    CommandSpec {
        name: "delete-folder",
        usage: "delete-folder",
        description: "Remove the selected folder; keep articles; u undoes",
        aliases: &["delete-list"],
    },
    CommandSpec {
        name: "favorites",
        usage: "favorites",
        description: "Open your favorite articles",
        aliases: &[],
    },
    CommandSpec {
        name: "favorite",
        usage: "favorite",
        description: "Toggle the selected article as a favorite",
        aliases: &[],
    },
    CommandSpec {
        name: "help",
        usage: "help",
        description: "Open the keyboard and command field guide",
        aliases: &[],
    },
    CommandSpec {
        name: "refresh",
        usage: "refresh",
        description: "Collect all enabled feeds in the background",
        aliases: &["r"],
    },
    CommandSpec {
        name: "feed",
        usage: "feed URL",
        description: "Subscribe directly to an RSS, Atom or JSON Feed URL",
        aliases: &[],
    },
    CommandSpec {
        name: "server",
        usage: "server URL TOKEN_ENV|--token",
        description: "Connect via an environment variable or masked TOML token entry",
        aliases: &[],
    },
    CommandSpec {
        name: "servers",
        usage: "servers",
        description: "Inspect server connections, errors and cached articles",
        aliases: &[],
    },
    CommandSpec {
        name: "unserver",
        usage: "unserver",
        description: "Remove the selected server connection; retain articles and notes",
        aliases: &[],
    },
    CommandSpec {
        name: "server-enable",
        usage: "server-enable",
        description: "Resume collection from the selected server",
        aliases: &[],
    },
    CommandSpec {
        name: "server-disable",
        usage: "server-disable",
        description: "Pause the selected server; keep cached articles readable",
        aliases: &[],
    },
    CommandSpec {
        name: "discover",
        usage: "discover URL",
        description: "Find feeds advertised by a website",
        aliases: &[],
    },
    CommandSpec {
        name: "unfeed",
        usage: "unfeed",
        description: "Disable the selected feed; retain cached articles",
        aliases: &[],
    },
    CommandSpec {
        name: "import",
        usage: "import PATH",
        description: "Import subscriptions from an OPML file",
        aliases: &[],
    },
    CommandSpec {
        name: "export",
        usage: "export PATH",
        description: "Export the selected article and notes to Markdown",
        aliases: &[],
    },
    CommandSpec {
        name: "export-notebook",
        usage: "export-notebook PATH",
        description: "Export all annotations to a new Markdown file",
        aliases: &[],
    },
    CommandSpec {
        name: "export-opml",
        usage: "export-opml PATH",
        description: "Export subscriptions to a new OPML file",
        aliases: &[],
    },
    CommandSpec {
        name: "palette",
        usage: "palette",
        description: "Search actions and library destinations",
        aliases: &[],
    },
    CommandSpec {
        name: "notebook",
        usage: "notebook",
        description: "Read articles with private annotations",
        aliases: &[],
    },
    CommandSpec {
        name: "queue",
        usage: "queue",
        description: "Open the read-next queue",
        aliases: &[],
    },
    CommandSpec {
        name: "smart",
        usage: "smart",
        description: "Create or edit a saved search",
        aliases: &[],
    },
    CommandSpec {
        name: "delete-smart",
        usage: "delete-smart",
        description: "Remove the selected smart list",
        aliases: &[],
    },
    CommandSpec {
        name: "undo",
        usage: "undo",
        description: "Reverse the last personal edit",
        aliases: &[],
    },
    CommandSpec {
        name: "links",
        usage: "links",
        description: "Choose a link from the selected article",
        aliases: &[],
    },
    CommandSpec {
        name: "editor",
        usage: "editor",
        description: "Write a private comment using your external editor",
        aliases: &[],
    },
    CommandSpec {
        name: "delete-comment",
        usage: "delete-comment",
        description: "Remove the selected private comment",
        aliases: &[],
    },
    CommandSpec {
        name: "zen",
        usage: "zen",
        description: "Toggle centered reading",
        aliases: &[],
    },
    CommandSpec {
        name: "sidebar",
        usage: "sidebar",
        description: "Toggle the library sidebar",
        aliases: &[],
    },
    CommandSpec {
        name: "articles",
        usage: "articles",
        description: "Toggle the headlines panel",
        aliases: &[],
    },
    CommandSpec {
        name: "compact",
        usage: "compact",
        description: "Toggle compact headlines",
        aliases: &[],
    },
    CommandSpec {
        name: "width",
        usage: "width COLUMNS",
        description: "Set prose width in terminal columns",
        aliases: &[],
    },
    CommandSpec {
        name: "fullscreen-width",
        usage: "fullscreen-width COLUMNS",
        description: "Set centered fullscreen prose width (40–140 columns)",
        aliases: &[],
    },
    CommandSpec {
        name: "sidebar-width",
        usage: "sidebar-width COLUMNS",
        description: "Set library width in terminal columns",
        aliases: &[],
    },
    CommandSpec {
        name: "articles-width",
        usage: "articles-width COLUMNS",
        description: "Set headline panel width in terminal columns",
        aliases: &[],
    },
    CommandSpec {
        name: "write",
        usage: "write",
        description: "Write the session; in note editors :w saves and stays",
        aliases: &["w", "w!", "write!", "wa", "wa!", "wall", "wall!"],
    },
    CommandSpec {
        name: "wq",
        usage: "wq",
        description: "Write and quit; in note editors save and return to reader",
        aliases: &["wq!", "x", "x!", "xit", "xit!", "exit", "exit!"],
    },
    CommandSpec {
        name: "wqa",
        usage: "wqa",
        description: "Write the open note and session, then quit Lume",
        aliases: &["wqa!", "wqall", "wqall!", "xa", "xa!", "xall", "xall!"],
    },
    CommandSpec {
        name: "quit",
        usage: "quit",
        description: "Save your session and leave Lume",
        aliases: &["q", "q!", "quit!", "qa", "qa!", "qall", "qall!"],
    },
    CommandSpec {
        name: "find",
        usage: "find TEXT",
        description: "Find exact text within this article",
        aliases: &[],
    },
    CommandSpec {
        name: "find-next",
        usage: "find-next",
        description: "Jump to the next reader search match",
        aliases: &[],
    },
    CommandSpec {
        name: "find-prev",
        usage: "find-prev",
        description: "Jump to the previous reader search match",
        aliases: &[],
    },
    CommandSpec {
        name: "toc",
        usage: "toc",
        description: "Navigate headings in the current article",
        aliases: &["outline"],
    },
    CommandSpec {
        name: "highlights",
        usage: "highlights",
        description: "Browse passage highlights; accepts a text filter",
        aliases: &[],
    },
    CommandSpec {
        name: "jump-back",
        usage: "jump-back",
        description: "Return to the previous reader passage",
        aliases: &[],
    },
    CommandSpec {
        name: "jump-forward",
        usage: "jump-forward",
        description: "Revisit a reader jump",
        aliases: &[],
    },
    CommandSpec {
        name: "copy-code",
        usage: "copy-code",
        description: "Copy the complete code block under the cursor",
        aliases: &[],
    },
    CommandSpec {
        name: "images",
        usage: "images [on|off|retry]",
        description: "Load or pause article images; retry failures",
        aliases: &[],
    },
    CommandSpec {
        name: "image",
        usage: "image",
        description: "Inspect image under cursor; zoom and pan from its source",
        aliases: &[],
    },
    CommandSpec {
        name: "image-protocol",
        usage: "image-protocol [auto|blocks|sixel|iterm2]",
        description: "Select native image pixels or portable character blocks",
        aliases: &[],
    },
    CommandSpec {
        name: "image-open",
        usage: "image-open",
        description: "Open the image under the Reader cursor in a browser",
        aliases: &[],
    },
    CommandSpec {
        name: "tables",
        usage: "tables",
        description: "Inspect full-width tables with horizontal scrolling",
        aliases: &[],
    },
    CommandSpec {
        name: "extract-preview",
        usage: "extract-preview SELECTOR|auto",
        description: "Preview one CSS container; confirm before saving the source rule",
        aliases: &[],
    },
    CommandSpec {
        name: "source-health",
        usage: "source-health",
        description: "Inspect collection errors, timestamps, retries and extraction rules",
        aliases: &["health"],
    },
    CommandSpec {
        name: "explain-search",
        usage: "explain-search",
        description: "Explain the current search; accepts an explicit query",
        aliases: &[],
    },
    CommandSpec {
        name: "select-article",
        usage: "select-article",
        description: "Toggle the captured article in the bulk selection",
        aliases: &[],
    },
    CommandSpec {
        name: "select-page",
        usage: "select-page",
        description: "Capture this page of articles for a batch operation",
        aliases: &[],
    },
    CommandSpec {
        name: "clear-selection",
        usage: "clear-selection",
        description: "Discard the captured bulk selection",
        aliases: &[],
    },
    CommandSpec {
        name: "bulk",
        usage: "bulk ACTION",
        description: "Apply read/unread/favorite/queue/tag/folder to the captured selection",
        aliases: &[],
    },
    CommandSpec {
        name: "tags",
        usage: "tags",
        description: "Inspect or replace article tags; accepts names or clear",
        aliases: &[],
    },
    CommandSpec {
        name: "highlight-tags",
        usage: "highlight-tags",
        description: "Inspect or replace the selected annotation tags",
        aliases: &[],
    },
    CommandSpec {
        name: "related",
        usage: "related",
        description: "Browse occurrences of this article across feeds",
        aliases: &[],
    },
    CommandSpec {
        name: "pin",
        usage: "pin",
        description: "Pin or unpin the current destination",
        aliases: &[],
    },
    CommandSpec {
        name: "collapse",
        usage: "collapse",
        description: "Fold a sidebar section; accepts feeds/servers/lists/folders/pinned",
        aliases: &[],
    },
    CommandSpec {
        name: "move-up",
        usage: "move-up",
        description: "Move the current destination up in personal order",
        aliases: &[],
    },
    CommandSpec {
        name: "move-down",
        usage: "move-down",
        description: "Move the current destination down in personal order",
        aliases: &[],
    },
    CommandSpec {
        name: "continue",
        usage: "continue",
        description: "Resume articles with a saved reading position",
        aliases: &[],
    },
    CommandSpec {
        name: "since-last-visit",
        usage: "since-last-visit",
        description: "Browse entries first collected since the previous session",
        aliases: &[],
    },
    CommandSpec {
        name: "grouped",
        usage: "grouped",
        description: "Browse one representative per exact canonical article URL",
        aliases: &[],
    },
    CommandSpec {
        name: "review",
        usage: "review [NOTE_QUERY]",
        description: "Review due passage highlights",
        aliases: &[],
    },
    CommandSpec {
        name: "cards",
        usage: "cards [NOTE_QUERY]",
        description: "Recall due user-authored questions before revealing answers",
        aliases: &[],
    },
    CommandSpec {
        name: "card",
        usage: "card",
        description: "Write a card on the selected annotation: Question | Answer",
        aliases: &[],
    },
    CommandSpec {
        name: "review-in",
        usage: "review-in DAYS",
        description: "Schedule the selected passage after an explicit review",
        aliases: &[],
    },
    CommandSpec {
        name: "link-note",
        usage: "link-note ARTICLE_ID",
        description: "Link the selected annotation to a related article",
        aliases: &[],
    },
    CommandSpec {
        name: "note-links",
        usage: "note-links",
        description: "Browse the selected note's related articles",
        aliases: &[],
    },
    CommandSpec {
        name: "export-notes",
        usage: "export-notes DIRECTORY",
        description: "Reconcile Markdown notes without replacing local edits",
        aliases: &[],
    },
    CommandSpec {
        name: "config",
        usage: "config",
        description: "Inspect the current declarative TOML configuration",
        aliases: &[],
    },
    CommandSpec {
        name: "config-reload",
        usage: "config-reload",
        description: "Reconcile file and pending definition edits",
        aliases: &[],
    },
    CommandSpec {
        name: "settings",
        usage: "settings",
        description: "Preview preferences; Enter edits, Ctrl-s saves, Esc discards",
        aliases: &[],
    },
    CommandSpec {
        name: "latex-formulas",
        usage: "latex-formulas on|off|toggle|text|graphics",
        description: "Select text, vector formulas or original source; persistent",
        aliases: &[],
    },
    CommandSpec {
        name: "justify",
        usage: "justify on|off|toggle",
        description: "Justify prose visually; preserve copied text and code",
        aliases: &[],
    },
    CommandSpec {
        name: "save-status",
        usage: "save-status",
        description: "Inspect session saving and the latest failure; :w retries",
        aliases: &[],
    },
    CommandSpec {
        name: "theme",
        usage: "theme lume|nord|gruvbox",
        description: "Select a persistent dark palette",
        aliases: &[],
    },
    CommandSpec {
        name: "bind",
        usage: "bind KEY COMMAND",
        description: "Bind a normal-mode key to an available command",
        aliases: &[],
    },
    CommandSpec {
        name: "unbind",
        usage: "unbind KEY",
        description: "Remove a personal normal-mode key binding",
        aliases: &[],
    },
    CommandSpec {
        name: "help",
        usage: "help [layout|vim|lists|sources|study|commands]",
        description: "Read the guide or jump to a named section",
        aliases: &[],
    },
    CommandSpec {
        name: "note",
        usage: "note",
        description: "Write a new private comment",
        aliases: &[],
    },
    CommandSpec {
        name: "annotate",
        usage: "annotate",
        description: "Write a note on the selected passage or current row",
        aliases: &[],
    },
    CommandSpec {
        name: "edit-note",
        usage: "edit-note",
        description: "Edit the selected note or highlight annotation",
        aliases: &[],
    },
    CommandSpec {
        name: "annotations",
        usage: "annotations",
        description: "Show or collapse article notes in source order",
        aliases: &[],
    },
    CommandSpec {
        name: "reader",
        usage: "reader",
        description: "Show or collapse the article reader",
        aliases: &[],
    },
    CommandSpec {
        name: "panel",
        usage: "panel library|articles|reader|annotations",
        description: "Toggle any pane, including the reader",
        aliases: &[],
    },
    CommandSpec {
        name: "panel-close",
        usage: "panel-close",
        description: "Collapse the focused pane",
        aliases: &[],
    },
    CommandSpec {
        name: "panel-focus",
        usage: "panel-focus PANE",
        description: "Expand and focus a pane",
        aliases: &[],
    },
    CommandSpec {
        name: "panel-move",
        usage: "panel-move left|right",
        description: "Move the focused pane through the layout",
        aliases: &[],
    },
    CommandSpec {
        name: "panel-resize",
        usage: "panel-resize +/-COLUMNS",
        description: "Resize the focused pane by terminal columns",
        aliases: &[],
    },
    CommandSpec {
        name: "mouse",
        usage: "mouse on|off|status",
        description: "Configure mouse capture or inspect received events",
        aliases: &[],
    },
    CommandSpec {
        name: "workspace",
        usage: "workspace list|save NAME|load NAME|delete NAME",
        description: "Named layouts: Reading, Study, Browse and your own",
        aliases: &[],
    },
    CommandSpec {
        name: "panel-dock",
        usage: "panel-dock PANE TARGET left|right|above|below",
        description: "Dock one pane beside or above/below another",
        aliases: &[],
    },
    CommandSpec {
        name: "panel-split",
        usage: "panel-split right|below",
        description: "Split the focused pane and open another · Ctrl-w v/s",
        aliases: &[],
    },
    CommandSpec {
        name: "layout-reset",
        usage: "layout-reset",
        description: "Restore standard pane order and sizes · Ctrl-w =",
        aliases: &[],
    },
    CommandSpec {
        name: "layout-columns",
        usage: "layout-columns",
        description: "Flatten splits into columns, keeping pane order and sizes",
        aliases: &[],
    },
    CommandSpec {
        name: "layout-undo",
        usage: "layout-undo",
        description: "Undo a layout change · Ctrl-w u",
        aliases: &[],
    },
    CommandSpec {
        name: "layout-redo",
        usage: "layout-redo",
        description: "Redo a layout change · Ctrl-w r",
        aliases: &[],
    },
    CommandSpec {
        name: "notes",
        usage: "notes [QUERY]",
        description: "Open independent note search with source context",
        aliases: &[],
    },
    CommandSpec {
        name: "note-search",
        usage: "note-search [QUERY]",
        description: "Search note bodies and quotations · Space /",
        aliases: &[],
    },
    CommandSpec {
        name: "note-type",
        usage: "note-type insight|question|definition|example|todo|synthesis",
        description: "Set the selected note type · Notebook T opens choices",
        aliases: &[],
    },
    CommandSpec {
        name: "note-status",
        usage: "note-status open|resolved",
        description: "Resolve or reopen a question/action · Notebook r toggles",
        aliases: &[],
    },
    CommandSpec {
        name: "note-pin",
        usage: "note-pin",
        description: "Pin or unpin the selected note · Notebook p",
        aliases: &[],
    },
    CommandSpec {
        name: "note-tags",
        usage: "note-tags [TAGS|clear]",
        description: "Edit independent note tags · Notebook t",
        aliases: &[],
    },
    CommandSpec {
        name: "note-collection",
        usage: "note-collection [NAME]",
        description: "Save the active note query as a dynamic collection",
        aliases: &[],
    },
    CommandSpec {
        name: "note-collection-delete",
        usage: "note-collection-delete NAME",
        description: "Delete a saved query while retaining its notes",
        aliases: &[],
    },
    CommandSpec {
        name: "note-recent",
        usage: "note-recent",
        description: "Choose from the latest 30 explicit note searches",
        aliases: &[],
    },
    CommandSpec {
        name: "note-backlinks",
        usage: "note-backlinks",
        description: "Navigate incoming and outgoing note references · Notebook b",
        aliases: &[],
    },
    CommandSpec {
        name: "note-history",
        usage: "note-history",
        description: "Preview and restore one of the latest 100 note versions",
        aliases: &[],
    },
    CommandSpec {
        name: "note-reanchor",
        usage: "note-reanchor",
        description: "Preview the current Reader selection as a replacement note anchor",
        aliases: &[],
    },
    CommandSpec {
        name: "note-synthesis",
        usage: "note-synthesis",
        description: "Compose evidence from notes marked with v · Notebook s",
        aliases: &[],
    },
    CommandSpec {
        name: "drafts",
        usage: "drafts",
        description: "Recover interrupted note/card buffers without deleting checkpoints",
        aliases: &[],
    },
    CommandSpec {
        name: "export-note-files",
        usage: "export-note-files DIRECTORY",
        description: "Incrementally export Note-ID.md files; preserve local edits",
        aliases: &[],
    },
    CommandSpec {
        name: "recommendations",
        usage: "recommendations",
        description: "Open the cached For you shortlist",
        aliases: &["for-you"],
    },
    CommandSpec {
        name: "recommend-profile",
        usage: "recommend-profile",
        description: "Show local interests and preferences",
        aliases: &[],
    },
    CommandSpec {
        name: "interest",
        usage: "interest NAME | PRIORITY | QUERY",
        description: "Add or replace an interest; priority 1–10",
        aliases: &[],
    },
    CommandSpec {
        name: "interest-remove",
        usage: "interest-remove NAME",
        description: "Remove one interest",
        aliases: &[],
    },
    CommandSpec {
        name: "recommend-exclude",
        usage: "recommend-exclude [QUERY]",
        description: "Exclude matches; empty clears exclusions",
        aliases: &[],
    },
    CommandSpec {
        name: "recommend-settings",
        usage: "recommend-settings minutes N/off | read on/off | source ID WEIGHT | reset",
        description: "Configure local recommendations",
        aliases: &[],
    },
    CommandSpec {
        name: "recommend-feedback",
        usage: "recommend-feedback useful/not-interested/too-basic/too-advanced/clear",
        description: "Rate the selected article explicitly",
        aliases: &[],
    },
    CommandSpec {
        name: "recommend-why",
        usage: "recommend-why",
        description: "Explain cached matching evidence",
        aliases: &["why"],
    },
    CommandSpec {
        name: "similar",
        usage: "similar",
        description: "Find articles sharing current interests",
        aliases: &[],
    },
    CommandSpec {
        name: "offline",
        usage: "offline prepare [MiB]|queue [MiB]|resume|cancel|release|status",
        description: "Prepare a folder or Read next, including full text and images",
        aliases: &[],
    },
    CommandSpec {
        name: "auto-read",
        usage: "auto-read [on|off|toggle|status]",
        description: "Mark read after configured active Reader time and displayed coverage",
        aliases: &[],
    },
    CommandSpec {
        name: "storage",
        usage: "storage [auto|MiB|GiB|bytes|details [UNIT]|status|clean images|clean articles|clean all]",
        description: "Inspect storage; preview explicit recoverable-cache cleanup",
        aliases: &[],
    },
    CommandSpec {
        name: "retention",
        usage: "retention [status|explain [ID]|preset [NAME]|preview|apply|cancel]",
        description: "Preview or apply opt-in retention with personal protections",
        aliases: &[],
    },
    CommandSpec {
        name: "session",
        usage: "session MINUTES|status|next|end",
        description: "Preview a diverse reading plan within a total time budget",
        aliases: &[],
    },
    CommandSpec {
        name: "open-link",
        usage: "open-link",
        description: "Open the reader cursor's link as a temporary reading",
        aliases: &[],
    },
    CommandSpec {
        name: "open-page",
        usage: "open-page URL",
        description: "Read a linked page inside Lume; Ctrl-o returns",
        aliases: &[],
    },
    CommandSpec {
        name: "save-page",
        usage: "save-page",
        description: "Save the current temporary page in Saved pages",
        aliases: &[],
    },
    CommandSpec {
        name: "performance",
        usage: "performance [on|off|reset|status]",
        description: "Inspect bounded processing percentiles and pending work",
        aliases: &[],
    },
];
