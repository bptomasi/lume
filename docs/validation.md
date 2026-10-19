# Validation record — Lume

## Continuous horizontal reader movement — 2026-10-19

`make check` passed with pinned Rust **1.99.1** and locked published sources:
**431 native tests**, all checked Rust workloads, **10 installer tests**,
Clippy, formatting and Python tooling syntax. No sources or toolchains were
substituted. Reader `l`/Right continues at the next display row; `h`/Left
returns to the preceding row. Counts preserve grapheme boundaries and stop at
document boundaries, including on empty rows. Visual selection across a soft
wrap retains the original intervening space, and the viewport follows the
cursor. Two new native scenarios exercise these behaviors through actual key
dispatch. The existing Unicode motion scenario now asserts reverse wrapping.

The checked reader workload includes repeated forward/backward row crossing
over the existing representative article/highlight fixture. It checks both
destinations and uses cached display rows; no speedup is inferred from this
change or its verification-mode timing.

## Local For you recommendations — 2026-10-19

`make check` passed using exact Rust **1.99.1**, locked published sources and
no dependency/source/toolchain substitution: **429 native tests**, all checked
Rust workloads, **10 installer tests**, examples and Python tooling syntax.
There are 23 new recommendation/storage/migration/interaction scenarios.
They cover publisher-only phrase/boolean matching, configured related vocabulary,
private-note separation, observed code/announcement evidence, source/time
preferences, exclusions, duplicate grouping, reversible explicit feedback,
Trash/undo, unchanged ingestion, URL/full-cache invalidation, ranked pagination,
64-row batches and restart. A real V9 database upgrades without altering private
notes, favorites, memberships or article text. Frames cover tiny through 144×44
viewports; 24 existing visual snapshots changed only for the new sidebar row
and consequent vertical/style shifts, across all three themes.

Actual key dispatch qualifies commands, Space R, saved settings, explanations,
Reader selection preservation and Notebook scope guards. Real SQLite locks
prove that input remains admissible while writes cannot commit. Queue overload
rejects a ninth unacknowledged edit, and shutdown preserves all eight admitted
preferences without replacing them. Preference and feedback writes own captured
identities; deletion checks share their transaction snapshot.

`make check-release` was attempted in the sandbox and reached its HTTP stage,
where socket creation is prohibited. A combined outside-sandbox attempt timed
out in automatic permission review before execution. Its constituents were then
qualified separately: `make check` in the sandbox, **all 8 real HTTP cases**
through `make check-http` outside it, and `make build` with `--locked --release`.
No HTTP case was skipped and no containers were started. These are constituent
results, not a claimed successful combined command in the sandbox.

A separate final release-executable Linux PTY run at 140×36 passed initial
recommendation publication, Space R, captured useful feedback, provenance
explanations, time preferences, shutdown write draining, private-note
preservation and terminal-attribute restoration. This is not physical Alacritty
paint/input qualification. The actual recommendation preview was exported from
Ratatui and visually reviewed in `preview-recommendations.png`.

### Measured recommendation workloads

`python3 tools/bench_recommendations.py --baseline-report
 docs/recommendation-performance-before.json` records three release runs at
10,000 and 50,000 articles over ten sources. The
[raw report](recommendation-benchmarks.json) records environment, exact binary
checksum, process wall time, peak RSS, CPU and filesystem block counters.
[Before measurements](recommendation-performance-before.json) retain three
runs of the experimental pre-optimization binary on the exact same 10k fixture.
This is an experimental baseline, not a previously shipped release.

Classification/publication, cached reads, single publisher changes, a useful/
clear feedback cycle and actual source navigation/frames all check correctness.
Setup uses normal bounded ingestion and is excluded from operation timers,
but included in process resource measurements. Body fixtures are repeated
publisher prose; this stresses library size rather than maximum document size.
Values below are medians of three per-run means in milliseconds.

| Workload | Before, 10k | After, 10k | After, 50k |
| --- | ---: | ---: | ---: |
| Complete initial incremental classification | 15340.492 | 920.017 | 10996.161 |
| Cached count + 100 metadata rows + idle refresh | 4.090 | 0.108 | 0.106 |
| One changed article + classification/publication | 4.253 | 4.478 | 10.504 |
| Useful + clear feedback, cached evidence reweight | 246.645 | 256.695 | 1346.473 |
| Article down/up + two actual TestBackend frames | 0.484 | 0.495 | 0.491 |
| Navigation pair p95, ms | 0.517 | 0.508 | 0.540 |
| Process peak RSS, KiB | 18,564 | 18,476 | 21,984 |

The observed gains concern initial batching and cached page queries; the other
workflows do not show a universal improvement. Feedback can reweight many
cached rows and remains off the UI thread. Initial indexing publishes partial
shortlists; production's event-poll cadence adds time beyond the batch-only
measurements. TestBackend source navigation here uses synchronous source loads;
these statistics exclude physical input, terminal-emulator/display latency and
the interactive worker polling delay. They are not idle-app RAM, allocation,
battery, hard real-time or recommendation-quality guarantees. Host scheduling
and cache state were not controlled. Recommendation usefulness requires
explicit reader feedback; vocabulary/code signals cannot verify correctness.

## Indexed study notebook — 2026-10-18

`make check` passed as the local constituent of `make check-release`, using
exact Rust **1.99.1**, locked published sources and no source/toolchain
substitution: **406 native tests**, all correctness-checked Rust workloads,
**10 installer tests**, examples and Python tooling syntax. The native harness
and custom workload runner now both receive `--test`: native tests still all
run, while the custom runner uses its existing verification mode. Full repeated
measurements remain separate through `make bench` and `make bench-notes`.

Migration V9 is exercised against a real V8 SQLite library, including private
text, tags, related articles, review dates, cards, favorites and folders, with
an idempotent reopen. Original migration bytes are unchanged. Independent FTS
scenarios verify same-note predicates, Unicode/phrases, bounded pages/snippets,
stable IDs, code-fence exclusions, backlinks through deletion/undo, retired
references, recovery collisions and checkpoint consumption. Version restoration
rejects stale previews, and the 100-version limit survives deletion/undo cycles.

Twenty-three Notebook interaction scenarios cover Reader-only creation, correct
cross-article command targets, protection of hidden article selections, query
cancellation/collections/history, source jumps and return, scoped keyboard/mouse
bursts, tags/types/pins/resolution, synthesis citations, reference completion,
editor search/templates, recovery, version previews and source repair. Actual
SQLite lock contention verifies that a write does not block input or close text
typed after its request. Queued toggles resolve against transactional state.
Context end/reflow and backlink selection remain within their viewports. Frames
are exercised at 1×1, 40×18, 80×28 and 144×44 across all three themes.
Compatibility commands for highlight tags, direct cards, related articles and
article-note exports use the same ordered interactive writer and captured owner.

Separate export/review cases verify exact-byte file ownership, stable wikilinks,
source citations, unchanged-file reconciliation and local edit preservation.
Postponement does not count a completed review; due-card filters apply before
the result limit. Existing editor, article reading, terminal input, extraction,
syntax, layout, drag and mouse cases continue to pass.

Exact editor bytes are also verified: indentation, tabs and trailing newlines
survive paste, commit, version undo and yank. Indented wikilink examples remain
code rather than silently becoming graph edges after trimming. Validation still
rejects empty general notes and terminal controls; text itself is not trimmed.

The dependency change enables the already pinned editor's search feature. It
adds one dependency edge to an already locked regex package, with no version,
package or accepted-license changes. Selection and responsibilities are recorded
in dependency-security.md and architecture.md.

`make check-release` remains unsuccessful at its explicit real-HTTP constituent:
all **8** loopback cases fail when creating the test server socket with
`Operation not permitted` (OS error 1). This is not a skipped or passed HTTP
gate. `make check-alacritty` also fails its prerequisite check because `xdotool`
is absent; no real Alacritty window or physical key-release claim is made.
No containers or services were started. Local HTTP qualification and Alacritty
qualification must still run in a host with their required capabilities.

`make build` passed with `--locked --release`. A separate **140×36 Linux PTY**
run against the final release executable passed Reader-only creation, bracketed
paste, repeated `:w` without ID duplication, Notebook query/pane rendering,
exact OSC52 yank, the context creation guard, `dd`/undo ownership, source jumps
and `Ctrl-o` return, clean exit and terminal-attribute restoration. A PTY is not
an Alacritty window and does not qualify physical key-release behavior.

The actual 144×44 Notebook cells were exported with the preview example and
rendered for visual review in `preview-notebook.png`. No mock screenshot is used.

### Notebook performance evidence

`make bench-notes` records three release runs of each representative/stress
suite in [notebook-benchmarks.json](notebook-benchmarks.json). The comparable
pre-change annotation fixture is retained in
[notebook-performance-before.json](notebook-performance-before.json).
Rust is 1.99.1 on the reported Intel Core Ultra 7 155H/Linux host. The artifact
records the exact benchmark binary SHA-256, process wall time, peak RSS, CPU time
and filesystem block counters. Setup is excluded from operation timers and
included in process resources; every measured operation checks correctness.

The original annotation suite retains the same 9,290/46,890-byte documents,
20/200 annotations and forty mutation iterations. Values below are medians of
three per-run means, in milliseconds; they are not terminal-emulator latency.

| Annotation operation | Before, 20 | After, 20 | Before, 200 | After, 200 |
| --- | ---: | ---: | ---: | ---: |
| Hover + popup/dismissal frames | 0.221 | 0.268 | 0.236 | 0.268 |
| Edit + `:w` + unchanged `:w` + frame | 1.168 | 0.824 | 5.211 | 3.135 |
| Card edit + writes + schedule check | 0.569 | 0.234 | 2.668 | 0.837 |
| Contextual `dd` + undo + restored marker | 2.084 | 1.198 | 10.023 | 5.394 |

These observations support lower costs for those write/delete workflows in
these fixtures. Hover is slightly slower here, so there is no blanket speedup
claim. Median process peak RSS is 12,876→13,396 KiB for the representative
annotation suite and 16,248→16,120 KiB for stress. The synchronous write suite
is comparable before/after; separate lock/failure tests qualify interactive
queue responsiveness and preservation of newer edits.

The new Notebook suite uses 10,000/100,000 notes across 100/1,000 articles.
It checks same-note FTS/tag/type/status filters, a late page and highlighted
snippets; coherent source context; recovery/reference/metadata/review/anchor/
version transactions plus seven exact undos; interactive context/frame delivery;
and atomic/incremental note files. Study fixtures contain four/sixteen cards.
Export fixtures contain 100/500 note files.

| Notebook workload | 10k notes | 100k notes |
| --- | ---: | ---: |
| Filtered FTS + last page + snippets, mean ms | 11.120 | 70.732 |
| Selected context + Markdown, mean ms | 0.118 | 0.062 |
| Complete study/undo cycle, mean ms | 4.543 | 5.584 |
| Navigation + ready context + frame, p50 ms | 0.492 | 0.281 |
| Navigation + ready context + frame, p95 ms | 0.727 | 0.450 |
| Navigation + ready context + frame, p99 ms | 1.395 | 0.494 |
| First 100/500 file publication, ms | 18.512 | 57.053 |
| Unchanged 100/500 file reconciliation, ms | 13.753 | 44.236 |
| Process peak RSS, KiB | 18,872 | 22,672 |

Navigation percentiles cover 200 alternating note moves per run, ready source
context and actual TestBackend frames. They are observed sorted statistics at
floor(N×p), with `yield_now` polling. Production polls active work every 16 ms;
these numbers do not measure the terminal emulator, display, physical input or
that event-poll delay. Full FTS reads remain slower than cached navigation and
run on a replaceable worker. Host scheduling/cache state were not controlled.
No allocation-count, idle-CPU, real-time or all-library-size guarantee is made.

## Vim-style note writes — 2026-10-18

`make check` passed with exact Rust **1.99.1**, published registry sources and
the locked graph: **362 native tests**, the ordinary correctness-checked
workloads, **10 installer tests**, examples and tooling syntax. No source or
toolchain substitution, dependency update, schema change or migration was used.
The new card-text update is operational SQL at its store owner.

Eighteen editor scenarios exercise actual key dispatch, separate command-line
paste, write/quit aliases, clean/dirty guards, repeated writes to one captured
note/card ID, unchanged-write idempotence, quotation preservation, Unicode
motions and command limits, ZZ/ZQ, Ctrl-s in all editing layers, session writes,
failed validation, deleted targets and session failure after a note commit.
The latter retains the already saved ID for retry. Store tests additionally
reject wrong card owners, missing cards, invalid text and Trash without an
undo entry, and verify that an edited card keeps its review schedule.
Existing extraction-preview tests now require explicitly closing the editor:
Esc alone preserves the draft and continues deferring the background preview.

Real Ratatui buffers verify dirty titles, separate Ex text and matching global
mode labels, with 1×1, 24×10, 40×12 and 100×30 viewports. The actual 144×44
editor preview was visually inspected after exporting its cells with
`cargo run --locked --example preview -- /tmp/note-write.json note-write`.
It shows the draft, command line and mode-specific hints independently.

`make build` passed with `--locked --release`. A separate release-executable
Linux PTY run verified bracketed paste containing literal Ex text, repeated
`:w` without duplicate notes, `:wq` returning to reading, a refused dirty `:q`,
and `:wqa` persisting the draft before exit and restoring original tty modes.
This qualifies terminal I/O, not physical Alacritty paint/input.

`make check-release` passed its local phase, including the note/card write
workloads, then **failed all eight real HTTP cases** at loopback socket creation
with `Operation not permitted (os error 1)`. No case was silently skipped.
`make check-alacritty` failed its prerequisite because **xdotool is missing**.
Release HTTP and physical GUI qualification remain incomplete in this sandbox.

