# Performance and qualification

Lume keeps expensive work away from navigation and rendering: bounded metadata
pages, replaceable read projections, independent catalog counts, ordered personal
writes, background content/image preparation and durable configuration
publication. Algorithm choice alone is not evidence of a speed improvement.

## Reproduce

`make check` runs correctness scenarios for every implemented benchmark. Full
measurements are separate:

```sh
make bench
make bench-configuration
make bench-code
make bench-interaction
make bench-reading
make bench-images
make bench-notes
make bench-recommendations
```

The native benchmark also accepts focused flags, including
`--configuration-only`, `--welcome-only`, `--code-viewport-only`, `--images-only`,
`--tables-only`, `--lists-only`, `--rich-text-only`, `--recommendations-only`,
`--pane-search-only`, `--query-editing-only`, `--library-sections-only` and
`--fluency-only`, `--server-refresh-only`, `--publisher-refresh-only`,
`--retention-images-only`, `--retention-only`, `--media-review-only` and
`--motion-review-only`.
Deletion has its own `--article-deletion-only` workload.
`--active-reading-only` checks cached and changing viewport observations in
batches of 1,000 polls, with simulated monotonic time and durable unread-state
verification. The formula workload also checks styled inline prose without
requesting graphics. These timings exclude terminal presentation.
`--read-catalog-only` exercises active-read commits with 46 distinct broad
smart-list queries. `--image-priority-only` replaces two obsolete SVG sources
and checks the newly visible source's pixels. Both accept `--stress`.
Add `--stress`
for larger workloads.
Setup precedes operation timers; resource reports identify process-wide
memory/I/O scope.
Routine reports go to ignored `.local/benchmarks/`. Promote reviewed evidence
here only when useful for a release or a specific comparison.

The section workload measures Enter through a full interactive TestBackend
frame with 13,000 articles, 200 feeds and 46 smart lists. Its stress variant
keeps 13,000 articles and widens the sidebar to 2,000 feeds. It verifies that
folding retains every destination, keeps focus on the header and queues no
catalog or article read. Fixture creation and the initial catalog counts are
outside the operation timer. TestBackend timings exclude terminal presentation.

Section counts and grouping are prepared when a catalog arrives. Folding only
projects this cache; pinning and reordering rebuild its presentation without
reading article counts. Refreshes still replace counts from a current database
snapshot. Native failure scenarios make catalog queries unavailable and verify
that keyboard and pointer folding remain usable.

## Retained evidence

These reports retain their original binary hashes, dates and measurement scope.
They describe those revisions and environments, not a blanket guarantee for all
current hardware or a substitute for measuring later changes.

| Contract | Evidence |
| --- | --- |
| Long Unicode/code rows | [Checked frame comparison](performance/code-viewport.json) |
| Large-library navigation | [Repeated measurements](performance/navigation.json), [samples](performance/navigation-samples.csv), [Foot qualification](performance/navigation-foot.json) |
| Notebook search/context | [Representative and stress workloads](performance/notebook.json) |
| Native image interaction | [Preparation/scroll evidence](performance/images.json), [real Foot](performance/images-foot.json) |
| Declarative configuration | [Publication and verified rereads](performance/configuration.json), [real Foot interaction](performance/configuration-terminal.json) |
| Full-history feeds | [Large archive decode and content checks](performance/archive-feeds.json) |
| Reader tables and code backgrounds | [Unicode layout, cached frames and inspection](performance/tables.json) |
| Dense article preparation | [Repeated reader comparison](performance/reader-review.json) |
| Native welcome frame | [Representative/stress frames and interaction qualification](performance/welcome.json) |
| Reading services | [Repeated checked workloads](performance/reading-services.json), [native Foot qualification](performance/reading-foot.json) |
| Reading-state and offline review | [Before/after workloads and native Foot](performance/reading-review.json) |
| Image source reuse and formula fallback | [Checked before/after measurements](performance/media-review.json) |
| Scroll frames and empty-cache cleanup | [Checked comparison](performance/motion-retention-review.json) |
| Article deletion feedback | [Committed deletion/frame comparison](performance/article-deletion.json) |
| Active read policy and inline LaTeX | [Checked representative/stress costs](performance/active-inline-reading.json) |
| Catalog invalidation, visible images and server pacing | [Paired checked workloads](performance/priority-review.json) |

