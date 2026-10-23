# Dependency policy

Dependencies must meet the actual contract with explicit resource limits,
compatible licenses and maintainable APIs. Prefer established libraries over
custom protocol, parser, cryptography or generic infrastructure implementations.
Popularity alone is insufficient evidence.

## Review and distribution

Before adding or changing a dependency, review its maintainer/contributor
breadth, community, release and security-response history, API stability,
compatibility, resource costs, transitive footprint and free-software license
obligations. Record the consequential selection and concrete integration limits
here. Pin direct versions and commit the reviewed Cargo.lock. Keep updates
intentional and test behavior, failure boundaries and representative workloads.

Permissive MIT, Apache-2.0, BSD and compatible free-software terms are preferred.
The extraction integration specifically permits MPL-2.0 transitives
`selectors`, `cssparser`, `cssparser-macros` and `dtoa-short`. Retain their
notices and make covered dependency source available under MPL terms when
distributing. This exception does not extend to unrelated copyleft additions.

Applicable notices are retained under `assets/licenses/full-article`,
`assets/licenses/images`, `assets/licenses/configuration` and `assets/syntax`.
Integration inventories retain their original scope; the complete locked
graph inventory and current advisory gate are described below. No external security audit is claimed.

Our Rust code forbids unsafe. Native and transitive dependencies still require
review. Types and a third-party library do not supply missing domain authority,
transaction guarantees or input limits. Publisher content never selects code,
local files, a shell command or executable configuration.

## Qualified integrations

| Capability | Selection | Responsibility and limits |
| --- | --- | --- |
| Terminal | Ratatui 0.29.0, Crossterm 0.28.1, tui-textarea 0.7.0 | Qualified compatible pair/editor. Library-owned event parsing, cached buffer rendering and bounded drafts; no custom terminal parser. |
| Feed and transport | feed-rs 3.0.0, ureq 3.4.2, Rustls 0.23.46, native-certs 0.8.4 | Multi-format decoding, bounded requests, redirects and verified OS trust. Offline policy and resource limits remain in Lume. |
| SQLite | rusqlite 0.32.1 with bundled SQLite, Refinery 0.8.16 | Real transactions, WAL/FTS5 and immutable checksum-verified migrations; no external database. |
| Extraction | dom_smoothie 0.18.2, dom_query 0.28.0 | Readability and CSS parsing, with explicit ambiguous/short/obstructed extraction handling. Qualified MPL scope applies. |
| Rich text | html5gum 0.8.3, pulldown-cmark 0.13.0, Syntect 5.3.0, textwrap 0.16.4 | Bounded HTML normalization, Markdown structure and source offsets, reviewed text grammars and Unicode wrapping. |
| Search | nom 8.0.0 and bundled SQLite FTS5 | Qualified grammar combinators and indexing. Lume owns fields, precedence, limits and parameterized compilation; queries are never raw user SQL. |
| Images | image 0.25.10, ratatui-image 8.1.1 | Restricted PNG/JPEG/WebP/GIF decoding, fixed prepared widgets and qualified Sixel encoding; no custom quantizer or image protocol. |
| SVG | resvg 0.48.1, roxmltree 0.21.1 | Bounded XML/vector rasterization, DTD disabled and resource resolvers rejecting linked/embedded resources. |
| Source offsets | unicode-segmentation 1.13.3, unicode-width 0.2.0 | Grapheme/cell semantics and visible-row projection; no custom Unicode parser. |
| File/CLI data | tempfile 3.27.0, shlex 2.0.1, SHA-2 0.11.1, Clap/Serde/Chrono/url | Private atomic exports, argument parsing without shell evaluation, cache/fingerprint identifiers and typed validated data. |

Feed collection limits decompressed input to 16 MiB and 2,000 entries, with
four download lanes and a bounded result channel. The decoded ureq reader is
wrapped in standard `Read::take` with a one-byte overflow probe before parsing;
ureq's configured body limit alone precedes gzip decoding in this pinned
version. Invalid downloads never advance stored HTTP validators. Plain and
gzip fixtures qualify the inclusive boundary and rejection above it. Parser
allocations can exceed input bytes; these limits are not a memory sandbox.