[Optimized write measurements](editor-write-benchmarks.json) record three
representative and three stress runs, with environment, binary checksum,
process wall time and peak RSS. Each has 40 note edit/write/unchanged-write/
editor-frame cycles and 40 card edit/write/unchanged-write/schedule checks.
Setup is excluded from operation timings; each operation verifies correctness.
The representative library has 200 paragraphs / 20 annotations; stress uses
1,000 paragraphs / 200 annotations. Note cycles measured 1.168–1.224 ms and
5.098–5.124 ms respectively; card cycles measured 0.567–0.592 ms and
2.634–2.678 ms. Whole-process wall/RSS also include fixture setup and the
hover/deletion workloads: 0.26–0.94 seconds and 12,452–16,312 KiB. These are
absolute measurements, without a comparable previous write baseline or a
physical input-to-display latency claim.

## Contextual article notes and pointer previews — 2026-10-18

Final `make check` passed with pinned Rust **1.99.1**, the published locked
registry graph and no source substitutions: **343 native tests**, ordinary
correctness-checked workloads, **10 installer tests**, examples and tooling
syntax. `make build` passed with `--locked --release`. No dependency, feature,
schema or migration changes were needed for this note interaction work.

Sixteen new native scenarios exercise cursor-local `dd` independently of the
previously selected comment, annotation-panel deletion, repeat suppression,
Escape/motion/focus-loss/reload cancellation, deletion undo and publisher-text
preservation. They also check exact-note editing without quotation changes,
Vim find/visual-selection precedence, reader-search `n`, current-row annotation,
explicit general notes, overlapping passages, ambiguous refreshed quotations,
Unicode hover, popup clicks, horizontal-scroll dismissal, disabled mouse and
safe failure for an invalid cursor. A jump to an overlapping annotation retains
that matching note as the deletion target.

Popup placement/painting is checked at 24×10, 40×18, 80×24 and 140×36 in all
three themes. The existing 36 cell snapshots pass; 18 reviewed snapshots
change only their reader footer to advertise `n note` and `:annotate` when no
search is active. Their trailing spaces remain deliberate exact-width cell
fixtures. [The note preview](preview-notes.png) was exported from actual Ratatui
cells and visually inspected, including the gutter, source quote, editor hint
and numbered position-ordered annotation cards. Reproduce its cell export with
`cargo run --locked --example preview -- /tmp/notes.json note-hover`.

A separate release-binary Linux PTY qualification created and persisted a
current-row passage note in an isolated demo database. SGR all-motion input
produced the expected popup text fragments; a reported popup click opened the
same note for editing without changing its quote. Keyboard `dd` removed only
the note, `u` restored the edited note and quotation, and normal exit restored
the original tty attributes. Ratatui may omit unchanged spaces between emitted
fragments; the native buffer assertions independently verify complete visible
text. This is terminal I/O qualification, not a physical-display measurement.

`make check-alacritty` failed its tool prerequisite because **xdotool is missing**;
it did not qualify an Alacritty window. No GUI test result is claimed. No HTTP
boundary changed; the previously recorded loopback restriction remains outside
this UI qualification.

[Fresh optimized measurements](note-interaction-benchmarks.json) retain three
representative runs (200 paragraphs / 20 annotations) and three stress runs
(1,000 paragraphs / 200 annotations). Both check 400 coalesced hover/preview/
dismissal pairs and 40 `dd`/undo cycles per process. Hover plus two TestBackend
frames measured 0.229–0.246 ms per cycle; deletion plus undo measured
2.128–2.220 ms representative and 10.109–10.236 ms stress. Operation timings
exclude setup; process wall/RSS include it. These are absolute measurements
without a comparable prior baseline or a physical input-to-display guarantee.

## Installed library paths and safe migration — 2026-10-18

`make check` passed with pinned Rust **1.99.1**, the ordinary Cargo home and
published registry sources, without path patches or source substitutions.
The Linux-filtered locked metadata contains 220 packages, with every external
package using a registry source. Cached Linux dependencies are now available;
the earlier DNS failure is no longer a build blocker for this host target.
`make build` passed with `--locked --release` and the unchanged lock graph.

New native tests check the fixed home-relative default across two working
directories, ignored legacy XDG configuration, the environment override without
HOME, directory flag precedence, separate demo data, empty-directory failures
and explicit database compatibility/conflicts. Ten Python installer tests
check a live WAL snapshot with an uncommitted writer, preserved notes/folder
membership/read/favorite flags, corrupt sources, backup deadlines, ambiguous
legacy paths, destination races, executable-copy failure and reinstallation.
They also exercise the Make target with isolated inputs and retained notices.

A separate end-to-end qualification used the real release executable and real
`make install` in a temporary prefix containing spaces. Migration retained
98 bundled subscriptions and the source database. The installed binary opened
identical library contents from two different working directories. Directory
flags overrode environment settings; environment-only selection also worked.
Installed executable bytes and retained license notices were checked. All
library and installation destinations were temporary; no private user library
was read, migrated or changed by these tests.

`make bench-tools` passed correctness checks for three snapshots each of
1,000 articles with 2 KiB bodies and 10,000 articles with 4 KiB bodies.
[Recorded measurements](install-benchmarks.json) include wall time, database
size, Python/SQLite/OS versions and Linux process peak RSS. Setup/verification
are excluded from timings; RSS includes setup and is not an allocation delta.
These are migration measurements without a comparable earlier baseline;
they establish no navigation or migration speedup.

`make check-release` passed its complete local prerequisite, including the
installer tests and tooling syntax, then failed all eight actual HTTP tests.
Binding a loopback socket still returns `Operation not permitted (os error 1)`.
No required HTTP case was skipped, and release qualification remains incomplete
at that boundary. The separate successful release build qualifies compilation
and the tested installer, not real HTTP. Alacritty was not rerun for this path
change; its harness now sets LUME_DATA_DIR to retain test isolation.

## Published-registry lock correction — 2026-10-18

This correction supersedes prior claims that the substituted sources and
portable lock represented an equivalent registry build. A cached source
labelled `tendril` 0.5.0 omitted its published `utf-8` dependency. Comparing
package/version lists was insufficient to verify source equivalence.
Previous native/benchmark/PTY results remain observations of the substituted
build only; they do not establish that the published-crate build passes.

Cargo resolved the existing manifest without path patches and added only
`utf-8` 0.7.6, including its registry checksum and the `tendril` edge. All 266
existing package identities/versions remain; the corrected graph has 267
packages. There are no direct dependency upgrades, feature changes, Rust code
changes or migrations. Upstream permissive license texts and the notices
inventory were updated. `--locked` remains enabled in every standard gate.

The original lock error was reproduced before correction. With the corrected
lock, `make build` passes resolution and attempts to download published
crates; it then fails because this executor cannot resolve `static.crates.io`.
No path patches were re-enabled to claim success. Full compilation and native
checks against published sources remain unqualified until downloads work.

## Review, centered fullscreen and precise wheel input — 2026-10-18

Fullscreen now centers its text at **80 columns** by default. The independent
`:fullscreen-width 40–140` preference reflows immediately, persists through
restart/profiles/history, and defaults correctly in older JSON. Normal panel
prose retains its existing width. Wrapping, painting and pointer selection
share the same centered rectangle. Settings scroll the selected field into
view even at 24×10. Native tests cover exact geometry, saved profiles, rejected
values without preference changes, preview cancellation and small terminals.

Removed application-side wheel multipliers: one reported event moves one
visual row, destination, picker entry or horizontal column. Coalesced bursts
retain exactly their event count and direction changes. Tests independently
check individual steps, boundaries, bursts, modal fields and library focus.
Alacritty's own default multiplier is a separate setting; the README documents
`[scrolling] multiplier = 1`. The user's terminal configuration was read for
diagnosis and remains unchanged. Pagination arithmetic also now shares the
storage contract's effective 1–500 limit, avoiding division by zero and cursor
arithmetic against a different limit than SQLite uses.

The root launcher was incorrectly ignored by the `lume` pattern. Removed that
pattern; `/target`, private libraries and bytecode remain ignored. No library
data, migrations, Rust dependencies or features changed. Selection benchmarks
now check colors relative to the actual text column, and the fullscreen
workload independently requires an 80-column block centered at column 40 in
a 160-column terminal. Previous full-width benchmark expectations were updated
to the requested behavior rather than removed.

Formatting, warnings-denied Clippy, **319 native tests**, all ordinary
correctness-checked workloads, examples and tool syntax checks passed.
An additional optimized stress reading-tools run checked 1,000 paragraphs,
200 highlights, 8,000 grouped-source articles and 4,000 trashed articles.
These are correctness qualifications, not evidence of a new speedup. Earlier
benchmark JSON records describe their original source checkpoints and are not
relabelled as measurements of this review.

The optimized binary, examples and benchmark rebuilt with Rust **1.99.1**.
Qualification used the same 41 temporary cached dependency-source overrides
and unchanged 266-package version/dependency graph. The portable lockfile and
temporary Cargo configuration were restored afterward. Isolated PTY
qualification passed the existing SGR/CSI mouse, Unicode clipboard,
annotations, split/workspace/history and terminal-restoration scenarios, plus
fullscreen width changes, invalid-value retention and saving the live width
preview. A failed ex command correctly keeps its draft open; the driver now
closes that draft before issuing another command. Publisher/cache rows stayed
unchanged. Updated fullscreen, settings and help previews were inspected from
the real Ratatui cell buffers; their Pillow rasterization is not Alacritty.

`make check-release` passed its local gates and **failed all eight real HTTP
cases** at loopback binding with `Operation not permitted`. The independent
`make check-http` reproduces this without silently skipping a case. Actual
Alacritty 0.15.1 launches failed with Wayland `NoCompositor` and X11
`XOpenDisplayFailed`. The new `make check-alacritty` harness additionally reports
the missing local xdotool dependency as a failure. GUI qualification has **not
passed locally or in a dispatched CI run**.

CI now separates local checks, HTTP, release build and Alacritty qualification,
so a preceding HTTP failure does not silently prevent the GUI attempt. The
real GUI harness uses Xvfb, XTest keyboard/mouse input, independently observed
OSC52 clipboard text, durable layout assertions and termios restoration.
An isolated configuration sets the wheel multiplier to one. Xwd captures and
JSON results are retained even on failure. It never opens the real reading
database or edits the user's terminal configuration. Its successful physical
execution, including a human review of captured paint, remains outstanding
until run in an environment with those tools and display permissions.

## Bounded interactive workspaces — 2026-10-18

The interactive terminal now enables separate bounded workers for replaceable
navigation reads, pure Markdown layout and ordered progress/preferences writes.
Syntax keeps its independent worker. Requests carry selected page indices and
article identities; generation checks reject stale results. Additional steps
inside a page still loading replace its captured selection. Loaded-source
identity guards progress, and a 512-position LRU retains the newest admitted
positions beyond the 64-slot writer queue, keyed by article, source and revision. Width changes preserve source
anchors and live selections, including documents followed by private notes.
Explicit source-bound tools keep immediate mapping qualification paths.

Layouts support binary row/column divisions with shared command/mouse docking,
shared rendered resize seams, collapse and responsive suppression. A row seam
uses the leading pane's bottom edge so the next title's controls remain usable.
Two columns or one row activate dragging; a thin insertion seam replaces the
large drop label. Oversized panes use bounded summary cards. Reading, Study,
Browse and user-named profiles retain layout only; 32 layout undo/redo entries
persist separately from article undo. Fullscreen profile/history restoration
forces focus onto the visible reader rather than a hidden sidebar.

`make check` passed formatting, warnings-denied Clippy, **310 native tests**,
the ordinary correctness-checked benchmarks, examples and tooling syntax.
Coverage includes a deliberately locked writer, stale/burst reads, pending-page
intent, source-switch cancellation, real syntax completion, saved anchors with
private notes, layout/profile restart, resize cancellation, keyboard resizing with collapsed siblings, keyboard/mouse
geometry equivalence, compact corners and wide-glyph clipping, atomic
journal rollback, persistent
write-failure reporting, bounded progress eviction and independent Feed/Full
revision positions. A fullscreen-profile
regression first reproduced the hidden-focus error before the owner fix.
36 reviewed text/style framebuffer fixtures span three themes, three sizes
and split, prefix-guide, selection and drag scenes. Ordinary tests compare
fixtures; only explicit regeneration updates them.

The optimized binary, examples and benchmark use pinned Rust **1.99.1** with
41 temporary cached upstream source substitutions and the unchanged
266-package version/dependency graph. The portable Cargo.lock and original
temporary Cargo configuration are restored after qualification. No dependency,
feature or migration changed; qualification used isolated test/demo databases
and did not open the user's real reading database.

The optimized isolated SGR/CSI PTY smoke passed prior mouse/keyboard, clipboard,
annotations and terminal-cleanup scenarios, plus row/column splits, named
profile switching, the Ctrl-w guide, undo/redo and preview-only row resizing
followed by durable release. Publisher/cache rows remained unchanged. Its
color test child alone removes inherited `NO_COLOR`. The [Study preview](preview-study.png),
[prefix guide](preview-layout-guide.png), 36-fixture contact sheet and refreshed
48-frame drag animation were reviewed from actual cell buffers.

[Repeated optimized measurements](workspace-benchmarks.json) separate initial
input/placeholder frames from completed text frames. Both synchronous and
background paths run against the same 2,000/10,000-article fixture, eight smart
lists and 320 navigations per mode per process. Each size has three processes,
40 bursts of 200 requests and 40 profile/guide/undo cycles. The pane workload
also checks and times bounded compact captures and their moving corners.
Reflow adds 160
width changes per process with 7,248/72,336 source bytes and 20/200 private
notes, checking exact source and selected bytes on every completion. Whole
process wall time and peak RSS include setup. These are TestBackend timings,
not physical key-to-paint latency or allocation counts. Completion uses a yield
loop and excludes the terminal runtime's 16 ms wake cadence. The final timing
trials ran after local gates and the PTY smoke; host scheduling and caches
were not isolated. No claim
that SQLite queries themselves became faster follows from moving their work.

