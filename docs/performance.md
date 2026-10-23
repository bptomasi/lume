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
`--fluency-only`. Add `--stress`
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

## Boundaries

TestBackend timings measure checked application work, not terminal transport or
physical display latency. The real Foot latency observer measures injected input
through a correlated synchronized frame and terminal DSR acknowledgement. It
excludes the compositor and physical key-to-photon time. At least 100 samples
per workload are required for its reported p99; compare repeated independent runs
before claiming an improvement.

Actual Foot and Alacritty qualification uses owned windows and temporary libraries,
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
scroll, selection, welcome entry and terminal cleanup also passed. Alacritty was
not rerun for this revision because xdotool was unavailable.

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
