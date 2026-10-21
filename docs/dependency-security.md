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
Each notice inventory identifies its scope; none is a claim of a completed
whole-project legal or advisory audit. Release preparation includes a current
advisory and license inventory review. No external security audit is claimed.

Our Rust code forbids unsafe. Native and transitive dependencies still require
review. Types and a third-party library do not supply missing domain authority,
transaction guarantees or input limits. Publisher content never selects code,
local files, a shell command or executable configuration.

## Qualified integrations

| Capability | Selection | Responsibility and limits |
| --- | --- | --- |
| Terminal | Ratatui 0.29.0, Crossterm 0.28.1, tui-textarea 0.7.0 | Qualified compatible pair/editor. Library-owned event parsing, cached buffer rendering and bounded drafts; no custom terminal parser. |
| Feed and transport | feed-rs 3.0.0, ureq 3.4.2, Rustls 0.23.45, native-certs 0.8.4 | Multi-format decoding, bounded requests, redirects and verified OS trust. Offline policy and resource limits remain in Lume. |
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
