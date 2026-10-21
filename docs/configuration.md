# Configuration

Lume uses TOML with schema version 1. A new library starts with a small file:

```toml
# Lume configuration. Personal reading data stays in SQLite.
schema_version = 1
```

Omitted settings use defaults. Sources, folders, smart lists, annotation
collections and recommendation interests are empty by default. Reading, Study
and Browse are built-in layout profiles; they contain no topic selections.
Use `lume config export` to inspect complete effective definitions.
A generic [example](../examples/config.toml) shows optional definitions.

## Paths and isolation

Configuration path precedence is:

1. `--config FILE`.
2. `LUME_CONFIG`.
3. An explicit `--database FILE` uses that file with a `.toml` extension.
4. `--data-dir DIR` or `LUME_DATA_DIR` uses `DIR/config.toml`.
5. `$XDG_CONFIG_HOME/lume/config.toml`, or `~/.config/lume/config.toml`.

An empty `LUME_CONFIG` is an error. Relative explicit file paths resolve against
the invocation directory; Lume never searches that directory implicitly.
Existing symlinks resolve to their targets and are retained. Demo mode is
isolated and does not load your real configuration.

Data paths retain the existing `--database`, `--data-dir`, `LUME_DATA_DIR` and
`~/.local/lume` contracts. Changing a configuration path does not move articles
or notes. Separate libraries should use separate data directories. A live
session reports when another process selects a different configuration file
for the same library; reopen using the new effective path.

## Sources and folders

The URLs below are illustrative placeholders, not installed defaults.

```toml
schema_version = 1

[[feeds]]
id = "journal"
url = "https://journal.example/feed"
enabled = true
selector = ""
priority = 0

[[folders]]
id = "weekend"
name = "Read this weekend"
```

IDs use 1–64 ASCII letters, digits, underscores or hyphens. They identify
records independently of editable display names and local SQLite row IDs.
Commands generate IDs automatically. Keep IDs stable when editing a name.
Source titles come from publisher metadata; configuration does not overwrite
them at every refresh.

`enabled = false` stops collection while retaining the subscription definition.
Removing a source archives it without deleting cached articles or annotations.
Removing folders, smart lists and annotation collections hides their definitions;
restoring the same ID restores them, including retained folder memberships.
Explicit in-app folder deletion retains the existing undo contract.

`selector` is a CSS full-article extraction override. Validate a rule with
`:extract-preview SELECTOR` and confirm the inspected preview with `Enter`.
Manual TOML rules are syntax-validated but do not imply that extraction from a
particular publication has been previewed. An empty selector uses automatic
extraction.

## Smart lists and annotation collections

```toml
[[smart_lists]]
id = "science"
name = "Science"
query = 'astronomy OR "climate change"'
unread = true
starred = false
feeds = ["journal"]
# feed = "journal"    # Optional single-source restriction
# days = 30           # Optional maximum publication age

[[note_collections]]
id = "questions"
name = "Open questions"
query = "type:question status:open"
```

Article smart-list conditions combine with AND; `query` supports its own
boolean expression. Feed references use portable source IDs, including disabled
sources. Remove references before removing their source definition, or retain
the source with `enabled = false`. Ordinary folders have no query.
Annotation collections use the Notebook search grammar.

## Appearance, shortcuts and layouts

```toml
[ui]
theme = "lume"
mouse = true

[ui.keybindings]
"ctrl-t" = "toc"

[ui.layout]
order = ["library", "articles", "reader", "annotations"]
visible = [true, true, true, false]
widths = [24, 36, 32]
reading_width = 80
fullscreen_width = 80
compact = false
zen = false
fullscreen = false
```

`ui.layout` is a complete layout when supplied. Widths refer to Library,
Articles and Annotations. Reading widths accept 40–140 columns; pane widths
retain their corresponding validation. Saved split trees and named workspaces
are exported in their typed TOML representation; use the layout controls to
create complex arrangements before exporting them.

