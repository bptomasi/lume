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

The reader enables pulldown-cmark's table events and captures styled cells with
their original byte mappings. `reader::tables` owns Unicode column widths,
alignment and rounded borders; Ratatui still owns terminal rendering. Prepared
grid rows share the indexed horizontal viewport used by code, with separate
row ranges retaining the ordinary panel background. The inspector reuses cached
rows when available. Selection and annotation project cell text back to source;
decorative borders and padding have no quotation mapping.

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

Sparse row checkpoints bound Unicode work for long code rows. Drawing projects
only visible cells and preserves syntax, search, selection and cursor offsets.
Layout and wrapping run outside input where they are expensive; tests cover
long rows, resizing and libraries larger than the displayed metadata page.

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

Native tests cover public behavior and credible failures. Visual snapshots cover
reviewed terminal cells, styles, themes and sizes. Real HTTP fixtures qualify
network boundaries. Owned Foot and Alacritty windows complement buffer tests;
protocol acknowledgement is not physical key-to-photon measurement.
See [development](development.md), [performance](performance.md),
[configuration](configuration.md) and [dependency policy](dependency-security.md).
