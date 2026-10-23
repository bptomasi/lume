# Disk compression review

This is an implementation proposal, not an enabled capability. No dependency,
cache format or compression setting changed in this review. Candidate codecs
must pass the dependency policy and representative measurements first.

## Priorities

| Target | Recommendation |
| --- | --- |
| Consistent SQLite backup snapshots | First experiment: low-level zstd after creating the snapshot. Normal reading/scroll pays no decoding cost. |
| Ordinary and pinned SVG originals | Compare safe LZ4 with zstd fast modes; skip small inputs and insufficient savings. |
| PNG/JPEG/WebP/GIF | Keep original bytes by default; these are already compressed formats. Use them as negative controls. |
| SQLite feed/Full bodies | Defer: FTS and existing triggers require a separate storage design. |
| Configuration, notes, progress and prepared layouts | Keep directly readable; small records or latency-sensitive state do not justify codec complexity without evidence. |
| Generated formula SVG | Currently memory-only; no disk bytes to reclaim. Keep prepared presentation uncompressed. |

`article_search` currently uses internal FTS5 content (V2), populated with feed
and Full bodies by V4. Compressing only `article_documents.body` leaves another
text copy in FTS and breaks SQL/triggers which consume the text. External-content
FTS is a separate migration candidate: measure table-level space first, then
qualify triggers, ranking, snippets, updates, deletes and restore. See the
[SQLite FTS5 content-table contract](https://www.sqlite.org/fts5.html#external_content_and_contentless_tables).

## Codec and format experiment

[LZ4](https://github.com/lz4/lz4) prioritizes fast compression/decompression;
[Zstandard](https://github.com/facebook/zstd) offers configurable levels,
including fast negative levels. These are project characteristics, not measured
Lume gains. Rust candidates include
[lz4_flex](https://github.com/PSeitz/lz4_flex) with safe encode/decode features
and [zstd-rs](https://github.com/gyscos/zstd-rs), which also requires reviewing
its native dependency. Existing transitively linked gzip support is not a
reason to select gzip or introduce an undeclared direct dependency.

Use a versioned envelope with magic, codec, original length and checksum.
Publish atomically and continue reading legacy raw files. Disabling compression
must stop new compressed writes while preserving reads of existing files.
Admission limits apply before allocation and after expansion: 8 MiB images,
512 KiB SVG, plus codec-window/input bounds. Unsupported/corrupt ordinary cache
is recoverable; pinned offline corruption must remain visible.

Future settings could select enabled/algorithm/targets/minimum size/minimum
savings and a backup level. Values must be chosen by experiment, not asserted
here as defaults. One compression lane, no nested codec parallelism or initial
dictionaries. Keep URL hashes and decoded-content identities stable. Account
for original versus physical bytes in offline budgets and storage reports.

Backups must use the existing consistent SQLite backup API before compression;
never compress the live database or WAL directly. Restore expands into private
staging, validates bounds/integrity, then follows the existing no-overwrite
contract. Compression does not encrypt stored API tokens.

Measure raw, safe LZ4 and zstd low/fast levels in sequential release processes
with the pinned Rust toolchain. Include varied SVGs, cold backups, raster
controls, physical size, CPU/RSS, cold/warm reads, publication, first-image time
and scrolling with prepared caches. Check exact content/pixels/source offsets,
legacy files, truncation, bogus lengths and checksums. No default should change
until those results establish a useful storage/latency tradeoff.
