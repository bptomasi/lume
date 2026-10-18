# Architecture

Lume is an independent local terminal application. One owned SQLite library
stores feeds, normalized Markdown and personal state. No server, account,
container or external database is required.

## Responsibilities and flow

Terminal events enter the app's modal controller. Explicit operations update
SQLite and rebuild bounded metadata/reader caches. Ratatui draws those caches
without SQL, HTTP, parsing or syntax compilation. CLI commands compose the
same storage, discovery and exchange contracts.

Fullscreen retains the entire reader frame but centers its text geometry at
80 columns by default. `Workspace::fullscreen_width` is independent of the
panel reading width; commands, settings previews and saved layout snapshots
share its 40–140-column validation. Older workspace/profile JSON defaults to
80. The same clamped, centered rectangle drives wrapping, painting, cursor
placement and mouse hit testing, so margins never become selectable text.
Wheel handling advances one row/item per reported event, including coalesced
bursts. Terminal-side multipliers remain a terminal preference: SGR reports
do not carry the originating hardware notch identity.

The terminal enables `App::enable_background_navigation`. The synchronous App
API remains useful for CLI-like callers, fixtures and source-bound workflows.
Interactive code layout uses `reader::LiveRenderer`: Markdown preparation runs
on a separate worker, then readable plain code admits nonblocking syntax jobs.
One-slot request/result queues limit outstanding jobs. Exact code/language
keys prevent an old article's result coloring another block. Completed styles
enter an approximately 4 MiB LRU; that is a style-retention budget, not a total
process-memory limit. Oversized blocks/results remain plain. Per-document
submission tracking prevents repeated work when blocks exceed the cache.
Dropping the controller disconnects queues without waiting for a grammar;
an already running grammar can finish independently. Worker failure clears
pending work and keeps plain text. The terminal polls admitted work at 16 ms
and rebuilds layout on completed results while preserving readable cursor,
selection and viewport. Unchanged geometry does not reflow. Synchronous
`reader::render` remains available for deterministic noninteractive consumers.

### Interactive navigation and layout ownership

`app::navigation_worker` owns one read connection and returns a bounded page
with the selected article's feed/cache/comments/progress snapshot. Selection
changes immediately; the old readable content is cleared rather than attached
to the new identity. Page boundaries and selected metadata indices are captured
in requests. Results apply only in normal mode without a visual selection.
`reading_article` guards progress ownership while metadata and text arrive at
different times. A bounded 512-entry LRU retains recent distinct admitted
positions keyed by article, source and revision, exceeding the 64-slot writer
queue. Eviction removes the oldest position rather than clearing pending
positions when the memo fills, so a quick revisit cannot restore a read that
raced its pending write.

`reader::layout_worker` prepares width-specific Markdown with cached syntax
styles. It retains the previous layout of unchanged source during reflow;
new documents display preparation state rather than old text. `poll_reader`
uses the existing source/selection mapping invariant when applying styles or
reflow. Explicit quote/highlight/source-history jumps use `render_now` to get
valid mappings immediately, seed the background cache and cancel stale reads.
The structure index and private-comment composition remain on the input owner.

Both replaceable workers use `latest::Latest`: one active job, one channel
slot, one deferred newest replacement and one result slot. Generation checks
reject stale work before execution and before delivery. A stale in-flight
SQLite operation or Markdown parse may finish; this does not preempt either
library. No domain mutation uses the replaceable worker. These two compatible
consumers establish the narrow generic contract; policies remain at their owners.

`app::session_writer` uses a separate connection and a 64-slot ordered queue.
Progress and workspace/history writes use nonblocking admission. A full or
disconnected queue rejects the action with a retry error, rather than silently
dropping an edit. Every preference path uses this same writer, preventing an
older queued layout from overwriting a newer synchronous preference change.
Failures increment a persistent-in-session counter and are surfaced by the
status/error badge; a consumed notification cannot make shutdown report success.
A flush barrier at explicit exit has a five-second admission/completion deadline.
Hard process termination can lose unconfirmed writes. Personal flag/annotation
transactions, catalog reload/counts and exchange operations remain synchronous.

### Split layouts and named workspaces

`model::PaneLayout` is a validated binary tree with four unique pane leaves,
maximum depth three and 15–85 percent split ratios. None keeps legacy column
geometry. `ui::layout::tree` is the sole geometry owner: collapsed children
release their whole rectangle, small splits retain the focused child, and
rendered split boundaries carry their exact tree paths into pointer resizing.
Visible children have at least 20 columns or 10 rows. Row seams use the leading
pane's bottom border, reserving the trailing pane's title/buttons for movement
and collapse. Keyboard resize adjusts the nearest visible focused ancestor by percentage
points; hidden siblings cannot consume the action. Mouse resizing and keyboard
resizing resolve the same rendered tree path. Classic layouts retain columns.
`app::layouts` and pointer docking use the same leaf-removal/insertion operation.

