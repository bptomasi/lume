# Exact Sixel encoder implementation

This slice implements the reviewed A0/A1 contract in
[sixel-next-release-review.md](sixel-next-release-review.md). It changes
integer band planning and writer calls, while preserving palette construction,
color order, resize/matte, diffusion, inline clipping and terminal framing.
The executable comparison report owns performance conclusions; complexity
alone is not evidence of a speed improvement.

## Ownership and writer contract

`src/media/sixel/encoding.rs` validates the indexed raster before writing a
header. The normal image geometry limits also bound arithmetic inside both
planners: W <= 10240, H <= 6144, W*H <= 4000000 and K <= 256. Thus band counts,
K*W, six-row storage, and `(column << 6) | row_bit` fit supported integer
representations. Every source index is checked against K in cancelable chunks.

`encoding/output.rs` is a private concrete adapter for pinned icy_sixel 0.1.3
(upstream revision `80038c275a9a2075a31992c61f8b6e1275152b31`). It selects a
palette through an empty `sixel_node`, only with no pending run. A positive
bounded run calls `put_pixel(mask)` once and checked-adds length-1 to
`save_count`. It never assigns `save_pixel`; upstream owns its ASCII offset,
repeat formatting, default 255-repeat limit, palette definitions and framing.
The footer also requires a flushed run. Dependency updates must re-review
this public-state contract and rerun the differential and writer-state tests.

A0 (`dense.rs`) retains K*W planning, borrows each plane prefix and aggregates
equal-mask columns. There is no owned map copy per color. A1 (`sparse.rs`)
counts colors and scatters events into contiguous color regions, visiting
columns before rows. Stable ascending columns permit same-column OR without
sorting. Color iteration remains ascending; leading zero gaps are retained,
trailing zeros omitted. Sparse emission passes each occupied column's mask;
icy_sixel coalesces equal masks. A separate pending-run experiment did not
improve the measured cases and was rejected; repeat syntax, coalescing and
255 splitting remain exclusively in the writer.
One reusable event Vec and two fixed bounded arrays serve all bands; neither
planner allocates the other's storage.

## Workspace, limits and cancellation

Dense planner storage is K*W bytes plus its Vec descriptor. Sparse planner
storage is 4*W*min(H,6) bytes plus two arrays of 257 and 256 usize values and
its Vec descriptor. The latter has fixed overhead, so width matters alongside
palette size when selecting a representation. Profiling reports the actual
planner allocation capacity plus writer String capacity. The String never
shrinks, making its capacity at footer the high-water mark. Output Vec bytes
are reported separately as wire bytes and also participate in process RSS;
these are not a complete allocator-wide peak measurement.

Before the footer, the cap includes written Vec length, pending String length
and the unchanged two-byte footer. After the footer it counts the Vec alone:
upstream retains the copied String. Upstream ignores Write errors, so a sink
returning an error cannot enforce this cap. Checks occur at natural color
flush boundaries, without breaking runs. An error can retain at most one
plane's W literal pixels plus its selector/separator beyond the cap; the
caller discards incomplete output. No successful stream exceeds 8 MiB.

Cancellation is checked before validation, in bounded validation chunks,
between bands, and during both planning and emission. Dense planning checks
at most 1024 indexed columns at a time; sparse scatter checks every 1024
columns (at most six source rows each). Emission checks at color boundaries
and bounded progress within planes. The longest uninterrupted plane scan is
bounded by W. A final check precedes the footer. Tests inject deterministic
mid-planning and mid-emission cancellation without sleeps or thread races.

## Independent controls and correctness

`--encoder dense|sparse|production` in the developer example controls the
encoder independently from `--strategy dedup`. Normal application settings
expose none of these controls. A Sample records the selected control, actual
dense/sparse frame counts and scratch high-water bytes. Inline results may
contain many independently framed strips, sharing the same prepared palette
and continuous diffusion state before splitting.

A literal copy of the previous dense writer lives only in
`encoding/tests/legacy.rs`. Native tests compare every wire byte against it,
then independently reconstruct index coverage and values. They include all
64 masks, run boundaries around 255, both GRI modes, palette/width boundaries,
partial bands, random indices, empty planes, consecutive palettes, malformed
inputs, cap boundaries, canceled work and reusable event storage.

The frozen external executable is the independent whole-pipeline baseline;
forcing Reference quantization inside the candidate is not a baseline for
its encoder. `--export-source-only` writes explicit fixture bytes so both
executables can consume identical inputs. The paired release tool additionally
checks fitted hashes, wire identity and independent ImageMagick decoding.
It separates cold/warm renders, CPU wall stages, process CPU/RSS and output
bytes. Production App/worker delivery and actual Foot behavior remain separate
release qualifications rather than inferred terminal-presentation results.

## Initial crossover probe

The conservative production policy requires K >= 48 and a complete sparse
planner bound no larger than dense storage. Small palettes remain dense;
fixed arrays also keep very narrow rich images dense. An explicit non-inlined
sparse boundary keeps its fixed arrays off the dense caller's stack under LTO.

The initial development probe used CPU affinity 2 on this host, 66 palette/
width combinations, three alternating independent dense/sparse processes and
nineteen warm renders per process. It compared 7524 warm renders, with exact
wire equality. Its provisional control executable is
`/tmp/lume-next-sixel-controls/sixel_profile`, SHA-256
`185e25c69b2176df5e0e0dbcd5f629b1033a56924f982ebaeb57bedbad6daffd`;
raw artifacts are `/tmp/lume-sixel-crossover/summary.json`. This probe preceded
final dispatch telemetry, explicit adjacent-mask grouping and the non-inlining
guard; it selects the policy,
not the release performance conclusion.

