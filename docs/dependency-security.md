# Dependency security and selection

Lume is an independent desktop/terminal reader. It does not require Miniflux,
PostgreSQL, containers, an account, or a hosted service. Domain data stays local.

## Inline article images (2026-10-19)

### Native Foot Sixel rows (2026-10-19)

Reuse the already pinned **ratatui-image 8.1.1** Sixel encoder and its existing
**icy_sixel 0.1.3** transitive dependency. Installed Foot **1.21.0** documents
native Wayland and Sixel support. The published encoder source was reviewed:
`Sixel::new` accepts pre-sized pixels and its fixed widget owns placement.
References: [ratatui-image](https://github.com/ratatui/ratatui-image),
[upstream Foot README](https://codeberg.org/dnkl/foot), and the installed
`foot(1)`/`foot.ini(5)` contracts matching the qualified terminal version.

The existing widget does not support arbitrary top clipping of a complete Sixel
image. Lume therefore supplies already fitted, cell-height image strips to that
encoder on the background worker. It implements no image parser, quantizer or
Sixel wire protocol. Encodings retain an 8 MiB per-image budget and share a
32 MiB prepared cache; four visible images are admitted for native inline use.
Cached scroll reuses strips without another decode, resize or quantization.
The encoder bins RGB channels into five-bit values before emitting integer
color percentages (`icy_sixel` quant.rs/tosixel.rs). It preserves pixel geometry
but cannot promise original 24-bit color precision. Qualification checks fixed
fixture colors after this quantization, allowing one value for decoder rounding.
Per-strip palettes may vary in photographic gradients. Known colors, exact
source-row clipping, malformed viewports, menu lifetime and source-based zoom
require native behavior tests and real Foot qualification.

Crossterm's existing synchronized-update API owns frame envelopes. No new crate,
feature or license is selected. The user requested removal of Kitty-specific
rendering and Überzug++ integration. The X11 adapter and its three exclusive
packages, x11rb, x11rb-protocol and gethostname, are removed from Cargo.lock and
the image license inventory. Unrelated locked packages remain unchanged.

Select `image` **0.25.10** (MIT OR Apache-2.0), with only PNG, JPEG, WebP and
GIF enabled. The multi-contributor image-rs project supplies format detection,
decoder limits, first-frame decoding and aspect-preserving resizing. Its
published API/source and security page were reviewed. Disable default formats,
Rayon, AVIF and native codecs to bound the dependency and runtime footprint.
This review is not a completed advisory audit. Remote bytes remain untrusted;
Lume enforces compressed-byte, dimension and pixel budgets before decoding.
The decoder allocation setting is best effort, not a process memory sandbox.

Select `ratatui-image` **8.1.1** (MIT), the compatible Ratatui 0.29 integration.
The active Ratatui community maintains newer releases, but those require a
different Ratatui version; upgrading the terminal/editor stack is outside this
capability. Review the published halfblock encoder and use only its fixed,
bounded buffer output. Disable default features, Chafa, Tokio and terminal
probing. Terminals without an explicitly supported native protocol receive
ordinary truecolor halfblocks. No external image helper is required.
Resize/encoding occur on a dedicated worker, with clipping performed from cached
cells during drawing.

Select RustCrypto `sha2` **0.11.1** (MIT OR Apache-2.0), without default features,
for stable URL-derived cache filenames. The multi-maintainer RustCrypto
implementation replaces custom hashing. This cache key is not authentication
or a claim of publisher integrity. Retain pinned versions and dependency notices.
Existing ureq/Rustls and tempfile own bounded HTTP and atomic cache publication.
Lume owns loading policy, cache eviction, offline behavior and stale results.

SVG uses Linebender `resvg` **0.48.1** (MIT OR Apache-2.0), with text enabled,
without system font discovery, memory-mapped fonts, SVGZ or embedded raster decoders.
The maintained multi-contributor SVG ecosystem owns paths, styles, text shaping
and rasterization. Its published usvg resource resolver explicitly allows local
files by default: Lume replaces both string and data resolvers with rejection.
`roxmltree` **0.21.1** (MIT OR Apache-2.0), also used by usvg, owns XML parsing
with DTD disabled and a node budget. Lume additionally caps SVG bytes and paints
only a thumbnail-sized surface. Scripts, external styles, linked images and
data-URL images are not loaded or executed. Only SVGs containing text load one
fallback font, on the worker, with a 4 MiB file budget and reuse across images.
Known OS font paths and explicit `LUME_SVG_FONT` replace an unbounded system font
scan; publisher strings can never choose a font path.
OS/user fonts are read in place and are not bundled or copied into the cache.
The resvg/usvg API and resource resolver contracts were checked before use:
[resvg](https://docs.rs/resvg/0.48.1/resvg/),
[resource resolver](https://docs.rs/usvg/0.48.1/usvg/struct.ImageHrefResolver.html),
[XML limits](https://docs.rs/roxmltree/0.21.1/roxmltree/struct.ParsingOptions.html).

Authoritative references: [image-rs](https://github.com/image-rs/image),
[decoder limits](https://docs.rs/image/0.25.10/image/struct.Limits.html),
[compatible widget manifest](https://github.com/ratatui/ratatui-image/blob/v8.1.1/Cargo.toml),
[halfblock encoder](https://docs.rs/ratatui-image/8.1.1/ratatui_image/protocol/halfblocks/index.html),
[RustCrypto SHA-2](https://github.com/RustCrypto/hashes/tree/master/sha2),
[Alacritty graphics request](https://github.com/alacritty/alacritty/issues/910).

## Local recommendations (2026-10-19)

Page hydration also reuses bundled SQLite's documented
[materialization hint](https://www.sqlite.org/lang_with.html#materialization_hints).
The ordinary article query first materializes its ordered, limited identities;
metadata and comment counts then join that bounded page. Counting still observes
the same transaction as pagination. Recommendation position ordering remains a
separate qualified query. No schema or dependency change is required. Existing
query/failure tests and repeated representative/stress navigation measurements
qualify the changed plan; a hint alone does not establish a speed improvement.

Reuse pinned bundled SQLite FTS5, rusqlite and the existing nom boolean query
compiler. The [FTS5 contract](https://www.sqlite.org/fts5.html) documents phrase,
prefix, column and rowid-constrained matching. Publisher-only compilation omits
the separate annotation index; recommendation filters cannot search private
notes. Bounded rowid ranges and correlated lookups constrain classification to
captured batches, preserving the exact outer identity set.
No tokenizer, parser, vector database, model runtime or dependency update is
introduced. The existing license, maintainer and security assessments apply.

The consequential policy choice is stable, explicit interest matching and
bounded evidence weights, rather than comparing BM25 scores across unrelated
interest queries or treating vocabulary frequency as editorial quality.
SQLite supplies matching, not personal preference or content authority. Lume
owns profiles, topic feedback, canonical grouping and bounded diversity. A
cached full article is eligible only while its captured URL remains current.

Reuse pinned pulldown-cmark to observe actual code-block events and the existing
content-availability/reading-time contracts. Code, short text and announcement
wording are weak observed features; none proves correctness or depth. Initial
interests are explicit, editable vocabulary, not an automatically trained model.

Rust standard-library bounded channels supply ordered preference/feedback
admission and separate result acknowledgement. Disposable refresh requests are
coalesced, while admitted writes are never replaced. Lume owns retries after
rollback, post-commit acknowledgement and shutdown draining. There is no async
runtime, subprocess model, network recommendation provider or extra service.
Behavioral/failure tests and representative/stress workloads qualify these
contracts; actual recommendation usefulness still requires user feedback.

## Indexed annotation notebook (2026-10-18)

Reuse bundled SQLite FTS5 through pinned rusqlite for independent note indexing,
Unicode prefix/phrase matching and `snippet` highlighting. The
[official FTS5 contract](https://www.sqlite.org/fts5.html) covers external-content
triggers, tokenizer behavior and snippets. Reuse the existing bounded nom grammar;
Lume owns field semantics, same-note correlation and bound SQL parameters.
Article text and annotation text now have separate indexes, avoiding a complete
publisher-document reindex for every private edit.

Enable `tui-textarea` 0.7.0's existing `search` feature, using its published
`set_search_pattern`, `search_forward` and `search_back` contracts and native
highlight rendering. The cached published source and
[upstream search example](https://github.com/rhysd/tui-textarea/blob/v0.7.0/examples/search.rs)
were reviewed before integration. This adds a lockfile dependency edge to the
already locked Rust regex crate; no package version or accepted license changes.
The existing maintainer, community and compatibility assessment below applies.
Regex patterns are limited to 256 bytes and note buffers to 16 KiB. Adding
another editor/search engine would duplicate capabilities already qualified here.

Use pinned pulldown-cmark 0.13.0 with `Options::ENABLE_WIKILINKS` to parse actual
Markdown references; code examples never become graph edges. Its published
parser and LinkType contracts own syntax. Lume owns permanent note identities,
reference limits, active navigation and restoration after deletion.

The standard library's
[bounded channels](https://doc.rust-lang.org/std/sync/mpsc/fn.sync_channel.html)
and the existing latest-read adapter cover ordered mutations and replaceable
search/context reads. Writes never use the replaceable-read adapter. Captured
identities, admission errors, transaction rollback, bounded SQLite busy retries
and shutdown durability remain application responsibilities. No async runtime,
custom protocol/parser or new generic worker framework is introduced.

Continue using tempfile and the existing exact-byte export reconciliation for
per-note Markdown files. Keep independent note manifests and permanent filenames.
Fixed review intervals are transparent user choices, not a custom adaptive
retention model or a claimed implementation of FSRS.

## Modal note writes (2026-10-18)

Reuse pinned `tui-textarea` 0.7.0 for editing, Unicode motions and undo, following
its [upstream Vim example](https://github.com/rhysd/tui-textarea/blob/v0.7.0/examples/vim.rs)
and the verified example/method contracts in the cached published crate.
The existing maintainer/license/compatibility assessment below still applies;
no dependency version, feature or accepted license changes are needed.
Adding a second editor or a general Vim emulator would duplicate text ownership
without a requirement for a full editing language.

The [official Neovim editing reference](https://neovim.io/doc/user/editing/)
defines write/quit, all-buffer variants, modified-buffer guards and ZZ/ZQ.
Lume adapts these to one captured note/card editor: closing that editor returns
to reading, while all-quit variants exit the application. Application policy
owns the bounded Ex vocabulary, dirty checks, transaction-returned identities
and session flush. Native storage/failure tests and representative/stress
write benchmarks qualify the integration; this is not full Vim emulation.

## Contextual article notes (2026-10-18)

Reuse pinned Crossterm 0.28.1 for mouse motion and keyboard event kinds,
Ratatui 0.29.0 for bounded popup painting and pulldown-cmark's existing source
mappings for passage positions. The installed Crossterm source's
`event::EnableMouseCapture` enables all-motion reporting (DECSET 1003) alongside
its existing SGR encoding. No custom terminal decoder or additional capture
command is introduced. Keeping the existing terminal guard preserves balanced
suspension and cleanup. Local contracts own event coalescing, drawing only on
hover changes, safe quote relocation and captured mutation targets.

Notes use the existing SQLite annotation/undo transactions and editor; no schema,
dependency, feature or license changes are needed. Native interaction tests
exercise actual storage and Ratatui buffers. A focused benchmark checks hover
burst reduction and reversible deletion with representative/stress annotations.

## Local installation and SQLite migration (2026-10-18)

Keep runtime path resolution in Rust's standard library and CLI parsing in the
already pinned Clap integration. The installer uses Python 3's standard-library
`sqlite3.Connection.backup`, whose
[documented contract](https://docs.python.org/3/library/sqlite3.html#sqlite3.Connection.backup)
supports backing up databases concurrently accessed by other clients. SQLite's
[online backup API](https://www.sqlite.org/backup.html) supplies the committed
snapshot semantics. A raw file copy cannot supply that contract for an active
WAL database. No custom SQLite parser, subprocess SQLite CLI, Rust dependency,
Cargo feature or license policy change is introduced.

Python and its system SQLite are build/install tooling, neither linked into
nor bundled with the installed executable. Their versions follow the host's
supported packages; the benchmark records both. Native application SQLite
remains bundled through the pinned rusqlite integration. The installer owns
source selection, the backup deadline and publication without overwrites.
Failure and WAL tests qualify those boundaries using actual SQLite files.
Installed notices retain the existing product, grammar and extraction licenses.

## Published registry lock repair (2026-10-18)

Restore `utf-8` 0.7.6 as a transitive dependency of published `tendril` 0.5.0.
Cargo's existing registry index requires this edge; resolving the current
manifest adds only that package and preserves every previously locked version.
The crate has no dependencies or feature switches. Its checksum is pinned in
Cargo.lock, and its MIT OR Apache-2.0 licenses fit the existing permissive
policy. Both upstream license texts are retained in the extraction notices.

The [upstream manifest](https://github.com/SimonSapin/rust-utf8/blob/master/Cargo.toml)
identifies version 0.7.6, Simon Sapin as author and those licenses. The upstream
repository is archived; this is a compatibility requirement of the pinned
HTML parser rather than a new decoder selection. The addition does not
establish an advisory audit or an active security-response guarantee.

Earlier source substitutions are not a qualified registry baseline. The local
source labelled `tendril` 0.5.0 omitted this published dependency, so equal
package/version lists did not establish equivalent manifests or source.
Release qualification must use published crates with their registry checksums,
without path patches or rewriting a substituted lockfile into registry form.
Keep `--locked` in build and verification commands. Source-substituted test
results can describe that experiment only; they cannot qualify a normal
registry build. No product code, direct version pin or accepted license changes.

## Real terminal qualification (2026-10-18)

The terminal qualification harness uses Python's standard library and existing
Alacritty/X11 tools, instead of adding an embedded terminal emulator or a Rust
GUI automation dependency. Alacritty owns protocol handling and rendering;
xdotool supplies XTest input, xclip independently reads the requested clipboard,
and xwd records the real window. These unmodified OS test tools run separately
and are neither linked into nor bundled with Lume. Their distribution remains
with the supported Ubuntu packages; this does not expand accepted product
dependency licenses. Python owns subprocess timeouts and isolated test paths.
The [Alacritty configuration contract](https://alacritty.org/config-alacritty.html)
defines the isolated zero-padding geometry and one-event scroll multiplier.
The official [upload-artifact v4.6.2 release](https://github.com/actions/upload-artifact/releases/tag/v4.6.2)
was checked; CI pins its verified commit and retains evidence for seven days.
No Rust dependency, feature or lock graph changes for these checks.

## Background navigation and split workspaces (2026-10-18)

Retain the already qualified Ratatui, Crossterm, rusqlite, Serde and Markdown
versions. Rust standard-library threads, atomics and bounded sync channels
supply the worker primitives; no executor, generic task framework, layout
engine or snapshot-testing dependency is added. The authoritative
[bounded channel contract](https://doc.rust-lang.org/std/sync/mpsc/)
and [Ratatui layout/rendering contracts](https://ratatui.rs/concepts/rendering/under-the-hood/)
were consulted. Ratatui still owns rectangle allocation, buffers and terminal
diffs; Lume owns the bounded four-pane tree and docking policy. Serde owns
profile serialization. Existing SQLite transactions own atomic profile/history
commits. Latest-request replacement applies only to reads/pure layout work;
ordered mutation admission and visible failure reporting have a separate owner.

Native style/text fixtures use Ratatui's real TestBackend rather than another
snapshot framework. Existing Pillow renders exported cells for review. Physical
Alacritty paint requires a working graphical session; a PTY cannot qualify it.
No dependency version, feature, accepted license or obligation changes.

## Interactive reader latency and Vim motions (2026-10-18)

Retain the qualified Syntect, Ratatui, Unicode Segmentation and Unicode Width
versions. Standard-library threads and one-slot sync channels cover syntax
work without adding an executor or worker framework. Use `try_send`/`try_recv`
on the input thread, exact content/language keys, a bounded style cache and
plain-code fallback. Grammar cancellation is not promised: disconnect queues
on shutdown rather than waiting for an in-flight regex. Tests use a controlled
blocked worker, real grammar completion and disconnected channels.

The existing Unicode libraries own grapheme boundaries and cell widths.
The [Neovim motion contract](https://github.com/neovim/neovim/blob/master/runtime/doc/motion.txt)
informs reader word/WORD, counts, character-find, line, viewport and visual
objects. Lume owns the fixed read-only navigation policy; it does not embed
an editing engine or claim full Vim compatibility. Indexed word destinations
reuse standard slice binary searches. No dependency, version, feature or
license-policy change is needed for this capability.

Keep rusqlite and bundled SQLite for read-state count acceleration. The
[SQLite query planner documentation](https://www.sqlite.org/queryplanner.html)
describes covering indexes; V8 adds `(deleted, unread)` without changing
existing migrations. A native V7 upgrade verifies preserved data, repeated
opening and the actual covering plan. Repeated interaction measurements,
rather than the index definition alone, qualify its performance effect.

## Reader expansion (2026-10-18)

Promote the already locked dom_query 0.28.0 (MIT) to an exact direct dependency
with optional features disabled. Its qualified DOM and CSS Matcher API now
also own source-specific container selection. No package version or license
obligation changes. The MPL decision below still covers this integration.
The authoritative 0.28.0 API and source were reviewed before use; bounded CSS
validation and native ambiguous/invalid selector tests qualify the boundary.
Source rules, quality warnings and confirmation authority remain Lume policy.

Keep pulldown-cmark for heading/code/table structure, including source offsets.
Do not implement alternate Markdown or CSS parsers. Literal in-article search
uses Rust string matching; it does not introduce a second query language.

Promote already locked nom 8.0.0 (MIT) for the bounded library-search grammar.
The official API and escaped-string combinator documentation were reviewed.
Its established contributor community and composable complete-input parsers
fit this small grammar without adding a new parser ecosystem or package version.
nom owns token decoding; Lume owns fields, precedence, byte/token/depth limits
and compilation to bound SQLite values. SQLite FTS5 remains the text index.
Queries never interpolate user SQL or pass arbitrary FTS syntax through.

The standard library's scoped threads, bounded channels, atomics and mutexes
cover the collector's four lanes and controlled test boundaries. No async
runtime or generic worker framework is added. ureq still owns transport and
redirect handling; hostname pacing refers to subscription URLs, not a global
limiter for all final redirect destinations. Chrono parses Retry-After HTTP
dates. tempfile still owns atomic export temporaries. Review dates are explicit
user choices rather than a custom retention algorithm or a claimed FSRS model.
The authoritative contracts consulted include [nom 8](https://docs.rs/nom/8.0.0/nom/),
[SQLite FTS5](https://www.sqlite.org/fts5.html) and
[dom_query 0.28](https://docs.rs/dom_query/0.28.0/dom_query/).

## Full article extraction (2026-10-17)

Select `dom_smoothie` 0.18.2 (MIT), without optional features, for article
extraction. On 2026-10-17 the user explicitly approved MPL-2.0 transitives for
this integration, retaining notices and distribution obligations. This scoped
decision includes `selectors`, `cssparser`, `cssparser-macros` and `dtoa-short`;
it does not relax the policy for unrelated dependency additions. Keep MPL notices
and provide covered dependency source under MPL terms when distributing.
Lume's own license remains MIT. The root crate's license does not characterize
its full dependency tree. The Readability-derived algorithm, typed errors,
configurable element limit and HTML output meet the reader's contract. Keep
classes in the
extracted fragment so the existing HTML5gum normalizer can preserve code
language metadata. Do not replace the feed parser or write an article-scoring
algorithm. The extractor never owns HTTP, persistence or annotation policy.

The [release history](https://github.com/niklak/dom_smoothie/releases) shows
ongoing maintenance, including September 2026 fixes with outside contributors.
Maintenance remains concentrated around one owner; this is a smaller community
than Mozilla Readability. The MIT manifest, source API and changelog at tag
`0.18.2` were reviewed. The older readability crate is not preferred over this
maintained implementation. Running Mozilla's JavaScript implementation would
require another runtime/process lifecycle absent from this Rust application.

The dependency introduces a real DOM and HTML5 parser through `dom_query`,
html5ever and their transitive graph, increasing binary size and memory use.
Qualify that cost with representative/stress workloads; no performance claim
follows from choosing Rust. Retain exact direct pins and lock transitives.
All 32 new package versions/licenses were inventoried from Cargo metadata.
Their retained texts and exact published source versions are in
[extraction notices](../assets/licenses/full-article/NOTICES.md).
Permissive transitive notices must also be retained. A connected advisory and full
distribution-notice audit remain release prerequisites, not completed claims.

Lume owns bounded downloads, redirects, extraction limits, explicit offline
behavior, per-source automatic-fetch policy and cache provenance. Extraction
does not execute JavaScript, authenticate, bypass subscription gates or prove
that a publisher's complete article was recovered. Feed content survives both
successful and failed extraction. Cache replacement must retain annotations
with their source/version identity and immutable quoted text.

## Partial-feed presentation (2026-10-17)

Retain pinned feed-rs 3.0.0 for RSS/Atom/JSON candidate decoding, HTML5gum
0.8.3 for sanitization and pulldown-cmark 0.13.0 for typed CommonMark events.
Their qualified source contracts distinguish content/summary candidates,
text/code tokens and nested link/image boundaries. No custom feed/Markdown
parser, new dependency or article-page extraction algorithm is introduced.
Lume owns the fallback and presentation policy, including legacy placeholders
and the boundary between publisher quotations and generated notices.
Native fixtures cover each feed format and credible sanitization failures;
benchmarks check mixed-content fallback and reader-cache behavior.

## Precise selection and clipboard (2026-10-17)

Keep pulldown-cmark's source offsets, Unicode Segmentation's grapheme/word
boundaries, Unicode Width's cell measurements and Ratatui's styled spans.
These existing libraries own parsing and Unicode algorithms; the reader owns
selection, projection and annotation policy. No alternate editor or parser is
needed for read-only article navigation.

Promote the already locked `base64` 0.23.1 (MIT/Apache-2.0) to an explicit
dependency, using its documented STANDARD engine. The established
multi-contributor project supplies encoding; do not implement an encoder.
Keep the exact existing version and its transitive graph. Enable only std
directly, without requesting SIMD. Existing consumers may unify features.
Its bounded input is at most 64 KiB and encoded output is checked in tests.

The terminal adapter emits the standard write-only
[XTerm OSC 52](https://invisible-island.net/xterm/ctlseqs/ctlseqs.html)
clipboard command only after an explicit yank. The
[Alacritty contract](https://alacritty.org/config-alacritty.html#terminal)
permits copying by default (`OnlyCopy`). No clipboard reads, shell process,
daemon or display-server connection is required. Native clipboard crates
would add platform libraries and ownership lifecycles unnecessary for this
terminal consumer. A successful write requests a copy; terminals may reject
it without acknowledgment. Report I/O failures and retain the selection.
Base64 API/feature evidence: https://docs.rs/base64/0.23.1/base64/.

## Policy

Direct dependencies use exact versions; Cargo.lock locks transitives. Updates
require review of upstream changes, advisories, licenses and boundary tests.
Accept permissive free-software licenses (MIT, Apache-2.0, BSD, ISC, Zlib,
Unicode and SQLite public domain) subject to retaining their notices. New
copyleft obligations require an explicit distribution decision. This is a
project policy, not a claim that a full legal/security audit was completed.
Our code forbids unsafe; native dependencies still need review.

## Decisions (2026-10-16)

### Reader expansion

The second slice keeps the current Ratatui/Crossterm pair and selects:

- `tui-textarea` 0.7.0 (MIT), maintained by rhysd and contributors, with native
  Ratatui 0.29/Crossterm 0.28 support. Its existing cursor, selection and edit
  history contracts replace the append-only editor. Only Crossterm is enabled;
  its bounded undo history and our byte limit control retained draft data.
- `html5gum` 0.8.3 (MIT), a smaller maintainer group with WHATWG tokenizer
  fixtures, for forgiving HTML attributes/entities and feed discovery. Disable
  SIMD/default dependencies; retain a streaming presentation adapter rather
  than introduce a browser DOM/CSS engine. Scraper was considered; CSS selector
  and DOM dependencies do not serve these token-stream consumers.
- `syntect` 5.3.0 (MIT), a mature multi-contributor syntax ecosystem. Enable
  reviewed embedded text grammars and the Rust regex backend, not native
  Oniguruma or filesystem grammar loading. Binary grammar downloads were
  unavailable; the upstream text corpus and its permissive license are
  retained in `assets/syntax`. Its copy/use/modify/sell/distribute grant was
  reviewed as permissive free-software terms; the Rust grammar additionally
  retains its directory-specific MIT notice. Grammar text compiles into an
  embedded pack at build time; runtime uses only our own generated pack.
  Only bounded code blocks are highlighted and
  errors fall back to plain code. No grammar from feeds is executed or loaded.
- `refinery` 0.8.16 (MIT/Apache-2.0), a multi-maintainer migration toolkit; this
  qualified release supports the existing rusqlite 0.32 connection. Adopt its
  checksum/version history before extending version-one databases. Preserve
  the old bootstrap SQL bytes and migrate existing libraries without resets.
- SQLite FTS5, already included with bundled SQLite, owns full-text indexing;
  no separate search service or custom tokenizer is introduced. Serde and
  quick-xml own typed OPML decoding/encoding; no hand-written XML parser.
- Existing `tempfile` becomes a runtime dependency for secure external-editor
  drafts and atomic export files. `shlex` 2.0.1 parses editor arguments without
  evaluating a shell command. Unicode width/segmentation libraries own cursor
  and terminal-cell boundaries.
- `pulldown-cmark` 0.13 (MIT), maintained by the CommonMark Rust community,
  owns Markdown parsing for normalized rich reader content and export. Enable
  only the library parser; inline styling/layout stays at the reader owner.
  This avoids inventing a Markdown parser for links, emphasis or fenced code.

Upstream documentation and manifests were checked before integration. New
boundary tests and representative benchmarks must qualify these decisions.
These selections do not claim a complete historical advisory/security audit.

| Owner | Selection and evidence | Tradeoff |
| --- | --- | --- |
| Terminal | [Ratatui](https://github.com/ratatui/ratatui) 0.29 + Crossterm 0.28, MIT; multi-maintainer TUI ecosystem, documented test backend and buffered rendering | Newer 0.30 exists; 0.29 is the locally available, qualified pair. Upgrade together. |
| Feed decoding | [feed-rs](https://github.com/feed-rs/feed-rs) 3.0, MIT; shared RSS/Atom/JSON model, real-world fixtures, recent release | Smaller maintainer group; isolate at the source adapter and cap input before parsing. |
| Transport | [ureq](https://github.com/algesten/ureq) 3.4, MIT/Apache; maintained synchronous HTTP with pooling, timeout and body limits | One dedicated worker; no async runtime needed. |
| TLS | Rustls 0.23 + native-certs, permissive licenses; maintained dedicated TLS projects | OS trust store; never disable verification. Explicit Ring provider uses ureq's version-sensitive TLS configuration; pins and transport tests own compatibility. |
| Storage | [rusqlite](https://github.com/rusqlite/rusqlite) 0.32, MIT + bundled SQLite public domain; transactions and parameterized queries | Older cached release; review before distribution. Refinery now adopts the immutable bootstrap before schema evolution; FTS5 is bundled, not a separate service. |
| Content | HTML5gum + pulldown-cmark + Syntect + textwrap; permissive licenses, explicit parsing boundaries | Token normalization and Markdown layout do not implement browser CSS or media. Code grammar loading is restricted to build-generated assets. |
| CLI/data | Clap, Serde, serde_json, Chrono, url, anyhow; established Rust ecosystem libraries | Keep business rules out of these adapters. |

No dependency is selected solely by popularity. Upstream public repositories
and release documentation were reviewed; no claim of a complete historical
security-response audit or supply-chain audit is made. `cargo audit` and a
fresh license inventory are release prerequisites when network access exists.

The implementation environment cannot resolve registry hosts. Uncached
sources are obtained through the connected GitHub reader into temporary
Cargo path sources; exact packages and provenance are recorded in validation. They are not modified or
vendored in this repository. Normal development uses crates.io and Cargo.lock.

## Upstream references

The integration decisions used upstream manifests and source contracts:
[tui-textarea](https://github.com/rhysd/tui-textarea/tree/v0.7.0),
[HTML5gum](https://github.com/untitaker/html5gum/tree/0.8.3),
[Syntect](https://github.com/trishume/syntect/tree/v5.3.0),
[Refinery](https://github.com/rust-db/refinery/tree/v0.8.16),
[pulldown-cmark](https://github.com/pulldown-cmark/pulldown-cmark/tree/v0.13.0).
These pinned integrations are compatibility decisions, not assertions that
each is the newest release. Maintainer breadth and dependency footprint were
reviewed qualitatively; a connected advisory and distribution-notice review
remains required. In particular Syntect's bincode/yaml-rust transitives warrant
review before upgrades; bincode receives only our build-generated trusted pack,
and YAML parsing receives only the reviewed embedded build inputs.


## Keyboard release integration (2026-10-17)

Keep the qualified Crossterm 0.28.1 dependency and use its keyboard capability
query, push/pop commands and typed press/repeat/release events. The owning
crate's source and example qualify these contracts; the upstream keyboard
specification and Alacritty 0.15.1 changelog inform compatibility. No custom
terminal parser or new dependency is introduced. All-key reporting includes
alternate codes so shifted shortcuts and punctuation retain their meaning.
The terminal adapter owns a 256-event drain bound and protocol restoration;
application input batching owns mode-sensitive navigation and cancellation.

## Multi-language highlighting (2026-10-17)

Retain pinned Syntect 5.3.0 and its Rust regex backend; no new Rust dependency
or custom language parser is needed. Extend its reviewed SublimeHQ corpus
with C++, Java/JavaDoc, JavaScript/embedded expressions, CSS, HTML and YAML.
Also include the Python expression grammar required by the existing Python
definition. Build-time link validation rejects missing embedded contexts.

Terraform/HCL uses the MIT definition maintained in the community's
[SublimeText/Terraform](https://github.com/SublimeText/Terraform) package.
The selected revision is the compatible grammar pinned by bat, rather than
the newer grammar's Sublime-only inheritance features. This smaller community
has less maintainer breadth than SublimeHQ; keep the asset isolated and pinned.
TypeScript uses bat's converted grammar from Microsoft's Sublime plugin,
retaining both Apache-2.0 and bat's MIT notices. The source grammar is an older
compatibility snapshot; highlighting does not validate the latest language
specification. The multi-contributor
[bat syntax corpus](https://github.com/sharkdp/bat/tree/979ba22628bc9d8171f2cffca2bd5c90c9fc0a9e/assets/syntaxes)
provides evidence of Syntect integration. Lume does not add bat at runtime.

A broad precompiled all-language pack would introduce additional unqualified
assets and notices. Instead, retain only the requested definitions and their
actual embedded dependencies. Text sources, immutable revisions, Git blob
identities and retained license files live in
[syntax provenance](../assets/syntax/PROVENANCE.md). Their permissive terms
fit the existing distribution policy. No change to accepted licenses occurs.

Syntect owns lexical state and regex matching. Lume owns metadata aliases,
palette, per-block byte/line bounds, plain fallback and cached styled output.
The larger embedded pack and cold regex compilation have resource costs;
the native benchmark separates first rendering from repeated mixed-language
layout and checks preserved code and token colors. Only trusted build inputs
reach YAML parsing; feed text never supplies grammars or regex patterns.
Behavioral tests qualify every exposed language and credible metadata/fence
failures. Asset provenance checks do not replace an advisory or legal audit.

## Pane and mouse integration — 2026-10-18

No dependency, version or feature changed. Crossterm 0.28.1 already owns
standard mouse capture and event decoding; Ratatui 0.29.0 owns constraints and
buffer rendering. The integration uses existing Unicode Segmentation, Unicode
Width and Textwrap contracts for source-safe selection and visible note cards.
No custom terminal parser, clipboard process or platform-specific library was
added. Reviewed authoritative inputs: cached Crossterm `src/event.rs`,
[XTerm mouse tracking](https://invisible-island.net/xterm/ctlseqs/ctlseqs.html)
and [Alacritty mouse configuration](https://alacritty.org/config-alacritty.html#mouse).
Mouse status records observed events; there is no portable capability ACK in
this integration. Capture I/O failure, balanced suspension/cleanup, source-bound
selection and persistence boundaries have native failure/behavior tests.

### Grip and keyboard corrections — 2026-10-18

The same pinned Crossterm/Ratatui integration owns corner grips and highlighted
drop previews; no dependency or protocol parser was added. Cached Crossterm
`event/sys/unix/parse.rs` confirms standalone modifier reports and shifted
base/alternate codepoints. The application ignores standalone modifier keys,
normalizes shifted layout keys and offers Alt-arrow/explicit ex alternatives.
Native tests reproduce modifier-prefix loss, refresh-canceled moves and
responsive hidden-neighbor moves, then verify displayed geometry and durable
order. A captured layout move owns pane identities and hit regions; it must
not depend on the selected article lifecycle.

### Pointer-following pictures — 2026-10-18

Reviewed the existing pinned Ratatui 0.29.0 Buffer/Cell rendering contract in
its cached source and [official rendering description](https://ratatui.rs/concepts/rendering/under-the-hood/).
Also researched [TachyonFX](https://github.com/ratatui/tachyonfx), its buffer
translation/effect composition and release history. The already-qualified
Ratatui buffer meets this feature's narrow requirement: capture a displayed
pane and place its cells elsewhere. Retain it without adding an effect graph,
executor or dependency. This is an owner-specific drag lifecycle, not a new
animation framework. Existing licensing, pins and features remain unchanged.
Capture limits, frame cadence, interruptions and interpolation are owned by
`app::pane_motion`; native failure/behavior tests and representative/stress
frame benchmarks qualify the integration. Physical terminal timing is not
inferred from TestBackend CPU timings.