`model::PanelLayout` captures only layout preferences. `LayoutHistory` owns
32 profiles and 32 undo/redo entries; new commits invalidate redo. Reading,
Study and Browse are seeded only for a library without saved profiles. Profiles
never replace the article, navigation destination, source, theme or keybindings.
`store::layouts` validates every entry and atomically commits active workspace
plus history through the existing settings table; no migration is needed.
Native injected-trigger failures verify rollback rather than partial saves.

`ui::key_guide` shows Ctrl-w operations without changing persistent geometry.
Reader selection retains its existing contextual yank/highlight/note guide.
36 checked-in framebuffer fixtures cover three themes, three terminal sizes,
and split/guide/selection/drag scenes. Text plus native style runs preserve
all cell colors/modifiers. Explicit regeneration is for review; ordinary gates
only compare. These fixtures qualify application cells, not physical font paint.

The source coordinator receives refresh/discovery requests through a capacity-one
channel. Four scoped download lanes return bounded parsed feeds through four
result slots; the coordinator alone commits SQLite, one feed at a time. Domains
are assigned wholly to one lane. A capacity-one UI channel reports incremental
progress and completion; stale progress can be coalesced while final completion
is retained. Discovery returns advertised choices without subscribing. The UI
preserves captured draft targets while reloading; the article lane is independent.

## Reader expansion ownership

Source extraction rules are typed `RuleRequest`/`RulePreview` values, capturing
article ID/URL, feed, previous rule and proposed selector. The article worker
downloads/extracts without modifying the rule or successful cache. The app stages
results until normal mode without a selection, then displays a scrollable
read-only document. Confirmation compares rule and article authority again in
one transaction with the cache write. Cancelling is not a source-policy change.
dom_query owns CSS matching; exactly one container is required. Short results
remain readable with a warning; likely consent/browser obstructions fail without
replacing an earlier cache. FeedOrigin is parser provenance, not a completeness
prediction, and legacy entries retain Feed until freshly collected.

`reader::index` scans parser-owned Markdown offsets for headings, code and tables
with a 50,000-event budget. It is independent of terminal width. ReaderTools owns
literal publisher-text matches and at most 128 source-bound jumps. Captured
context relocates only when its original anchor is valid or its text is unique;
ambiguous revisions never navigate to a guessed occurrence. Search ranges are
sorted before drawing, so each visible row visits only intersecting matches.
`reader::motion::WordIndex` caches grapheme-safe word/WORD boundaries when
readable text changes. Counted word destinations use binary searches; cursor
placement binary-searches display-row ranges. Transient Vim counts/prefixes
belong to the app, never storage. Explicit ex commands retain note/favorite
actions where reader motions occupy their former single-key bindings.
Reflow preserves the readable cursor and explicit jumps clear pending progress
restoration. Reload snapshots preserve the exact cursor/viewport only for the
same article, source, revision and body. A cursor in private notes additionally
requires unchanged readable text. Note selection is restored by durable ID. Notebook text never reuses publisher-search ranges.

HTML table boundaries come from HTML5gum. The adapter preserves cells, safe
links and escaped pipes in Markdown, flattening nested tables. It avoids padding
sparse source rows and retains tables over 64 columns as unaligned text. The
dedicated table inspector uses CommonMark cell events, limits aligned structure
to 64 columns/2,000 rows, caps column padding and output size, and falls back to
unaligned source when structure exceeds the limit. Both output paths enforce
a UTF-8-safe 1 MiB budget including truncation notices. Its notices are inspection
UI, outside publisher quotation mappings.

`search` owns the boolean domain grammar composed with nom. Literal leaves
become bound FTS5 expressions; fields compile only to owner-selected SQL. Byte,
token and nesting budgets bound grammar work. Live queries and saved-list text
share parsing/validation; extra saved flags/source groups still combine with
AND. Count and page observe the same read transaction. Grouped URL views choose
representatives from already filtered candidates, ordered by publication time
and ID through a window function. Each occurrence remains stored separately.
Anchored page recovery locates a still-matching article, computes count and
loads its bounded page in one SQLite snapshot with one frozen criteria object.
Query assembly includes only active owner-defined scope/filter predicates,
retaining bound values while exposing direct feed/state predicates to SQLite's
planner. One smart-list query reads only its own definition. Opening an unread
article updates Inbox/Read and read-sensitive smart counts, including nested
boolean predicates. Reopening a read article performs no redundant mutation
or full navigation recount.
Migration V8 adds a covering `(deleted, unread)` index for those read-state
counts. It prevents counting flags from visiting body-bearing article rows.
V1–V7 remain immutable; the new index changes access paths, not owned content.
It never jumps to All or bypasses a saved filter when a refresh shifts pages.
Derived URL/duration metadata backfill uses 32-body pages and a durable marker;
publisher fields and existing migration bytes are never rewritten.

Organization batches capture at most 1,000 identities before mutation. Each
batch validates active targets, records one inverse and commits all changes
together. Independent tag owners distinguish articles from annotations. Sidebar
pins/order/collapse are workspace preferences; source/list identities remain
stable. SinceLastVisit stores a session boundary in the view while comparing
persistent destination identity independently of that boundary. Layout saves do
not advance it; only terminal-session exit does.

