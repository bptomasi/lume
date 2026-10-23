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
  concurrency, and extracts original articles. It also mirrors optional
  Miniflux subscriptions and entries with bounded pages. Publisher refreshes
  preserve personal flags, memberships, notes and article deletion.
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
HTTP(S) destinations open as temporary read-only linked readings. Same-page fragments use
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

HTML lists retain paragraph ownership, numbering and nested indentation when
normalized to Markdown. The reader defers each marker until item content begins,
so loose-list paragraph boundaries cannot flush an isolated bullet. Wrapped
continuations carry visual insets separate from readable bytes. The cached cell
index applies those insets to drawing, mouse hits and cursor positions; copying
and source-based annotation anchors retain their original byte correspondence.
Previously saved caches can contain repeated empty list items followed by
detached paragraphs. The reader recognizes only the early normalizer's exact
whitespace signature using CommonMark events. It joins those markers in a
temporary parser view and translates every event range back to the original
bytes. This works offline and requires no database or note migration; ordinary
empty items and code samples retain CommonMark semantics.

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

Catalog snapshots batch ordinary totals and feed/server/folder groups;
identical saved filters share one count within that snapshot. Read/unread flag
changes invalidate Inbox, Read, recommendations and read-sensitive smart lists.
Other counts survive, including children hidden by section folding. Repeated
requests at the same revision reuse an active/prepared snapshot; a stronger
full-library refresh upgrades it. Unknown edits, failures, deletion, undo and
publisher updates retain full reconciliation.

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

Headline presentation retains at most 64 wrapped title/date entries. Identity,
exact title, publication time and pane width determine reuse; metadata larger
than 4 KiB is displayed without retention. Reader motion can reuse this work
without caching live selection, favorite/read flags or feed labels. Images
skip workspace geometry/admission when their visible-slot interval is empty.

Article deletion acknowledges a committed visibility change separately from
generic library edits. In ordinary reading mode, acknowledgement removes its
bounded metadata row immediately and preserves the selected surviving identity.
The next page/source and sidebar counts refresh independently. The count result
does not enqueue a second page load. Generation checks reject earlier snapshots;
source-bound editors and selections defer publication through the existing
navigation guards. Failed writes never remove a row. Grouped-page counts come
from the authoritative query because another occurrence can replace a deleted
representative. The synchronous adapter performs one catalog reload.

`ui::layout` computes fixed columns and binary splits with bounded rectangle
arithmetic. Rendering, focus and pointer hit testing share that geometry.
Each split reserves the combined minimum dimensions of its expanded children;
resizing cannot suppress Articles while the terminal can fit the whole tree.
Focus cycling retains the saved pane order independently of spatial focus.
Standard reset restores canonical pane order, visibility and widths as one undoable
change, retaining source selection and prose preferences. Notebook layout
remains independent. Built-in profiles start from canonical dimensions rather
than inheriting a rearranged active workspace; saved profiles are preserved.

Library search caches normalized destination names when the catalog changes.
Typing filters those names without SQLite work; article search reuses the
bounded latest-request navigation worker and the existing destination query.
An active article query ranks title starts, contiguous title phrases and other
title matches ahead of body-only matches. SQLite FTS5 uses the same Unicode
tokenizer for membership and ranking; weighted BM25 resolves relevance within
each tier, followed by publication time and identity for stable ties. Ranking
uses only positive text from the active query. Saved-list filters, exclusions
and destination scopes remain membership constraints. Empty and metadata-only
queries keep the destination's normal order. Count queries do not score, and
only the requested page loads article metadata. Identity-based page jumps use
the same relevance order as pagination, including in the background worker.
Both fields reserve rows inside their own panes. Renderers and pointer hit
testing share that geometry. Cancel restores the sidebar destination by identity,
so a catalog refresh cannot retarget it. Reader Escape clears source-local search
marks before changing fullscreen or library state. Subsequent Escape restores
the layout and returns focus to Articles, revealing that pane if collapsed.
Leaving Reader with an empty search does not cause a redundant article reload.

Query editors reuse results while only their caret moves. Native editor change
signals avoid rebuilding draft text on those motions; article and notebook
queries only refresh when their text changes. Sidebar refreshes preserve an
uncommitted match by destination identity. Its row window walks backward from
the selected match only far enough to fill the viewport, then streams visible
rows. Drawing and pointer hits share that projection without a separate cache
that could become stale when the catalog changes. Title truncation and count
alignment use grapheme widths in terminal cells.

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
Directional focus uses those rendered rectangles, preferring neighbors with
overlapping rows or columns. It never wraps at an edge or treats a pane's center
as proof that it lies in that direction. Responsive suppression falls back to
the same topology at a size that fits every expanded pane; explicit collapse
still excludes a pane. Tab cycling retains the saved panel order.

Image workers enforce byte, dimension, pixel, encoding and cache budgets. SVG
resource resolvers reject external and embedded resource loads. Native Foot
rendering uses prepared cell-height Sixel strips from the qualified encoder,
not a custom protocol implementation. Scrolling reuses those strips; menus and
selections clip only intersecting regions. The inspector fits or zooms from
source bytes/paths. Portable halfblock previews remain available independently
of native protocols.