`make check-release` reached its real HTTP stage and **failed all eight cases**:
the sandbox rejects loopback socket creation with `Operation not permitted`.
Those cases were attempted, never silently skipped. A real Alacritty launch
was also attempted with an isolated demo/configuration and failed with
`WaylandError(Connection(NoCompositor))`; X display access was unavailable too.
Physical Alacritty paint/input and clean-registry/real-HTTP release qualification
remain outstanding. Source-bound jumps, personal transactions, catalog/count
reloads and some exchange work remain synchronous; private-note composition
also stays with its input owner. There is no hard latency guarantee.

## Continuous rounded drag surfaces — 2026-10-18

The four-corner background mask left a visible whole-cell staircase around
otherwise rounded glyphs. The held pane now uses the same dim neutral surface
as the workspace across its entire rectangle. Its border and text remain
bright; code, selection and control backgrounds retain their captured colors.
All cells still overwrite underlying content, including the corners. Captures
remain unchanged, and release restores the original surface along with the
canvas. This avoids both the bright rectangular plate and corner-cell notches
without assuming pixel clipping or a terminal graphics protocol.

`make check` passed formatting, warnings-denied Clippy, **287 native tests**,
the ordinary correctness-checked benchmarks, examples and tooling syntax.
Updated framebuffer coverage checks all cells over conflicting background
content, preserved code colors and source immutability. Mouse scenarios check
adjacent corner, border and body colors in all three themes, clipping and
Escape restoration. Existing complete-outline, neighbor insertion and
release-transition scenarios remain passing.

The optimized binary, examples and benchmark use exact Rust **1.99.1** with
41 temporary cached upstream source substitutions and the unchanged
266-package version/dependency graph. The portable lockfile and original
temporary Cargo configuration are restored after building. No dependencies,
migrations or real-library data changed. The isolated optimized SGR/CSI PTY
smoke passed mouse and keyboard movement, release order, Unicode yank,
annotation operations, publisher-cache preservation and terminal cleanup.
Only its color-enabled child removes inherited `NO_COLOR`.

The actual [held-frame corner enlargement](preview-corner.png) was visually
reviewed, alongside the refreshed [48-frame animation](preview-drag.gif) and
[held Reader preview](preview-reordering.png). The Pillow font rasterizer is
not Alacritty; physical Alacritty input and paint remain unqualified. Glyph
curvature depends on the terminal's font or built-in box drawing. The uniform
surface fixes the application's rectangular color boundary, rather than
promising pixel-level rounded clipping. Animation pacing is illustrative.
[Fresh measurements](pane-curves-benchmarks.json) retain three optimized
processes per size, wall time and peak RSS, without a speedup claim.

## Footprint-driven insertion targets — 2026-10-18

Reproduced the reported problem before changing the controller: Reader already
overlapped Articles, but its grip remained inside Reader and the candidate
was `None`. Insertion now follows the panel's leading edge and vertical
coverage against original slot geometry. Crossing 25 percent of the smaller
pane surface enters a slot; retaining it down to 12.5 percent prevents boundary
jitter. Already crossed slots remain eligible near terminal edges. Explicit
top tabs retain pointer targeting, including collapsed panes, while drops
outside the viewport or vertically clear of the workspace cancel.

Native tests qualify anticipatory left/right insertion, committing while the
grip remains in the source, exact entry/retention boundaries, repeated small
reversals, farthest-slot selection and offscreen/vertical behavior. Existing
rounded-outline coverage allows a legitimate insertion marker across the old
slot while still rejecting stale corners. Benchmark gestures now place the
panel footprint over its destination; the insertion workload independently
checks the neighbor, the empty slot and unchanged saved order.

`make check` passed formatting, warnings-denied Clippy, **287 native tests**,
the ordinary correctness-checked benchmarks, examples and tooling syntax.
Validation uses pinned Rust **1.99.1**, 41 temporary cached upstream source
substitutions and the unchanged 266-package version/dependency graph. The
portable lockfile and original temporary Cargo configuration are restored
after building. No dependencies, migrations or real-library data changed.
Physical Alacritty input/paint remains outside this environment's qualification.

The optimized isolated SGR/CSI PTY smoke passed anticipatory Reader insertion
and release while its grip remained inside the original Reader rectangle.
The saved order stayed unchanged during the preview, then matched the
footprint-based candidate on release. Existing mouse/keyboard, Unicode yank,
publisher-cache preservation and terminal cleanup scenarios also passed.
Only the isolated color test child removes inherited `NO_COLOR`.

The [48-frame preview](preview-drag.gif) now drags Reader left while its grip
stays inside Reader throughout the gesture. Library starts moving as the
panel covers it. Pacing remains illustrative rather than a transport benchmark.
[Fresh measurements](pane-overlap-benchmarks.json) retain three optimized
processes per size, wall time and peak RSS, without claiming a speedup.

## Rounded surfaces and live neighbor insertion — 2026-10-18

Rounded border glyphs previously retained the opaque panel background across
the whole corner cell. The compositor now keeps the underlying backdrop color
at each of the four corners, while preserving the opaque panel interior and
border foreground. A framebuffer test independently checks corner and
interior colors; the multi-theme mouse test checks the dimmed corner backdrop.

A drag captures bounded neighbor pictures alongside the held pane. The same
insertion-order function drives transient targets and durable commits. Neighbors
slide into their prospective slots, opening a gap before release. Reversing
the pointer retargets from the last displayed position. Stable original hit
regions avoid feedback from moving images. Previewing never saves layout or
reflows text. Repeating the same target does not restart a transition, and a
finished held preview becomes idle. Release preserves the current positions
until settling completes, including when the held pane is already stationary.

Native scenarios qualify intermediate and final neighbor positions, unchanged
saved order, reversal without jumping, release continuity, a stationary held
pane during neighbor return, Escape/resize cancellation and the snapshot
budget. Captures are limited to 20,000 cells per pane and 40,000 cells for the
workspace. Geometry changes use real reflow. The representative motion
benchmark now includes 400 insertion-animation frames per process with
independent checks on the final neighbor position and unchanged saved order.

`make check` passed formatting, warnings-denied Clippy, **280 native tests**,
all ordinary correctness-checked benchmarks, examples and tooling syntax.
Validation uses exact Rust **1.99.1**, the same 41 temporary cached upstream
source substitutions and unchanged 266-package dependency graph. Portable
Cargo.lock and the original temporary Cargo configuration are restored after
building. No dependency, migration or real-library data changed. Physical
Alacritty input/paint remains outside this environment's qualification.

The optimized isolated SGR/CSI PTY smoke passed actual mouse input, a moving
corner with an undimmed border foreground and backdrop-colored background,
durable release order, keyboard alternatives, Unicode yank and terminal
cleanup. Its publisher rows and cache remained unchanged; `NO_COLOR` is
removed only in the test child to qualify color-enabled output.

The [animated preview](preview-drag.gif) uses 48 actual buffers, including
neighbor movement between pointer positions; pacing is illustrative. The
[held Reader preview](preview-reordering.png) also uses the actual compositor.
[Fresh measurements](pane-shuffle-benchmarks.json) retain three optimized
processes per workload, wall time and peak RSS. They measure composition work,
not physical terminal input or paint, and make no before/after speedup claim.

## Complete moving pane outlines — 2026-10-18

A native regression first reproduced the reported top-corner defect: the
captured left corner was `│` instead of `╭`. Destination hints had repainted
the source with a partial border before capture, replacing its top corners.
Hints now decorate only destinations. The source retains its entire rounded
outline, title and controls, and the vacated slot is a borderless surface
instead of a second stationary frame.

The regression covers all four panes, both a separately drawn mouse-down
and a down/drag burst before capture. It checks all four corners in the
snapshot and moved output, plus removal of the old slot's bottom corners.
The existing representative motion benchmark now checks captured and visible
rounded corners on every held frame, alongside offsets and backdrop colors.

`make check` passed formatting, warnings-denied Clippy, **272 native tests**,
the ordinary correctness-checked benchmarks, example compilation and tooling
syntax. Optimized builds use pinned Rust **1.99.1**, with the same 41 temporary
cached upstream source substitutions and unchanged 266-package dependency
graph. The portable lockfile and original Cargo configuration are restored
after building. No dependencies, migrations or real-library data changed.
Physical Alacritty paint remains outside this environment's qualification.
The isolated optimized PTY smoke passed actual SGR mouse input, an undimmed
moving rounded corner on the wire, persisted layout, keyboard equivalents,
Unicode selection, unchanged publisher content and terminal cleanup. It
removes inherited `NO_COLOR` only in that isolated child, as before.

The [updated animation](preview-drag.gif) and [held reader](preview-reordering.png)
show the complete moving outline and empty vacated slot.
[Fresh measurements](pane-outline-benchmarks.json) retain three optimized
processes per workload, including wall time and peak RSS. No speedup is claimed.

## Two-axis pane dragging and consistent surfaces — 2026-10-18

The workspace canvas now uses the panel surface in all three themes, including
the identity row, pane tabs and footer. Cursor and selection contrast retain
their independent dark foreground. Preview padding follows the actual buffer
background rather than introducing another surface color.

Held pictures follow horizontal and vertical pointer coordinates and clip at
terminal edges. During dragging, the rendered backdrop uses 60 percent of its
RGB channel values; the floating picture keeps its original colors. Each
frame starts from the regular theme, so fading cannot accumulate. Release
restores the backdrop immediately and settles both coordinates. A release
without a destination returns to the original slot without saving an order.

`make check` passed formatting, warnings-denied Clippy, **271 native tests**,
all ordinary correctness-checked benchmarks, examples and tooling syntax.
Coverage checks frozen cells across reloads, exact diagonal offsets, clipping
at the top/bottom/right, backdrop restoration across all themes, vertical-only
return motion, unchanged durable layout and the uniform normal canvas.
Partial wide glyphs at a clipped right edge are replaced with a blank cell,
preventing terminal autowrap from corrupting the following row.
The existing motion benchmark now checks vertical positions and dimmed colors
on every held frame, with changing pointer rows.

Validation uses exact Rust **1.99.1** with the same 41 temporary cached upstream
source substitutions and unchanged 266-package version/dependency graph.
The portable lockfile and temporary Cargo configuration are restored after
building. No dependency or real-library data changes are involved. Physical
Alacritty input/paint remains outside the native framebuffer qualification.

The optimized SGR/CSI PTY smoke also checks a dimmed RGB background on the
actual wire, held motion, immediate layout persistence, keyboard alternatives,
collapse/restore, Unicode yank and terminal cleanup. Its isolated child removes
the harness's inherited `NO_COLOR` to simulate the color-enabled terminal.
The [animated preview](preview-drag.gif) includes changing pointer rows and
two-axis settling. All 16 static previews use the consistent normal surface.
[Fresh motion measurements](pane-backdrop-benchmarks.json) retain three
optimized processes per workload, wall time and peak RSS; frame samples
exclude physical input/output and make no before/after speedup claim.

## Held-pane motion, compact header and ignore policy — 2026-10-18

A held pane now follows the exact horizontal grip offset as a frozen Ratatui
picture above the workspace. The original slot stays masked and drop targets
keep their original geometry. Capture happens once before terminal output,
with a 20,000-cell limit; no article reflow or database access is added to
motion frames. A 160 ms release-only transition uses elapsed-time cubic easing
and 16 ms polling while active. Delayed frames skip ahead; input, wheel, focus
loss and resize interrupt it. Durable layout commits before animation begins.
Different-size destinations use real reflow rather than stretching cells.

The identity and pane tabs now occupy two header rows. Removing the redundant
`library / Inbox` breadcrumb gives the reading area one extra row. A native
buffer test checks both controls and pane origin; the presentation benchmark
was updated from row three to row two to reflect this intentional contract.
The `.gitignore` review added private environment/tool directories, additional
SQLite transaction sidecars, Python bytecode and editor/OS recovery files.
Actual `git check-ignore --no-index` queries verified 12 generated/private
paths are ignored and 12 source/lock/example/fixture paths remain eligible,
including Cargo.lock, AGENTS.md, SQL fixtures and `.env` templates.

`make check` passed formatting, warnings-denied Clippy, **268 native tests**,
all ordinary correctness-checked benchmarks, example compilation and tooling
syntax. Native coverage adds fixed-time interpolation/late ticks, snapshot
limits, exact frozen-cell/offset drawing, held capture across article reload,
release-before-settling persistence, input/wheel/resize interruption and the
compact header. No sleeps stand in for correctness. The explicit release HTTP
gate attempted all eight scenarios; each failed at loopback bind with **EPERM**.
Full release qualification remains incomplete. A separate optimized build
passed for the executable, benchmark and both preview examples.

Exact Rust **1.99.1** and 41 temporary upstream source substitutions preserved
the same 266-package version/dependency graph. Portable Cargo.lock and the
original temporary Cargo configuration were restored. No dependency, migration
or real-library data changed. Clean-registry/advisory qualification remains
unavailable. The optimized real SGR/CSI PTY smoke passed held-pane drawing,
frozen slot, release, compact-header hit coordinates, keyboard alternatives,
Unicode yank, annotation workflows, capture suspension and terminal cleanup.
Its subscription rows, publisher bodies and full-text cache stayed unchanged.
Physical Alacritty transport/paint is not qualified by this environment.

The [animated preview](preview-drag.gif) uses 21 actual buffers exported by
`examples/drag_preview.rs` (identical GIF frames may merge), with illustrative
pacing rather than a physical-terminal timing claim. The middle drag and final
placed frames were visually reviewed. Sixteen static product previews were
also regenerated for the compact header.

