# Dependency policy

Direct versions and enabled features are authoritative in
[Cargo.toml](../Cargo.toml); the resolved graph is in
[Cargo.lock](../Cargo.lock). Prefer established libraries over custom parsers,
cryptography and generic infrastructure, with explicit authority and resource
limits at the integration boundary. Popularity alone is insufficient evidence.

## Review and distribution

Before adding or updating a dependency:

- Review maintainer breadth, release/security-response history, API stability,
  compiler compatibility, enabled features, transitive costs and license terms.
- Pin the intended direct version and update only the necessary locked packages.
  Record consequential selection decisions and integration limits here.
- Qualify behavior, failure cleanup and representative workloads. Parser,
  database and transport changes must preserve source offsets, private data,
  transaction boundaries and input limits.
- Regenerate notices with `make licenses`, then run `make check-licenses` and
  the current `make check-security` gate. See
  [development](development.md#quality-gates) for the remaining release checks.

Permissive MIT, Apache-2.0, BSD and compatible free-software terms are preferred.
The extraction integration specifically permits MPL-2.0 transitives
`selectors`, `cssparser`, `cssparser-macros` and `dtoa-short`. Retain their notices
and provide the covered dependency source under the MPL terms when distributing.
This exception does not authorize unrelated copyleft additions.

Notices and source obligations are retained under `assets/licenses/` and
`assets/syntax/` and included by the installer/package tools. RaTeX's embedded
KaTeX font outlines have SIL OFL-1.1 notices in
`assets/licenses/images/ratex-fonts`, separately from the MIT Rust code. Font
bundling does not change Lume's application license.

Our Rust code forbids unsafe. Native/transitive dependencies still need review;
a safe wrapper does not establish input limits or domain authority. Publisher
content never selects executable configuration, shell commands or local files.
No independent external security audit is claimed by adopting a library.

## Integration ownership

| Capability | Libraries | Integration contract |
| --- | --- | --- |
| Terminal/editor | Ratatui, Crossterm, tui-textarea | Upgrade as a compatibility unit; qualify actual input, cached rendering and cleanup. |
| Feeds/HTTP/TLS | feed-rs, ureq, Rustls, native-certs | Bound decoded input, redirects and concurrency; verify OS trust and honor offline policy. |
| Database | Bundled rusqlite and Refinery | Keep real transactions, WAL/FTS and immutable checksum-verified migrations. |
| Extraction | dom_smoothie, dom_query | Keep ambiguous/short/obstructed extraction explicit and honor the scoped MPL obligations. |
| Markdown/HTML/syntax | pulldown-cmark, html5gum, Syntect, textwrap | Preserve source offsets; compile only reviewed bundled grammars, never publisher-provided serialized grammar objects. |
| Search | nom and SQLite FTS5 | Compile bounded validated fields/precedence into bound SQL; never accept user SQL. |
| Raster/placement | image, ratatui-image | Restrict codecs and retain byte/pixel/allocation/cache budgets; placement consumes prepared widgets. |
| SVG | resvg, roxmltree | Disable DTD and linked resource resolution; publisher strings cannot select font files or system scans. |
| Formulas | math-core and optional RaTeX | Bounded local parsing/projection/outlines, with explicit raw-source fallback. |
| Palette | Quantette, with existing icy_sixel integration | Shared Wu/Oklab palette and continuous diffusion; qualify exact encoding, pixels and memory behavior. |
| TOML | toml_edit | Typed validation and comment-preserving publication; default recursion limits stay enabled. |
| Unicode/files/CLI | unicode-segmentation, unicode-width, tempfile, shlex, SHA-2, Clap/Serde/Chrono/url | Preserve cell/source correspondence, private atomic files, typed values and argument parsing without shell evaluation. |

[Build capabilities](build-features.md) select optional consumers without
changing configuration or database formats. The lock/notices cover the supported
feature/platform union; omitting a Lume consumer does not guarantee that every
similarly named transitive package disappears.

## Publisher-input boundaries

Feed collection caps decompressed input at 16 MiB and 2,000 entries. HTML feed
discovery has a separate 4 MiB budget. Article pages are capped at 2 MiB,
images at 8 MiB and SVG XML at 512 KiB. The shared
reader applies limits after decompression with an overflow probe; gzip never
raises the budget. Invalid downloads do not advance validators, publish partial
cache entries or replace the last successful extracted document. Parser and
decoder allocations can exceed input size: admission limits are not a memory
sandbox.

Raster decoding retains independent dimension, pixel and allocation bounds.
SVG parsers reject DTD and external resources. Controlled fallback fonts are
loaded on demand; publisher content cannot provide font bytes or file paths.
Image workers preserve the same proxy/TLS/redirect/HTTP policy while checking
cancellation between bounded read waits. DNS/connector setup keeps provider
bounds and the original request deadline. Requalify the ureq transport adapter
when upgrading its unversioned connector boundary.

CommonMark owns math delimiters. math-core owns the bounded TeX-to-MathML parse;
Lume projects it with expression/document/depth/expansion budgets. Invalid,
unsupported or excessive expressions retain source. RaTeX is a smaller,
pre-1.0 upstream and needs pinned APIs, bounded worker input, golden/failure
checks and fallback. Graphic export accepts only embedded unmodified font
outlines and rejects unsupported fallback identifiers before export.

Quantette enables only its reviewed standard-library path; k-means, random
sources, Rayon and image-loading adapters remain disabled. Lume prepares one
palette/index buffer per image before strip encoding. Its exact Sixel planner
and output integration require independent decoding and fidelity checks; see
[measurement](performance.md#image-and-sixel-measurements). Upstream benchmark
claims do not establish Lume performance or security.

Configuration uses bounded toml_edit parse/display/Serde features; `unbounded`
parsing is forbidden. Lume additionally bounds file size and definition counts
and validates queries, selectors, references and dimensions before adoption.
File locks serialize cooperating Lume writers; external editors need not honor
those locks. Byte comparison and three-way merge preserve conflicts, while the
final compare/rename window remains an OS-level race. See
[configuration](configuration.md) for recovery and supported formatting.

## SQLite and migration compatibility

The bundled engine must retain the fix for
[RUSTSEC-2026-0459](https://github.com/RustSec/advisory-db/blob/main/crates/libsqlite3-sys/RUSTSEC-2026-0459.md)
when opening crafted database/WAL pairs. Restore and legacy import make this
boundary relevant. Do not substitute an unreviewed system SQLite or waive the
vulnerability to preserve a wrapper version. The selected Refinery facade/core
pair must support the locked rusqlite adapter.

Keep `bundled`, `backup`, `hooks`, statement caching and explicit `fallible_uint`
semantics. Borrowed-value conversion must preserve UTF-8 and checked numeric
failures. Hooks use disposable, thread-owned read connections. Registration and
removal errors propagate; cleanup disables the callback even when unwinding or
unregistering fails. Personal writes never install a read-cancellation callback.

Refinery owns migration ordering, checksums and grouped transactions. Its
history adapter assumes valid decimal checksums and RFC3339 timestamps. Lume
prevalidates those representations at Store opening with the same selected
`time` parser; it does not rewrite history or implement another migrator.
Maintain the old-engine fixtures under `tests/fixtures/sqlite-3.46.0` and qualify
upgrade, FTS, progress, cancellation, restore and no-clobber backup behavior.

SQLite's online backup API owns snapshot consistency. Lume owns deadlines,
private permissions, identity/integrity validation and non-overwrite restore.
Storage diagnostics do not authorize an implicit VACUUM or a schema migration.
See the [backup contract](https://sqlite.org/backup.html) and
[SQL ownership](sql-ownership.md).

## License inventory and advisory gate

`make licenses` uses pinned cargo-about to collect actual license texts and
SPDX evidence for the active feature/platform union. It records inactive lock
entries separately and retains MPL crate archives verified against Cargo.lock
checksums. `make check-licenses` verifies the committed inventory offline.
Do not replace SPDX resolution with a handwritten expression parser.

`make check-security` uses pinned cargo-audit with its current RustSec database.
It records database identity and the lock fingerprint and fails on reported
vulnerabilities or new/changed informational warnings. Install the tool versions
required by the [Makefile](../Makefile) deliberately; normal checks do not
bootstrap tools or treat cached advisory results as a fresh audit.

[advisory-reviews.toml](../tools/advisory-reviews.toml) is the authoritative list
of expiring, checksum-specific informational reviews. Obsolete reviews fail the
gate too. Preserve the reviewed reachability evidence when upgrading:

| Dependency | Restricted consumer |
| --- | --- |
| lru through Ratatui | Layout cache paths avoid the affected iterator/pop APIs; layout keys have no panicking destructor. |
| bincode through Syntect | Only trusted build-generated syntax objects are decoded. |
| yaml-rust through Syntect | Only reviewed bundled grammars are compiled at build time. |
| paste through Ratatui | Compile-time macro use; coordinate replacement with the terminal/editor stack. |
| ttf-parser through RaTeX | The outline exporter receives embedded unmodified KaTeX fonts; publisher-controlled fonts and unsupported CJK/emoji fallback identifiers are excluded. |

These restrictions do not make a dependency sound for arbitrary use. Changed
consumers, authority, features, versions or checksums require renewed review.
Parser updates must preserve the patched notebook wikilink path as well as
reader images, math, tables and offsets; a parser advisory must not be hidden
by disabling its check.

Pillow and the tools in `tools/requirements-visual.txt` are optional development
qualification tools, not runtime dependencies. GitHub Actions dependencies are
pinned and permission-scoped in [the workflow](../.github/workflows/check.yml).
Review action updates and cache contents; credentials, user libraries and
advisory results do not belong in shared build caches. Release assets must come
from the successful qualification jobs of the same run. See
[release policy](release.md).