Two download lanes own replaceable source requests: crop/zoom changes for the
same URL retarget the existing download, while sources outside the current
viewport stop before cache access or decoding when observed in time. The latest
crop is captured after acquiring the shared decoder lock. Obsolete completions
do not enter the presentation cache or request a frame. Queue saturation is
retryable rather than a permanent image error. Dropping the worker cancels its
remaining intentions without joining HTTP. An HTTP request already waiting for
response headers still owns a lane until its existing six-second deadline;
this change does not provide transport-level cancellation.

Image source reuse is bounded independently from prepared terminal cells. Two
raster entries share 32 MiB, with eviction before allocating a new decoded
source; format headers and pixel/allocation admission are checked before that
allocation. Opaque-source classification is retained instead of rescanning
alpha on every fit. Two SVG trees share a 512 KiB raw-input budget and preserve
shared font setup. Raster sources survive intervening vector preparations.
SVG paints directly onto an opaque theme canvas, avoiding an unpremultiply/
recomposite round trip; fitted opaque pixels need only the optional odd-row
padding used by halfblocks. Native encoders and their existing bounds own wire
format and color quantization.

Unicode formula projection admits explicit enclosures, braces, tensor indices
and wide/lower accents from the existing parser. Browser-specific MathML
restrictions are irrelevant to that bounded Unicode projection; macro state
remains local and unknown commands still fail closed to raw source. Positive
and negative text/vector results share the existing bounded per-document
caches; repeated failures still consume the document admission budget.
Generated MathML text-style wrappers project transparently, normalizing mtext
nonbreaking spaces for paragraph wrapping and preserving uniform text modifiers.
Graphic admission follows spatial MathML structure instead of source length:
styled prose and linear expressions do not request image work. Display math
and two-dimensional notation retain vector outlines and exact source mappings.

Automatic read intent is owned by `app::auto_read`, independently of selection
and saved position. A bounded union of at most 1,024 displayed byte intervals
tracks publisher coverage, excluding appended notes and horizontally hidden
code. Cached viewport keys avoid recording unchanged rows on idle polls. A
monotonic clock counts only eligible focused Reader time; dialogs, gestures,
selection and lost terminal focus pause it, and gaps over one second are not
credited. Source changes reset coverage; unchanged publisher text survives
reflow. Explicit read/unread intent blocks automation for that article visit.
One qualifying observation admits the existing ordered personal write; there
are no database reads/writes, new timers or frames on ordinary idle polls.

Server credential entry is a distinct masked mode, outside general command
and draft editors. The modal owns only a transient bounded value; the store
validates the API-token type and atomically reconnects the existing portable
connection. Publication/redaction uses the ordinary configuration worker.

## Verification and evolution

Refinery owns immutable, contiguous migration history and checksum validation.
The bootstrap and executed migrations are not renamed or rewritten. V11 adds
configuration revision/binding state and reversible definition archival. Older
populated libraries retain the former implicit recommendation profile explicitly;
new libraries start neutral. Migration tests exercise real owned SQLite files.
V13 observes justification-only appearance changes and admits previously enabled
preferences for configuration publication without replacing personal data.
V19 replaces unstable remote page offsets with entry IDs and retains multiple
remote identities for one canonical article. Incomplete legacy passes restart
from their retained timestamp watermark; cached articles and notes are preserved.

Native tests cover public behavior and credible failures. Visual snapshots cover
reviewed terminal cells, styles, themes and sizes. Real HTTP fixtures qualify
network boundaries. Owned Foot windows complement buffer tests;
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

## Explicit reading services

Article selectors use independent lazy `Latest` lanes for queries and previews.
Each connection belongs to its thread. Query edits cancel old preview generations;
reopening a selector resets both lanes. Sixteen bounded previews are retained
only while that selector is open. Moving a generic selector does not refilter
its unchanged candidate query. Rendering consumes prepared rows only.

Linked readings own an independent `WebPage`, not a fabricated article identity.
The existing HTTP extractor, Markdown parser and cursor/projection contracts
prepare text on a worker. Page history retains at most eight prepared pages.
Resizing requests replacement layout by source anchor. HTTP is disabled by the
runtime's offline/demo modes. Images become explicit links in this text view;
the original Markdown remains intact for an explicit save. Save creates a real
article in a disabled, noncollecting manual source and an ordinary Saved pages
folder. Existing article bodies and personal fields remain untouched. A delayed
save carries its captured visit generation, so it cannot redirect a later
navigation, including another visit to the same URL. Back during a pending open
discards that response before consuming section history. Page/reflow delivery
invalidates mouse drag anchors, whose byte offsets belong to the old layout.
Failed opens retain the previous page and its ability to reflow.
The HTTP owner distinguishes extracted HTML from browser resources before
reading response bodies. Non-HTML types and explicit attachments return their
final URL as a typed outcome. Only a current linked-page completion may publish
a one-shot browser request; the terminal owns OS launching. Navigation cancels
undelivered requests, and the previous reading is restored before dispatch.
Background article extraction continues to reject these resources without
modifying its last successful cache or launching applications.