Study material belongs to an annotation. Full annotation snapshots retain tags,
related article IDs, review dates and user-authored cards through delete/undo.
Review intervals are explicit days; revealing a card is not completion. Captured
card identity/content/review state is checked before rescheduling. Passage
review previews similarly capture immutable note text and the observed schedule;
stale content, a concurrent review or Trash rejects the action transactionally. Trash is
excluded from due views. Markdown export includes tags, links and cards.
Incremental export reconciles stable filenames against exact previous bytes in
SQLite. A directory lock serializes cooperating exporters; a temporary file is
synced then atomically published before its manifest entry advances. If a crash
separates publication and manifest commit, identical desired bytes repair the
manifest next time. User modifications, symlinks and unowned files conflict
instead of being overwritten. No filesystem deletion is inferred from Trash.
SQLite and the filesystem are not one distributed transaction; callers receive
partial progress/errors and safe repeatable reconciliation.

Settings previews snapshot the workspace, mutate only in-memory visual values,
and save on explicit confirmation; cancellation and exit without confirmation
restore that snapshot. The
default palette incurs no remapping pass. Alternate palettes map owned UI colors
on the completed framebuffer, preserving other syntax styles. Normal-mode key
bindings dispatch discoverable commands; input batching honors them before
coalescing motion. Draft editors retain their own key semantics.

Collection pacing applies to subscription hostnames. Lane-local Retry-After
deadlines defer further requests to that hostname. Server deadlines are also
persisted under owned `collector_retry:` settings keys, independently of feed
IDs; new subscriptions and re-enabled sources observe them after restart. The
longest deadline wins and expired entries are ignored/pruned. Effective source
health merges hostname and per-feed deadlines. Ordinary exponential failure
backoff remains per feed. Conditional validators still
commit with publisher content. No SQLite transaction spans HTTP. Stop is checked
after downloads, before sending results and before each coordinator commit.
Redirects remain
ureq's responsibility; the four-lane bound applies globally, while hostname
pacing is not a global limiter for redirected destinations. Benchmarks separate
setup from timed work and report input-to-TestBackend-frame/open/reflow percentiles;
they do not measure physical Alacritty rendering or network throughput.

- model owns article, annotation, query, saved-search and layout contracts.
- store owns SQL, transactional mutations, migration history, FTS and inverses.
- source owns HTTP limits, conditional requests, feed decoding and discovery.
  Its article adapter separately owns full-page transport and extraction.
- content maps HTML5 tokens to safe Markdown; it does not implement HTML/CSS.
- reader owns CommonMark presentation, source ranges and code highlighting.
- app owns focus, modal editors/selectors, navigation and caches.
- exchange owns bounded OPML and atomic Markdown/OPML file exchange.
- external_editor owns private temporary drafts and process argument parsing.
- ui owns responsive geometry, theme and pure cached-state rendering.
- terminal owns terminal suspension/restoration, browser launch and event I/O.

Dependencies follow these responsibilities. No generic infrastructure crate
or universal DTO is introduced.

### Partial feed bodies

`source::body` normalizes a content candidate before deciding whether it is
usable. Empty, whitespace-only or active-only HTML falls through to the
publisher's summary; usable full content retains precedence. RSS, Atom and
JSON Feed share this policy through feed-rs's model. External Atom content
references do not trigger page requests. Missing content retains the legacy
placeholder contract, avoiding an unnecessary cache rewrite or migration.

`content::body_kind` uses the existing CommonMark parser to distinguish
readable text from links/media only and missing content. It does not estimate
completeness from word counts: even a short summary counts as text. Nested
image/link labels remain references. The classification runs during ingestion
and selected-body loading, never during drawing.

The app caches availability, omits fabricated reading-time estimates for
missing/reference-only bodies, and appends an explanatory browser shortcut.
The notice is generated presentation after the article's source-mapping
boundary; it cannot become a publisher highlight. It is neither stored nor
indexed/exported as publisher content and is omitted from Notebook. Existing
cached link-only entries benefit immediately on reopening. Feed normalization
fixes apply when a publisher supplies a fresh response; HTTP 304 keeps the
existing cached body. Full-page extraction is an independent cache operation.

### Full articles, cache provenance and completion

`source::article` composes ureq, dom_smoothie and the existing HTML5gum normalizer.
Its worker owns a separate HTTP agent, SQLite connection and capacity-one job
and result channels. At most one job runs and one waits at that boundary; the
terminal admits only one article job at a time. Slow feed collection cannot
occupy the article lane. The UI reads Feed immediately and never performs page
download or Readability parsing. Each request captures an article ID and URL;
the commit transaction checks both against the current active article, rejecting
results after URL changes or movement to Trash. No transaction spans HTTP.