The priority review compares `0cd095b` with the revised working tree, using
three optimized processes per workload/size and Rust 1.99.1. Its standalone
native drivers use identical workload code against each optimized application
library and its exact Cargo runtime dependencies. Setup and undo are excluded
from native operation timers. Active time is simulated; writes and final
database state are real. Terminal/compositor presentation is not measured.

| Operation | Before | After |
| --- | ---: | ---: |
| Automatic read + counts, 2,000 articles / 46 distinct lists | 23.325 ms | 1.099 ms |
| Automatic read + counts, 13,000 articles / 46 distinct lists | 151.446 ms | 3.081 ms |
| Visible image after two obsolete 800-path SVG sources | 3.409 ms | 1.144 ms |
| Visible image after two obsolete 3,000-path SVG sources | 11.431 ms | 2.937 ms |
| Local Miniflux sync, 8 feeds / 9 requests | 3,527.526 ms | 42.870 ms |
| Local Miniflux sync, 16 feeds / 17 requests | 7,525.756 ms | 45.292 ms |

Values are medians of process iteration means; server values are medians of
whole CLI calls, including fresh database/config initialization and discovery.
The HTTP fixture returns empty entry pages immediately. It measures scheduler
pacing rather than publisher/server throughput on a real network. Image
results use offline source files and verify the visible blue cell color.
An already-started decode/encoding is not preempted, and an image HTTP request
waiting for response headers can still occupy a lane until its six-second
deadline. Real Foot qualification remains pending in this environment.

The active-read/inline-LaTeX report uses three optimized processes per workload
and size with Rust 1.99.1. Observing a cached viewport costs median 0.042/0.044 ms
per 1,000 polls with 200/2,000 paragraphs; changing the viewport costs
0.189/0.198 ms. Checks retain the same selected article and durable unread
state without reaching the configured time threshold. Styled inline TeX with
20/120 sentence copies costs 0.027/0.127 ms per parse/layout and requests zero
graphics. These are absolute local costs without a prior-version comparison.
The release executable builds successfully; real Foot qualification for this
revision remains pending because this environment lacks Xvfb and Weston.

The reading-service workloads use three release processes per size with Rust
1.99.1 on the recorded Core Ultra 7 environment. A selector movement pair plus
two cached TestBackend frames takes median iteration means of 0.799/0.802 ms
with 200/2,000 library articles and 50 displayed choices. Cached linked-page
admission, background layout and delivery takes 2.061/16.103 ms for 50/500
sections. Neither measurement includes HTTP or physical presentation.

Planning from 100/512 scored candidates takes 0.042/0.181 ms. Preparing
10/100 cached full articles with a decoded, pinned SVG takes 7.169/24.900 ms;
reclaiming the same number of eligible Full caches takes 1.084/10.050 ms.
Operation timers exclude fixture creation and cache refill. Process-wide peak
RSS across these workloads ranges from 16,328 to 36,228 KiB, including setup.
The retained report contains repeated wall time, CPU and filesystem I/O counts;
it establishes current costs, without an equivalent previous speedup baseline.

Native Foot qualification covers linked text views without parent-image
leakage, back navigation restoring cached image strips, opt-in diagnostics,
selection, zoom and terminal restoration. The controlled fixture's publisher
text remains unchanged. Startup waits for prepared article content before
issuing source-bound commands; it does not rely on a fixed loading delay.

The [offline review](performance/reading-review.json) applies the same expanded
workload to `e885e22` and the revised implementation. With 10 cached articles and
128 prior pinned images, median iteration means fall from 8.672 to 7.618 ms
(12.15% less). With 100 articles and 2,048 prior images, they fall from 232.805
to 29.887 ms (87.16% less). A package-local accounting/decoder lifetime removes
repeated directory scans while retaining decoder checks and atomic publication.
Each size uses three release processes on the same recorded machine/toolchain.
These measure cached offline preparation, excluding HTTP and fixture creation;
they do not establish a whole-application or physical display speedup.

