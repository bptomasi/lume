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
fetch_policy = "missing"
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

### Optional Miniflux servers

An existing Miniflux account can supply subscriptions and articles. The
connection is explicit and optional; local libraries never require it.

```toml
[[servers]]
id = "home"
url = "https://reader.example"
token_env = "LUME_MINIFLUX_TOKEN"
enabled = true
```

Manage these definitions without editing TOML:

```text
:server https://reader.example LUME_MINIFLUX_TOKEN
:server https://reader.example --token
:servers
:server-disable
:server-enable
:unserver
```

Use `:server URL --token` to open a masked API-key prompt. The destination is
shown before saving. Paste the key and press Enter to save it in TOML;
Esc discards the entry, Ctrl-u clears it, and Backspace edits it. The secret is
excluded from ordinary command text, annotation drafts, debug output and
frames. Reconnecting the same URL retains its portable identity and replaces
the previous credential source. This option belongs to the internal command;
the noninteractive `lume server add` command still accepts an environment name.

The Library has a **SERVERS** section with one aggregate destination per
connection and its remote feeds below it. Select a connection and use `dd` to
remove it. `:collapse servers` folds the section; pinning and Library search
also work for server destinations. Paused connections remain readable offline.
Errors and the credential source are available through `:servers`; inline
credentials appear as `TOML (hidden)`.

Library section titles (`SMART LISTS`, `FEEDS`, `SERVERS`, `YOUR FOLDERS` and
`PINNED`) are selectable controls. Click a title, or move to it with `j/k` or
the arrow keys and press **Enter**, to fold or unfold its children. `▾` marks
an expanded section and `▸` a folded one. The state survives reopening Lume.

The same edits are available outside the UI:

```sh
lume server add https://reader.example LUME_MINIFLUX_TOKEN
lume servers
lume server disable 1
lume server enable 1
lume server remove 1
```

The final argument to `server add` is the **environment variable name**, not
the token value. Commands publish `[[servers]]` to TOML and preserve its stable
ID across removal and reconnection. Removing a connection never unsubscribes
feeds on the remote server. OPML exports contain local subscriptions; portable
remote connection definitions travel with TOML.

Smart lists can restrict sources to remote feeds. TOML stores these restrictions
as `server:<connection-id>:<remote-feed-id>`; restoring a configuration keeps the
scope empty until that feed is discovered, then reuses the same local identity.
A connection used by a smart list can be paused. To remove it, first remove
that list's source restriction; removal reports the referencing list rather
than leaving an invalid configuration.

Create an API key in Miniflux under *Settings → API Keys*. Set exactly one of
`token_env` (the environment variable name) or `token` (the literal key):

```toml
[[servers]]
id = "office"
url = "https://office-reader.example"
token = "your-miniflux-api-key"
```

For `token_env`, export that variable before starting Lume or running
`lume sync`; its value is never persisted. A missing environment value is an
error, with no fallback to another credential. For `token`, Lume stores the
literal in TOML, its SQLite configuration projection and private recovery
backups. These files contain the key in plaintext. Newly created configuration
files and exports are private; existing file permissions remain unchanged.

`:config`, `lume config show`, `lume config export`, server listings and debug
output hide inline keys. Explicit `lume config show --include-secrets` or
`lume config export reader-backup.toml --include-secrets` includes them for a
complete portable copy. A redacted export needs a real key or an environment
reference before it can authenticate. Parser errors omit source excerpts so a
neighboring inline key cannot leak. Formatting and ordinary definition edits
retain the actual key in the private file.

`url` must be an absolute HTTP(S) root without credentials, a query or a
fragment; a deployment below a path prefix is supported.

Server-owned feeds do not appear as `[[feeds]]` records and are read-only in the
configuration file. Their articles are cached locally like publisher content.
On first observation, unread and starred state is copied from the server; after
that, Lume's local state is authoritative in this read-only mirror. Local
read/starred changes are not sent back to Miniflux yet, and remote changes to
already cached articles do not overwrite local state.