HTTP accepts HTML/XHTML, reads at most 2 MiB after decompression and requires
UTF-8. The agent's deadline is 12 seconds with at most five redirects. Final
URLs resolve links. Readability checks 20,000 elements after constructing its
DOM, keeps classes for code metadata and skips JSON-LD metadata unnecessary to
the feed's identity. HTML5gum suppresses active content and normalizes bounded
article fragments. No browser engine, JavaScript, media requests or automatic
authentication are involved. These limits do not impose a hard parser CPU or
memory deadline, and extraction does not prove publisher completeness.

Migration V4 adds `article_documents`, source policy and annotation provenance.
V1–V3 bytes and all publisher/user columns remain unchanged. A successful
cache stores original/final URLs, normalized body, timestamps and a monotonic
revision. Identical text at the same URL keeps its revision/progress; changed
text or URL increments the revision and resets Full progress. Failure updates
only attempt metadata and error text. Feed and Full never overwrite each other.
FTS triggers atomically combine both bodies only when the successful cache URL
matches the current article URL. Publisher refreshes retain that combined index.
Exports include Full provenance and retain the Feed body/discussion reference.

Workspace preference chooses Feed or Full; unavailable Full falls back to Feed.
Only selected bodies are loaded. Full progress belongs to its revision, while
legacy Feed progress stays in `articles`. Delayed saves cannot overwrite the
progress of a newer Full revision. References from link-only Feed content are
retained in the Full link picker, deduplicated in encounter order.

Automatic policy belongs to each subscription: `missing` opens entries without
independent text, `always` includes summaries and `manual` requires `:fetch`.
Fetching starts only with reader focus in normal mode, never for every headline
visited or in Notebook/Trash/demo/offline mode. Existing successful caches are
reused. Failed automatic attempts wait 15 minutes; explicit requests bypass the
cooldown. A session attempt timestamp also bounds retries when failure metadata
cannot be written. `--offline` disables TUI and explicit CLI network operations.

The terminal stages feed/article completion while drafts or visual selections
are active. Results stay durable on disk; normal-mode application checks the
captured target before loading it. Completion for another selection cannot
replace the opened body. An unchanged body retains its exact reader cursor and
page even after failure. A deliberate switch back to Feed during a request is
respected when it completes. Cancellation is cooperative: worker Drop signals
stop, checked after HTTP and before cache writing; a commit already in progress
may complete. Exiting does not join a blocked HTTP request.

Captured `Passage` values carry quote, byte anchor, source and revision through
the draft. Saving Full passages verifies the current URL, revision and exact
source bytes transactionally. A stale save retains the draft. Old comments and
undo payloads default to Feed/zero without rewriting them. Painting filters by
source; after a Full revision changes, only a unique quotation may relocate,
with a revision notice beside the immutable quote. Ambiguous/removed quotes
remain in private notes. Undo preserves all provenance fields.

## Query and rendering bounds

The app loads a navigation catalog and at most 200 article metadata rows.
Moving beyond a page queries the next page; first/last jumps use absolute
matching positions. Every destination queries the complete library. The
legacy store.library API retains its explicitly bounded 10,000-row snapshot
for existing consumers/benchmarks; it is not the TUI's navigation contract.

SQLite FTS5 indexes title, source, author, normalized body and annotations.
The bounded boolean grammar compiles literal terms into parameterized FTS
subqueries. Prefixes and exact token phrases can combine with AND, OR and NOT;
field, state, date, tag and duration predicates retain their own typed meaning.
Saved-list text and the current query intersect with the saved unread, favorite,
source and publication-age predicates. FTS evaluates terms into an ID set,
intersected with newest-first metadata.
The selection predicate short-circuits FTS evaluation for empty searches.
A directly joined FTS plan was measured and rejected for the broad-prefix
interactive workload; it increased typing-cycle latency.
Count and page queries share owner-local selection predicates and one read
transaction, so concurrent collection cannot give them different snapshots.

Only the selected body and annotations are fetched. Markdown layout produces
styled lines, code markers, safe links and UTF-8 source ranges. Prose wraps by
terminal cell width. Code preserves columns and scrolls horizontally.
Very large code blocks fall back to readable plain code. Reviewed grammar
text compiles during the build; the executable loads only its own embedded
generated grammar pack. No downloaded or feed-provided grammar is loaded.

The content adapter buffers each HTML `pre` as literal code. Language metadata
on `pre` or its `code` child survives normalization; child metadata takes
precedence. A fence longer than any literal tilde run keeps publisher text
inside the block, including indentation and trailing spaces. The reader
normalizes familiar language aliases before selecting an embedded grammar.
The build checks cross-grammar references, including embedded expressions.
The grammar set and dark palette are shared for the process lifetime;
highlight state belongs to one block so multiline comments retain context
without leaking state into the next block. Unknown languages, blocks over
64 KiB and lines over 4 KiB retain plain code. Parser errors fall back on the
affected line. These are input bounds, not a hard CPU deadline for regex work.

The reader caches layout until content or width changes. Drawing builds only
visible rows. Editors retain their native cursor, selection and bounded undo
history; rendering borrows the editor rather than cloning its history.
The event loop redraws after input, resize or completion and otherwise polls
at 100 ms, or 16 ms while reader/navigation/motion work is pending. Interactive
navigation reads, regular Markdown reflow and progress/preferences writes have
bounded workers. Explicit source-bound jumps, personal transactions, catalog
counts and exchange operations can still delay input. No constant-latency
claim follows from the page or channel limits.