The same review qualifies late-save isolation across pending navigation and
repeated URL visits, pending-page cancellation before local history, mouse
anchor invalidation between pages, retained-page reflow after errors and byte
accounting after rejected pins. Native Foot images, selection, zoom, linked
history, diagnostics and terminal cleanup also pass on the recorded binary.

Configuration measurements cover actual source/folder edits, stable portable
identities, TOML publication, comment retention and verified rereads. Three
release processes measured median iteration means of 14.660 ms at 250 sources
and 275.152 ms at 5,000 sources on the recorded Core Ultra 7 environment.
Publication runs on its own lane; these are not frame latency measurements or
comparisons against an earlier implementation. The source workloads and resource runner remain executable even
when a previous raw report is no longer retained.

The archive workload uses full-content RSS documents of 5,253,056 bytes with
200 entries and 13,365,856 bytes with 2,000 entries. Three release processes per
scenario, each with 40 iterations, measured median iteration means of 73.473 ms
and 192.036 ms respectively. Every article's identity, title and complete body
is checked. These describe bounded background parsing, not UI latency or a
speedup over the former 4 MiB rejection. Repeat with
`cargo bench --locked --bench reader -- --archive-feeds-only`, adding `--stress`
for the larger input; resource measurements include setup.

The table workload checks Unicode alignment and exact source correspondence at
64 rows/four columns and 1,000 rows/12 columns. Three release processes per
scenario measured median iteration means of 0.375/10.826 ms for preparation,
0.511/0.753 ms for cursor movement plus a selected cached frame, and
0.021/0.738 ms for opening and closing the cached inspector. Each process runs
40 layout/inspector iterations and 1,000 frames, including a fenced code block
with short and blank rows. Process RSS, including grammar and fixture setup,
ranges from 20,976–21,492 KiB and 42,892–44,216 KiB respectively. These measure
the implemented behavior without a speedup baseline. Repeat with
`cargo bench --locked --bench reader -- --tables-only`, adding `--stress` for
the larger input.

The [reader review](performance/reader-review.json) compares the same checked
workloads against `102c4e7`, using the pinned toolchain and machine. With 200
links, median paragraph preparation fell from 2.261 to 0.938 ms; at 2,000 links,
from 100.435 to 7.505 ms (92.53% less). Each scenario uses three release
processes and 40 iterations, checking complete readable text, all link labels
and destinations, and the final source anchor. Source sizes are 14,985 and
153,785 bytes. These measure layout preparation, not whole-application latency.

Five alternating processes per revision also checked 1,000 cached code frames
each. Median iteration means were 0.253/0.249 ms for the smaller row and
0.238/0.255 ms for the larger row, before/after respectively. Results are mixed;
no frame speedup is established. The proposed projection allocation change was
removed, and the retained viewport projection code is unchanged. Resource
records include process-wide setup, memory and I/O.

The [retention measurements](performance/retention.json) use 1,000 and 10,000
articles with 290-byte feed summaries and 9,280-byte Full caches. Three local
unoptimized test processes per scenario measured median previews of 8.120 and
77.109 ms, and complete applications of 162.637 and 1,812.783 ms. Application
removes 497/4,997 old read articles and clears 498/4,998 unread Full caches;
five articles with a favorite, queue item, folder, note or smart-list match
retain their caches and personal state. Transactions contain at most 32 article
actions. The stress workload explicitly raises the per-run limit to 10,000;
the default remains 1,000. Setup and post-application checks are outside timers.
Images, network, terminal presentation and resource consumption are not timed.
This is a new capability with no speedup baseline, and the unoptimized figures
are not release latency claims. Reproduce with
`cargo test --locked --bench reader -- --test --retention-only`, adding
`--stress` for 10,000 articles. The report records the executable hash and
local qualification results, including the graphics-tool substitutions used
for real Foot and the uncommitted-source package status.

The server-refresh report also retains a revalidation after the retention and
credential changes: median unchanged discovery was 2.193 ms and a 2,000-entry
unchanged ingest was 23.888 ms, using the same unoptimized fixture and three
processes. Native regression tests separately assert zero SQLite changes for
identical publisher content and redundant personal flag writes.

