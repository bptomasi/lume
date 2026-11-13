# Architecture review and qualification — 2026-11-13

## Result and scope

All nine architecture findings were addressed through sequential implementation
and independent corrective review. The integrated matrix and terminal snapshot
is `fe81ac6b3e5078f10fa8ce90f27bb303cfec1faa`. A separately reviewed prototype,
`92f3fdb1bbbe18194b759c89f919b451d0a26a25`, attempted prepared note-summary
statement reuse. Controlled measurements rejected it; revert
`c03b596edd673a291dbb9a76e8d7d3073b932a5e` restores the exact qualified runtime
source. The final evidence commit changes only documentation.

This is an ownership, correctness and build-capability improvement; it does
not establish a general runtime speedup. Navigation remained approximately
stable. The measured notebook regressions remain a known limitation of the
combined update, which includes the necessary SQLite security upgrade.

The baseline predates both the state extraction and a required SQLite security
upgrade. Its SQLite 3.46 engine differs from the candidate's SQLite 3.54.1.
These are whole-change measurements, not an experiment isolating ownership
or attributing every difference to SQLite. No vulnerable engine was restored
for the sake of a favorable result.

## Changes at each boundary

| Finding | Implemented contract |
| --- | --- |
| Central App authority | App fell from 117 fields, 52 public, to 23 private fields. Reader, navigation, notebook, layout, interaction, session, personal operations, article requests, network and maintenance have explicit owners. Cached borrowed views expose presentation state. |
| Module cycles | Neutral validation, commands, URLs, extraction, presentation DTOs and cancellation contracts remove reverse dependencies. Runtime configuration reconciliation belongs to `configuration_service`. Production root-module graphs are checked separately for library, binary and build script. |
| String command identities | The registry defines 152 typed command identities and closed owner routing, with exhaustive dispatch and alias-result coverage. `PersonalFlag` selects fixed SQL, including batch inverse/rollback behavior. |
| SQL ownership | Queries and test fault injection live in their owner's `sql/`. The root check includes a SQL ownership guard and regressions. Static V1–V28 migrations remain byte-for-byte unchanged. |
| Database contention | Store classifies busy/locked errors through the error chain. Application workers do not inspect rusqlite error codes. |
| Deadline drains | Session admission/flush uses channel notification and one deadline rather than periodic capacity polling. UI admission stays nonblocking. Shutdown preserves failure, timeout, ordering and durability guarantees; actual contention still has bounded retry backoff. |
| Dependency scope | Six optional capabilities retain full defaults and portable configuration/data. Reduced builds have explicit source-text, raster, Blocks and server-collection fallbacks. |
| Overlapping names | Orchestration is `backup_service`, `retention_service`, `offline_service`; disk inspection is `disk_usage`. Store retains its own `snapshots`, `backup_schedule` and durable primitives. |
| Process and documentation | CI derives the pinned compiler from `rust-toolchain.toml` and minimum compiler from Cargo.toml. Capability/MSRV qualification is required before release. Architecture/performance summaries, module diagram and focused-change guidance make the contracts discoverable. |

UI intentionally imports borrowed view types from `app`; it receives no App
reference or mutation authority. App does not depend on UI. The separate frame
compositor preserves base drawing, native overlay returns, capture and
foreground order. Lazy footer resolution avoids introducing a per-frame note
scan when the relevant presentation is absent.

The architecture guard is a documented lexical policy tool, not a Rust type
checker or general macro expander. It follows production roots, imports,
aliases, reexports and reachability within its supported syntax. Native tests
remain necessary for lifecycle and state invariants.

Independent reviews corrected real defects during the extraction, including
prefix handling through a copied getter, stale worker publication, maintenance
completion lifecycle, retaining consumed worker failures, and unsupported SVG
cache hits triggering unnecessary HTTP. Unsupported cached vectors now keep
readable offline text and report Partial without redownloading or pinning an
unsupported resource. A real listening socket regression detects an attempted
connection rather than accepting an unstarted server as proof.

## Security and data compatibility

The final security gate exposed advisories in the old bundled SQLite and
rustix. The updated graph uses rusqlite 0.41.0, libsqlite3-sys 0.39.1 with SQLite
3.54.1, Refinery 0.10.0/core 0.10.1 and rustix 1.1.6. The existing locked time
0.3.55 parser is an explicit narrow dependency for migration representation
validation. See [dependency security](../dependency-security.md).

