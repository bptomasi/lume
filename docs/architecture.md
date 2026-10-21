# Architecture

Lume is an independent, local terminal reader. The library is the product's
persistence boundary; sources, terminal protocols and filesystem publication
are adapters with explicit limits. No component requires a hosted service,
container, external database or model runtime.

## Ownership and flow

- `main`, `cli`, `configuration` and `terminal` compose paths, configuration
  adoption, noninteractive commands and terminal lifetime.
- `config` owns the TOML schema, validation, structural merge, document editing,
  file publication and bounded background reconciliation.
- `store` owns schema evolution, transactions, stable local identities, search,
  history, publisher caches and private reading state. Its configuration adapter
  maps portable IDs onto local definitions.
- `source` validates URLs, discovers and decodes feeds, collects with bounded
  concurrency, and extracts original articles. Publisher refreshes preserve
  personal flags, memberships, notes and article deletion.
- `app` owns focus, modes, selection and workflows, composing typed storage and
  background contracts. `ui` reads cached presentation state and never runs SQL,
  HTTP, Markdown parsing, syntax compilation or image encoding while drawing.
- `reader` owns source offsets, wrapping, code styling and source-based motion.
  `media` owns bounded image retrieval, decoding, SVG rasterization and caches.
- `recommendation` owns explicit matching policy, transparent scores and bounded
  diversity. Its default profile is empty. `catalog` is an opt-in preset adapter.
- `exchange` owns OPML and private atomic Markdown export, including incremental
  exports that retain local edits as conflicts.

## Declarative configuration and private state

Configuration contains subscriptions, extraction selectors, folder/query
identities, explicit recommendation preferences, shortcuts and appearance.
Articles, notes, highlights, feedback, favorites, memberships, progress and
session/history data remain in SQLite. Exporting configuration never exports
article bodies or annotation text.

`configuration_bindings` maps stable portable keys onto local row IDs. These
bindings prevent renames from creating new folders or detaching source groups.
Generated keys use SQLite random identifiers; they are identifiers, not access
credentials. Local and file definitions normalize by key for deterministic
comparison, while the TOML document preserves existing record order.

Definition triggers increment a durable revision. Workspace triggers compare
only appearance fields; selection, progress and last-visit updates do not
schedule configuration publication. Publisher titles, validators, source health,
article ingestion and annotation writes are outside this revision boundary.

The last successful configuration snapshot and revision form a publication base.
Reconciliation merges that base, current library definitions and the edited file.
Unrelated fields and keyed records compose; incompatible edits to the same field
return a conflict instead of overwriting either input. Unknown keys, versions,
queries, selectors, references and dimensions fail before application.

A reconciler acquires the adjacent file lock before a SQLite IMMEDIATE
transaction. It captures state, merges definitions, applies only changed records,
archives removed definitions, publishes a same-directory temporary file and
acknowledges the new base. It never deletes article or annotation rows. Soft
archival of folders preserves memberships for restoration by the same ID.

Filesystem replacement and SQLite commit are not one atomic transaction. If
publication succeeds and database acknowledgement fails, the complete file and
pending SQLite edit survive. On replay, still-unbound existing definitions adopt
matching portable keys from that file before the merge, avoiding duplicate
identities after a crash. Explicit recovery backs up both competing inputs
before choosing file or library authority.

Lume's writers share an advisory file lock. Its guard explicitly unlocks before
closing, so a child spawn's briefly inherited file description cannot retain a
completed writer's lock until exec. Publication compares current bytes
before replacement to detect an external editor's intervening save. An editor
need not honor advisory locks, so the final comparison/rename window cannot
provide a universal external-writer transaction guarantee. Temporary files are
private by default; existing configuration permissions and symlinks are retained.

`config::formatting` applies layout to the existing `toml_edit` tree: generated
records become table arrays and long scalar arrays become multiline values.
Explicit formatting validates typed equivalence before atomic publication and
does not open SQLite. Commented inline containers retain their layout because
the library's table conversion clears child decoration. Definition publication
patches either representation by portable identity, preserving comments when a
source is changed or disabled. Formatting runs outside the input/render lane.

## Responsive interactive work

Within a paragraph, `reader::wrapping` consumes ordered display/source spans
as wrapped rows advance. It retires completed prefixes instead of rescanning
all formatting for every row. `reader::links` separately collects bounded labels
from parser events, including hard breaks and image alternatives. Destination
lookup preserves first-seen order without copying the preceding paragraph.
Eligible prose rows can use cached word-gap expansion. `reader::viewport`
projects these extra cells back to the original byte of the word separator,
so mouse hits, Vim motion, search, yanks and note anchors share one text identity.
Each unwrapped code/table row also retains its source block key; horizontal
positions belong to that key and survive reflow without moving sibling blocks.
`reader::index::Navigation` separately retains each link's source range, so
Enter follows the occurrence under the cursor even when labels repeat or wrap.
HTTP(S) destinations open through the system browser. Same-page fragments use
preserved HTML IDs or heading slugs and the reader's bounded jump history;
missing sections report their absence without guessing a different passage.