[Motion benchmark data](pane-motion-benchmarks.json) retains three fresh
optimized processes for each size. Capture includes allocation and cell
cloning; 200 held frames per process validate grab offsets and 400 controlled
time settling frames validate commit order and completion. Source sizes are
7,400 bytes/20 notes and 74,310 bytes/200 notes. Event setup and initial syntax
completion stay outside operation timers. Whole-process wall and peak RSS
include setup. Frame work timings exclude the polling interval and physical
terminal output. New workloads and the taller reader make this a fresh
baseline, without a before/after speedup claim.


## Panel reordering corrections — 2026-10-18

Reproduced two failures with native tests before changing production code:
standalone Shift reports consumed the Ctrl-w prefix, and article reloads
canceled a captured panel move. The fix ignores standalone modifiers, handles
shifted base keys, and separates layout capture from article-text capture.
A third responsive-layout issue moved relative to suppressed panes without a
visible swap. Keyboard moves now prefer displayed neighbors. Header capture
keeps focus/geometry stable until release rather than replacing its target
when an inactive sidebar header is pressed.

Every pane has a visible `[≡]` corner grip beside `[−]`. Captured pane/top-tab
regions expose a typed source/destination preview; the candidate destination
turns blue, with a cleared `Drop here` label. Only release changes saved order;
Esc, focus loss, terminal resize and out-of-target drops leave it unchanged.
Collapsed top tabs accept moves without expanding. `Alt-Left/Right`, Ctrl-w
then Left/Right, Ctrl-w then Shift-h/Shift-l, and `:panel-move` share the same
owner. Edge operations report that no further move is available.

The local `make check` gate passed formatting, Clippy with warnings denied,
**261 native tests**, ordinary correctness-checked benchmarks and tooling
syntax. Nine added regressions cover protocol modifiers, background reload,
visible target geometry, grip preview/no SQL write, persisted placement after
restart, cancellation and collapsed-tab drops. Actual drawing, rather than
only the saved order array, is checked. `make check-release` attempted all
eight real HTTP scenarios; local socket bind failed with **EPERM** for every
scenario. The release gate remains incomplete. No dependency or migration
changed. Exact Rust **1.99.1** was used with 41 temporary upstream source
substitutions and an unchanged 266-package version/dependency graph; the
portable lock and original temporary Cargo configuration were restored.
Clean-registry/advisory qualification remains unavailable.

The optimized SGR/CSI PTY smoke passed the visible corner grip and drop preview,
release-only persistence, standalone Shift followed by shifted base `h`, both
Alt arrows, pane collapse/recovery, Unicode yank, annotation jumps/delete/undo,
external-editor capture suspension/resume and terminal cleanup. Its isolated
subscription rows, publisher bodies and cached full text were unchanged.
Physical Alacritty transport/paint remains unqualified. The executable was
rebuilt; an already-running Lume must be closed and reopened to load it.

The [actual drag preview](preview-reordering.png), annotation panel and help
snapshots were rendered and visually reviewed. The expanded
[reordering workload record](panel-reordering-benchmarks.json) includes actual
preview/commit frames and durable layout changes, with 128 drag events per
roundtrip and keyboard roundtrips. Each intermediate/final order is checked;
setup, event preparation and initial syntax completion are outside timers.
Three fresh processes per size retain wall time and peak RSS. This is a new
workload, without a pre-feature speedup claim or physical-terminal latency
claim.


## Collapsible panes, mouse and article notes — 2026-10-18

All four panes now collapse/reopen independently, including Reader and the
new article-local Annotations pane. Widths, order, visibility and mouse
preference persist in backward-compatible Workspace JSON. No dependency,
license, migration or publisher-data change was needed. Native tests cover
all-collapsed recovery at 24×10, bounded maximum-width geometry, legacy defaults,
invalid orders, divider preview/commit/cancellation, header order, keyboard
layout equivalents, idle-motion filtering, wheel batching and modal isolation.

Reader drag selection checks exact UTF-8 bytes for combining accents, wide
characters, emoji, tabs and horizontally scrolled code. Crossing a divider
cannot select another pane. A reproduced regression lost publisher selection
when long private comments rewrapped; the fixed test preserves that selection.
Keyboard navigation batched by the runtime also cancels uncommitted resize
previews. Notes created in reverse order display by validated source position;
Feed/Full, ambiguous quotes and general notes retain explicit identities.
Deleting a focused note preserves the article and supports undo.

`make check` passed formatting, Clippy with warnings denied, **252 native tests**,
ordinary correctness-checked benchmarks and tooling syntax. `make check-release`
attempted all eight explicit HTTP scenarios; every one failed at local socket
bind with **EPERM**, so the complete release gate remains incomplete. A separate
optimized build passed for executable, benchmark and preview. Exact Rust
**1.99.1** was used with 41 temporary upstream source substitutions, preserving
the same 266-package version/dependency graph. Portable Cargo.lock and the
original temporary Cargo configuration were restored afterward; stale unused
patch metadata was removed without changing any selected package. Clean-registry
builds and connected advisory/distribution audits remain unqualified.

An optimized real-PTY smoke passed actual SGR mouse resize/reorder, no SQL
writes during divider preview, focus-loss cancellation, keyboard equivalents,
all-pane collapse/recovery, exact Unicode clipboard requests, source-order
annotation jumps, note deletion/undo, mouse enable/disable, external-editor
suspend/resume, mouse help and terminal restoration. Publisher bodies, cached
full text and subscription rows remained unchanged in its isolated database.
Three native terminal-adapter tests cover balanced capture, partial enable
failure without retry spinning and cleanup retries after a failed disable.
Physical Alacritty transport/paint latency is not established by these tests.

The [annotation panel](preview-annotations.png),
[all-collapsed workspace](preview-collapsed.png),
[layout guide](preview-help.png) and [settings](preview-settings.png) snapshots
were generated from actual buffers and visually inspected. All content and
controls remain English.

[Pane workload measurements](panes-benchmarks.json) retain three fresh optimized
processes per size, 40 resize roundtrips and 200 selection/note-jump samples
per process. Setup/event preparation and initial syntax completion are outside
operation timers; each result is independently checked. Timings include
application dispatch, actual reflow, style polling and TestBackend frames.
This is a new workload without a comparable pre-feature baseline, so it makes
no speedup claim. Whole-process RSS includes setup and is not an allocation
count. The stress workload has **74,310 source bytes and 200 notes**; the
ordinary workload has 7,400 bytes and 20 notes. Full measurements, wall time
and memory are preserved in the record rather than a physical-terminal
latency claim.


## Interactive latency and Vim review — 2026-10-18

Reproduced the reported smart-list pause on a consistent private copy of the
actual library: **7,133 articles, 98 sources and 46 smart lists**. Three fresh
optimized processes showed a 269–273 ms worst first switch. Cold syntax grammar
work was running on the input thread. Interactive layout now returns readable
plain code immediately, with bounded asynchronous coloring and exact cache
keys. Native controlled-worker tests prove that layout, navigation and Drop
do not wait for a blocked grammar. Real coloring preserves text, cursor,
selection and source identity; disconnected workers retain plain code.

Queries now assemble only applicable fixed predicates and load only the chosen
smart-list definition. Opening a read article avoids another mutation/recount;
opening an unread one refreshes read-sensitive counts, including nested
boolean saved criteria. A second measurement isolated remaining read-count
row visits. **V8 adds `(deleted, unread)` as a covering index**, leaving V1–V7
unchanged. Native V7 upgrade/reopen tests verify content, private notes,
memberships, flags, Trash and the actual covering plan. An additional private
upgrade of the actual library preserved every row in **21 nonjournal tables,
including FTS**, and a second open preserved them again. The original library
was not opened for mutation during this review.

Reader movement caches Unicode word/WORD boundaries and binary-searches cursor
rows. Unchanged terminal geometry does not reflow; code clipping stops at the
visible right edge. Note numbering no longer repeatedly searches the note list.
Vim tests cover word/end motions, counts, Unicode, rows, viewport positioning,
page controls, character/till searches, repeated/reversed finds, exact visual
yanks and word objects. Find targets such as `fy`/`fv` correctly take priority
over standalone yank/visual actions. Counted input bypasses coalescing, while
reported releases also cancel word/horizontal autorepeats. Reader `e`, `f`,
`n/N` and `z` have motion meanings; note/favorite actions retain explicit ex
commands. Help, footer hints, annotation status and README agree on those
contexts. `:help layout` explains themes, live settings, widths, focus, visible
panels, Zen, fullscreen and small terminals; later help sections are reachable.

The final `make check-release` passed its entire local `make check` prerequisite:
formatting, Clippy with warnings denied, **231 native tests**, all ordinary
correctness-checked benchmark workloads, and launcher/preview syntax. All eight
explicit real-HTTP scenarios were attempted and failed at loopback bind with
**EPERM**. The complete release gate remains incomplete. A separate optimized
build passed for the executable, benchmark and preview. Exact Rust **1.99.1**
and 41 temporary local upstream source substitutions were used. The same
266-package version/dependency graph and portable registry lock were retained;
the original temporary Cargo configuration was restored after qualification.
Clean registry builds and connected advisory/distribution audits remain
unqualified; this environment also fails DNS resolution for `index.crates.io`.

The final optimized PTY smoke test passed layout help, exact word/SQL clipboard
bytes, settings, a custom outline binding, jump history, tables, local search,
tags, cards/reviews, related notes, captured bulk undo, incremental export,
source health and terminal restoration. It verifies discarding an unconfirmed
preview on Ctrl-c. Fresh help/polyglot buffers were rendered and inspected;
the [layout guide preview](preview-help.png) shows the actual Ratatui buffer.
The [capability audit](feature-audit.md) maps the requested features to behavior,
entry points, tests and deliberate boundaries.

Measurements and resource data are retained in
[the smoothness benchmark record](reader-smoothness-benchmarks.json).
Private before/after probes agree on all 276 destination/count/body-size
records. The direct probe times `App::set_view`, not physical terminal drawing:

| Private-library operation | Before, ms | After V8, ms |
| --- | --- | --- |
| App construction, excluding database open/migration | 423.309–448.563 | 196.273–202.636 |
| Worst first smart-list switch until readable text | 269.051–273.406 | 4.654–4.909 |
| Worst warmed smart-list switch until readable text | 18.958–20.662 | 4.570–4.958 |

Before, this return included syntax coloring; after, colors arrive separately.
These values establish reduced blocking before readable text, not faster full
grammar execution. A separate current-runtime probe applies styles, resizes
and draws frames between destination changes. Across three fresh processes,
its smart-list/frame p99 was 5.617–5.885 ms, and incremental poll/resize/frame
p99 was 0.902–0.926 ms. Timers exclude its 16 ms pacing pauses.

The unchanged expansion workload has three fresh optimized trials per size,
40 cycles and 200 samples per interaction. Comparing the preceding checkpoint
with this build at **20,001 articles**:

| Interaction, p99 ms | Previous checkpoint | Current |
| --- | --- | --- |
| Reader motion/frame | 0.196–0.232 | 0.168–0.189 |
| Cached article selection/open/frame | 110.673–117.325 | 1.223–1.309 |
| Resize/reflow/themed frame | 8.456–8.589 | 0.786–0.922 |
| Reader input/frame during publisher writes | 0.210–0.358 | 0.193–0.231 |

Immediately before V8, cached selection/open/frame was still 22.986–26.544 ms
at p99 in three trials; the new index removed that remaining bottleneck.
The new smoothness workload additionally includes the runtime's resize and
style-poll steps on every frame. At **20,000 articles and 200 smart lists**,
three fresh processes recorded smart-list/frame p99 of 3.036–3.152 ms and Vim
input/runtime/frame p99 of 0.136–0.160 ms, with independently checked results.
The 2,000-article/20-list trials recorded 0.652–0.719 ms and 0.138–0.203 ms.
This workload is part of ordinary/stress root benchmark composition; its
focused command is `cargo bench --locked --bench reader -- --smoothness-only`
with optional `--stress`. Setup is excluded from operation timers. Its reported
syntax tail after 40 navigation cycles is remaining work, not cold coloring
latency. Whole-process wall/RSS include setup and are not allocation counts.

Shared-host scheduling and caches were uncontrolled. TestBackend percentiles
do not measure physical Alacritty key-to-display latency, transport throughput
or hard real-time release behavior. Earlier records below describe their own
builds and remain historical checkpoints.

## Correctness review — 2026-10-18

A second review reproduced six failures before their fixes: domain matching
missed valid non-default ports; feed normalization discarded article URL
fragments; a late imported older occurrence replaced the newest publication;
personal edits/refresh reset the precise reader cursor; Ctrl-c persisted an
unconfirmed visual preview; and a passage-review picker accepted a note edited
since its preview. Regression scenarios failed against the preceding code and
passed after correction.

Further boundary review corrected hostname Retry-After persistence for feeds
subscribed after a failed pass/restart, misleading NEW/OK source diagnostics,
and the table inspector's unaligned/notice paths exceeding its advertised
1 MiB output cap. Hostname deadlines are independent of feed IDs; ordinary
failure backoff does not suppress other feeds on that host. Diagnostics expose
the effective deadline and URL. Both table paths now retain a readable UTF-8
prefix within the byte budget, including notices, leaving the source intact.

Background refresh recovery now retains a still-matching article across page
boundaries and restores selected notes by ID. Its position, count and metadata
page share a SQLite snapshot and one validated criteria object, including its
publication-age boundary. A target that no longer matches is not reopened via
All or outside the active query. Viewport/cursor reuse requires matching source,
revision and body; private-note cursor reuse additionally checks readable text.
Captured passage reviews compare displayed note content and observed schedule;
concurrent reviews, changed notes and Trash reject without writing another
review or undo entry. Study mutations require restoring a trashed owner first.