Refinery still owns migration ordering, checksums and grouped transactions.
The compatibility preflight rejects representations that would panic upstream;
it does not rewrite user history. Immutable old-engine V7/V27 fixtures exercise
legacy upgrade, personal/publisher data, FTS integrity, snapshots and metadata.
Cancellation hooks use fallible APIs with explicit callback cleanup.

Actual cargo-audit 0.22.2 qualification found zero vulnerabilities with the
existing six scoped informational reviews unchanged. RustSec database commit:
`af7b3ec33460b8e2de8d422e16d628518891a519`, 1,490 advisories, updated November 12.
The audited Cargo.lock SHA-256 is
`d13eb51ce1706b5390f2137f423f409c4a9f6b475389e7bef5b4d18e3802939a`.
Actual cargo-about 0.9.2 regeneration covers 398 licensed packages including
Lume and four explicitly inactive lock entries. No vulnerability waiver,
system-SQLite substitution or custom migration driver was introduced.

## Optimized build scope

Both products were built sequentially from the qualified source, locked and
offline, with actual Rust 1.99.1 (`58cf8f9c3`, LLVM 23.1.1), thin LTO, one
codegen unit and debuginfo stripping. Incremental compilation was disabled;
build jobs and test threads were capped at two. No extra Rust flags, source
substitution, containers or alternate product target directory were used.

| Product | Bytes | MiB | Active Linux normal/build dependencies |
| --- | ---: | ---: | ---: |
| Full defaults | 28,353,800 | 27.04 | 342 |
| No default features | 21,269,656 | 20.28 | 246 |

The reduced executable is 25.0% smaller for these artifacts. It omits
capabilities, so this is not an equivalent-functionality speed comparison.
Counts exclude Lume and deduplicate package/version pairs; optional packages
remain locked. `icy_sixel` remains reachable through ratatui-image even when
Lume's own Sixel path is omitted. The complete feature behavior is documented
in [build capabilities](../build-features.md).

Both executables passed `--help` and `--version`. Cargo restored the full
product at `target/release/lume` after the reduced build. Build durations have
different cache histories and are not a compile-speed comparison.

| Artifact | SHA-256 |
| --- | --- |
| Full product | `b3ed3c9ae7d047c3fe467b46c5dbe9f704f2b0717cf68ae44c6f096a32a01321` |
| Reduced product | `b3cc6f68065b3118320e31e0dc70402387793623f3c302520282d598a59eead5` |
| Candidate reader benchmark | `cd93d198692fbe007fc6365068b87d6a80f420f558ea65f71304dac8fea78adc` |
| Frozen reader benchmark | `008ad4f6174165c588a749a871cbf4e37b78f100a405a0688ecef675adef5881` |

## Paired measurements

The frozen executable uses runtime source `b0e81a3`, with only the benchmark
harness and the new shutdown fixture added before runtime changes. Its source
index SHA-256 is
`9cbbaa5438adecd93aa361e91780f318c2ffa629cdc9ce746c12723ce0488d00`.
It was preserved without rebuilding. Both artifacts use the same compiler,
release profile and full defaults on an Intel Core Ultra 7 155H host running
Linux 7.1.8 and glibc 2.41. CPU frequency and host scheduling were not pinned;
no confidence interval or broad platform claim is inferred from these runs.

For each workload, one full run of each binary warmed the paths and was
excluded. Three sequential ABBA blocks then produced six observations per
binary. No build, test suite or child-agent job overlapped the timings.
Ordinary host scheduling and light evidence-file reads/writes were not
controlled. Every run retained its correctness assertions and exited
successfully: 56 processes including eight excluded warmups. Process peak RSS,
command order, harness samples and wall times are retained in the
[observations CSV](data/architecture-2026-11-13-observations.csv); compiler,
artifact, qualification and aggregate evidence are in the
[summary JSON](data/architecture-2026-11-13-summary.json).

The table reports the median of six **run means**, except section toggles,
which report the median of six per-run p50 values. It is not a pooled
per-operation median. Harness output has 0.001 ms precision. Negative change
means less elapsed time; small changes with overlapping run ranges are not
claimed as wins.