Representative encoder process-median results, in microseconds:

| Palette K | W | Dense A0 | Sparse A1 | A1 / A0 |
| --- | --- | --- | --- | --- |
| 1 | 1200 | 98.1 | 581.7 | 5.93 |
| 8 | 1200 | 1245.0 | 1821.4 | 1.46 |
| 16 | 1200 | 2641.3 | 2668.6 | 1.01 |
| 32 | 1200 | 3961.0 | 3739.5 | 0.94 |
| 64 | 128 | 582.2 | 517.6 | 0.89 |
| 128 | 1200 | 6017.8 | 4720.0 | 0.78 |
| 256 | 32 | 358.9 | 289.7 | 0.81 |
| 256 | 1200 | 7427.9 | 5059.2 | 0.68 |

The K=256,W=32 case passes the workspace comparison; K=128,W=32 does not,
even though its initial timing favored sparse. Tiny W=1/8 cases differed by
fractions to a few microseconds and are not evidence for a broad speed claim.
These are random indexed-pattern PNGs with exact terminal-distinguishable
colors, not independent photographs. Their whole pipeline was not used to
claim a release gain.

Reproduce with an optimized explicit executable, without a concurrent build:

```
/tmp/lume-next-sixel-visual/bin/python tools/probe_sixel_crossover.py \
  --binary target/release/examples/sixel_profile --cpu 2 \
  --output /tmp/lume-sixel-crossover-final
```

The final paired baseline/candidate corpus must validate both isolated A0 and
the shipped default, including natural inputs and production App observations.

## Structured fallback and rejected run accumulator

The first full external comparison exposed a real counterexample to the
initial K >= 32 floor: the antialias fixture has 34 colors at width 1200.
Sparse preparation took about 10.6 ms warm versus about 9.0 ms in the original
executable; borrowed dense took about 8.7 ms. Random K/W probes alone had not
established a universal crossover for structured content.

A minimal explicit pending `(mask, length)` experiment preserved exact wire
bytes but did not fix that case. Against the frozen first candidate, seven
focused native scenarios (including photo zoom 2) used three alternating
process pairs and nineteen warm renders each. Antialias encoder medians stayed
5.923 to 5.961 ms. Gradient and photo encoder time rose about 6.9% and 5.3%;
whole warm preparation rose roughly 1–3% across the tested cases. There was no
measured benefit to justify the additional state, so it was removed.

The rejected executable is
`/tmp/lume-next-sixel-coalesced/sixel_profile`, SHA-256
`bd1709c88402657649b2ee572ea51eb03e6c8c391540413f0e102324988abf82`.
Its manifest, exact matching `sparse.rs` and patch against the final planner
are preserved in that directory. The comparison report is
`/tmp/lume-sixel-coalescing-focused/report.json`. These temporary artifacts
are evidence of the experiment, not required application files.

The resulting production policy raises the palette floor to 48, retaining
the complete workspace comparison. The known 34-color counterexample uses
borrowed dense; the 53/55-color structural fixtures and richer natural inputs
remain eligible for sparse storage. This changes representation only; it
adds no content classifier, source pass, user option or pixel difference.
The final external matrix must measure this actual production default.

## Matched App memory controls

The developer reader benchmark accepts `LUME_BENCH_SIXEL_INPUT` only in
`benches/workloads/image_profiling.rs`. Without it the existing NASA input
remains borrowed static bytes. An external input is read once into owned
bytes, with an explicit read error, source label, byte count and SHA-256 in
each profile record. It does not add an application option or runtime file
access selected by article content.

After the inspector-delivery timer stops, the benchmark borrows the ready
native Sixel String to calculate its SHA-256 and byte count. It makes no full
payload clone or concatenation. Default NASA still requires Decode, Fit,
Palette, Dither, Encode and Work diagnostics. External fixtures require the
base Decode/Fit/Encode/Work stages; Palette/Dither may be absent for an exact
palette and remain visible whenever recorded.

To compare production cache/worker memory fairly, an isolated archive of
revision `1b683efe00693d928205f0bb1350d468f276f1df` was verified against all
1168 original Git blobs. Only the identical benchmark-fixture patch was then
applied to both old and candidate sources. The old production implementation
was not changed. Bench controls use the exact Rust 1.99.1 toolchain and the
same release profile/build flags; the archive's missing Git metadata is
replaced explicitly with the original full `LUME_BUILD_REVISION` value in
both controls. The actual user binary is built separately with normal Git
metadata and no revision override. Source/patch hashes and all overrides are
recorded with the preserved controls.

Matched invocations use the same explicit source for both executables:

```
LUME_BENCH_SIXEL_INPUT=/absolute/input.png \
  /absolute/reader --sixel-production-only
```

Source and native-wire hashes must agree before comparing delivery, process
RSS or CPU. Process RSS includes the real App, decoded sources, prepared
inline/native caches, worker stacks and allocator retention. It is not the
encoder scratch bound, and a microbenchmark's higher RSS cannot be dismissed
merely because this bound decreased. The release report owns the matched
memory observations and their acceptance decision.