### Panels, pointer capture and article annotations

`model::Pane` is the shared identity for focus, visibility and saved order.
Workspace JSON defaults preserve older preferences; no schema migration is
needed. Stored layouts reject duplicate/missing pane identities and invalid
widths. `ui::layout` owns geometry for drawing and hit testing, including
responsive suppression, toolbar restoration buttons and picker rows. Every
pane can remain collapsed. Fullscreen/Zen use Reader temporarily; expanding a
pane exits those modes without losing saved order or widths.

`app::panels` owns layout commands and Ctrl-w equivalents. Pointer gestures
reuse those commits. Divider motion snapshots only width preferences, previews
without SQL writes and saves on release. Esc, focus loss and terminal resize
cancel the gesture and restore provisional dimensions. Article reload cancels
only text selection; layout gestures are independent of source content.
Title/corner-grip moves capture stable pane and toolbar hit regions, preserve
focus during capture and expose a typed preview for highlighted destinations.
Saved order changes only on release; collapsed toolbar tabs also accept drops.
Keyboard moves prioritize visible neighbors and report edges without rewriting
unchanged preferences. Standalone modifier reports do not consume Ctrl-w;
shifted base characters and Alt-arrow alternatives use the same move owner.
Reader selection captures article/source/revision
and clamps pointer cells to the original reader column, using existing Unicode
and horizontal-code geometry. Source changes invalidate capture. Publisher
selection survives width changes even when private comments rewrap differently.

Crossterm owns mouse protocol commands/parsing. The terminal guard balances
capture on start, requested preference changes, external-editor suspension,
resume and exit, including partial writes. Failed capture reports keyboard
fallback; status records observed input rather than inventing a terminal
capability acknowledgment. Input batches drop idle motion before dirtying a
frame, coalesce compatible drag/wheel bursts and preserve following commands.
Modal wheel input stays in the overlay; background panes cannot receive clicks
through it. Regular reflow and session writes use the bounded workers described
above; explicit source-bound workflows retain synchronous qualification paths.

`app::annotations` derives position-ordered entries when comments reload,
without per-frame SQL or source scanning. Current-source quotes use the same
exact-anchor/unique-relocation contract as highlights. Alternate sources are
separately labeled and validated upon jumping; unresolved quotations and
non-positional comments remain saved. `ui::annotations` wraps visible cards
only. Enter/click uses existing source-bound jump history; note editing,
deletion and undo reuse annotation transactions rather than a second store.

### Pointer-following pane pictures

`ui::draw_interactive` paints the regular themed workspace and captures the
held pane plus its visible neighbors once, before drawing destination hints.
Each picture includes its complete rounded outline, title and controls.
Movement begins after two columns or one row; a click focuses without saving
layout history. Panes wider than three quarters of the viewport or beyond the
20,000-cell capture limit use a 38-by-9-cell summary card. Its visual footprint
is independent of the full-pane collision footprint. A thin seam marks the
destination; explicit top/bottom content edges request row docking.
Ratatui owns cell storage and terminal diffs. Each pane is limited to 20,000
snapshot cells; the whole captured workspace is limited to 40,000 cells.
Oversized captures keep the existing destination-hint fallback.

`app::pointer::reorder` owns stable original slot geometry and candidate
selection. Body targets use the moving panel's leading edge rather than the
grip position. A crossing enters at 25 percent of the smaller pane surface
and remains selected until below 12.5 percent, accounting for vertical overlap.
The farthest crossed slot in the movement direction wins; already crossed
slots stay eligible at viewport edges. Explicit top-tab hits still take
priority, and a pointer outside the viewport cancels the candidate. Animated
neighbor positions never feed back into this calculation.
`app::panels::reordered` computes the same insertion order for the transient
preview and the eventual durable commit, retaining collapsed pane positions.
`app::pane_motion::Neighbors` projects that order onto the original visible
pane widths. Neighbor pictures slide into their projected slots, leaving an
empty insertion gap, while the held picture follows the pointer in both axes.
Hit testing retains the original pane/tab regions so moving images cannot
oscillate their own targets. Previewing performs no SQL or article reflow.

The compositor clears the captured workspace region, draws the insertion
hint and neighboring pictures, dims the backdrop to 60 percent RGB, and draws
the held picture above it. While held, its neutral surface also uses the dim
canvas color, with foregrounds and non-neutral backgrounds (code, selections
and controls) retained. Every captured cell overwrites background content.
Terminal backgrounds fill rectangular cells; making only the four corner
cells transparent leaves whole-cell notches, and an opaque bright surface
makes a square plate behind the rounded glyphs. A uniform dim neutral surface
lets the complete bright outline define the curved boundary instead. After
release, the neutral picture surface returns to its captured color, matching
the restored canvas. No pixel-level clipping or graphics protocol is assumed.
The normal canvas and pane surfaces share one background in each theme.
Partial wide glyphs at a clipped right edge become blanks to prevent autowrap.