| Checked operation | Before ms | After ms | Change |
| --- | ---: | ---: | ---: |
| Session saves, personal/preferences edits and durable flush | 7.221 | 7.245 | +0.3% |
| FEEDS Enter and frame | 0.1915 | 0.1920 | +0.3% |
| SMART LISTS Enter and frame | 0.1955 | 0.1945 | −0.5% |
| SERVERS Enter and frame | 0.1965 | 0.1935 | −1.5% |
| Reader j/k pair and two full pane frames | 0.3915 | 0.3880 | −0.9% |
| Empty image-cache cleanup | 0.002 | 0.002 | 0.0% |
| Note FTS, filtered last page and snippets | 5.9105 | 6.3645 | +7.7% |
| Note/source context and Markdown layout | 0.054 | 0.078 | +44.4% |
| Note checkpoint, metadata, review, repair/version and undo | 2.634 | 2.994 | +13.7% |
| Nested image wikilinks and checked backlink | 0.276 | 0.325 | +17.8% |
| Note key, ready context and actual frame | 0.2615 | 0.2890 | +10.5% |
| Initial note files, citations and publication | 9.945 | 13.6525 | +37.3% |
| Incremental note files and exact-byte ownership | 7.0335 | 9.999 | +42.2% |

Section fixtures contain 13,000 articles, 100 publisher feeds, 100 server feed
projections and 46 smart lists. They assert destination counts and that toggles
do not queue database reads. Motion checks 1,200 j/k pairs, publisher-body and
article identity preservation, with two 160×44 TestBackend frames per pair.
Notebook fixtures contain 10,000 notes over 100 sources; export uses a separate
100-note fixture. The initial export has one operation per process; its median
is across six processes. Normal harness means generally cover 40 operations;
note frame work covers 200. The shutdown fixture calls unchanged session saves
24 times, which progress memory deduplicates, then makes eight actual favorite
edits and four preference edits and verifies persisted state after flush.
Those calls are not 24 independently admitted progress writes.

Changed fixture work is excluded from this comparison: synthetic versus real
library destinations, raw-field versus real-command persistence, code/table
selection, changing-viewport auto-read, contextual note deletion and cached
image/Sixel scrolling. Reduced/full capability inputs are also not an
identical-output comparison. RSS is the whole process peak including fixture
construction, not a measurement of App or image-cache heap use. Timing uses
TestBackend or storage/filesystem work, not physical display or network latency.

## Rejected prepared-statement prototype

A read-only follow-up confirmed that Store notebook/export implementations and
SQL did not change between the frozen baseline and `fe81ac6`. The migration
representation preflight runs at Store opening, outside these operation timers.
App/view extraction is also outside direct Store/export call paths. These
facts narrow the investigation but do not isolate the engine, wrapper or
individual query cost.

`note_summary` recompiles the same joined query for each context/export read.
A narrow prototype reused the existing bounded connection statement cache,
with identical SQL, binding, mapper and error context. It introduced no result
cache, capacity change or transaction lifetime extension. Independent review,
61 affected native cases in both default/minimal builds, strict affected Clippy
and ownership/format checks passed. New tests covered another connection's
committed edits, alternating IDs, missing rows, Trash, restore and deletion/undo.

Correctness was insufficient for performance acceptance. The prototype reader
SHA-256 was
`894a8d52c6d6b23cfebcd6968f76658f8c69ffc13faa45e8a99b9d3d090fcb67`.
With identical compiler, release profile, feature set and SQLite 3.54.1, another
warmup and three ABBA blocks measured the unchanged notebook workload:

| Operation | `fe81ac6` ms | Prototype ms | Change |
| --- | ---: | ---: | ---: |
| Note FTS, filtered last page and snippets | 6.421 | 6.608 | +2.9% |
| Note/source context and Markdown layout | 0.078 | 0.0865 | +10.9% |
| Checkpoint, metadata, review, repair/version and undo | 3.028 | 3.033 | +0.2% |
| Nested image wikilinks and checked backlink | 0.3345 | 0.3325 | −0.6% |
| Note key, ready context and actual frame | 0.2885 | 0.3045 | +5.5% |
| Initial note files, citations and publication | 13.603 | 14.3035 | +5.1% |
| Incremental note files and exact-byte ownership | 10.048 | 10.947 | +8.9% |

This control provides no compensating benefit and records slower target paths;
it does not support adopting the prototype. It also does not establish why
statement reuse lost here: cache misses, preparation flags and binary layout
were not individually profiled. Do not attribute the result to one of them.
The prototype and its additional tests were reverted, retaining the original
fully qualified runtime. The patched database dependencies remain in place.

All 14 same-engine runs, including two excluded warmups, succeeded. A second
14-run old-engine/prototype comparison also succeeded and showed no benefit
over the original frozen notebook baseline; those observations are retained
without relabeling them as an isolated cache experiment:

- [Same-engine observations](data/architecture-2026-11-13-rejected-cache-same-engine-observations.csv)
  and [summary](data/architecture-2026-11-13-rejected-cache-same-engine-summary.json).