The [refresh and checkpoint review](performance/refresh-noops.json) compares
identical checked workloads against `ca220f6`. Three sequential local test
processes per scenario, with two timed iterations each, measured the following
median iteration means. These unoptimized timings exclude fixture setup and
include each workload's independent state assertions.

| Operation | Before | After |
| --- | ---: | ---: |
| Unchanged publisher refresh, 200 articles | 43.000 ms | 1.730 ms |
| Unchanged publisher refresh, 2,000 articles | 478.115 ms | 16.539 ms |
| One publisher correction among 2,000 articles | 477.345 ms | 16.185 ms |
| 100 identical reading checkpoints, 2,000-article fixture | 31.460 ms | 27.414 ms |
| Fresh-image retention preview, 2,000 articles/32 files | 495.698 ms | 5.005 ms |

Publisher equality is checked before Markdown duration estimation and canonical
URL derivation; conditional updates also avoid unchanged validator/error writes.
Native tests verify zero SQLite changes and no other-connection `data_version`
advance for unchanged refreshes and reading checkpoints. Checkpoint elapsed-time
gains are modest: layout validation/serialization still runs, while redundant
logical writes are removed. Saves still observe intervening external changes.

Image retention filters files using the shortest configured lifetime before
loading historical bodies. When older files exist, reference bookkeeping retains
only paths actually present in that candidate cache. It visits long galleries
without applying the offline package's 64-download limit; shared protected or
unlimited sources still keep their files. Timed images are fresh owned files;
expired deletion and decoding are excluded. Memory and physical I/O are not
measured, and these figures do not establish production or end-to-end latency.

The [media review](performance/media-review.json) compares the same checked
workloads against `4a6cb89`. Three sequential local unoptimized test processes
per size each run two timed iterations. Sources are prepared before timing;
each image iteration alternates both sources and verifies the resulting
dimensions and visible output. Formula iterations verify every raw fallback.

| Operation | Before | After |
| --- | ---: | ---: |
| Alternate two 1200×600 PNG sources and fit | 163.013 ms | 138.975 ms |
| Alternate two 2400×1200 PNG sources and fit | 641.792 ms | 538.031 ms |
| Alternate two SVG sources, 150 paths each | 5.929 ms | 3.518 ms |
| Alternate two SVG sources, 900 paths each | 33.197 ms | 19.348 ms |
| Prepare 20 repeated unsupported formulas | 1.886 ms | 0.439 ms |
| Prepare 120 repeated unsupported formulas | 9.175 ms | 0.936 ms |

These measure bounded decoded-source/tree reuse and cached failed conversions,
including fitting and output checks. They exclude downloads, physical terminal
presentation and process memory/I/O accounting. They do not establish release
or whole-application latency. JPEG orientation, UHD admission, 16-bit alpha
composition and switching theme backgrounds have separate native correctness
tests; these timings do not measure those cases. Reproduce with
`cargo test --locked --bench reader -- --test --media-review-only`, adding
`--stress` for the larger inputs. The report preserves raw samples, executable
hashes and qualification details, including local graphics-tool substitutions.

The [motion and storage review](performance/motion-retention-review.json)
compares identical checked workloads against the preceding headline and cleanup
implementations in `3ceade2`. Each size uses three sequential unoptimized local
test processes. Fixture setup precedes timing; each process checks two empty
image-cleanup passes and 60 reader motion pairs with two complete pane frames.

| Operation | Before | After |
| --- | ---: | ---: |
| Motion pair + two frames, 200 articles/260-byte longest headline | 6.101 ms | 5.405 ms |
| Motion pair + two frames, 2,000 articles/943-byte longest headline | 7.239 ms | 5.883 ms |
| Empty image-cache cleanup, 200 articles | 33.786 ms | 0.044 ms |
| Empty image-cache cleanup, 2,000 articles | 326.888 ms | 0.048 ms |