`app::pane_motion::Slide` owns 160 ms elapsed-time cubic translation in both
axes. Changing the target starts from the currently displayed position;
repeating the same target does not restart the clock. Neighbor transitions
run at 16 ms polling while needed, including while the mouse is held still.
Finished held previews become idle. Delayed ticks jump ahead without backlog.

On release, the final order commits immediately and the captured pictures
continue toward the actual committed slots. A stationary held picture remains
visible until its neighbors finish. A drop without a destination restores the
original arrangement without writing layout state. New input, focus loss or
resize cancels motion; changed pane dimensions use actual reflow rather than
stretching frozen text. There is no generic effect graph or background task.
Native tests use controlled time rather than sleeps.

The header has identity at row zero and pane tabs at row one; body geometry
starts at row two. Drawing, pointer capture and tests share this geometry.
The removed breadcrumb no longer consumes a reading row.

### Precise reader selection

The reader retains readable text before soft wrapping, each display row's
range in that text, and CommonMark token-to-source mappings. Application
cursor state uses a display row and a grapheme-aligned byte; visual anchors
use readable-text offsets. Horizontal movement uses Unicode Segmentation;
vertical movement retains a desired cell column measured with Unicode Width.
Four-column tab stops are shared by cursor geometry and visible-row drawing.
Reflow restores the readable offset, keeping the selected bytes unchanged.
Panel switching uses Tab while reader h/l move the cursor.

Yank slices readable text, preserving actual newlines and omitting soft-wrap
breaks and Markdown delimiters. It returns a typed Copy action; only the
terminal runner writes OSC 52 through the explicit base64 dependency. Drawing
cannot access the clipboard. I/O failures retain selection; successful writes
clear it and report a request rather than an unobservable clipboard acknowledgment.
No clipboard read or external command is involved. Payloads over 64 KiB fail
before writing, without truncating content or evaluating selected text.

Annotation capture projects selected readable bytes to the original source;
the existing quote/anchor transaction and undo history own persistence.
Entity/escape transformations retain an entire parser token when byte-exact
projection is impossible. Inline code narrows to its contents when they occur
verbatim. No migration or alternate Markdown parser is needed. Visible marks
prefer the exact source anchor; if it is invalid, relocation requires one
unique source quotation. Removed/ambiguous quotations remain saved in the
notebook. Highlight ranges are merged, then applied during reflow in source
order, retaining syntax colors. Only visible rows receive transient selection
and cursor overlays during drawing; selection bounds are computed once per frame.

### Source reading groups

Catalog installation imports computing, security, AI and infrastructure OPML.
`catalog::groups` resolves source-group URL references from a typed JSON asset
to durable local subscription IDs. Missing, empty, duplicate or oversized source
references fail explicitly, never becoming an unrestricted query. A SmartFilter
contains each source union; existing criteria still combine with AND. Empty
source groups retain legacy semantics through a serde default. SQLite's
bundled JSON table function binds at most 128 positive source IDs as data,
without interpolating SQL. Count/page retain their shared read transaction.
The smart form retains the source group when editing other criteria and
displays its size. Installation preserves existing query criteria and personal
data; the project launcher versions its installation marker so an existing
library receives new subscriptions once. New articles enter their source-group
view through their source identity, without a copy or an ingestion-side policy.

## Article deletion and cursor visibility

Migration V3 adds a checked deletion flag and a visibility/order index while
retaining publisher identities and the immutable V1/V2 migrations. Trash is a
query destination, not a second collection or a copy of an article. The shared
count/page predicate selects deleted rows only for Trash and active rows for
all other destinations, including retained Inbox selections and smart lists.
Legacy metadata snapshots and global-jump positions use the same visibility
policy. Explicit single-article reads/exports still accept Trash identities;
Notebook export follows its normal active-only destination.

`store::deletion` owns transactional flag changes, idempotence and their undo
records. A dedicated history variant keeps old flag snapshots compatible and
prevents favorite/read/queue undo from changing deletion state. A failed history
write rolls the visibility update back. Publisher upserts never write the
flag, so refreshes can update content without resurrecting a deleted identity.
Notes, memberships and reading state remain in their existing tables. Recovery
clears only the deletion flag, preserving subsequent publisher corrections.

`app::deletion` routes ex commands and captured palette actions through that
contract, invalidates a retained target and reloads counts/pages. The palette
captures article IDs before background reloads. Empty views, duplicate requests
and stale saved selections have explicit behavior. Normal-mode D/Delete is
ignored in the library and during passage selection; reported autorepeats are
ignored, while editor Delete events retain text-editing behavior.

`ui::reader_text` paints one ivory cursor block, independent of amber selection
and saved highlights. It resets dim/reverse styling on that grapheme, preserving
other syntax spans and Unicode geometry. The reader never requests a hardware
cursor at the same cell: terminal block-cursor inversion would otherwise cancel
the painted contrast in Alacritty. Ratatui hides the hardware cursor during
frames and restores it when the terminal is dropped. A real Crossterm backend
with a captured writer qualifies hide/show bytes without relying solely on a
buffer screenshot. The preview comes from the same cached Ratatui drawing path.