- [Combined observations](data/architecture-2026-11-13-rejected-cache-combined-observations.csv)
  and [summary](data/architecture-2026-11-13-rejected-cache-combined-summary.json).

The corrective reviewer briefly submitted the second focused Cargo test before
collecting the first; Cargo reported waiting for the build lock. Both completed
successfully. No timing overlapped those commands. All optimized build and
measurement phases remained sequential. Broad matrix/Foot tests were not
repeated for a rejected prototype whose runtime/test diff was fully reverted.

## Integrated qualification

All phases below passed sequentially at the integrated `fe81ac6` snapshot. The
pinned compiler was actual Rust 1.99.1; the minimum was actual Rust 1.96.0,
matching the unchanged Cargo.toml minimum. No compiler or source substitution
was used. Counts include successful native test harness cases; checked
benchmark scenarios also ran as required by their targets.

| Compiler/profile | Native passed | Native ignored HTTP | Failed |
| --- | ---: | ---: | ---: |
| Pinned/default, root `make check` | 1,186 | 34 | 0 |
| Pinned/minimal | 1,101 | 29 | 0 |
| Pinned/text | 1,133 | 35 | 0 |
| Pinned/graphics | 1,165 | 28 | 0 |
| Pinned/svg | 1,101 | 28 | 0 |
| Pinned/sixel | 1,127 | 29 | 0 |
| Minimum/default | 1,186 | 34 | 0 |
| Minimum/minimal | 1,101 | 29 | 0 |

These repeated profiles are not summed as unique tests. Pinned profiles passed
strict all-target Clippy. Root checks also passed formatting, SQL ownership,
architecture, license inventory and 142 Python tool tests. The source-defined
CI helper ran each reduced/minimum profile offline. Example commands:

```sh
make -j1 check 'CARGO=cargo +1.99.1'
python3 tools/ci.py qualify --compiler pinned --profile minimal --offline
python3 tools/ci.py qualify --compiler minimum --profile default --offline
```

Ignored network cases were qualified separately with actual loopback access:
34 default and 29 minimal HTTP tests passed, zero ignored or failed. Their
recorded HEAD is `02171da` plus the then-uncommitted offline fix; the tested
file hashes match that fix committed as `fe81ac6`. This is source-equivalent
qualification, not a claim that the HTTP command ran after that commit.

A native link initially failed with SIGBUS when the workspace filled. Scoped
cleanup removed only completed Lume development artifacts, retaining dependency
and release caches, source, fixtures and evidence. The unchanged source and
compiler then passed all final phases above. No failed gate was skipped.

## Real terminal and pixels

The optimized full artifact passed the existing real Foot qualification on
native Wayland: Foot 1.21.0, 120×40 cells, 14×29 pixels per cell. Pillow 13.0.0
came from the pinned visual requirements in a temporary virtual environment.
The owned Foot window/private PTY exercised startup, live input, selection,
Unicode/formula help, theme changes, 16 scroll positions, inspector zoom/pan,
rapid replacement, reopening after scroll, linked-page images and return,
settings editing, graphic formulas, original-source copy and clean shutdown.
Terminal attributes were restored and publisher content remained unchanged.

Twelve Sixel reader strips and graphic formulas were decoded independently
from the protocol. The combined reader image and a formula were also visually
inspected: diagram boxes, arrows, labels and formula glyphs were intact.
These are decoded wire pixels, not desktop screenshots or a physical-display
latency measurement. Native Wayland was used locally; the private Xvfb/Weston
CI launch was not rerun here. Hosted CI has not been run for these commits.

## Reproduction and retained evidence

The optimized builds were:

```sh
cargo +1.99.1 build --locked --release --offline
cargo +1.99.1 bench --locked --bench reader --no-run --offline
cargo +1.99.1 build --locked --release --no-default-features --offline
```

Preserve the first full executable and benchmark before changing features.
Use the resulting reader executable with `--shutdown-only`,
`--library-sections-only`, `--motion-review-only` and `--notebook-only`.
For each flag, warm both binaries once, then run baseline, candidate,
candidate, baseline three times. Do not add `--test` for the timings; it reduces
the harness to two correctness iterations. `/usr/bin/time -f %M` records whole
process peak RSS in KiB. Keep the assertions, all observations and mixed results.

The versioned CSV and JSON retain this run's numerical observations and source
identities. Full local logs, frozen source copies, benchmark executables and
pixel PNGs remain under `/tmp/lume-architecture-{before,after,final,measurements}`.
Rejected prototype evidence is under `/tmp/lume-notebook-cache-*`. Those
temporary artifacts are not distributed or permanently retained by Git.