The final reviewed `make check-release` completed its entire local `make check`
prerequisite: formatter, Clippy with warnings denied, **212 native tests**, all
ordinary correctness-checked benchmarks and launcher/preview syntax. All eight
explicit real-HTTP scenarios were then attempted and failed at loopback bind
with EPERM. The complete release gate remains incomplete. A separate optimized
build of the executable, benchmark and preview passed. Exact Rust **1.99.1**
and the same 41 environment-only upstream source substitutions were used;
no dependency version or migration bytes changed. The portable registry graph
and original temporary Cargo configuration were restored after qualification.
A clean registry build and live HTTP remain unqualified in this environment.

The optimized real-PTY scenario passed settings, a bound outline/jump history,
wide tables, local search, exact SQL OSC 52, independent tags, cards/reviews,
related notes, captured bulk undo, incremental export, query explanation and
source health. It now also exits with Ctrl-c during an unconfirmed visual
preview and checks that the saved theme is unchanged. Raw/alternate-screen
cleanup and unchanged feed/cache/subscription content were checked. Fresh
144×44 settings/outline/table buffers were rendered and inspected; this is
not physical Alacritty GUI instrumentation. All fixtures remained private;
this review introduced no schema migration or writes to the real library.

The expansion benchmark additionally qualifies anchored filtered-page recovery
at 2,001/20,001 entries with independent offset/count/target assertions. Timers
exclude setup; each size has three fresh optimized processes with 40 cycles
and 200 samples per interaction. Full post-review measurements belong to
[the review benchmark data](reader-review-benchmarks.json). The earlier data
below remain the preceding checkpoint, rather than a claim about this build.
Process scheduling and caches are uncontrolled; collector transport throughput
and physical key-to-display latency are not measured.

Post-review p99 ranges across the three fresh trials are recorded below.
Values are input-to-TestBackend-frame milliseconds, not physical display latency.

| Interaction | 2,001 entries, p99 ms | 20,001 entries, p99 ms |
| --- | --- | --- |
| reader motion to frame | 0.228–0.247 | 0.196–0.232 |
| cached article selection/open to frame | 9.448–10.303 | 110.673–117.325 |
| responsive resize/reflow to themed frame | 1.036–1.187 | 8.456–8.589 |
| reader input/frame during publisher writes | 0.218–0.375 | 0.210–0.358 |

Representative process wall seconds: 2.77/2.8/2.75; peak RSS KiB:
24348/24520/24584. Anchored filtered-page recovery: 2.824/2.872/2.827 ms/cycle.

Stress process wall seconds: 29.95/29.71/30.09; peak RSS KiB:
26196/25832/26236. Anchored filtered-page recovery: 34.233/34.21/34.255 ms/cycle.

The additional anchored workload changes complete-process totals. The common
interaction measurements remain close to the preceding checkpoint; this review
claims correctness improvements, not a demonstrated performance speedup. Every
trial committed actual concurrent publisher batches; filesystem input/output
counters were zero for these temporary runs and are not a count of SQL writes.

## Reader, organization and study expansion — 2026-10-18

All seven requested areas have executable local workflows: reviewed source
selectors and health; source-bound reader tools; shared boolean smart queries;
personal navigation, tags, occurrence grouping and atomic captured batches;
explicit study reviews/cards and incremental Markdown; live visual preferences
and normal-mode bindings; bounded collection with incremental progress/backoff.
The README documents commands and product limits; architecture and dependency
contracts describe ownership and the qualified library choices.

The final reviewed `make check-release` passed its complete local `make check`
prerequisite: formatter, Clippy with warnings denied, **197 native tests**, all
ordinary correctness-checked benchmark workloads, and preview/launcher syntax.
Its eight explicit real-HTTP tests then all failed at loopback socket creation
with EPERM. They were attempted and not silently skipped. These include the
new source-selector and durable Retry-After HTTP scenarios. The complete
release gate remains incomplete; real publisher transport/redirect behavior is
not qualified here. A separate optimized build of the executable, benchmark
and preview example passed with exact Rust **1.99.1**.

Native additions cover captured/stale selector confirmation and last-good-cache
preservation; summary provenance, suspicious extraction and HTML table retention;
source-bound reader search/outline/history through reflow and changed revisions;
boolean precedence, fields, quoted phrases, UTC date and duration boundaries,
invalid syntax and bound values; post-filter occurrence grouping; captured
all-or-nothing edits and complete undo; navigation identities across visits;
annotation tags/links/cards/review preservation; explicit due boundaries and
stale card protection; unchanged exports and local/unowned-file conflicts;
settings cancellation/save and key validation; four-lane overlap, same-host
pacing/Retry-After and cancellation before further queued commits.

The optimized binary also passed an isolated real-PTY smoke scenario: live
settings cancel/save, a custom outline binding, source jumps/history, wide tables,
local search and exact full SQL block bytes in OSC 52, article/highlight tags,
user-authored card reveal/review, explicit passage review, related-note links,
captured bulk favorite/undo, unchanged incremental export, query explanation,
source health and terminal restoration. The first harness incorrectly treated
`j` as picker navigation and selected an existing demo note; arrow navigation
and a private single-note fixture corrected those harness assumptions. Synthetic
terminal capability replies were supplied. Physical Alacritty rendering was
not instrumented. Actual 144×44 Ratatui buffers in `preview-settings.png`,
`preview-outline.png` and `preview-tables.png` were rendered and inspected.

Schema V4→V7 was first exercised on a consistent private SQLite copy, including
repeat open, then on the actual workspace after a private backup. SHA-256
snapshots matched every pre-existing column and row identity in **7,133
articles/search rows**, **two Full caches**, **98 sources**, **46 smart lists**,
settings, undo, folders, memberships and comments. All four prior migration
records matched and foreign-key validation found no violations. Only new
schema/derived metadata and the backfill marker were introduced. No publisher
content or prior migration bytes were rewritten. The private backup and detailed
preservation report remain under `/tmp/lume-before-expansion-*.sqlite3` and
`/tmp/lume-expansion-workspace-preservation.json` in this environment.

Dependency versions remain unchanged. dom_query and nom were promoted from
already locked transitives to exact direct dependencies. Qualification used the
same 41 temporary upstream path substitutions documented in the preceding
extraction record, without editing dependency code. After building, the portable
266-package registry lock and original temporary Cargo configuration were
restored, checking package versions and every dependency edge; only Lume's two
new direct edges differ from the prior portable graph. A clean registry build,
connected advisory review and complete distribution audit remain unqualified.

### Measured expansion workloads

Optimized Rust 1.99.1, Intel Core Ultra 7 155H, Linux 7.1.8 x86_64, private
local temporary SQLite. Three fresh processes per size ran 40 correctness-checked
operation cycles and 200 samples per interaction. Setup is outside operation
timers and inside process wall/RSS. Scheduling and filesystem caches were
uncontrolled. Complete per-trial p50/p95/p99, operation timing and process
measurements are retained in [the benchmark data](reader-expansion-benchmarks.json).
These are initial baselines, not evidence of an overall speedup.

Each fixture has 2,001 or 20,001 metadata entries and a technical document with
20 or 200 rich Markdown repetitions. Workloads validate query count/page;
CSS selection and normalized code; 100/500 captured favorites with one atomic
undo; card review with complete undo; unchanged export; input/frame, 200 unique
cached article selections/opens, and 80×24↔144×44 themed reflow. A separate
SQLite connection repeatedly ingests 100/500-entry publisher batches while
reader identity and input results are checked. Its 3–7 committed batches per
trial confirm actual concurrent writes, not HTTP throughput.

| Operation, ms/cycle | 2,001 articles, trials 1–3 | 20,001 articles, trials 1–3 |
| --- | --- | --- |
| Boolean query/count/page | 2.147 / 2.312 / 2.202 | 23.294 / 22.615 / 22.658 |
| CSS extraction | 0.110 / 0.113 / 0.103 | 2.006 / 2.022 / 2.094 |
| Captured batch + undo | 2.888 / 3.065 / 2.922 | 30.789 / 30.607 / 30.201 |
| Card + review + undo | 0.836 / 0.854 / 0.886 | 0.415 / 0.424 / 0.425 |
| Unchanged export | 1.737 / 1.801 / 1.826 | 16.301 / 16.008 / 16.709 |

| Interaction | 2,001 articles, p99 range ms | 20,001 articles, p99 range ms |
| --- | --- | --- |
| Reader motion/frame | 0.190–0.316 | 0.182–0.269 |
| Cached selection/open/frame | 9.757–16.826 | 106.066–108.653 |
| Resize/themed frame | 1.157–1.970 | 8.668–9.115 |
| Reader/frame during writes | 0.204–0.293 | 0.185–0.360 |

Complete process wall times were 2.60/2.63/2.83 seconds and
27.45/27.37/27.40 seconds; peak RSS was 24,532/24,508/24,212 KiB and
26,328/25,808/26,044 KiB. `/usr/bin/time` reported zero filesystem input/output
counters for these temporary runs; this does not mean SQLite performed no
writes. No allocator counts or physical terminal timing were measured.

An initial debug run exposed a correlated grouping count that delayed cached
article opening. A single diagnostic on a 2,001-entry private fixture returned
the same count in roughly 1,926 ms versus 2 ms using a grouped candidate query.
At that checkpoint the owner used GROUP BY after filtering, with regressions
for filtered occurrence identity and distinct empty URLs. The review above
changes representative ordering through a window function without restoring
correlated counts. That uncontrolled single
comparison is diagnostic evidence, not a repeated performance baseline. The
optimized results above qualify the final implementation. Large-library opens
still include metadata/count/personal-state work and remain slower than cached
reader motion; measured values make that distinction explicit.

## Full article extraction and offline cache — 2026-10-17

The reader now retrieves the original page in a dedicated worker, extracts its
main HTML and normalizes it through the existing content adapter. Default
automatic fetching applies only to opened entries without independent text.
Manual fetching, Feed/Full switching, per-source policies and an explicit
offline boundary are qualified by native scenarios. Publisher feed text stays
separate; successful cache revisions, reading positions and passage provenance
survive refreshes. Failures retain an existing successful document and expose
their reason. Completion waits for active drafts or visual selections.

Root `make check-release` passed its complete local `make check` prerequisite:
formatting, Clippy with warnings denied, **165 native tests**, all ordinary
correctness-checked benchmark workloads and preview/launcher syntax. The six
explicit real-HTTP tests then failed at loopback bind with EPERM. These include
redirects/final URL handling, non-HTML and HTTP-error preservation, and response
size limits. The full release gate remains incomplete; live publisher fetching
is not qualified in this environment. Separate optimized application and
benchmark builds passed.

New native coverage includes extraction of owned main text, code languages,
indentation, tables and relative links; invalid/oversized/active-only pages;
cache/search transactions; stale URLs and Trash; failed refetch preservation;
worker queue bounds and cancellation; legacy undo payloads; source/revision
quotation identity; ambiguous relocation; deferred completion; draft retention;
independent reading positions; offline CLI rejection and export provenance.
The 20,000-element check follows DOM construction; it is not a hard parsing
memory or CPU deadline. Extraction remains best effort for JavaScript, login
and subscription pages. The original browser action remains available.

Exact Rust 1.99.1 was used throughout. The approved MPL-2.0 decision, library
selection and 32-package license inventory live at the dependency owner and
in `assets/licenses/full-article/`. Network restrictions required 41 temporary
path-source substitutions: the earlier 18 plus 23 packages for this slice.
The 335 newly obtained source files were checked against upstream Git blob
identities. Three documentation files initially had an extra trailing newline;
their exact upstream bytes were restored. Six library tests and three
extraction tests passed again, followed by another optimized build. No runtime
dependency code was changed. After qualification, the portable 266-package
registry lockfile and original temporary Cargo configuration were restored;
the version/dependency graphs match, retaining all 234 prior package versions.
No `/tmp` source path is a product input. A clean registry build, connected
advisory review and complete distribution audit remain unqualified.

An isolated real PTY exercised the optimized application with an original
private fixture: cached Full text, persistent Feed/Full switching, exact
selected `Alph` bytes in OSC 52, offline fetch rejection and normal exit.
SQLite checks confirmed unchanged feed/cache bodies and subscriptions.
Terminal checks confirmed restored raw mode, alternate screen, bracketed
paste and shell cursor. Synthetic capability replies were supplied; Alacritty's
GUI was not instrumented. The harness was corrected for Ratatui differential
output and OSC 52's valid ST terminator before the final passing run. These
were harness assumptions, not changes to terminal behavior. The actual 144×44
Ratatui buffer in `preview-full-article.png` was rendered and inspected with an
original fictional community article, Rust/SQL colors and the visible cursor.

The optimized binary migrated the real workspace from schema 3 to 4 after a
consistent private SQLite backup. SHA-256 snapshots matched every original
column across 7,132 articles and search rows, 98 subscriptions, 46 smart lists,
collections, memberships, comments, workspace settings and nine undo records.
All three prior migration records matched; foreign-key validation found no
violations. New source policies are `missing`; no full-page cache or fictional
publisher content was inserted into the real library. Restart an older running
process to use the new binary and schema contract.

### Full article baseline

Optimized Rust 1.99.1, Intel Core Ultra 7 155H, Linux 7.1.8 x86_64/glibc 2.41,
local temporary SQLite. Three fresh processes per size used `/usr/bin/time`;
scheduling and filesystem caches were uncontrolled. Setup is outside operation
timers and inside process wall time/RSS. These are initial measurements, not
speedups. HTTP transport is outside these extraction/cache measurements.

The focused `--full-article-only` workload uses an original 3,067-byte HTML
fixture producing 1,991 Markdown bytes. Stress repeats its paragraph group
100 times in one article container: 263,041 HTML / 198,956 Markdown bytes.
Each workload runs 40 cycles and participates in ordinary/stress root benchmark
composition. Checks cover expected text/code/table content, removed navigation,
cache/search identity, revision stability, failure preservation and actual
Feed/Full frames with source badges and community links.