## Durable personal state

Publisher identity is (feed_id, external_id). Ingestion changes only publisher
columns. Favorite/read/queue state, comments, quote snapshots, reading anchors
and memberships survive publisher refresh. Validators advance in the same
transaction as content; failed ingestion cannot advance them. A 304 retains
content and clears the prior error. An unchanged source title is not rewritten,
avoiding redundant FTS updates of every article from that source.

FTS triggers keep indexing in the transaction that changes an article, feed
title or annotation. A migration backfills existing data. Foreign keys and WAL
are configured on every owned connection; the busy timeout is 500 ms.

Personal mutations write inverse snapshots into a persistent 256-operation
journal. Applying the inverse and removing its record commit together.
Failures roll back both. Publisher refresh, workspace preferences, subscription
changes and saved smart-query edits are outside that journal. A text editor's
Ctrl-z history is separate from normal-mode durable undo.

Membership replacement validates the article and commits every selected list
together. Removing a list deletes its memberships but preserves article data;
the journal can restore the list and all memberships. Unsubscribing disables
collection and retains cached articles. Article/comment identities guard note
edits and deletes. An editor or selector captures its article target before
background reloads; a validation failure keeps the draft.

Quote snapshots never change when a publisher changes its text or a user
edits the associated note. An absent passage is marked as changed rather than
reattached to unrelated content. Visual selection maps displayed lines back
to original normalized Markdown bytes; transformed entity/escape tokens retain
their complete source range.

Workspace preferences retain pane visibility, widths, density, destination,
page and article identity. Reading progress stores a source-byte anchor rather
than a wrapped-line number. The controller retains that anchor until actual
terminal dimensions arrive. Inbox reading retains its newly read article
until the user changes views/search; session restoration retains that pin.

## Schema lifecycle

The original version-one bootstrap remains byte-for-byte unchanged.
Refinery adopts it with contiguous immutable V1/V2 migrations, storing names
and checksums. Grouped execution makes history and schema/data changes atomic.
Unsupported future user_version values fail without resetting data.
The migration adds personal workspace state, journal and FTS while preserving
old articles, list memberships and comments. Future evolution needs a new
migration, never modification of an executed migration.

## Exchange, lifecycle and failures

HTML5gum owns attribute/entity tokenization and raw-text state recovery.
Active content is suppressed; links resolve against validated HTTP(S) bases.
Pulldown-cmark owns Markdown parsing. Terminal controls are removed from
displayed content and metadata. Images and CSS cause no network requests.

OPML uses quick-xml/Serde with depth, node, size and subscription bounds.
All URLs validate before one import transaction. Nested folders flatten into
subscriptions; they do not manufacture article-list semantics. Discovery is
bounded and uses the first advertised base URL. Subscription is an explicit
selection after discovery.

Markdown exports stream bounded article pages with source links, immutable
quotes and notes. Temporary files are private and in the destination directory;
completed files publish without replacing an existing file. Export queries
are individual SQLite snapshots, not a global export transaction across all
pages; another process editing the library concurrently can affect the result.

External editing suspends the TUI, creates a private temporary Markdown file
and runs parsed program arguments without a shell. The terminal resumes even
after an unsuccessful editor exit. Invalid/oversized output leaves the old
draft available. Browser launch passes a validated URL as one argument and
reaps the child separately.

HTTP accepts only HTTP(S), rejects embedded credentials, validates certificates
with native roots and an explicit Ring provider, limits redirects/time/body
size. A single HTTP attempt is not retried internally; reader policy applies
the separate article cooldown described above. A feed failure is stored separately;
other feeds still commit. There is no daemon or periodic scheduler.

Dropping the worker signals cancellation, checked between requests, after
download and before each coordinator commit. Requests have a 12-second
deadline; exit does not wait for an in-flight request.
SQLite commits or recovers complete transactions. Terminal cleanup restores
raw/alternate-screen/bracketed-paste modes on ordinary errors and unwinding.

Performance measurements and environment restrictions belong to validation.md.


## Curated sources and command discovery

`catalog` owns the opt-in source selection. Bundled OPML is its canonical
subscription asset; a separate typed JSON asset defines literal saved searches.
Installation uses Store import contracts, preserving personal state and existing
search criteria. Subscription import is atomic; searches commit individually,
and retry can finish a partially installed selection. Reinstallation explicitly
re-enables selected disabled feeds. `starter` performs no network requests.
The root launcher selects the workspace-local library and installs once;
normal TUI startup or explicit `sync` owns collection.


The selection contains 98 sources and 46 smart lists. Six source-ID groups
cover Security, Artificial intelligence, Platform engineering, Kernel and
systems, Networks and dataplanes, and DevOps and delivery. Future posts enter
through subscription identity, without keyword matches. Source groups are
broad publisher collections, not semantic article classification. The other
40 lists use literal full-text prefixes. OPML owns subscriptions; the group
asset owns membership references, allowing the same source in several groups
without subscribing twice. Existing named groups retain customized criteria.
OPML group headings and descriptions are English editorial metadata, not
article-folder mappings or fetched article translations.