Headline reuse avoids repeated title wrapping/date formatting during reader
motion; live status/styles and metadata corrections remain current. Empty image
cleanup returns before loading bodies. Other cases still inspect references,
including long galleries, and skip files replaced after inventory. Separate
native scenarios verify storage units/totals, preset publication/protections,
cancelled automatic scheduling, source-preserving spatial inline formulas and
two-press article deletion/undo.

These measurements include state/output assertions and exclude HTTP, source
preparation, native Sixel encoding and physical terminal presentation. Memory
and I/O accounting are not measured. They do not establish release latency,
expired-image deletion speed or a whole-application speedup. Reproduce with
`cargo test --locked --bench reader -- --test --motion-review-only`, adding
`--stress` for the larger fixture. The report retains raw samples, executable
hashes and local qualification details.

The [article deletion comparison](performance/article-deletion.json) uses
2,000/13,000 articles and 46 populated smart lists. Three sequential local
unoptimized processes per size each perform two deletions through the real `dd`
key path. Undo restores the captured identity outside each operation timer.

| Operation | Before | After |
| --- | ---: | ---: |
| Background `dd` to first committed frame, 2,000 articles | 61.093 ms | 5.377 ms |
| Background `dd` to first committed frame, 13,000 articles | 340.278 ms | 5.164 ms |
| Background `dd`, page/source/counts settled, 13,000 articles | 340.344 ms | 317.636 ms |
| Synchronous `dd` and committed frame, 13,000 articles | 673.501 ms | 330.049 ms |

Commit acknowledgement updates the bounded metadata page before recounting
smart lists or preparing the next source. Counts and source/page reads proceed
independently; count publication does not repeat that page load. No row is
hidden before its write commits, and failed writes retain the existing row.
The synchronous path no longer reloads the whole library twice.

First-frame timings mean the updated Articles list is drawn; reader preparation
and sidebar counts can still be pending. Complete reconciliation remains much
more expensive than this initial feedback. Timers include a 1 ms poll cadence
and TestBackend drawing; setup, undo, network and physical presentation are
excluded. Process RSS and I/O are not measured. Native regressions separately
cover rejected writes, source-bound editors, successive deletions, grouped
representatives, last-page refill, restoration and undo. Reproduce with
`cargo test --locked --bench reader -- --test --article-deletion-only`, adding
`--stress` for 13,000 articles. These measurements do not establish release or
physical-display latency.

## Boundaries

The [server refresh comparison](performance/server-refresh.json) checks 100
remote feeds and 2,000 cached articles across three independent processes per
version. Median unchanged discovery fell from 173.224 ms to 2.184 ms; applying
an unchanged 2,000-entry batch fell from 715.703 ms to 24.775 ms. These are
unoptimized local store measurements with two iterations per process, excluding
network parsing and terminal work. They do not establish production latency.
Fixture setup precedes each timer, and each operation checks identities,
counts and preservation of personal state.

Unchanged discovery now avoids title-triggered FTS rebuilding and marking every
remote feed absent before restoring it. Repeated entries compare publisher
fields before normalization metadata or index updates, reuse prepared SQL and
retain all local flags. Native regressions verify zero SQLite changes for an
identical repeat and correct search-index replacement for changed bodies.

TestBackend timings measure checked application work, not terminal transport or
physical display latency. The real Foot latency observer measures injected input
through a correlated synchronized frame and terminal DSR acknowledgement. It
excludes the compositor and physical key-to-photon time. At least 100 samples
per workload are required for its reported p99; compare repeated independent runs
before claiming an improvement.

Actual Foot qualification uses owned windows and temporary libraries,
checks cleanup and never changes terminal preferences or the host clipboard.
Desktop-dependent checks remain separate from ordinary local verification and
must be reported as unavailable if their environment cannot be provided.
See [development](development.md).

## Configuration and installer review

The [repeated before/after workloads](performance/review.json) compare the same
pinned toolchain and machine, with exact result, identity and comment checks.
At 5,000 subscriptions, median publication-plus-reread time fell from 269.069
ms to 187.792 ms (30% less). The actual revision-query SQL and row mapping fell
from 163.680 microseconds to 1.535 microseconds per check. At 250 subscriptions,
publication fell from 14.712 ms to 11.088 ms; revision checks fell from 9.400 to
1.705 microseconds. These are scoped operations, not whole-application speedups.