| Workload | Representative trials 1–3 | Stress trials 1–3 |
| --- | --- | --- |
| Cold extraction, ms | 0.622 / 0.437 / 0.542 | 11.204 / 10.134 / 10.307 |
| Extraction/normalization, ms/cycle | 0.197 / 0.155 / 0.208 | 9.661 / 9.794 / 9.709 |
| Cache/FTS/revision, ms/cycle | 0.208 / 0.200 / 0.211 | 1.720 / 1.749 / 1.831 |
| Failed fetch preservation, ms/cycle | 0.021 / 0.020 / 0.021 | 0.047 / 0.048 / 0.049 |
| Feed/Full switch/frame, ms/cycle | 0.537 / 0.577 / 0.529 | 20.146 / 20.119 / 20.152 |
| Complete process wall seconds | 0.09 / 0.08 / 0.09 | 1.36 / 1.37 / 1.37 |
| Complete process peak RSS KiB | 25,596 / 25,576 / 25,688 | 30,544 / 30,596 / 30,592 |

## Partial feed bodies and reader availability — 2026-10-17

Read-only inspection of the reported Xbox article found a 79-byte cached
Markdown body containing only the Lobsters discussion link. The author's
original page contains the article. This reproduces the screenshot without
evidence of text loss during drawing. Other cached feeds explicitly retain
the existing missing-body placeholder. No publisher text was reconstructed
from research tools, and the real library was not changed during this fix.

The source adapter now checks normalized content before choosing it over a
summary. Empty, whitespace-only and active-only HTML fall through correctly.
The reader classifies missing/reference-only bodies on loading, keeps their
references, removes fabricated minute estimates and displays the original-URL
shortcut. Existing placeholders are recognized without migration. Generated
notices remain outside publisher quotation mappings, storage and Notebook.
The actual 144×44 Ratatui buffer in `preview-partial-feed.png` was rendered
and visually inspected using an original fictional community entry.

Root `make check-release` passed its complete local `make check` prerequisite:
formatting, Clippy with warnings denied, **133 native tests**, all ordinary
correctness-checked benchmark workloads and preview/launcher syntax. Nine new
scenarios cover RSS/Atom/JSON fallback, nested references, literal text/code,
legacy cached entries, missing original URLs, browser action targets, notice
quotation rejection, reflow/fullscreen and Notebook behavior. The three real
HTTP tests then failed at loopback bind with EPERM. The complete release gate
remains incomplete. Separate optimized application and benchmark builds passed.

Exact Rust 1.99.1 was used throughout. No dependency was added or upgraded;
the documented 18 temporary path sources were used again. The portable
registry lockfile and temporary Cargo configuration were restored after
checking the unchanged 234-package version/dependency graph. No temporary
source path is a product input; a clean registry build remains unqualified.

### Partial-feed baseline

Optimized Rust 1.99.1, Intel Core Ultra 7 155H, Linux 7.1.8 x86_64/glibc 2.41.
Three fresh processes per size used `/usr/bin/time`; scheduling and filesystem
caches were uncontrolled. Fixtures/setup are outside operation timers and
inside process wall time/RSS. These are initial measurements, not speedups.

`--feed-content-only` parses and validates 200 entries / 44,646 bytes; its
stress variant uses 2,000 entries / 445,746 bytes. Every five-entry group
independently checks the expected summary bodies and reference/missing cases.
The second workload selects five cached articles and draws actual 144×44
frames per cycle, checking cached availability notices. Its long reference
body contains 200/2,000 discussion links. Both workloads run 40 cycles and
are included in the root benchmark's ordinary/stress composition.

| Workload | Representative trials 1–3 | Stress trials 1–3 |
| --- | --- | --- |
| Mixed parsing/availability, ms/cycle | 0.793 / 0.741 / 0.772 | 7.524 / 7.610 / 7.712 |
| Five selections/frames, ms/cycle | 1.031 / 1.054 / 1.075 | 3.233 / 3.262 / 3.251 |
| Complete process wall seconds | 0.07 / 0.07 / 0.07 | 0.43 / 0.44 / 0.44 |
| Complete process peak RSS KiB | 10,228 / 10,372 / 10,464 | 12,292 / 12,516 / 12,360 |

## Visible reader cursor and recoverable article deletion — 2026-10-17

Root `make check-release` passed its full local `make check` prerequisite on
exact Rust 1.99.1: formatting, Clippy with warnings denied, **124 native tests**,
correctness-checked workloads and preview/launcher syntax. All three explicit
real-HTTP release tests then failed at loopback bind with EPERM; the complete
release gate remains incomplete. Separate optimized application and benchmark
builds passed. The preview's final cursor position was compiled, rendered and
inspected after that gate; it changes only an example key sequence.

No dependency was added or upgraded. The documented 18 temporary path-source
substitutions qualified the build; the portable registry lockfile and temporary
Cargo configuration were restored after verifying the unchanged 234-package
version/dependency graph. No temporary source path is a product input. A clean
registry build/advisory audit remains unqualified in this environment.

The reader now paints one ivory cursor block with dark bold text, distinct
from amber visual selection. It no longer positions a second hardware cursor
on that cell, which could reverse the painted contrast in Alacritty. Native
scenarios verify cell colors/modifiers, wide graphemes, empty rows, selection,
unfocused rendering, actual Crossterm hide/show bytes and cursor cleanup.
`preview-cursor.png` comes from the actual 144×44 fullscreen Ratatui buffer and
was inspected with the cursor on a letter. It uses original fictional demo
content, not copied publisher content or a manually reconstructed interface.

Article deletion adds migration V3, a checked visibility flag and its order
index. V1/V2 names/bytes remain unchanged. Native scenarios cover all ordinary
destinations and their counts, retained Inbox selection, FTS, smart lists,
source/folder visibility, Trash pagination/tie-breaking, global jump positions,
repeated/missing-target requests, transaction rollback on history-write failure,
publisher updates, reopen/undo, explicit export and Notebook exclusion. A real
version-two fixture qualifies migration of annotations, memberships, progress,
workspace JSON and old flag-undo payloads. TUI scenarios cover discoverable
aliases, Delete/D repeat suppression, library/visual focus guards, restore,
captured palette targets across selection changes and stale saved sessions.

An isolated real PTY exercised the optimized binary: fullscreen ivory cursor,
D to Trash, `:trash`, `:restore` and exit. It checked background-color bytes,
absence of a second hardware cursor during reading, deletion flags in SQLite,
retained publisher content/notes/read/favorite/queue state, zero exit status,
and restored raw mode/alternate screen/bracketed paste/shell cursor. Synthetic
terminal-capability replies were supplied. The inherited `NO_COLOR=1` was
removed only for this color-qualification child process. The actual Alacritty
GUI was not instrumented. An earlier attempt with NO_COLOR inherited could
not assert color bytes; the fixture was corrected before qualification.

The new binary migrated the real workspace database from schema 2 to 3.
A consistent SQLite backup was created privately under `/tmp` before the
migration. Before/after SHA-256 snapshots of every original column matched
across 7,070 articles, feeds, smart lists, comments, collections, memberships,
workspace settings and undo history. All earlier migration records matched,
foreign-key validation returned no violations, and every article's new flag
was zero. The 98 subscriptions and 46 smart lists were retained. No real post
was deleted and no demo content was ingested into this library. Restarting an
older running process is required to use the new binary and schema contract.

### Reading tools and Trash baseline

Optimized Rust 1.99.1, Intel Core Ultra 7 155H, Linux 7.1.8 x86_64/glibc 2.41,
local temporary SQLite. Three fresh processes per size used `/usr/bin/time`.
Scheduling and filesystem caches were not controlled. The focused
`--reading-tools-only` workload and its `--stress` variant include these checks
in ordinary and stress root benchmark composition. Setup/restoration occurs
outside operation timers and inside whole-process wall time/peak RSS.

The existing 200 selection/OSC52/frame cycles now also verify the ivory cursor
alongside amber selected cells. Inputs use 9,690/48,890 readable bytes,
200/1,000 Unicode paragraphs and 20/200 pre-existing highlights. Existing
highlight/undo and source-union workloads continue to check their contracts.
The added Trash fixtures contain 2,000/8,000 articles, half deleted. Forty
oldest-page FTS queries check full counts and the independently expected final
page order. Forty lifecycle cycles delete, check active/Trash FTS visibility,
restore and undo both changes while retaining a private note and favorite.
These are initial measurements for the new lifecycle work, not speedup claims.

| Workload, ms/cycle | Representative trials 1–3 | Stress trials 1–3 |
| --- | --- | --- |
| Selection, OSC52 and cached frame | 0.140 / 0.150 / 0.139 | 0.146 / 0.141 / 0.147 |
| Trash FTS count and oldest page | 1.275 / 1.202 / 1.276 | 4.551 / 4.588 / 4.627 |
| Delete, visibility, restore and undo | 0.356 / 0.343 / 0.363 | 1.011 / 1.035 / 1.013 |
| Complete process wall seconds | 0.30 / 0.29 / 0.29 | 1.33 / 1.31 / 1.34 |
| Complete process peak RSS KiB | 11,232 / 10,520 / 10,700 | 12,504 / 12,268 / 13,016 |

## Platform, kernel and network reading expansion — 2026-10-17

Root `make check-release` passed its complete local `make check` prerequisite
on exact Rust 1.99.1: formatting, project Clippy with warnings denied,
**110 native tests**, correctness-checked benchmark workloads and preview/
launcher syntax. The three explicit real-HTTP tests then failed at loopback
bind with EPERM. The full release gate remains incomplete; separate optimized
application and benchmark builds passed.

No dependency was added or upgraded. Qualification again used the documented
18 environment-only path-source substitutions under `/tmp/lume-vendor`.
The portable registry lockfile and temporary Cargo configuration were restored
after verifying the unchanged 234-package version/dependency graph. These
source paths are not product inputs; a clean registry build remains unqualified.

The catalog adds 12 subscriptions and 16 smart lists, reaching 98 unique URLs
and 46 distinct list names. Four new publisher collections cover platforms,
kernels, dataplanes and delivery; twelve new literal queries cover eBPF,
Cilium, CNI, policies, containers, Terraform, kernels, BGP, controllers,
filesystems, schedulers and supply chains. The six source groups now use a
single typed membership asset with canonical subscription-URL references.
Resolution rejects empty, unknown, duplicate and oversized reference sets,
including a partially resolvable set, instead of creating unrestricted queries.
Existing named queries retain their IDs and customized criteria.

Native scenarios verify each new source, future publication inclusion without
keywords, expected group counts, unrelated-source exclusion, all twelve text
queries, membership combined with text/unread criteria, and preservation on
reinstallation. Existing AI, security, English-label upgrades, collision,
active-view, CLI and data-preservation scenarios also pass.

The optimized launcher upgraded the actual workspace library from 86 to 98
subscriptions and 30 to 46 smart lists. Before/after SHA-256 snapshots matched
all rows of articles, comments, collections, memberships, settings, undo and
migration history. All 30 prior query IDs, names and definition bytes were
retained, as were existing feed rows. Each new group resolved to its exact
configured source URLs. There were 5,745 cached articles at this observation.
The new Platform, Kernel, Networks and DevOps groups already exposed 75, 45,
55 and 105 cached articles respectively through existing subscriptions; their
memberships may overlap. No article was copied or reconstructed for the update.

Actual shell requests to Bootlin and Backstage failed with temporary DNS
resolution errors. New subscriptions await working collection. Research checked
publisher pages and endpoints: eleven additions returned XML feed types to the
research browser, while Tekton's response was an opaque HTTP-OK tool error.
Neither observation qualifies item parsing/import or transport. Endpoint and
manual-reference decisions are recorded in `reading-sources.md`.

### Expanded catalog baseline

Optimized Rust 1.99.1, Intel Core Ultra 7 155H, Linux 7.1.8 x86_64/glibc 2.41,
local temporary SQLite. Three fresh processes per size used `/usr/bin/time`.
Shared-host scheduling was not controlled. The `--catalog-only` benchmark uses
98 sources, 46 default queries and 40/1,000 custom queries. Database setup and
fixture restoration/removal occur outside operation timers and inside total
process wall time/RSS. It is included in ordinary and stress root composition.

Each process performs 40 label-upgrade cycles with a destination collision,
checking IDs, criteria, query/source counts and unrelated custom definitions.
Then 40 cycles install the four missing infrastructure groups, checking exact
added counts, group sizes and representative source identities. The latter is
new work; the earlier 86-source measurements are not a like-for-like baseline
for claiming improvement. No speedup is claimed.

| Custom queries | Label upgrade µs/cycle, trials 1–3 | Four groups µs/cycle, trials 1–3 | Process wall seconds | Peak RSS KiB |
| --- | --- | --- | --- | --- |
| 40 | 692.4 / 686.4 / 701.9 | 715.7 / 719.4 / 710.8 | 0.07 / 0.07 / 0.07 | 9,192 / 9,264 / 8,880 |
| 1,000 | 1,257.4 / 1,309.7 / 1,324.2 | 1,278.9 / 1,320.0 / 1,308.1 | 0.15 / 0.16 / 0.16 | 9,256 / 9,236 / 9,364 |

## English catalog and AI reading selection — 2026-10-17

Root `make check` passed on exact Rust 1.99.1: formatting, project Clippy with
warnings denied, **105 native tests**, all correctness-checked benchmark
workloads and preview/launcher syntax. Root `make check-release` repeated its
successful local prerequisite and explicitly ran the three real-HTTP tests.
All three failed at loopback bind with EPERM, so the full release gate remains
incomplete. Separate optimized application and native benchmark builds passed.

No Rust dependency was added or upgraded. The same 18 environment-only path
sources under `/tmp/lume-vendor` qualified the build; their limitations remain
as documented below. The portable 234-package registry lockfile and temporary
Cargo configuration were restored after verifying the exact package/version/
dependency graph against the qualified build. No temporary source paths are
product inputs. A clean registry build and advisory audit remain unqualified.