Ratatui/Crossterm are maintained as a compatibility unit. Upgrade the terminal
and editor stack together and requalify actual input/cleanup. Qualified versions
are not assertions that every dependency is the newest available release.

Image preparation occurs on a bounded worker. Native Foot images use cached
cell-height Sixel strips and synchronized updates, clipped to viewport and modal
regions. Encoder quantization does not guarantee original 24-bit color precision.
The source inspector reuses original bytes/paths for zoom rather than enlarging
character previews. Decoder allocation settings are best effort, not a memory
sandbox; Lume retains its own byte, dimension, pixel and cache budgets.

SVG has no system font scan or remote/local resource access selected by publisher
strings. A bounded fallback font is loaded on demand for text. OS fonts are not
bundled or copied into the cache. Syntect parses reviewed bundled grammars at
build time; publisher text cannot load a grammar. Its trusted binary cache does
not accept remote serialized objects.

Recommendation matching reuses the qualified search/SQLite boundary. It does not
introduce a model, vector database, alternate tokenizer or hosted service. Scores
use explicit interests and observed publisher evidence; they do not establish
quality, factual correctness or personal relevance. Private annotation text is not
publisher evidence.

Table layout reuses pulldown-cmark's GFM table events, Ratatui `Line`/`Span`
styles and unicode-width measurements. A widget-only table would lose the
reader's byte correspondence for selection and annotations, so prepared rows
retain those mappings instead. No Markdown/HTML parser, ANSI formatter or new
dependency is introduced. Column, row, width and padding budgets are enforced
before publishing a grid; bounded inspection falls back to original Markdown.

## Declarative configuration

Select `toml_edit` **0.25.15+spec-1.1.0**, pinned exactly, with only `parse`,
`display` and `serde`. The multi-contributor toml-rs project owns the TOML
grammar, typed decoding and comment-preserving document tree. The qualified
integration uses its published API and MIT OR Apache-2.0 terms, with a locked
transitive inventory and native round-trip, conflict and failure tests. Default parser recursion limits remain enabled;
the `unbounded` feature is forbidden. Lume additionally limits configuration
to 2 MiB, 5,000 feeds and bounded named definitions and validates all queries,
references, selectors and layout dimensions before application.

The qualified footprint adds indexmap/hashbrown, serde_spanned, toml_datetime,
toml_parser, toml_writer and winnow. They are permissively licensed MIT and/or
Apache-2.0. No native codec, executable configuration, async runtime or network
dependency is added. This is not a completed advisory audit. Preserve the
configuration dependency notices with distribution artifacts.

Existing tempfile owns same-directory atomic replacement; standard-library
File locks serialize Lume writers without another lock crate. External editors
need not honor advisory locks, so publication compares the last read bytes
before replacement; uncooperative edits in that final comparison/rename window
remain an OS-level race. SQLite owns the durable publication base and pending
revision, not a claim of a distributed filesystem/database transaction.
Three-way conflicts stop publication and preserve both file and pending data.
Documented formatting limitations include dotted-key ordering and final
newlines. Behavioral round-trip and recovery tests qualify our supported form.
The same parsed tree owns layout changes; no separate TOML parser or formatter
dependency is added. Public table conversion is used only for inline containers
without comments, since it clears child decoration. Explicit formatting checks
typed equivalence, comment retention and idempotence with native tests.

