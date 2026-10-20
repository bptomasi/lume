# Requested capability audit — 2026-10-19

This audit covers executable behavior, entry points and native regression
evidence. Buffer/PTY tests are complemented by real Alacritty X11 qualification
with synthetic input and full bitmap comparisons. These do not measure human
perception or physical key-to-pixel latency. Current gate results and environment
limitations belong to [validation](validation.md).

| Requested capability | Available behavior | Owning evidence |
| --- | --- | --- |
| Independent Rust reader | Local SQLite; RSS, Atom and JSON Feed; offline cache; no server | `tests/feeds.rs`, `tests/storage.rs`, `tests/cli.rs` |
| Computing reading material | 98 curated sources and 46 English smart lists, including security, AI, systems and infrastructure | `tests/catalog.rs`, `tests/catalog_infrastructure.rs`, `tests/catalog_upgrade.rs`; [source catalog](reading-sources.md) |
| English UI | English commands, help, source groups and historical label upgrade preserving custom criteria | `tests/catalog_upgrade.rs`, `tests/command_line.rs` |
| Discoverable ex commands | `:` suggestions, syntax, descriptions, filtering and completion | `tests/command_line.rs`, `tests/interface.rs` |
| Favorites, folders and Read | Independent favorite/read flags, multiple folder memberships and automatic Read destination | `tests/read_state.rs`, `tests/workspace.rs`, `tests/interactions.rs` |
| Delete posts | Recoverable Trash, restore and persistent undo; feed updates retain deletion | `tests/trash.rs`, `tests/article_deletion.rs` |
| Dark responsive interface | Lume/Nord/Gruvbox, persisted widths, panel visibility, compact mode and fullscreen | `tests/fullscreen.rs`, `tests/interface.rs`, `tests/library_workflows.rs` |
| Collapsible panes and gestures | All four panes collapse/reopen; persisted order/widths, divider/grip/title dragging with target preview, refresh-safe capture and Ctrl-w/Alt-arrow equivalents | `tests/panes_mouse.rs` |
| Pointer-following motion | Bounded frozen pane follows the exact grab offset; release settling, no input wait, controlled time/capture limits | `tests/panes_mouse.rs`, `src/app/pane_motion/tests.rs`, `benches/workloads/pane_drag.rs` |
| Pane-local mouse selection | Exact Unicode/code selection, pointer-pane wheel, modal isolation, idle-motion filtering and balanced capture | `tests/panes_mouse.rs`, `src/terminal/input/tests.rs` |
| Article annotation panel | Cached source-position ordering, explicit unplaced/general notes, safe source jumps and note-only deletion/undo | `tests/panes_mouse.rs` |
| Layout guidance | `:help layout` opens settings, widths, focus, visibility, Zen and fullscreen instructions | `tests/live_reader.rs`; `src/app/help.txt` |
| Vim reading | Counted word/WORD, end/previous-end, character, row, viewport, paragraph, bracket and find motions | `tests/vim_reader.rs`; `src/app/vim/` |
| Exact cursor/selection | Visible block cursor, grapheme-safe visual selections and OSC 52 yank | `tests/reader_cursor.rs`, `tests/passages.rs`, `tests/clipboard.rs` |
| Comments and highlights | Private notes, exact source-bound quotes, editing, deletion and undo | `tests/interactions.rs`, `tests/passages.rs`, `tests/study.rs` |
| Programming languages | C/C++, Rust, HCL, JS/TS, Go, Java, Python, Bash, SQL, YAML, JSON, HTML/CSS | `tests/highlighting.rs` checks actual token colors and exact text |
| Nonblocking syntax | Immediate readable code, bounded worker/cache, exact keys, failure fallback, stable cursor and selection | `src/reader/live/tests.rs`, `tests/live_reader.rs` |
| Release-aware input | Coalesced vertical navigation; reported releases cancel vertical, horizontal and word repeats | `tests/input_batch.rs`, `tests/vim_reader.rs` |
| Full articles | Background fetch/cache, source policy, manual retry, browser fallback and independent feed/full progress | `tests/article_reading.rs`, `tests/article_cache.rs`, `tests/feed_availability.rs` |
| Reliable extraction | Per-source CSS rule, preview/confirmation, quality warnings and last-good cache preservation | `tests/extraction_rules.rs`, `tests/article_extraction.rs` |
| Transparent source health | Feed/Summary/Full origin, attempts, success/error and effective next retry | `tests/extraction_rules.rs`, `src/source/scheduler/tests.rs` |
| Reader navigation tools | Literal search with n/N, outline, jump history, code-block copy and wide-table inspector | `tests/reader_tools.rs`, `tests/vim_reader.rs` |
| Expressive smart lists | Shared live/saved AND/OR/NOT grammar, exclusions, exact token phrases, dates, author, domain, duration and tags | `tests/advanced_search.rs` |
| Query discovery | Field/operator completion and `:explain-search` including saved conditions | `tests/advanced_search.rs`, `tests/library_workflows.rs`, `src/search/` |
| Large-library navigation | Materialized page identities before metadata hydration, full filtered totals, anchor recovery and selective read-count updates | `tests/advanced_search.rs`, `tests/workspace.rs`, `tests/input_batch.rs`, `tests/read_state.rs`, `tests/reader_tools.rs` |
| Read-count acceleration | V8 covering index; real V7 upgrade preserves content, notes, folders and read/Trash counts | `src/store/migrations/tests.rs` |
| Sidebar organization | Collapsible groups, pins and custom destination order | `tests/library_workflows.rs` |
| Tags and multiple articles | Independent article/highlight tags; captured atomic bulk operations with one undo | `tests/library_workflows.rs`, `tests/study.rs` |
| Duplicate occurrences | Canonical URL grouping after filters, retaining separate discussions/notes | `tests/library_workflows.rs` |
| Study notebook | Searchable highlights, tags, related article links, due reviews and user-authored question/answer cards | `tests/study.rs`, `tests/reader_tools.rs` |
| Obsidian workflow | Incremental Markdown export, stable identities and conflict reporting preserving local edits | `tests/study.rs` |
| Continue and new arrivals | `:continue` and `:since-last-visit`, using durable progress/session boundaries | `tests/library_workflows.rs` |
| Configurable shortcuts | Command bindings respect modal editors and input batching | `tests/library_workflows.rs` |
| Parallel collection | Four bounded lanes, subscription-host pacing, incremental results and durable Retry-After | `src/source/scheduler/tests.rs`; real HTTP qualification remains required |
| Performance evidence | Correctness-checked representative/stress frames, smart-list switches, reflow and concurrent writes | `benches/workloads/smoothness.rs`, `benches/workloads/expansion.rs`, `benches/workloads/panes.rs`; [validation](validation.md) |
| Inline images | Bounded raster/SVG previews; Foot Sixel strips, popup-aware clipping, click-to-inspect and source zoom/pan; portable halfblock fallback | `tests/images.rs`, `tests/image_inspector.rs`, `tests/sixel_images.rs`, `tests/image_http.rs`, `tools/qualify_foot.py` |
| Native terminal image protocols | Kitty, Sixel and iTerm2 inspector encodings, conservative automatic selection and manual override | `tests/image_inspector.rs`; built-in encoders remain confined to the inspector |