The catalog now contains 86 unique subscription URLs, 28 literal topic searches
and two automatic source-group lists: Security (eight sources) and Artificial
intelligence (14). Native scenarios verify every AI source's membership,
future article inclusion without keyword matching, unrelated-source exclusion,
and all ten AI text queries. Upgrade scenarios cover every historical bundled
label, retained IDs/criteria/active view, destination collisions with successive
English suffixes, idempotent installation, preservation of custom names and
queries, and atomic rollback on stale state, invalid names or conflicts.
Archive-title tests preserve publisher bodies, annotations and favorite/read
state while protecting later publisher titles. No schema migration was needed.

The optimized launcher upgraded the real workspace library from 72 to 86
subscriptions and 19 to 30 smart lists. Read-only before/after SHA-256 snapshots
matched every row of articles, comments, collections, memberships, settings,
undo history and migration history. All 19 existing query IDs and definitions
were retained. Every old feed row was retained, except the explicitly allowed
conditional archive-label translation. There were 3,194 cached articles at
this inspection, including 130 from the security sources. The new AI sources
had zero cached articles. No fictional or reconstructed publisher content was
ingested into this library.

Actual shell requests to Hugging Face and Google Research failed with temporary
DNS-resolution errors. The new subscriptions are installed for the next working
collection. The source guide distinguishes official-site/feed research from
actual item parsing and ingestion: 13 AI endpoints returned XML feed types to
the research browser; the combined arXiv URL follows its official RSS contract
but its response was not opened. This is not end-to-end transport qualification.

### Catalog upgrade baseline

Optimized Rust 1.99.1, Intel Core Ultra 7 155H, Linux 7.1.8 x86_64 with glibc
2.41, temporary local SQLite databases. Three fresh processes per size used
`/usr/bin/time` for wall time and peak RSS. Shared-host scheduling was not
controlled. These are initial measurements, with no speedup claim.

The native `--catalog-only` workload uses 86 existing sources, 30 default
queries, a legacy security label colliding with the current English name,
and 40/1,000 unrelated custom queries. Setup and restoration of the legacy
label occur outside each operation timer. Each of 40 timed installations
checks retained identity and criteria, correct English collision suffix,
source count, custom query definitions and absence of duplicate subscriptions
or searches. Whole-process wall time/RSS include setup and verification.
The workload also runs in the ordinary and stress root benchmark composition.

| Custom queries | Trial 1 µs/cycle | Trial 2 µs/cycle | Trial 3 µs/cycle | Process wall seconds | Peak RSS KiB |
| --- | ---: | ---: | ---: | --- | --- |
| 40 | 612.9 | 692.1 | 589.5 | 0.03 / 0.04 / 0.03 | 8,828 / 8,924 / 8,752 |
| 1,000 | 1,200.0 | 1,215.7 | 1,300.5 | 0.07 / 0.07 / 0.09 | 9,292 / 9,308 / 9,476 |

## Security reading group and precise selections — 2026-10-17

The local prerequisite of root `make check-release` passed on exact Rust
1.99.1: formatting, project Clippy with warnings denied, 98 native tests,
all correctness-checked benchmark workloads and preview/launcher syntax.
All three required real-HTTP tests then failed at loopback bind with EPERM;
the full release gate remains incomplete. Separate optimized application and
benchmark builds passed. The only dependency change promotes already locked
base64 0.23.1 to an explicit direct dependency. No package/version was added
or upgraded. The qualified 234-package graph matches its portable registry
lockfile, which was restored after the same 18 environment-only source
substitutions described below. A clean registry build/advisory audit remains
unqualified; these temporary paths are not product inputs.

New native scenarios cover exact forward/reverse selections, source mapping
through links/emphasis/entities/inline and indented code, Unicode combining
marks/CJK/emoji, vertical column retention, visual-row selections, empty-row
reflow, viewport cursor geometry at 24×10 through 144×44, copy/highlight limits,
clipboard write/flush errors, durable marks/notes/undo, overlapping mark unions,
duplicate quotation identity, unique relocation and ambiguous publisher edits.
Explicit autorepeat events cannot repeat visual commands or accidentally
change queue membership after saving a highlight. Draft text and normal
cursor repeats retain their existing input-batch qualification.

The actual 144×44 fullscreen Ratatui buffer was rendered to
`preview-selection.png` and inspected. It shows a saved source highlight and
a live character selection while retaining code colors. Two isolated actual
PTY runs exercised the optimized executable: move by words, select exactly
“elegance”, yank, save the passage and edit its note. Each independently
decoded exactly one OSC 52 payload, checked the quote/body/source anchor in
SQLite, exited zero and restored raw mode, alternate screen and bracketed
paste. This qualifies request framing and lifecycle, not the desktop clipboard
or terminal permission acknowledgment. Synthetic terminal capability replies
were used; the actual Alacritty GUI was not instrumented.

Catalog tests cover eight distinct security sources, automatic inclusion of
future source articles without keyword matching, exclusion of unrelated
sources, preservation of existing custom searches, retained groups when
editing criteria, legacy JSON defaults and invalid/oversized source groups.
No schema migration was needed. `./lume feeds` upgraded the real workspace
library: eight subscriptions were added and one source-group list created.
A read-only inspection confirmed eight source IDs and zero cached security
articles; the existing library contained 3,059 articles at inspection. This
session did not ingest fictional or manually reconstructed publisher content
into that library. Network attempts to fetch Project Zero/Trail of Bits failed
with DNS resolution errors. The sources are installed for the next working
collection; upstream research and endpoint qualifications are recorded in
`reading-sources.md`. User cache/notes/favorites/folders were preserved.

### Reading-tools baseline

Optimized Rust 1.99.1, Intel Core Ultra 7 155H, Linux x86_64, temporary SQLite
on the local filesystem. `--reading-tools-only` selects this native workload;
add `--stress` for the larger data. Article/annotation/database setup occurs
outside operation timers, inside process wall/RSS. There are three fresh
processes per size, measured with `/usr/bin/time`. Shared-host scheduling was
not controlled; the isolated PTY qualification overlapped part of the run.

Selection inputs contain 9,690/48,890 readable Markdown bytes, 200/1,000
original Unicode paragraphs and 20/200 pre-existing source highlights.
Each of 200 timed cycles moves/selects, verifies the exact Copy action,
checks independent OSC 52 bytes, draws a real 144×44 buffer, checks selection
color and cancels. Forty durable cycles save an exact source quotation,
check its identity/count, reflow and undo while preserving prior marks.
The grouped-source workload uses 1,600/8,000 articles across eight sources,
four included; forty queries verify the complete count, oldest page order
and exclusion of other sources. All scenarios are part of root check/bench.

| Workload | Representative runs 1 / 2 / 3, ms/iteration | Stress runs 1 / 2 / 3, ms/iteration |
| --- | --- | --- |
| Cursor + exact yank + OSC 52 + cached frame | 0.518 / 0.174 / 0.145 | 0.145 / 0.146 / 0.151 |
| Durable highlight + reflow + undo | 1.838 / 1.280 / 1.232 | 5.370 / 5.457 / 5.436 |
| Source-group count + oldest page | 0.430 / 0.442 / 0.438 | 2.280 / 2.319 / 2.760 |
| Process wall seconds | 0.24 / 0.14 / 0.13 | 0.63 / 0.64 / 0.67 |
| Process peak RSS KiB | 9972 / 10252 / 9952 | 11884 / 11804 / 11872 |

Raw results: `benchmarks-reading-tools-2026-10-17.txt`. These are initial
observations, not a speedup, physical input-latency or isolated-allocation
claim. Clipboard output is measured into a byte buffer; no desktop clipboard
is timed. This text workload does not initialize code grammars: the larger
syntax/regex memory costs in the highlighting baseline below still apply.

## Multi-language code highlighting — 2026-10-17

Root `make check` passed on pinned Rust 1.99.1 with project Clippy warnings
denied, 80 native tests, correctness-checked benchmark targets and preview/
launcher syntax checks. An optimized application and benchmark build passed.
`make check-release` passed its local prerequisite but failed all three
required real-HTTP cases at loopback bind with EPERM. Full release
qualification remains incomplete. No Rust dependency was added or upgraded.
The same 18 temporary path-source substitutions described below were needed;
the portable 234-package graph was verified unchanged and registry lockfile
restored. A clean registry build and advisory audit remain unqualified.

Six new behavioral scenarios exercise all 15 exposed languages, independent
keyword/string colors, source/indentation preservation, familiar aliases,
multiline comment state, HTML `pre`/`code` language metadata, literal fence
collisions, metadata precedence and unknown/missing/oversized fallbacks.
Build-time validation also rejects unresolved embedded grammar references.
All 14 added grammar/license files matched their pinned upstream Git blob
identities, recorded in `assets/syntax/PROVENANCE.md`. The actual 144×44
Ratatui fullscreen buffer with Rust, Terraform, TypeScript and Go was rendered
to `preview-highlighting.png` and visually inspected. Fixtures use original
code and an isolated temporary database; the reading library was untouched.

### Highlighting baseline

Optimized Rust 1.99.1, Intel Core Ultra 7 155H, Linux x86_64. Run the native
benchmark with `--highlighting-only`, adding `--stress` for the larger input.
Fifteen original language snippets repeat 20/100 times: 28,920/144,600 input
bytes, 300/1,500 fenced blocks and 1,260/6,300 code lines. Article construction
is outside operation timing but inside process wall/RSS. Each process times
its first render separately, including lazy grammar/regex initialization,
then 40 fresh layouts. Every iteration checks exact code lines; independent
expected token colors are also checked. Three fresh processes per workload
were measured with `/usr/bin/time`. Host scheduling was not controlled;
other project qualification overlapped part of this run.

| Workload | Representative | Stress |
| --- | ---: | ---: |
| First render ms, runs 1 / 2 / 3 | 551.466 / 545.380 / 563.726 | 876.868 / 908.682 / 834.905 |
| Warm layout ms/iteration, runs 1 / 2 / 3 | 75.963 / 71.461 / 77.014 | 388.879 / 370.551 / 362.350 |
| Median warm layout ms | 75.963 | 370.551 |
| Process wall seconds, runs 1 / 2 / 3 | 3.60 / 3.41 / 3.65 | 16.45 / 15.74 / 15.34 |
| Peak RSS KiB, runs 1 / 2 / 3 | 149896 / 150220 / 150564 | 156616 / 157560 / 157204 |

Raw output: `benchmarks-highlighting-2026-10-17.txt`. This is an initial
baseline, not a speedup claim. Exercising all 15 grammars retains compiled
regex state and has a material memory cost. Per-block bounds do not establish
a hard CPU deadline. The app retains styled lines across redraw/scroll;
these measurements cover full article layout, not physical input latency,
per-frame drawing or isolated allocations. Grammar versions are compatible
snapshots, not language validators or a promise of newest syntax support.

## Fullscreen and keyboard responsiveness — 2026-10-17

Root `make check` passed on pinned Rust 1.99.1 with project Clippy warnings
denied, 74 native tests, correctness-checked benchmark targets and preview/
launcher syntax checks. A separate optimized application and benchmark build
passed. `make check-release` again failed its three explicit real-HTTP cases
because loopback binding returned EPERM; this release gate remains incomplete.
The same 18 environment-only path sources described below were used, without
adding/updating dependencies. The portable 234-package registry dependency
graph was checked unchanged and its lockfile restored after qualification.

Native scenarios cover fullscreen geometry at 24×10 through 240×70,
restoration of focus/panes/widths, nested modal Esc handling, old workspace JSON,
restart, reflow anchors, empty views and navigation exit. The actual 144×44
fullscreen buffer was rendered to `preview-fullscreen.png` and inspected.
Input scenarios cover 220 legacy movements crossing a metadata page in one
load, repeat cancellation after release, distinct taps, direction changes,
editor repeats/paste, queued quit and unchanged sidebar counts.

Two actual PTY runs negotiated the extended keyboard protocol with synthetic
terminal replies and CSI-u input. They verified Shift-Z and shifted colon,
fullscreen, preservation of prose width, and no further output after a burst
of 120 Up autorepeats followed by a release. A saved source-byte anchor stayed
above zero, independently checking that those released repeats had not scrolled
to the top. The second run opened VISUAL, saved its edited private note and
verified exactly balanced keyboard push/pop operations across suspend/resume
and exit. Both exited zero and restored raw mode, alternate screen and paste.
Alacritty 0.15.1 was detected locally; its actual GUI input latency was not
instrumented. The PTY emulates its documented protocol, not its renderer.

### Navigation comparison

Optimized Rust 1.99.1, Intel Core Ultra 7 155H, Linux x86_64, temporary local
SQLite, 10,000 articles, 64 bundled sources plus the fixture subscription and
18 saved searches. Each timed iteration navigates Library Down from Inbox to
All and Up back to Inbox, independently checking both destinations and the
full article count. Setup occurs outside operation timing but inside process
wall/RSS. Each executable was run three times, 40 iterations per run.

| Version | Mean ms per Down/Up pair, runs 1 / 2 / 3 | Median ms |
| --- | --- | ---: |
| Before count reuse | 835.949 / 827.200 / 823.990 | 827.200 |
| After count reuse | 8.977 / 8.975 / 8.899 | 8.975 |

Before process wall times were 34.97 / 34.49 / 34.35 seconds and peak RSS
19,252 / 19,372 / 19,360 KiB. After wall times were 1.71 / 1.74 / 1.74 seconds
and peak RSS 19,176 / 19,316 / 19,400 KiB. This measures the specific sidebar
workflow, not physical keyboard latency, all query shapes or overall app speed.
The removed work was recomputing every sidebar count on each view change;
read-state mutations and worker completion still refresh catalog counts.