The revision query excludes the serialized snapshot and reuses its statement.
Single-writer merges avoid an intermediate JSON tree, projection reuses validated
definitions, and document patching reuses the validated input. Configuration
publication remains outside the input/render lane. Process RSS includes fixtures;
benchmarks are not physical key-to-photon measurements.

The same report qualifies exact-byte executable upgrades at 16/64 MiB and SQLite
snapshot migration at 1,000/10,000 articles. Upgrades verify that library and
configuration bytes remain unchanged. These new installer workloads have no
speedup baseline.

The [table-layout measurements](performance/configuration-format.json) record
current publication-plus-reread medians of 12.517 ms at 250 sources and
213.455 ms at 5,000 sources. Rewriting compact input, formatting atomically and
verifying a reread takes 7.064 ms and 168.330 ms respectively. Idle revision
checks take 1.525 and 1.550 microseconds. Each scenario uses three release
processes with 40 iterations; process RSS, including setup and all workloads,
ranges from 17,148–18,120 KiB and 83,096–88,716 KiB. These characterize the new
document representation, including its publication cost; they do not establish
a speedup. Explicit formatting and publication remain outside input/render work.

Real Foot qualification of the reviewed application used 50,002 articles,
16 sources and 64 smart lists, with 100 correlated samples per scenario. The
input-to-terminal-acknowledgement p99 was 5.464 ms during collection, 6.141 ms
with cached images and 22.174 ms for article switching. This excludes physical
presentation; it is not a before/after terminal speedup comparison. Native image
scroll, selection, welcome entry and terminal cleanup also passed.

See the [durability and navigation qualification](performance/audit-remediation.md)
for dense annotations, startup phases and validated snapshot measurements.

The [reader presentation measurements](performance/reader-presentation.json)
qualify cached justification, Vim selection and exact copying at 200 and 1,200
paragraphs. Across three release processes, the median checked cycle took
0.250 ms and 0.259 ms respectively. Each process performed 800 cycles; source
layout and justification indexes preceded operation timers. Peak process RSS,
including setup, ranged from 16,512 to 20,872 KiB. These are TestBackend work
measurements, without a baseline speedup or physical terminal latency claim.

The [inline sidebar search measurements](performance/pane-search.json) cover
200 and 5,000 sources. A cycle types ten query characters, selects the exact
next matching destination, renders a cached frame and cancels the filter.
Across three release processes of 800 cycles each, median cycle times were
0.287 ms and 0.796 ms respectively. Setup and normalized-name indexing precede
the operation timer. Peak process RSS, including setup, ranged from 14,412 to
17,084 KiB. This characterizes the new feature; it is neither a before/after
speedup comparison nor physical terminal latency.

## Query editing and navigation review

The [checked comparison](performance/interaction-review.json) uses production
code from `2e03a8a` as the baseline, with the same new query workload applied to
both builds. Three alternating process pairs per size measure 800 cycles of
two query-caret motions and a full cached TestBackend frame. Setup precedes
the operation timer; process resource records include setup.

At 2,000 matching articles, the median iteration mean fell from 5.528 to
0.301 ms (94.55% less); at 20,000 articles, from 47.598 to 0.329 ms
(99.31% less). Unchanged query text no longer reissues a database query,
replaces results or reloads the selected article. Native editor change signals
also avoid reconstructing unchanged draft text on caret motions.

Sidebar projection now bounds work by visible rows and preserves Unicode count
alignment. This does not establish a frame speedup: full sidebar-frame medians
were 0.312/0.339 ms before/after at 128 destinations and 0.344/0.353 ms at
4,096 destinations. Selected reader-frame results were also slightly slower.
These differences remain visible in the report; the query improvement is not
a claim that all interactions became faster. Peak process RSS across workloads
was 15,188–25,984 KiB before and 15,128–26,020 KiB after.

Repeat with `cargo bench --locked --bench reader -- --query-editing-only` and
`--interaction-only`, adding `--stress` for larger fixtures. These measurements
cover application work, excluding terminal transport and physical display
latency.