The reader enables pulldown-cmark's table events and captures styled cells with
their original byte mappings. `reader::tables` owns Unicode column widths,
alignment and rounded borders; Ratatui still owns terminal rendering. Prepared
grid rows share the indexed horizontal viewport used by code, with separate
row ranges retaining the ordinary panel background. The inspector reuses cached
rows when available. Selection and annotation project cell text back to source;
decorative borders and padding have no quotation mapping.
HTML `thead`/`th` and fully bold first rows identify publisher headers. Plain or
partly emphasized data rows do not become invented column labels.

Padded grid output has a shared 1 MiB budget per article, at most 64 columns,
2,000 rows including the header, and 4,096 terminal cells per row. Exceeding a
limit displays an explicit notice and the original Markdown, preserving later
article content. The inspector has its own total 1 MiB preview bound. Code
backgrounds are painted across visible rows before token and selection styles;
this never pads the stored or copied code text. Table preparation stays on the
existing layout worker, and drawing only projects cached visible cells.

The terminal initializes an application from reconciled state, then presents the
native-cell lighthouse. The first actual key press is consumed. Focus, modifier,
release and resize events cannot accidentally activate a library operation;
resize redraws the scene. Collection can continue behind the welcome screen.

Interactive navigation uses a replaceable read lane and a separate catalog/count
lane, each with its own SQLite connection. Pages hydrate only a bounded set of
ordered identities; count and page share a read snapshot. Epochs reject obsolete
successes and failures after navigation or mutation. Previous readable content
remains visible while replacement Markdown and syntax work finish. Stale source
presentation cannot authorize article mutations.

Personal edits use ordered bounded writes. They are not replaced by later edits
or acknowledged before their completions reach the UI. Progress and layout writes
have their own ordered admission and explicit flush boundary. Where a workflow
reads mutable state before writing, an IMMEDIATE transaction acquires write
authority first, avoiding WAL read-to-write snapshot upgrade failures.

Note completions update catalog counts independently of the reader. Replacing
visible annotations also requires the selected and displayed article identities
to match the completion; retained content from another article is left intact
until navigation delivers its matching snapshot. Cursor offsets falling within
whitespace omitted by wrapping clamp to the preceding visible row.

The configuration lane owns file observation, parsing and publication. It polls
only the owner/revision fields and file metadata while idle; the publication
snapshot is excluded from this hot query. It does not reread TOML on each
key or frame. Startup always validates the current file. Successful observations
retain the input timestamp so an edit made during reconciliation is retried.
Single-writer merges reuse validated definitions without building a JSON tree;
only concurrent structural merges need that representation. Complete bounded
notifications allow safe coalescing. Definitions
wait for ordinary reading mode without selections or active gestures. UI
installation compares the last presented appearance against current edits so
an old completion cannot overwrite newer in-memory choices. Catalog fingerprints
avoid reloading metadata for appearance-only publications.

Shutdown flushes recommendation, personal and session writers before stopping
configuration reconciliation. Publication errors remain visible and pending
SQLite edits survive restart. Worker failures are reported rather than silently
turning the file and library into unrelated configuration authorities.

## Reading, layout and images

Normalized Markdown retains source offsets for cursor motion, Unicode selections,
search, links and annotations. Highlights retain their original quotations and
source identity; unmatched positions remain explicit rather than guessing a new
anchor. Feed and full-article provenance remain distinguishable. Full-article
extraction uses the qualified Readability/CSS libraries and reports short,
obstructed or failed extraction without replacing a good cached document.
Clear copyright-only results are rejected even when wrapped in invisible anchor
markup. If automatic extraction fails, unique `article`, `main` or `role=main`
containers are tried under the same size/element limits. Explicit CSS rules
remain authoritative. Previously cached obstructions fall back to the original
feed in the reader; neither source nor its annotations are deleted.

Sparse row checkpoints bound Unicode work for long code rows. Drawing projects
only visible cells and preserves syntax, search, selection and cursor offsets.
Layout and wrapping run outside input where they are expensive; tests cover
long rows, resizing and libraries larger than the displayed metadata page.

Library search caches normalized destination names when the catalog changes.
Typing filters those names without SQLite work; article search reuses the
bounded latest-request navigation worker and the existing destination query.
Both fields reserve rows inside their own panes. Renderers and pointer hit
testing share that geometry. Cancel restores the sidebar destination by identity,
so a catalog refresh cannot retarget it. Reader Escape clears source-local search
marks before changing fullscreen or library state. Subsequent Escape restores
the layout and returns focus to Articles, revealing that pane if collapsed.
Leaving Reader with an empty search does not cause a redundant article reload.