The final full representative benchmark took 5.83 seconds, peak RSS 20,332 KiB.
Its 40-iteration fullscreen/reflow/frame/restore workload averaged 0.305 ms.
The correctness-checked workload consuming 240 queued repeat events, a release
and quit averaged approximately 0.001 ms; selection stayed unchanged and quit
was delivered. That timer covers in-process dispatch, not terminal transport.
`make bench` includes representative/stress navigation runs explicitly.
No hard real-time stop is claimed: an already-running operation completes,
and legacy terminals cannot report the physical release instant.

## Reading selection, command discovery and read folder — 2026-10-17

- Root `make check`, as the prerequisite of `check-release`, passed with the exact pinned
  `rustc 1.99.1 (58cf8f9c3 2026-10-13)`, including formatting, Clippy with
  project warnings denied, 61 native tests, correctness-checked benchmark
  workloads, Python preview syntax and the root launcher's Bash syntax.
- `make check-release` passed its local prerequisite but failed all three
  required real HTTP cases: binding loopback sockets returned EPERM. Tests
  were executed and failed explicitly. Full release qualification is incomplete.
- A separate `cargo build --locked --release` passed on Rust 1.99.1.
  Unlike the earlier record below, this run required no compiler substitution.
- The same 18 temporary path-source substitutions described below were still
  needed because registry DNS was unavailable. No dependency was added or
  upgraded. The 234-package version/dependency graph matched the portable
  registry lockfile exactly. The registry lockfile and original temporary
  Cargo configuration were restored after qualification; path sources are
  not shipped. A clean registry build remains unqualified.
- Tests cover command filtering/completion, required arguments, unknown-command
  draft retention, Unicode arguments, legacy aliases, folder browsing with
  no articles, multiple memberships, rename, deletion/undo and favorites.
  Native storage tests continue to qualify persistence and article/note
  preservation. Existing lists retain their collection identities. Read-folder
  tests cover mark/unmark, preservation of personal data, idempotent commands,
  undo, automatic marking on open, restart and articles older than 12,000
  newer entries. No migration or collection backfill was needed.
- Ratatui tests exercise the command menu at 24×10 through 144×44 and verify
  visible syntax/descriptions and filtering. The actual 144×44 buffer was
  rendered to `preview-commands.png` and visually inspected.
- An actual 120×32 PTY opened `./lume --offline`, displayed command suggestions,
  completed a prefix and opened the folder browser without cached articles.
  It exited with status zero, restoring raw mode, alternate screen and
  bracketed paste. No fictional content was inserted in this library.
  A separate isolated demonstration PTY exercised `:mark-read`, `:read`,
  `:mark-unread` and `:undo`. A SQLite read independently confirmed one read
  article; the process exited zero and restored the terminal.
- `./lume feeds` installed 64 real subscriptions and 18 saved topic searches
  into the workspace's `.local/library.sqlite3`, with file mode 0600.
  `./lume sync` attempted every subscription: 64 failed because DNS lookup
  returned "Temporary failure in name resolution". The first read confirmed
  zero cached articles. A later read observed a concurrent library update:
  3,058 cached articles across 62 sources, with two collection errors. Dan Luu
  and Istio exceeded the existing 4 MiB feed limit. The successful collector
  was not instrumented by this test run; these cached rows are an observed
  final state, not the result of the failed restricted sync. The home-directory
  default library was not modified by this session. Source selection/endpoint
  research is documented in `reading-sources.md`; real HTTP remains blocked
  in the qualified test environment.

The new release-profile baseline used an Intel Core Ultra 7 155H, Linux
x86_64, temporary SQLite databases on local storage and the exact compiler
above. Setup was outside per-operation timers but inside process wall/RSS.
The representative workload used 10,000 articles and 40 iterations per new
operation (with existing workloads also executed):

| Operation | Mean ms/iteration |
| --- | ---: |
| Decode 64-source catalog and repeat installation with 18 existing searches | 0.354 |
| Count 500 read articles and query their oldest page among 10,000 | 9.239 |
| Filter three export commands, render 120×32 menu and complete verb | 0.153 |

The full benchmark process took 5.77 seconds and peaked at 19,328 KiB RSS,
measured with `/usr/bin/time`. These are initial observations, not a speedup
claim or isolated allocation/latency measurements. Registry archive equivalence,
HTTP/TLS/redirect behavior and a complete release gate remain outstanding.

# Earlier validation — Lume 0.2 — 2026-10-16

## Local qualification

The implementation was reviewed for responsibility, ownership, readability,
failure behavior and resource bounds in addition to formatter/linter checks.
No delegated agent review was used. No user subscriptions or personal data
were imported.

- Root local gate passed through
  CARGO_HOME=/tmp/lume-cargo make check CARGO='cargo +stable'.
  Its final execution as the prerequisite of check-release passed formatting,
  Clippy with project warnings denied, 48 native behavioral tests, all targets
  including correctness-checked benchmark workloads, and Python preview syntax.
- The final release-profile application and benchmark builds passed using the
  explicitly substituted Rust/source environment described below.
- Ratatui buffers qualified normal, editor, help, leader, palette, article/list
  selectors, smart forms and Zen layouts. Layout tests include 1×1 through
  160×50; compact smart forms expose every field on small terminals.
- Actual 144×44 TUI buffers were rendered to preview.png and preview-zen.png
  and visually inspected. These are application output, not concept mockups.
- An actual 120×32 PTY opened the demonstration, read an article, selected a
  passage, saved a note, edited it through VISUAL, preserved the quote, toggled
  and undid queue membership, opened the notebook via the palette, exported it
  and quit with exit code zero. A separate SQLite read verified the edited
  text and captured quote. Raw mode, alternate screen and bracketed paste were
  restored; the created database had Unix mode 0600.
- Linux dependency metadata enumerated 187 packages. Observed license
  expressions offer permissive choices covered by dependency-security.md.
  Grammar notices, including the Rust grammar's separate MIT notice, are
  retained. This is metadata/notice qualification, not a full legal audit.

Native tests cover feed formats and stable identity; HTML5 discovery with
base URLs, script raw-text state, entities and duplicates; rich code/link
layout and Unicode source ranges; OPML depth, invalid URLs, deduplication,
idempotence and round trips; export source/quote preservation and refusal to
overwrite; version-one data migration and FTS backfill; FTS body/note changes
through edit/delete/undo; literal query escaping and smart criteria; atomic
membership failure; reversible list deletion; independent queue/favorite/read
state; quotes surviving publisher replacement; historical favorites and notes
beyond 12,000 newer articles; cross-page navigation and restored destination;
reading anchors at changed widths; cursor editing and text history; captured
draft targets, validation failures and bounded paste; external-editor exit
failures; CLI import/search/export; private files and real/demo isolation.

Count/page SQL uses one read transaction, preventing a collector commit from
giving the two results different snapshots. Existing ownership and transaction
rollback tests remain part of the gate.

## Required checks that did not pass here

Root check-release ran its successful local prerequisites, then explicitly
executed all three required real-HTTP tests. Each failed at loopback bind with
Operation not permitted:

- conditional collection and HTTP 304;
- HTTP failure while preserving cached data;
- discovery following a redirect and presenting advertised feeds.

They were not silently skipped. Actual conditional HTTP, redirects, publisher
reachability, timeouts and TLS remain unqualified in this environment. A
standalone optimized build passed, but the full release gate did not pass.

The repository and CI pin Rust 1.99.1 exactly. The earlier release check used
the [official announcement](https://blog.rust-lang.org/2026/10/15/Rust-1.99.1/).
Downloading the pinned toolchain failed because static.rust-lang.org could
not resolve. Local builds therefore explicitly used preinstalled Rust 1.96.0
(ac68faa20, 2026-05-25) through +stable. The exact pinned compiler/Clippy,
GitHub workflow and cross-platform builds have not been qualified here.

A clean registry build with Rust 1.99.1, make check-release, a current
advisory audit and distribution notice review remain release requirements.

## Temporary source substitutions

The shell could not resolve registry hosts. Most dependencies were cached;
18 uncached packages used temporary Cargo path sources under /tmp/lume-vendor.
No path override or vendor source is part of the product repository. Production
Rust sources/manifests were fetched through the public GitHub reader and
checked against their Git blob identities; not every upstream test/doc file
was materialized. These checks do not establish complete registry archive
equivalence.

| Package | Qualified upstream source |
| --- | --- |
| feed-rs 3.0.0 | feed-rs/feed-rs v3.0.0, 8f6ea6d6f448b38eabe0aeb7d38a2eabdc9cbb13 |
| quick-xml 0.42.0 | tafia/quick-xml v0.42.0, 36a2c52a4f6c90878c3f60c3c3b5c62efaef28a9 |
| mediatype 0.23.0 | 527706e30e249b8a00b358809379db6774446be6 |
| tui-textarea 0.7.0 | rhysd/tui-textarea v0.7.0, f2de5ff1efc803de82f2364eed7741edc34ae3e3 |
| html5gum 0.8.3 | untitaker/html5gum 0.8.3, a826d6b46e90f1d22ff8fce217447f0f53feef73 |
| syntect 5.3.0 | trishume/syntect v5.3.0, e4670846ecf16d8832db6c43d531bec466214e27 |
| refinery / core / macros 0.8.16 | rust-db/refinery v0.8.16, 44dd768fc8db72b3234f56ed0dff09eb3d39124a |
| fnv 1.0.7 | servo/rust-fnv v1.0.7, 4b4784ebfd3332dc61f0640764d6f1140e03a9ab |
| yaml-rust 0.4.5 | chyh1990/yaml-rust 0.4.5, 6cd3ce4abe6894443645c48bdc375808ec911493 |
| fancy-regex 0.16.2 | fancy-regex/fancy-regex 0.16.2, 68d300b2853cfb390a99020719114c22254878e5 |
| linked-hash-map 0.5.6 | contain-rs/linked-hash-map v0.5.6, dfc4a57278b79314b0c1892d6c3918f2e00e249b |
| bit-set 0.8.0 | contain-rs/bit-set 0.8.0, 301f3c5dcee73bb7836ca1016be64a1e01d91402 |
| bit-vec 0.8.0 | contain-rs/bit-vec 0.8.0, c45bd814e3e45d268f27c6eec992f2ebf3dbe260 |
| bincode 1.3.3 | Mozilla gecko-dev third_party/rust/bincode, versioned vendor mirror |
| pulldown-cmark 0.13.0 | pulldown-cmark/pulldown-cmark v0.13.0 |
| unicase 2.8.1 | seanmonstar/unicase v2.8.1 |

For bincode, all 16 materialized files also matched the vendor checksum
manifest's SHA-256 values, with zero mismatches. Only Cargo.lock and an unused
upstream test were absent. That manifest records package archive checksum
b1f45e9417d87227c7a56d22e471c6206462cba514c7590c09aff4cf6d1ddcad.
The original upstream repository was unavailable through the reader.

Reviewed text grammars are pinned separately in assets/syntax/PROVENANCE.md.
They compile into an embedded pack during the build. Runtime deserialization
uses that trusted generated pack, not downloaded binary grammars or user data.

The final Cargo.lock uses crates.io registry sources and checksums. Its
234-package version/dependency graph was verified identical to the graph
used by the local substituted build (including platform-specific packages). A clean registry build remains required; a matching graph
does not prove source archive equivalence.

## Performance workload and limits

Environment: Linux 7.1.8+deb13-amd64 x86_64, Intel Core Ultra 7 155H,
32,347,088 KiB system RAM; Rust 1.96.0. Release profile uses thin LTO and one
codegen unit. Temporary SQLite files and synthetic computing articles are
used. Shared-host scheduling, filesystem/cache state and disk characteristics
were not controlled.

Three runs each use 10,000 and 50,000 articles. Setup is outside operation
timers, but included in process wall time and peak resident memory. Forty
iterations qualify durable mutations/undo, feed parsing, body search plus an
old matching page, Markdown/syntax layout, OPML/discovery and notebook export.
Rendering uses 200 actual TestBackend frames. The interactive search workload
types /systems one key at a time, verifies all matches and cancels; it includes
seven FTS queries, selected-body loading and cancellation, not a single search.

Normal/stress inputs include 200/2,000 RSS entries, 1,000/5,000 OPML feeds,
100 advertised HTML feeds and 100/500 annotated notebook articles with two
new annotations each. Every workload verifies its result. These are separate
resource costs; a small render time does not establish small search latency.

Final raw measurements: benchmarks-0.2-2026-10-16.txt.
Initial expansion and the rejected directly joined FTS experiment are retained
separately. The joined plan increased broad-prefix typing-cycle latency, so
the final query retains the ID-set predicate. Routine ingestion also avoids
rewriting an unchanged feed title and rebuilding its entire search index.

No general speedup, terminal-emulator latency, idle CPU, network throughput
or allocator claim is made. The historical 0.1 workload searched only its
10,000-row metadata window; it is not equivalent to 0.2's whole-library body
and annotation search. Compare equivalent repeated workloads before claiming
an improvement. Contended disks, very large subscriptions and concurrent
multi-process exports need additional qualification.

## Measured 0.2 baseline

Medians of the three per-run means; operation times are milliseconds unless
the row states otherwise. The render backend measures buffer work only.

| Workload | 10k articles | 50k articles |
| --- | ---: | ---: |
| metadata snapshot (no bodies) | 10.620 | 10.379 |
| durable comment/favorite/list cycle | 0.361 | 0.345 |
| RSS decode + content normalization | 0.768 | 6.929 |
| indexed body search + last page | 11.800 | 64.989 |
| Markdown wrap + syntax highlight | 0.042 | 0.042 |
| OPML decode + URL validation | 0.775 | 3.853 |
| HTML5 discovery (100 advertised feeds) | 0.118 | 0.115 |
| stream notebook export (2 notes/article) | 23.376 | 194.616 |
| three-pane frame (144x44) | 0.216 | 0.219 |
| FTS live search + selected body | 58.393 | 311.610 |
| Process wall time (seconds) | 5.390 | 29.100 |
| Process peak RSS (KiB) | 19364.000 | 23872.000 |