Disabling a server stops collection and retains all cached articles. Removing
one also deletes unsaved articles, keeping favorites, personal folders, Read
next, annotations and other explicit personal saves. Matching a smart list
alone does not protect an article from source removal. Lists restricted to
removed server feeds lose those source references; if no source remains, the
query is constrained to match nothing. Their original search text is retained.
The first observation per feed reads up to 2,000 of the
newest entries; later refreshes read changed entries in bounded pages and resume
incomplete passes by remote entry ID, including timestamp ties. Deleting earlier
entries does not shift the saved cursor. Miniflux requests on one hostname run
sequentially without the 500 ms spacing used for publisher feeds. Collection
retains four hostname lanes and the same bounded response/pagination limits.
HTTP Retry-After pauses requests to
that hostname, including discovery, and survives restart. Enabling or
reconnecting a server clears stale feed failures while respecting that shared
HTTP deadline. Miniflux 2.0.49 or
later is required for the [`changed_after` API filter](https://miniflux.app/docs/api.html). Authenticated API
requests do not follow redirects; use the final Miniflux root URL. Missing
or revoked tokens, an unreachable server and malformed responses leave the
cached library readable and report the failure through collection status and
`lume feeds --errors` for affected feeds.

Per-server history import is configurable:

```toml
[servers.sync]
initial_entries = 2000 # per feed; 0 imports all available history
entries_per_page = 200 # 1–200
pages_per_sync = 10    # 1–10; bounds one refresh to at most 2,000 records/feed
```

Each completed refresh commits its articles and history cursor together.
Incomplete imports resume on the next refresh, including after restart. Initial
history uses descending entry IDs; newer arrivals do not shift pending pages.
`lume servers` and `:servers` show durable per-feed progress. Failed/cancelled
page sets can repeat the bounded current set, without losing committed work or
changing personal read/favorite state. Limits affect first imports. To revisit
history for an existing server, select it and use `:server-history`, or run
`lume server history ID` followed by `lume sync`; cached articles are retained.

The [Miniflux API](https://miniflux.app/docs/api.html#get-entries) also offers
all-feed collection. Lume retains per-feed cursors so an initial import cannot
consume a shared history cap on a few busy feeds. A global incremental cursor
would need separate feed-initialization and removal semantics before replacing
these requests. Requests to one server remain sequential without publisher
spacing; other hosts can collect concurrently.

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

## Automatic backups and recovery bundles

Automatic snapshots are disabled by default:

```toml
[backups]
automatic = false
interval_hours = 24 # 1–8760
keep = 7            # 1–365 complete managed bundles
include_offline = false
# directory = "/absolute/path/to/backups" # default: DATA_DIR/backups
```

When enabled, backups run on the TUI maintenance worker while reading is idle,
and after successful `lume sync`, before automatic retention. No process runs
while Lume is closed. The last successful publication survives restart.
`:backup status` shows the effective policy and location; `:backup now` creates
a managed bundle immediately, using that policy and rotation; `:backup cancel`
requests cancellation between SQLite page steps or file-copy chunks. A final
publication already admitted may complete before cancellation is observed.

Each bundle contains a consistent SQLite snapshot and a SHA-256 manifest.
Articles, notes, highlights and the library's declarative projection share one
WAL snapshot. Optional offline originals are captured while holding the same
cross-process lock as offline prepare/release. Ordinary recoverable image caches
are omitted. Checksums are computed as offline files stream to disk. Publication
uses same-filesystem staging and hard links, with the manifest published last.
Files/directories created by Lume are private; snapshots contain API tokens and
personal data just like the source library.

Rotation only considers complete managed bundles belonging to the source's
canonical database path. Another library sharing the directory, extra user
files, symlinks, and incomplete staging directories are preserved. Moving a
library changes its rotation namespace; old-location backups remain untouched.
A rotation failure keeps the new snapshot and reports the error. The newest
snapshot is always retained. The configured count applies to eligible bundles;
bundles with extra/modified-size files can leave more than `keep` directories.

Explicit exports are independent of automatic rotation:

```sh
lume backup snapshot.sqlite3
lume backup --bundle new-backup-directory
lume backup --include-offline new-offline-backup-directory
lume restore new-offline-backup-directory new-library-directory
```

`--include-offline` implies a directory bundle. All destinations must be new.
Backup/restore remain available even if the active TOML is invalid. Restore
validates bundle names, sizes and checksums in private staging before creating
the final destination; migrations and configuration export apply only to its
isolated database copy. Restored checkpoints never claim a worker is still
running. When offline images were omitted, affected package entries become
partial (or failed if their target is unavailable), report zero cached images
and retain their targets for `:offline resume`. Bundles including offline files
preserve captured availability. Existing SQLite snapshot files remain supported.

## Retention and recoverable caches

Retention is unlimited and automatic maintenance is disabled by default.
Article lifetimes and recoverable-cache lifetimes are independent:

Use `:retention preset` or `lume retention preset` to compare starting points.
Choose one with `:retention preset balanced` or
`lume retention preset balanced`:

| Preset | Full text | Images | Read articles | Unread articles |
| --- | --- | --- | --- | --- |
| `archive` | Unlimited | Unlimited | Unlimited | Unlimited |
| `cache-only` | 30 days | 30 days | Unlimited | Unlimited |
| `balanced` | 30 days | 30 days | 90 days | Unlimited |
| `compact` | 7 days | 14 days | 30 days | Unlimited |

A preset writes ordinary `[retention.defaults]` values to TOML and sets
`automatic = false`; it does not delete content. Every generated value remains
editable. Source overrides, personal protections, smart-list exceptions,
scheduling and run limits are preserved. An `archive` preset therefore keeps
global defaults unlimited while explicit source overrides still apply. There
is no hidden preset layer or dependency on a preset name in stored policies.
Preview the resulting policy before applying or enabling automatic retention.

```toml
[retention]
automatic = false
interval_hours = 24
batch_size = 32
max_articles_per_run = 1000
max_images_per_run = 1000
protect_smart_lists = true
unprotected_smart_lists = []

[retention.defaults]
full_text_days = 30
image_days = 30
read_days = 90
unread_days = "unlimited"

[[retention.sources]]
id = "journal"             # Existing [[feeds]] ID
read_days = "unlimited"    # Omitted fields inherit the defaults
full_text_days = 14

# A discovered server feed uses its portable identity:
# [[retention.sources]]
# id = "server:home:42"
# unread_days = 180
```

Use positive whole days (1–365,000) or `"unlimited"`; zero is rejected. Read and
unread lifetimes start at the later of local admission and the last read/unread
transition, rather than the publisher's date. Marking an old item unread gives
it a fresh unread lifetime. Upgraded libraries receive a grace period for
existing items whose transition date was not recorded. Full-text age starts
at its successful download. Ordinary images expire only when their referencing
articles and the files themselves are old enough; a fresh download survives.

Favorites, queued articles, every personal folder membership (including hidden
folders), notes, highlights, note-link targets, tags, saved web pages, recoverable
drafts/undo, export manifests, offline-package items, remaining reading-session
items and the current reader are always protected. These protections also keep
their Full text and images. Active smart-list matches are protected by default;
`unprotected_smart_lists = ["temporary-list-id"]` exempts individual dynamic
queries, or `protect_smart_lists = false` exempts all dynamic queries. Personal
folders and the other protections still apply. Deleting a smart list also
removes its exception and requires a fresh preview. Overrides reference local
feed IDs or `server:<connection-id>:<remote-feed-id>`, never display names.
Removing a server also removes retention overrides for its sources. Pausing
it instead preserves the overrides and every cached article.

Inspect and acknowledge a policy before applying it:

```sh
lume retention status
lume retention explain 123
lume retention preview
lume retention apply
```

The TUI supports `:retention status`, `:retention preview` (Enter applies),
`:retention apply`, `:retention explain [ID]` and `:retention cancel`.
Explanation lists each personal protection and the effective expiry cutoff;
`:retention explain` also identifies protection of the currently selected Reader
article. An explicit ID does not turn that other article into an active Reader.
Inspection evaluates only that article, includes smart-list matches even under
an unlimited policy, and accepts `:retention cancel`. It does not acknowledge a
policy or remove data. CLI commands work offline. Preview
counts all currently eligible content without removing it; application obeys
both per-run budgets. Changing lifetimes, source overrides or smart-list
protection requires another preview. Changing scheduling/budgets or enabling
`automatic = true` after preview retains that acknowledgement. A preview does
not itself enable automatic maintenance.

Automatic passes run on the TUI maintenance worker while ordinary reading is
idle, and after successful `lume sync`; the interval survives restarts. No work
runs while Lume is closed. Application rechecks personal protections inside
small transactions and interrupts when another connection commits, rebuilding
its plan on a later pass. Completed batches survive cancellation or a later
failure. `:retention cancel` stops between batches.
Cancelled automatic passes do not record a successful run or postpone the next
attempt by the configured interval.

Image retention maintains durable URL references for Feed and Full content.
Only changed bodies are parsed in bounded background batches; generation checks
reject stale publisher revisions. Existing libraries build the index lazily on
the first cleanup that needs it. Cancellation keeps completed index batches and
never acknowledges an incomplete preview. Planning checks cancellation between
256-article batches and smart-list queries. A compact digest-prefix index limits
image lookup to files actually present; complete digest paths are still checked
before applying shared protections or expiry. The first cleanup after an index
upgrade can rebuild references lazily; later plans reuse them.

### Storage reports

`lume storage --details --unit MiB` and `:storage details MiB` break down
logical text by source and physical SQLite pages by table/index.
`lume storage --json` exposes exact byte counts, including the Feed/Full body
copy retained by the internal-content FTS table. Logical copies are diagnostic
and are not added to filesystem totals. The `duplication` object reports the
publisher-body total, FTS body amplification ratio and allocated content/other
FTS pages. An empty publisher corpus has no ratio. Separators and metadata make
logical byte ratios different from page allocation. Inspection never enables
compression.

Interactive storage inspections use their own replaceable read worker, so
backups and offline preparation can continue independently. A newer storage
request interrupts the previous inspection; `:storage cancel` stops the
inspection and any active cache cleanup, leaving backups and offline preparation
alone. SQLite scans and image-directory entries check cancellation. Esc
closes the report without cancelling durable maintenance.

`:storage` uses readable binary units automatically. Use `:storage MiB`,
`:storage GiB` or `:storage bytes` to select a fixed unit, and `:storage auto`
for automatic selection. MiB is 1,048,576 bytes; GiB is 1,073,741,824.
Small nonzero values display as `<0.01 MiB/GiB` instead of a misleading zero.

The report leads with the total size of library and image files, followed by
article/note counts. Physical database, WAL/shared-memory, ordinary image and
pinned offline files are separated from logical feed/Full/note text. Logical
content and reusable pages are already part of the database/WAL, so they are
not added a second time. Totals exclude backups and configuration. Inspection
runs on the maintenance worker; selecting units does not trigger cleanup.

Manual image cleanup inventories owned files before loading historical bodies.
An empty cache requires no article-body reads. Long galleries are supported
without the offline package's 64-image download limit; shared references and
pinned copies remain intact. Candidate files replaced after inventory are
skipped, and cancellation also stops reference collection.

Expiring Full text preserves title, link and the original feed body; fetch it
again normally. Expiring articles permanently removes eligible local news,
leaving compact source/GUID/URL tombstones so refreshes do not reimport them.
It never deletes news from the remote server. Ordinary image cleanup preserves
shared images needed by protected or unexpired articles, foreign files,
symlinks and the separate offline namespace. Unreferenced images use the global
image lifetime; with an unlimited global lifetime and only source overrides,
unknown orphan images are retained. A fresh cache skips historical image-link
scans; long galleries do not inherit offline-package download limits.
SQLite reuses freed pages, so logical bytes
released need not shrink the database file.

## Appearance, shortcuts and layouts

```toml
[ui]
theme = "lume"
mouse = true
justify = true
latex_formulas = true
latex_graphics = false

[ui.auto_read]
enabled = true
min_active_seconds = 10
min_coverage_percent = 90

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

Article clicks select a preview and Enter opens the Reader without changing
read state. Automatic marking requires both ten active Reader seconds and
90% cumulative displayed publisher text by default. Jumping directly to the
end does not count skipped text. A short article visible on one screen needs
only the active time. Notes are excluded from coverage; code outside the
horizontal viewport is excluded too.

Automatic time pauses outside the Reader, in dialogs, during selections or
gestures, and when the terminal reports lost window focus. Long event-loop
stalls do not count. Coverage and time reset when the article/source changes;
ordinary soft wrapping preserves them. Manual read/unread changes suppress
automatic marking for that article visit, including an explicit unread action
on an already unread article. `m`, `:mark-read`, `:mark-unread` and undo remain
available. Progress restored from disk does not imply displayed coverage.

`:auto-read on|off|toggle|status` controls the policy. `:settings` previews the
switch and thresholds; Ctrl-s saves and Esc discards changes. TOML accepts
1–3,600 active seconds and 1–100 percent coverage. Disable through `enabled`,
rather than setting a threshold to zero. Timing uses terminal focus reports;
terminals that do not send these reports cannot communicate window focus.

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
`:config` shows the cached, validated document with inline keys hidden; `:config-reload` requests
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

## Extraction and images

`feeds.fetch_policy` accepts `missing` (default: fetch reference-only entries),
`always` (fetch uncached full articles on opening) or `manual` (explicit fetch
only). `:fetch-policy` changes the same declarative definition. Existing TOML
without the field retains the default. Failed extraction retains the last good
cache; changing policy never deletes articles or notes.

```toml
[ui.images]
enabled = false
protocol = "auto"
```

Images are decoded from original bytes, with native Lanczos filtering and
source-based inspection rather than enlarging a character preview. The source
budget accepts up to 8,388,608 pixels, including UHD 3840×2160, while retaining
the 8 MiB download/cache-file and 64 MiB decoder allocation limits. Two raster
sources can share a 32 MiB decoded cache; larger sources evict older entries or
are prepared without retention. Two SVG trees share a 512 KiB source-input
budget. Sources can be reused across image/formula switches, zoom and resize.
Camera/EXIF orientation is applied once before fitting and caching the source.
Transparent raster edges are matted before filtering, preserving 16-bit source
precision until resampling; existing RGBA pixel buffers are borrowed instead of
cloned for compositing. SVG antialiasing composites directly onto the theme
background. Native Sixel output still has its palette/terminal limitations.

Image preferences persist across sessions. `:images on/off` changes `enabled`;
`:image-protocol auto|blocks|sixel|iterm2` changes the protocol preference.
`auto` conservatively detects the current terminal, independently per session.
Loading off prevents new automatic image work; already cached previews may
remain visible, and explicitly opening an image is a user action. Offline mode
separately prevents network access. See the [support matrix](development.md#terminal-support).

## Prose justification

Set `justify = true` under `[ui]`, or use `:justify on` / `:justify off`.
The visual settings panel also previews this preference. The default is off.
Commands publish the same preference to the declarative file, including when
justification is the only setting changed. It survives restarting the reader.

Only nonfinal prose rows expand existing word gaps, by at most three extra
cells per gap. Code blocks, tables, headings, lists and quotations keep their
original alignment. Readable text, clipboard contents, links and annotation
source ranges retain their original bytes.

## LaTeX formulas

Formula rendering defaults to `latex_formulas = true` under `[ui]`.
`:latex-formulas on|off|toggle` changes and saves the same preference;
`:settings` offers a reversible preview. With rendering off, `$...$` and
`$$...$$` retain their original source. Changes apply to articles, table
inspection and temporary linked pages, including back/forward history.

Choose lightweight text or local vector typesetting explicitly:

Set `[ui] latex_formulas = true` and `[ui] latex_graphics = false` for
text, or use `latex_graphics = true` for graphical formula blocks on Sixel
terminals.

`:latex-formulas text` and `:latex-formulas graphics` select and save the mode.
`:latex-formulas off` shows original source; `on` preserves the chosen style.
Settings exposes both switches. Text is the default and initializes no graphic
font resources. Graphics use self-contained vector outlines on the layout/image
workers, without network access, a browser, Node or a TeX installation. Small
inline variables and linear expressions remain in the paragraph as selectable
text, regardless of their length. Text commands such as `\textit`, `\textbf`
and `\texttt` stay inline too; uniform italic/bold styling is retained.
Words and punctuation wrap with the surrounding prose instead of becoming
centered image rows. Selection continues to map to the original delimited TeX.
Display equations and complex inline layouts use compact graphic blocks, with
one em occupying 80% of a terminal row rather than stretching to the slot.
Short boxed/cancelled expressions, braces and wide accents also request local
outlines in graphics mode; simple inline sums and variables stay in the paragraph.
Click a graphic to inspect and zoom; `y` copies the original TeX.

Numbered and nested lists retain their markers around formula blocks. Table
cells and table inspection always use text formulas to preserve their columns.
Terminals without Sixel and temporary linked text pages also retain the
lightweight projection. Unsupported or excessive expressions remain readable;
a document admits at most 32 graphic formulas and 32 ordinary image slots.

Text formulas also support boxed/cancelled terms (`\boxed`, `\cancel`,
`\bcancel`, `\xcancel`), over/under braces, tensor prescripts, wide accents and
under arrows. Spatial notation without a compact Unicode glyph uses explicit
labels such as `boxed(x² + y²)` or `underbrace(a + b)ₙ`. Formula selection and
notes continue to reference the exact original TeX. Unsupported results are
cached within the same bounded document preparation without relaxing the
expression-count, source-size or expansion limits; unsupported commands still
remain verbatim. Graphic coverage is independently limited by the qualified
outline renderer, with selectable text fallback.