`app::maintenance` admits one explicit operation at a time on an ordered lane.
Mutations are never superseded by new requests. Progress delivery is bounded
and may coalesce; final results are acknowledged. Closing a report does not
reopen it when work finishes. Dropping requests cancellation without waiting
for HTTP; already admitted SQLite transactions finish atomically.

`offline` captures at most 100 article identities and URLs from Read next or a
personal folder. The existing extractor/decoder validate every new body/image.
An adjacent advisory file lock serializes preparations across processes.
Checkpoints in SQLite retain per-item availability, errors and logical bytes;
resume revalidates entries. A package defaults to 64 MiB and cannot exceed its
explicit 1–512 MiB budget. Original images live in the separate `images/offline`
namespace, limited globally to 512 MiB and 4096 owned files. Ordinary image
pruning never touches that namespace. Releasing a package removes only owned
pinned files and its checkpoint, retaining article text. In-flight network work
obeys existing HTTP deadlines; cancellation is observed between requests.
The lock also owns a package-local image accounting/decoder lifetime. It scans
the pinned namespace once per preparation, not once per article. Capacity is
updated only after atomic publication succeeds, so rejected pins cannot consume
the budget available to later items.

`storage` inspects text and disk usage on the same maintenance lane. Full-cache
reclamation uses small IMMEDIATE transactions, protecting current personal,
offline and session context, including recoverable note drafts. Clearing body
bytes retains cache identities and
fires the existing FTS/recommendation triggers. Image reclamation first scans
remaining publisher bodies, then removes only unreferenced owned regular files
older than that scan. It preserves symlinks, foreign files and pinned images.
SQLite page reuse is reported separately from physical file shrinkage. A
multi-batch operation can partially complete; it is never presented as one
filesystem/database transaction.

Retention has a separate declarative policy and a local preview acknowledgement.
Its SQLite owner materializes source cutoffs, smart-list matches and candidate
identities in temporary tables under a read snapshot; this stage never acquires
an IMMEDIATE write lease or hydrates all bodies. Each bounded mutation batch
then acquires that lease, checks `data_version` against the captured plan and
rechecks mandatory personal protections. An external commit interrupts the pass
before another batch. Full-cache clearing reuses existing document/FTS revision
semantics; article deletion records original and remote-alias tombstones first.
A monotonic article identity clock prevents deleted IDs from being reused by
later news. A single adoption marker supplies legacy reading-state grace
without rewriting the library during migration.

Image planning is skipped when no image lifetime applies or the ordinary cache
is empty. Otherwise shared body references resolve once to the strictest
lifetime, with protection/unlimited taking precedence. Cleanup rechecks file
type, size and modification time under a bounded write lease. It never treats
filesystem and SQLite changes as an atomic unit. Automatic scheduling caches
its policy/deadline in the UI and persists last-run time; input and drawing do
not poll SQLite or parse TOML. The same maintenance domain serves CLI commands.

Routine publisher refreshes compare stored publisher fields before deriving
reading duration or canonical URLs. Unchanged article payloads, HTTP validators
and failure strings do not write SQLite or touch FTS. An IMMEDIATE lease covers
the comparison and any subsequent update, preserving validator rollback and
avoiding read-to-write snapshot races. Feed/Full progress and workspace/layout
checkpoints use conditional database writes, so idempotence also survives
intervening changes from another connection.

Retention filters young ordinary cache files before hydrating image references.
It inventories only existing candidate paths, streams long galleries without
offline download caps, and preserves shared protections and unlimited overrides.

Inline server credentials use a redacting debug type. Only authenticated HTTP
and private configuration storage expose bytes. Public configuration inspection
uses a redacted document prepared on the publication worker; raw TOML and its
SQLite snapshots intentionally retain the key. Environment references never
persist expanded credentials.

Reading sessions greedily select at most eight cached unread recommendations
within a 1–240 minute total estimate. Repeated sources/interests have diminishing
returns and canonical duplicates are excluded; this is not an optimal knapsack
solver or a quality guarantee. Confirmation persists the captured order and
current index. Advancing uses an IMMEDIATE transaction, skips unavailable or
retargeted identities, and leaves read flags and feedback unchanged. Sessions,
offline checkpoints and temporary pages are reading state, not portable TOML
subscription definitions.

Optional diagnostics retain at most 512 microsecond samples for each processing
boundary. Percentiles sort only on inspection. Disabled diagnostics record no
timestamps or samples. Timings cover input dispatch, frame/flush, article-ready
navigation, selector search/preview and ordered save admission-to-delivery;
they do not include compositor display latency or send telemetry.

Notebook listing snapshots also capture the recent-search generation. A local
history acknowledgement advances it, so an older list completion cannot erase
newly saved searches. New list requests can still adopt a current snapshot.
