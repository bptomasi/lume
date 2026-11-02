# Sixel ownership review for 0.2.3

The final review removes avoidable bitmap copies without changing quantization,
resolution, dithering, matte arithmetic or wire bytes. It also makes benchmark
artifacts immutable and rejects invalid timing evidence. Cargo.toml, the root
lock entry, generated license evidence and release instructions agree on
version 0.2.3. Pulldown-cmark is patched to 0.13.6, without feature or transitive
dependency changes, for RUSTSEC-2026-0433.

## Changes and boundaries

Inline preparation consumes its fitted bitmap. RGB8 storage moves into the
indexing step; other input formats use image 0.25.10's existing conversion.
After palette/index construction, only dimensions, palette and indexed bytes
are needed, so the temporary RGB bitmap is released before strip encoding.

Native RGB8 uses the same ownership policy through a separate helper. Other
native formats keep the previous conversion code and buffer lifetimes. Tests
of more aggressive conversion lifetimes and a single-pass RGBA16 matte did
not show consistent gains; those experiments were discarded. No new blending
arithmetic or high-precision conversion policy ships in this release.

Dense Sixel planes are cleared only before subsequent bands. The final clear
was unnecessary because the allocation is immediately dropped. Cancellation
is checked before that work. The existing frozen-writer differential tests
continue to qualify complete planes, masks, partial bands and exact streams.

Both comparison tools now claim a fresh output directory before launching a
child or exporting sources. Reusing a directory fails without overwriting its
evidence. Nonfinite durations, invalid integer stage samples and nonpositive
comparison durations cannot produce a claimed speedup.

The security fix covers an active parsing path: the notebook link index enables
wikilinks, although the Reader does not. A regression test and a checked
benchmark exercise thousands of nested image wikilinks alongside a real note
reference. Notebook reference SQL now belongs to its owning SQL directory;
edge insertion prepares one cached statement per transaction instead of
preparing again for every edge. Empty insertion sets avoid that work entirely.
No database schema or user-data migration is needed.

## Measurements

[Retained summaries and provenance](sixel-0.2.3-review.json) compare the final
candidate against the previously qualified encoder, whose implementation
matches `f6173fd` (frozen profiling executable SHA-256
`3149f7a1c4cd27c9d73e2f3c87f25c29e8b222f02eaf6601dc511b4e9ce9d03e`).
This is an additional comparison against the preceding development revision,
not a comparison against the earlier published v0.2.2 tag.
The retained timing executable predates the security-only Markdown parser
update and notebook changes. These measurements qualify the Sixel changes;
they must not be presented as timings of the final release executable.

Rust 1.99.1, release thin LTO and one codegen unit were used consistently.
Processes alternated serially with CPU affinity 2 on the same Intel Core Ultra
7 155H. No compilation or other test run overlapped timed measurements.

All 32 correctness scenarios preserved source/fitted hashes, frame counts,
wire length, wire SHA and independently decoded RGB/coverage. This included
128 cold/warm renders and 64 independently decoded outputs, native and inline,
photo zoom, dark/light transparency, true RGBA16 and partial band/cell sizes.
The timing comparison used three process pairs and nineteen warm renders per
process on ten selected scenarios (1,200 renders including cold).

Peak RSS is for the complete profiler process, including setup and reference
export. Values below are medians of three process peaks; time reductions are
medians of paired whole-preparation reductions. These are not terminal display
latencies or allocator-wide live-byte measurements.

| Input/view | Old peak KiB | New peak KiB | Whole-time reduction |
| --- | ---: | ---: | ---: |
| Photo, native | 19920 | 17608 | 2.4% |
| Photo, inline | 21440 | 19556 | 4.9% |
| Noise, native | 38228 | 35268 | 2.3% |
| Noise, inline | 44888 | 41780 | 1.5% |
| RGBA16 dark, native | 36192 | 36776 | -6.9% |
| RGBA16 dark, inline | 39044 | 37624 | -0.2% |
| RGBA16 light, native | 36880 | 36664 | -3.0% |
| RGBA16 light, inline | 38652 | 37416 | 1.2% |
| Photo zoom, native | 23876 | 23544 | -3.3% |
| Photo zoom, inline | 26708 | 25892 | -0.2% |

The RGB cases confirm lower process peaks. There is no uniform timing win:
the dark native RGBA16 case was about 6.9% slower despite retaining the previous
matte/conversion implementation. A unique cause was not established; compiler
layout and allocation history must not be presented as proven explanations.
This is an explicit limitation, not a universal performance pass. The release
keeps the simpler established conversion path rather than shipping the
unproven RGBA16 experiments. Existing qualified encoder gains and fidelity
remain documented in the [previous review](sixel-release-2026-11-02.md).

## Qualification

`make check-release` passed with the pinned Rust 1.99.1: license inventory,
formatting, warning-free Clippy, 1,086 native tests, checked representative
workloads, 77 tooling tests, 34 real loopback HTTP tests and the optimized
0.2.3 build. Current RustSec evidence reports zero vulnerabilities and six
explicitly scoped informational reviews. The nested-link workload checks the
persisted backlink in a 10,000-note notebook, within the 16 KiB comment limit.

The final executable also passed complete native Wayland qualification on
Foot 1.21.0. Independently decoded strips, theme mattes, clipping, selection,
zoom/pan, cached inspector reopening, linked images, graphical formulas and
terminal restoration all passed. The executable hash and compact qualification
evidence are retained in the JSON report; its Git build identity names the
pre-commit base revision, not the subsequently created tag commit.

The first Foot attempt timed out waiting for fullscreen rows. Its capture
showed an incomplete command and an unknown-command message, while inline
Sixel output was already present. A fresh attempt with the identical executable
passed the complete suite. The input intermittency was not reproduced or
attributed to a proven cause; this review does not claim to fix it. Both local
artifact directories were retained as `/tmp/lume-v023-foot-final` and
`/tmp/lume-v023-foot-recheck`.

An initial local build exhausted filesystem space in accumulated generated
artifacts and left invalid debug metadata. Only obsolete generated binaries
and the debug build cache were removed; source, user libraries and frozen
benchmark controls were retained. The final checks use the CI settings
`CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0`, with two build jobs and two
native test threads. Debug assertions remain enabled; only development debug
symbols/incremental state are disabled. Release flags, dependencies, source and
the pinned Rust compiler are unchanged by those environment settings.