## Deliberate product boundaries

Folders are flat, with multiple membership; sidebar sections provide hierarchy.
Mouse gestures require terminal reporting; status reports observed events,
not a capability acknowledgment. Alacritty X11 input, clipboard, restoration
and image pixels are qualified in an isolated Xvfb display; other compositors
and human perception remain outside that evidence.
Vim support is a documented read-only subset: fixed Unicode word classes, display-row
motions and word/paragraph visual objects; no publisher-text editing engine or
configurable `iskeyword`. Quotes retain exact source identity across reflow;
ambiguous publisher revisions do not receive guessed highlight anchors.

Extraction does not execute JavaScript or bypass login/paywalls. A successful
extraction is not proof that every publisher paragraph was recovered. Source
health and provenance expose failures, summaries and manual overrides. Curated
source installation is qualified locally; current live feed availability is
not established by offline tests.

URL grouping retains separate occurrences rather than merging annotations.
Study schedules use explicit intervals; they do not infer recall quality.
Incremental exports report conflicts instead of overwriting edited files.
Reported key releases cancel queued repeats; legacy input cannot reveal the
physical instant a finger leaves a key. Ongoing storage/layout/draw operations
are not preempted, and no hard real-time guarantee is made.

Real loopback HTTP and the pinned published-registry build are qualified.
Current live-publisher availability, connected dependency/advisory audits and
physical terminal latency remain explicit validation limits.


The interactive publication review additionally qualifies ordered background
read flags, independent catalog counts, retained source previews, indexed long
code rows and shared popup footprints. Native regression coverage lives in
`tests/background_navigation.rs`, `src/reader/viewport/tests.rs` and
`tests/sixel_images.rs`. `make check-latency` requires real Foot/Wayland and
loopback HTTP; it measures matching-frame terminal acknowledgement, not physical
presentation. `make bench-code` records repeated checked CPU/resource evidence.