### Terminal frame map

| Surface | Frame owner |
| --- | --- |
| Library, Articles, Reader, Annotations and Notebook panes | `ui::theme::panel` |
| Help, commands, note/card editors, forms, pickers and documents | `ui::popup::draw_frame` |
| Settings, Space actions, Ctrl-w guide and hovered notes | `ui::popup::draw_frame` |
| Floating and compact dragged panes | Captured panel or `ui::drag_picture` |
| Reader tables and their text inspections | `reader::tables` |

Frames retain the Unicode arcs `╭ ╮ ╰ ╯`. Popup titles leave connected border
segments beside both upper corners, even with wide Unicode text. The terminal
owns glyph rasterization; these character-cell outlines have no pixel radius
setting. Fullscreen image views, welcome art and split insertion markers are
not framed windows. Native image occlusion uses the same popup footprints.
Hovered note previews bound cleaning and row allocation rather than preparing
an entire long note on every frame.

The workspace owns pane topology, visibility and widths. Keyboard and mouse use
those same operations and hit regions. The reader's centered fullscreen column
defaults to 80 terminal cells, independent of panel widths. Captured pane images
support bounded pointer-following drag and settling; gestures persist on commit.

Image workers enforce byte, dimension, pixel, encoding and cache budgets. SVG
resource resolvers reject external and embedded resource loads. Native Foot
rendering uses prepared cell-height Sixel strips from the qualified encoder,
not a custom protocol implementation. Scrolling reuses those strips; menus and
selections clip only intersecting regions. The inspector fits or zooms from
source bytes/paths. Portable halfblock previews remain available independently
of native protocols.

## Verification and evolution

Refinery owns immutable, contiguous migration history and checksum validation.
The bootstrap and executed migrations are not renamed or rewritten. V11 adds
configuration revision/binding state and reversible definition archival. Older
populated libraries retain the former implicit recommendation profile explicitly;
new libraries start neutral. Migration tests exercise real owned SQLite files.
V13 observes justification-only appearance changes and admits previously enabled
preferences for configuration publication without replacing personal data.

Native tests cover public behavior and credible failures. Visual snapshots cover
reviewed terminal cells, styles, themes and sizes. Real HTTP fixtures qualify
network boundaries. Owned Foot and Alacritty windows complement buffer tests;
protocol acknowledgement is not physical key-to-photon measurement.
See [development](development.md), [performance](performance.md),
[configuration](configuration.md) and [dependency policy](dependency-security.md).

## Interactive work and durability

The terminal paints the lighthouse before constructing article projections.
Interactive construction restores metadata/preferences; separate latest-result
workers count destinations and load the saved page/document. Migration and
initial configuration adoption precede the interactive runtime; those phases
remain explicit startup work. Navigation requests carry captured identities,
library revisions and optional reader-location/synthesis arrivals. An obsolete
completion cannot install source text or run the arrival action against a newer
selection. `ReaderIdentity` combines displayed article/source/revision; storage
reading snapshots carry their article identity and installation verifies it.
Rendered parser mappings form the prepared reader projection consumed by UI.

One bounded ordered personal writer owns notes, read/favorite/queue flags,
trash, tags, folders, memberships and undo. Admission happens before optimistic
flag presentation. Captured flags overlay older database pages until their
acknowledgements; failures request an authoritative reconciliation. Folder
creation/membership commits atomically with one inverse history entry. The
folder selector stages names until Enter instead of inventing SQLite IDs or
writing from cursor input. A disconnected writer acknowledges each outstanding
ticket as uncertain failure; it cannot leave an endless pending indicator.

Appearance/progress use a separate ordered session writer because they do not
share personal undo. Only these idempotent writes retry transient SQLite locks,
with three attempts and bounded worker-side backoff. Failed latest values remain
per-key in a bounded recovery backlog. An explicit flush retries them; a newer
successful value clears only its own key. `:save-status` retains the last error
in the session, and `:w` retries without reporting unresolved writes as saved.
External configuration publication uses a nonblocking
barrier plus a local-edit guard before replacing cached appearance. Drafts,
ranking edits, session saves and configuration publication have independently
attempted shutdown boundaries, including drawing/input/setup errors. Errors are
aggregated, terminal restoration still runs, and a failed unrelated edit cannot
suppress dirty-note recovery. Shutdown may wait for admitted durable work;
ordinary input never waits for the configuration barrier.

Dense note markers build one source interval index per projection, preserving
original display order even for reordered/overlapping mappings. Exact saved
anchors are checked first; repeated unresolved quotations share one relocation
result during annotation reconstruction. Rendering never writes note state.
These indexes retain domain-specific ambiguity rules; no universal task
executor, parser or cache framework is introduced.