`catalog::upgrade` owns exact historical label mappings. It plans names using
the current saved-search snapshot and commits conditional renames together.
Store compares the observed name and durable ID before each update, rolling
back the batch on stale state or destination conflicts. IDs, criteria and
saved workspace destinations remain stable. When an English name exists,
the historical list receives an available English `(imported)` suffix; both
queries survive. Retry sees no legacy labels and adds no duplicate. Other
user-created names are not translated. An exact old archive feed label has
its own conditional update, preserving later publisher titles and article data.
A launcher version marker applies the new selection once per workspace library.

`app::completion` owns ex-command syntax, descriptions and compatible aliases.
The dispatcher normalizes aliases against the same registry used for matching.
The editor retains cursor state, argument bytes and a suggestion index; edits
reset that index. Tab replaces only the verb and never executes a command.
Enter completes an incomplete verb before executing; missing arguments and
command errors retain the draft. Matching work depends on the fixed command
registry, never on the number of articles. Rendering uses this registry and
cached application state without SQL or HTTP.

Personal folders are the existing collections and memberships, with unchanged
storage identities and undo semantics. They are flat article destinations;
smart lists remain queries and OPML groups remain subscription-file structure.
The folder browser works without a selected article. Saving/organizing captures
the selected article identity; an article can belong to several folders.
Deleting a folder preserves articles, notes and favorite state. No schema
migration or reclassification of existing user data is necessary.


`View::Read` is a derived built-in destination over the existing unread flag.
The query owner uses the same scope predicate for its count and bounded pages,
including articles older than the normal metadata page. Marking read does not
create collection membership, so undo, publisher refreshes and marking unread
cannot leave a stale copy in another folder. Explicit read commands are
idempotent: repeating the same state does not consume an undo entry. Opening
an article still marks it read and retains the open inbox selection; manual
state changes release that temporary retention. Existing read articles appear
in the destination immediately, without a migration or backfill.


Fullscreen is a workspace presentation preference layered over the saved panel
and Zen settings. It does not overwrite pane visibility or the prose-width
preference. Its persisted boolean defaults to false for older workspace JSON;
no schema migration is needed. The controller captures the previous focus for
same-session restoration and restores reader focus when reopening fullscreen.
Normal focus navigation exits fullscreen; modal Esc handling takes priority
over leaving it, preserving drafts and visual-selection semantics.

`ui::workspace_areas` assigns the complete terminal rectangle to the reader in
fullscreen. `workspace_reader_content` supplies the same width/height to the
terminal's reflow step, reader renderer and preview exporter. This prevents
clipped lines caused by rendering at a different width from cached Markdown.
The existing source-byte anchor retains the current passage during reflow.
Application chrome is hidden; the article frame and progress row keep the
exit hint and recoverable status visible. Rendering continues to perform no
SQL, HTTP or Markdown parsing.


## Keyboard latency and release lifecycle

The terminal adapter drains at most 256 immediately available events before
application work. `app::InputBatch` retains event order and consults the current
mode for each dispatch. Adjacent normal-mode vertical motions become one
clamped movement, loading only the final article/page. Press/release pairs
retain distinct taps; a pending release cancels unconsumed autorepeats. Draft,
search and picker text retain every event. Later commands are never flushed
as a side effect of discarding repetition. Each batch produces one frame,
while input arrival wakes the event poll immediately.
Reader word and horizontal repeats also honor reported releases. Pending Vim
counts/prefixes bypass vertical coalescing so `3j` remains a counted command.

Navigation counts belong to the loaded catalog snapshot. Changing views does
not mutate the catalog, so `set_view` reuses those counts; mutations and worker
completion continue to rebuild them through reload. Boundary motion performs
no redundant progress write or article load. This removes full-sidebar FTS
counts from each repeat without introducing another cache invalidation path.

The existing Crossterm 0.28.1 library owns detection and keyboard-protocol
parsing; no custom wire decoder was introduced. The pinned library's event
flags and official example were reviewed alongside the
[protocol specification](https://sw.kovidgoyal.net/kitty/keyboard-protocol/)
and [Alacritty 0.15.1 changelog](https://github.com/alacritty/alacritty/blob/v0.15.1/CHANGELOG.md).
Enable disambiguation, alternate-key reporting, event types and all-key
reporting after raw-mode setup. Alternate codes preserve shifted text and
shortcuts such as `:`, `Z` and `G` under all-key reporting.
The runtime guard tracks the active push and pops it on cleanup. External
editing pops before terminal suspension, then reapplies after successful
initialization; failed resume cannot double-pop the caller's protocol stack.
Unsupported detection falls back to legacy input. Legacy events contain no
physical release time, so precise release cancellation is unavailable there.
One already-running SQLite/reflow/draw operation is not preempted; the reader
makes no hard real-time guarantee.