## Live search and formula preparation

`--fluency-only` checks incremental article metadata publication and frames with
2,000 articles (20,000 under stress), a 140,600-byte selected source, help
search and Unicode formula preparation. Article preview does not fetch or reflow a new
Reader source until committed. Existing results remain visible while replacement
queries run, and generation checks discard obsolete replies. Sidebar matching
narrows its existing in-memory identities for extending queries and rebuilds the
candidate set when a query broadens. These workloads exclude fixture setup.

The Foot qualifier additionally decodes Sixel strips independently and measures
mean RGB spread across six strips in three known flat regions. A two-level
channel budget permits continuous diffusion while rejecting per-row palette
bands; this supplements the independently known fixture color checks. Two real
HTTP image lanes are qualified by observing both requests before either response
is released; crops of the same URL share one download. No physical display or
whole-application speed claim follows from preparation benchmarks alone.

`--formulas-only` qualifies sums, integrals, accents, matrices and function
application across 40 formulas (250 under stress). It separately measures
rendered and raw-source layout, then checks repeated policy/cache transitions.
Formula-off layouts never invoke the LaTeX converter. Background layout keys
include formula policy; pending replies from previous policies are discarded.

Nested list workloads include mixed ordered/unordered levels, empty publisher
items and invisible anchor-only entries. Empty entries do not paint markers;
ordered entries retain their numbering authority. Rendering does not rewrite
cached publisher bytes or invalidate annotation offsets.

## Optional typesetting and image color review

Formula preparation caches up to 32 successful text projections and 32 shared
vector sources per document layout; input count/byte limits still apply to cache
hits. The `--formulas-only` workload checks text, raw and graphic preparation,
including bounded slots and exact source identity. Graphic font parsing is lazy
and excluded from lightweight mode. Complex and display equations occupy compact
blocks while short inline expressions retain paragraph flow. The arXiv abstract
workload checks compact layout and its two complex formula slots.

The image worker retains one bounded parsed SVG tree for repeated zoom/pan,
while retaining the original raster precision. Transparent raster pixels are
composited against their actual matte before filtering, including a separate
16-bit path. Native Sixel inspection and inline strips share Quantette's Oklab
palette preparation; one continuous index image avoids per-row palette seams.
Sixel still quantizes to at most 256 colors and integer color percentages.
These limits do not permit claiming lossless photographs or recovering detail
absent from a publisher's source. All preparation remains outside drawing.

On 2026-10-22, three alternating optimized process pairs compared this review
with `79e54c5`, using Rust 1.99.1 and the same fixtures. Median milliseconds per
iteration are below. Image preparation uses 40 iterations per process; cached
Sixel scrolling uses 200, and other cached frames use 2,000. Formula preparation
uses 40 expressions with five distinct sources, including repeated cache hits.

| Workload | Before | After |
| --- | ---: | ---: |
| Repeated SVG paths/text, block preparation | 9.965 | 0.982 |
| Repeated SVG zoom, block preparation | 9.653 | 0.480 |
| SVG source and Sixel inspector encoding | 93.059 | 48.222 |
| SVG source and inline Sixel strips | 22.038 | 9.305 |
| Cached Sixel scroll and two clipped frames | 5.510 | 5.495 |
| Cold raster decode and block preparation | 4.438 | 4.553 |
| Repeated text formula layout | 0.391 | 0.106 |
| Graphic formula layout and vector sources | — | 1.075 |

The SVG gains reflect retained parsing and improved color preparation. Cached
scrolling is effectively unchanged; cold raster preparation shows a small cost
in this sample. Graphic layout excludes pixel encoding and costs more than
text, so it remains opt-in. These controlled workloads establish neither
physical display latency nor uniform improvements for every article.

Foot 1.21.0 qualification additionally decoded the native formula pixels,
checked zoom and original-TeX copying, then returned to persisted text mode.
Its diagram fixture had zero inter-strip channel spread in all three sampled
flat regions, within two RGB levels of the original source colors. Selection,
bidirectional scroll, popup restoration and terminal cleanup also passed.