Sources: [toml_edit API](https://docs.rs/toml_edit/0.25.15+spec-1.1.0/toml_edit/),
[toml-rs source and licenses](https://github.com/toml-rs/toml),
[standard File locks](https://doc.rust-lang.org/std/fs/struct.File.html#method.try_lock).

## Authoritative references

[Ratatui](https://ratatui.rs/),
[Crossterm](https://docs.rs/crossterm/0.28.1/crossterm/),
[SQLite transactions](https://sqlite.org/lang_transaction.html),
[SQLite FTS5](https://sqlite.org/fts5.html),
[rusqlite](https://docs.rs/rusqlite/0.32.1/rusqlite/),
[Refinery](https://docs.rs/refinery/0.8.16/refinery/),
[dom_smoothie](https://docs.rs/dom_smoothie/0.18.2/dom_smoothie/),
[dom_query](https://docs.rs/dom_query/0.28.0/dom_query/),
[image-rs](https://github.com/image-rs/image),
[resvg](https://github.com/linebender/resvg),
[ratatui-image](https://github.com/ratatui/ratatui-image),
[Foot](https://codeberg.org/dnkl/foot),
[Syntect](https://github.com/trishume/syntect),
[Pulldown-cmark](https://github.com/pulldown-cmark/pulldown-cmark).

## Distribution inventory and advisory gate

`about.toml` qualifies cargo-about **0.9.2** (MIT OR Apache-2.0), maintained by
Embark's multi-contributor tooling project, for SPDX resolution and collection
of actual license texts. It is development tooling, not a runtime dependency.
`make licenses` refreshes the complete active runtime/build graph under
`assets/licenses`, records feature-inactive lock entries separately and retains
exact MPL crate archives verified against Cargo.lock checksums. The installer
and release package include this evidence and the separate grammar notices.
`make check-licenses` verifies the committed inventory without network access.
Do not replace this tool with a hand-written SPDX expression parser.

`make check-security` uses RustSec's **cargo-audit 0.22.2**, with its current
advisory database, and records database revision, UTC date and lock fingerprint.
It fails on every reported vulnerability and every new or changed informational
warning. `tools/advisory-reviews.toml` contains expiring, checksum-specific
reviews; obsolete exceptions also fail. RustSec and cargo-about are established,
independently maintained tools with public issue/security histories and pinned
APIs. Install/update them deliberately; ordinary checks never bootstrap tools.

The 2026-10-21 qualification updates Rustls to **0.23.46** and WebPKI to
**0.103.16**, addressing RUSTSEC-2026-0346 through 0349 and 0382. Features remain
restricted to ring/std/TLS1.2 without Brotli or custom session stores. These
are compatible patch releases with the same license and transport API;
real plain/gzip HTTP failure and cache tests qualify integration. No vulnerability
allow-list is used. Informational reviews currently cover:

| Dependency | Reachability and disposition |
| --- | --- |
| lru 0.12.5 via Ratatui 0.29 | Ratatui's layout cache calls get_or_insert/resize, not the affected iter_mut/pop APIs; Rect/Layout keys have no panicking Drop. Preserve this call-site evidence when upgrading the terminal stack. |
| bincode 1.3.3 via Syntect | Unmaintained; only trusted build-generated syntax objects are decoded. Publisher bytes cannot choose serialized syntax input. |
| yaml-rust 0.4.5 via Syntect | Unmaintained; reviewed bundled grammars are compiled at build time. Publisher content cannot load YAML grammars. |
| paste 1.0.15 via Ratatui | Unmaintained compile-time macro dependency. Track replacement with the compatible terminal/editor stack. |

This disposition does not make an affected library sound for arbitrary use.
A feature, consumer, version or checksum change requires renewed review.
Sources: [RustSec](https://github.com/rustsec/rustsec),
[cargo-about](https://embarkstudios.github.io/cargo-about/),
[Rustls patch release](https://github.com/rustls/rustls/releases/tag/v/0.23.46),
[WebPKI fix](https://github.com/rustls/webpki/pull/546),
[Ratatui layout cache](https://github.com/ratatui/ratatui/blob/v0.29.0/src/layout/layout.rs).

Article and image downloads use the same decoded HTTP byte reader as feeds;
limits remain 2 MiB for pages and 8 MiB for images. SVG additionally caps its
XML input at 512 KiB. Gzip never raises those limits. Failures cannot publish
partial cache entries or replace the last good extracted document.

The SQLite online backup API is enabled on the existing pinned rusqlite
**0.32.1** dependency. This adds no library to the runtime graph. SQLite owns
snapshot consistency; Lume owns deadline, private permissions, identity checks,
non-overwrite publication and isolated restore validation. See
[SQLite's backup contract](https://sqlite.org/backup.html).

Pillow **13.0.0** is pinned in `tools/requirements-visual.txt` for optional visual
qualification only. The mature multi-maintainer Pillow project supplies pixel
inspection, under its permissive HPND-derived terms. It processes controlled
qualification fixtures and captured cells, not runtime publisher images.
It is installed in a development virtualenv and is not bundled in Lume.

The release workflow uses GitHub-maintained `actions/cache` v4 at
`0057852bfaa89a56745cba8c7296529d2fc39830` and `actions/download-artifact` v4
at `d3f86a106a0bac45b974a628896c90dbdf5c8093`, verified against their upstream
tag refs. These MIT-licensed development actions add no runtime dependency.
Cache restores include the runner OS, pinned Rust/security tool versions and
lock hash; compilation and advisory checks still run after restoration.
Release artifacts come from the successful prerequisite job in the same run.
Only the publication job receives repository write permission. Updates require
review of the upstream action code, permissions and release history.

Prose justification reuses textwrap's prepared line breaks and the qualified
Unicode cell/grapheme contracts. Textwrap supplies no justified alignment API;
Lume's presentation policy expands existing word gaps in eligible cached rows.
The expansion maps back to original readable bytes, including mouse hits and
selection. It introduces no parser, dependency or text rewrite.

Unlabelled code hints inspect at most 16 KiB and 128 lines for strong SQL or
shell evidence. Explicit languages, including plain text, take precedence.
Syntect owns all grammar matching; `LumeSQL.sublime-syntax` is an MIT overlay
including the unchanged reviewed SQL definition. Publisher wrapper hints are
bounded to 128 tracked container levels and cannot select external grammars.

Linked pages and offline preparation reuse the qualified `ureq`, `dom_smoothie`,
`dom_query`, `pulldown-cmark`, image/Resvg and Rusqlite integrations above.
No browser engine, model runtime, service or dependency feature is added.
The existing HTTP deadlines and decoded-byte limits remain authoritative;
original offline image bytes pass the same decoder before publication.
The standard library's advisory file lock serializes offline package writers.
SQLite's existing transaction/JSON/FTS contracts own checkpoint consistency and
cache-index updates. Storage inspection uses the documented
[page-count and free-list pragmas](https://www.sqlite.org/pragma.html);
no implicit VACUUM or auto-vacuum migration is introduced. Behavioral, HTTP
failure and representative workload checks qualify these integrations.

## Terminal formulas and shared image palettes

`math-core` **0.9.0** (MIT), with default features disabled and only `std`,
owns LaTeX-to-MathML conversion. The multi-contributor tmke8/math-core project
has sustained parser and conformance work; the pre-1.0 API is pinned. Its ten
additional locked packages are permissively licensed Rust libraries (PHF tables,
macro/arena/string helpers); no TeX process, browser runtime or network renderer
is added. `pulldown-latex` was considered, but its own production-testing caveat
and lack of the same configurable macro-expansion boundary favored math-core.
This selection does not assert an independent security audit.

CommonMark owns inline/display delimiters. Lume limits each expression to
4 KiB, 256 backslashes, 64 nested braces and 64 macro expansions; a document
converts at most 256 expressions totaling 64 KiB. Generated MathML is capped at
64 KiB, 4,096 XML nodes and 64 projection levels, with DTD disabled. The Unicode
projection is limited to 8 KiB and runs during source preparation. Unknown,
invalid or excessive expressions preserve raw source; conversion uses local
macro state, never filesystem/remote resource resolution. These are admission
budgets, not a memory sandbox.

`icy_sixel` **0.1.3** (MIT), already locked through ratatui-image, is now an
explicit direct dependency for its public palette and encoder APIs. One palette
and one diffusion pass cover each fitted image before independent cell-height
encoding. This removes per-row palette seams and repeated quantizer setup;
there is no new image codec or protocol implementation. Image dimensions,
prepared-payload and shared decoder-cache budgets remain enforced. Two bounded
HTTP lanes overlap downloads while serializing decoding through one cache.

## Graphic formulas and shared-palette color preparation

RaTeX 0.1.15 (`ratex-parser`, `ratex-layout`, `ratex-types`, `ratex-svg`)
provides the optional runtime graphic presentation. It has a small upstream
maintainer base and a pre-1.0 API; pinned versions, bounded publisher input,
behavioral/golden checks and explicit fallback are required. Its parser has
recursion and macro-expansion limits. Layout and SVG paths stay in worker
preparation, without a browser, JavaScript runtime, shell or external service.
Embedded KaTeX font outlines avoid system font discovery for admitted math.
The font binaries are SIL OFL-1.1, separately from the MIT Rust code. Their
unaltered notices are retained in `assets/licenses/images/ratex-fonts` and
installed with the image integration notices. This scoped font redistribution
uses OFL's bundling permission; it does not change the application's MIT license.

Quantette 0.6.0 (MIT OR Apache-2.0, Rust 1.90 minimum) owns Wu palette generation
and continuous error diffusion in Oklab. Its upstream has a small maintainer
base and is also used by newer icy_sixel releases. Only `std` is enabled;
k-means, random generators, Rayon and image-loading adapters are disabled.
Lume retains one palette/index buffer for the whole image and delegates PAL8
wire encoding to the already qualified icy_sixel 0.1.3. Keeping that encoder
avoids introducing a second protocol implementation or per-strip palettes.
These choices require independent pixel checks and measured preparation costs;
upstream benchmark claims do not establish Lume performance or security.

The reviewed graphic formula path adds 51 active packages to the previous
348-package feature graph, primarily the RaTeX font/layout stack and palette
arithmetic. It does not load its font data in text mode. The complete lock and
inventory qualify 399 active packages and four explicitly inactive entries.
Responsive image selection admits width descriptors through 4096 pixels and
densities through 4x; decoded image byte, dimension and pixel limits remain
independent of the publisher's descriptor.

The optional RaTeX outline exporter introduces `ttf-parser 0.25.1` through
`ab_glyph` and fallback font adapters. RUSTSEC-2026-0192 reports discontinued
maintenance, with no patched version, rather than a reported vulnerability.
The checksum-specific review expires on 2027-01-19. Only embedded, unmodified
KaTeX font bytes reach the exporter: CJK and emoji fallback identifiers are
rejected before export, and publisher input cannot select a font file, font
bytes or a resource resolver. Unsupported text remains in the lightweight
projection. A change to those consumers or authorities invalidates the review.
Migration of the upstream exporter to a maintained font parser remains a
maintenance requirement; the advisory gate retains and checks this finding.

## Image and formula implementation review

This review changes no dependencies. Raster decoding admits 8,388,608 pixels
(including UHD 3840×2160) with the existing 64 MiB allocation bound. It reserves
decoder output and evicts old sources before allocation; two retained entries
share 32 MiB, with source precision preserved. Two SVG trees share a 512 KiB
source-input budget; this does not claim a bound on parsed-tree memory. Existing
XML/node limits and resource resolvers are unchanged. Opaque SVG preparation
eliminates the unpremultiply/recomposite pass without changing allowed assets.

The existing math-core parser admits its browser-layout-limited wide accents
for Lume's explicit Unicode projection. This adds no parser, executable macros,
font source or resource resolver. Projection adds enclosures, prescripts and
brace/arrow annotations while retaining original source maps. Cached failures
remain subject to document/expression budgets. Unknown or unrepresentable
content stays raw rather than silently discarding mathematical material.