The `workspaces` table stores named layout definitions, not their undo history.
Current article, selected destination, progress, layout history and search
history stay in the library. Shortcuts map normal-mode key names to available
ex commands; they do not replace editor-specific bindings. `Ctrl-c` stays
reserved for exit.

## Recommendations

```toml
[recommendations]
include_read = false
exclude = "title:advertisement"
minutes = 20

[[recommendations.interests]]
name = "Astronomy"
weight = 8
query = 'astronomy OR telescope OR "space exploration"'
```

Interests are explicit vocabulary searches, not neural semantic classification.
Weights range from 1 to 10. Each source's `priority` ranges from -10 to 10;
zero is neutral. Empty interests deliberately produce no recommendations.
Article ratings, learned affinities and reading state remain private SQLite
state. Resetting recommendation settings returns to an empty, neutral profile.
The engineering preset adds its interests only when explicitly installed into
an empty profile.

## Commands and publication

```sh
lume config path
lume config check
lume config check another-reader.toml
lume config format
lume config format another-reader.toml
lume config show
lume config export
lume config export reader-backup.toml
lume config reload
```

`path`, `check` and `format` do not create or migrate a library. Export creates
a private file and refuses to overwrite an existing destination. Inside the TUI,
`:config` shows the cached, validated document; `:config-reload` requests
reconciliation.
The background worker also observes ordinary file changes. Installation never
installs a reading preset or clears personal data.

Generated definitions use table blocks such as `[[feeds]]`, `[[smart_lists]]`
and `[[recommendations.interests]]`. Long scalar arrays use one item per line.
`lume config format` applies this layout to an existing file, preserving values,
comments and quoted literals. It validates before and after formatting, retains
permissions and symlinks, and replaces the file atomically. Repeating it leaves
the same bytes. Inline containers with internal comments retain their existing
layout to avoid discarding those comments; long URLs and query strings remain
intact. Ordinary in-app edits preserve existing layout and comments instead of
reformatting the entire file.

CLI definition edits finish publication before a successful command exit.
Interactive edits are journaled durably in SQLite and published on a bounded
background lane. Shutdown flushes admitted writers before the configuration
worker stops. Navigation and drawing never read or serialize configuration.
External definitions wait until ordinary reading mode is safe to update;
drafts, selections, Notebook sessions and active panel gestures are retained.

Unrelated file and interface changes merge against their last published base.
Conflicting changes to the same field stop publication and display an error.
Invalid TOML, unknown keys, bad references and unsupported schema versions are
rejected without partial replacement of the library. Pending interface edits
remain durable in SQLite until publication succeeds or you resolve the conflict.

## Recovery and migration

```sh
lume config recover library
lume config recover file
```

Recovery is explicit: `library` publishes pending library definitions; `file`
applies the edited file. Both inputs receive private `.lume-config-recovery-*`
backups beside the configuration before replacement. Articles, notes, highlights,
favorites and reading progress are retained. Make the selected file writable
before requesting recovery if permissions currently prevent publication.

On first adoption, an existing library contributes its definitions instead of
being reset to an empty file. Previously saved interests are preserved; populated
older libraries retain their former implicit engineering profile as an explicit
profile during the schema upgrade. Fresh libraries receive no such profile.
Never delete a real library merely to reset configuration.

Documents are limited to 2 MiB, 5,000 sources, 1,000 folders/smart lists,
100 annotation collections, 32 workspaces, 128 custom bindings, 24 interests
and 128 non-neutral source preferences. Parsed definition collections are
normalized by ID; the document retains the order of existing records.
`toml_edit` preserves comments and untouched literals within its documented
limitations, including dotted-key order and final newlines. Lume serializes
its own writers with advisory locks and checks for external edits before atomic
replacement; external editors need not honor those locks, so a final OS-level
comparison/rename race remains possible.