The subsequent compact-layout review corrects two separate presentation bugs:
fitting every equation to its slot enlarged even single variables, and the
four-publisher-image native budget also downgraded later formulas to blocks.
Graphic canvases now use font-relative rows with bounded padding. Formula
slots always use native Sixel when selected, independently of the four native
publisher images. A 40-entry cache admits all 32 formula slots, four native
publisher images and two inspector crops without visible-slot churn; the
32 MiB encoded-strip budget remains enforced. Pending or failed formulas
retain selectable text instead of covering it with image-loading messages.

Library windows retain a transient origin and scroll minimally to reveal an
offscreen selection. Clicking an already visible destination preserves every
pointer target; clicking the active destination avoids reloading its articles.
Full-row reference tests cover forward/reverse navigation, repeated group
headers and filtered identities at multiple heights. Drawing never mutates the
viewport state or scans the entire catalog to find a visible row.

Viewport capture belongs to destination changes, filtered-selection movement
and resize, not every query keystroke. During a formula policy change, later
keyboard or mouse movement retargets the pending source anchor. A completed
layout therefore preserves the user's newer position instead of restoring the
cursor captured before the policy change. This path remains inactive when no
formula projection is pending.

The progress footer follows the reader cursor over the article's display rows.
It advances while reading the first screen, before the viewport scrolls, and
works the same way for articles fitting entirely in one screen. The first row
reports 0% and the final row 100%; empty documents report 0% and a single row
reports 100%. Saved reading anchors remain source-based. Boundary and runtime
frame tests cover forward/reverse movement independently of persistence.

Article cards retain a transient viewport origin. Clicking a visible card
preserves every pointer target; keyboard motion scrolls only enough to reveal
an offscreen selection. Drawing and pointer lookup share the same bounded
window, including compact cards and the inline search field.

The `--panes-only` workload checks those pointer identities through actual
clicks, background delivery and runtime frames. One release process with
2,000 articles measured 0.566 ms per pair of clicks/frames with regular cards
and 0.614 ms with compact cards, across 800 iterations each. Setup precedes
timing; these TestBackend observations establish no before/after speedup.

The [syntax-cache review](performance/syntax-cache-review.json) compares three
alternating release processes against `eeb0297`. After visiting 12 code blocks
of 48 rows each, admission of a small article's resize fell from a median
0.355494 to 0.002343 ms across 2,000 admissions per process. Immutable syntax
rows are shared with admitted layout jobs; unchanged requests do not build
another snapshot. Polling a completed layout also avoids constructing code
rows that would immediately be discarded. The approximate 4 MiB retained
syntax budget and bounded worker queues remain in place.

Each admission checks retained readable text; revisiting the code checks
source preservation and syntax colors. The final workload verifies all 576
nonempty code rows before and after. Unit tests cover snapshot reuse across
source/width/style changes and ownership after releasing the original cache.
Existing blocked/disconnected-worker tests continue to qualify readable
fallbacks and nonblocking navigation.

Checked cached frames remained close to baseline: median iteration means were
0.310459/0.313051 ms for justified selection, 0.301427/0.293017 ms for deep
Unicode code, and 0.394765/0.394268 ms for sidebar search, before/after. A
speculative viewport allocation change was removed after failing to improve
frames. These results establish a syntax-cache admission gain, not a uniform
speedup or a physical display latency improvement.

One final Foot run with 10,002 articles, 16 feeds and 64 smart lists collected
100 samples per scenario. Input-to-terminal-ack p95 was 3.887 ms for scrolling
during collection, 3.636 ms for cached-image scrolling and 20.881 ms for article
switches. Native image/formula qualification and terminal cleanup also passed.
The shared startup driver now accepts each fixture's own article marker, so
the latency test does not wait for text belonging to the image fixture.
These final-runtime observations provide no before/after latency comparison
and exclude compositor and physical presentation time.

The extended personal-invalidation/layout stage adds `--recent-layouts-only`.
It checks three warm exact-source layouts without requesting Markdown work;
its stress form retains 2,000 paragraphs per source. The ordinary unoptimized
correctness run observed 0.236 ms per three-source round trip; this is an
absolute local check, without terminal presentation or a release comparison.
