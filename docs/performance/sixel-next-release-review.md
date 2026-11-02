# Independent review of the next Sixel release proposal

Review date: 2026-11-02. Decision: **conditionally approved for the exact
encoder work A0/A1 only**. This is an implementation contract, not evidence
that a prototype has already improved performance. No production code or
benchmark executable was changed during this review.

Read this document alongside [the proposal](sixel-next-release-proposal.md)
and [the previous measurements](sixel-review-2026-10-31.md). Where their
requirements differ, the decisions below define the reviewed scope.

## Decision and scope

Implement and measure two separate steps:

1. A0: retain dense band planning, borrow its color planes, and submit equal
   mask runs through a narrow adapter to the pinned icy_sixel writer.
2. A1: replace unnecessary dense planning with stable counting scatter for
   suitable bands/images. Retain the measured dense path where appropriate.

Keep the current fitted pixels, palette order, Wu/Oklab quantization, 256
color policy, serial error diffusion, alpha matte, crop and resize behavior.
Keep native and inline framing, self-contained palettes and UI occlusion.
No new dependency, user configuration or generic protocol abstraction is
needed. A0 and A1 must have independently attributable measurements.

Do not promote Direct Oklab, a cardinality classifier, contiguous expanded
Oklab diffusion, a SIMD resizer, new image caches or decoder-lock changes in
this slice. These are separate experiments with separate correctness and
memory risks. The noise result does not establish a production policy.

## Corrections and qualifications to the proposal

| Topic | Reviewed requirement |
| --- | --- |
| Public writer state | The adapter is acceptable only for pinned 0.1.3, with differential tests and explicit state invariants. Public fields alone do not establish a stable cross-version contract. |
| Repeat limits | `sixel_output::new` enables the 255-repeat argument limit. `set_gri_arg_limit` is public. Preserve the production default and test both modes. |
| Output accounting | Count the written Vec and pending String before the footer. After the footer, count the Vec alone because upstream retains the copied String. |
| Failure handling | Upstream ignores `Write::write` results. A sink that returns errors cannot enforce the limit reliably. Keep the narrow Vec sink and explicit checks. |
| Sparse complexity | Band planning is O(6W + K); total encoding also includes emitted runs and protocol bytes. Do not describe the whole operation as independent of output size. |
| Memory | Sparse events cost up to 24W bytes, plus fixed arrays and output buffers. Sparse is not automatically smaller for tiny palettes. |
| Corpus size | Replace the arbitrary twenty-new-natural-image minimum for this exact encoder with the proportional corpus and stronger index-level tests below. A future pixel-changing quantizer/resize promotion still requires a broader visual corpus. |
| Terminal latency | Exact wire equivalence preserves the terminal's payload and parsing workload. An encoder CPU gain is not a measured screen-presentation gain. |

## Pinned writer contract

I inspected the local Cargo source of icy_sixel 0.1.3, including `output.rs`,
`tosixel.rs` and its VCS metadata. The corresponding upstream revision is
`80038c275a9a2075a31992c61f8b6e1275152b31`.

`put_pixel` accepts a six-bit mask, converts it to the ASCII Sixel character,
and updates `save_pixel`/`save_count`. `put_flash` writes the run, splits it
at 255 when required, and resets both fields. Consequently, to append one
positive run:

1. Validate mask 0..63 and a positive run length bounded by image width.
2. Call `put_pixel(mask)` once. This handles a preceding different mask.
3. Checked-add `length - 1` to `save_count` only. Never assign `save_pixel`
   from the unshifted mask, and never format repeat commands independently.
4. Flush at the same color-plane boundaries as the old implementation.

An empty `sixel_node` with `sx = mx = x = 0` selects a palette without
reading its empty map. However, `put_node` changes the palette *before*
flushing a pending run. The adapter must enter palette selection with
`save_count == 0`; otherwise pixels can be emitted under the wrong color.
Use the existing `keycolor = -1` behavior, including two-color images.
Preserve ascending colors, leading zero masks, omitted trailing zeros,
`$` between painted color planes and `-` between bands. Flush the final
plane before any separator or footer.

The adapter should remain private to the Sixel encoder, concrete over its
Vec-backed writer, and documented with these invariants and the exact
dependency version. No domain state belongs in it. Do not expose the mutable
writer to planners unnecessarily. Keep planning and run emission cohesive;
do not create a general quantizer or protocol framework. A dependency update
must re-review these source assumptions and rerun the exact-stream tests.

The borrowed per-column API remains the fallback if the public-state run
adapter cannot meet these contracts. Do not recreate DCS, palette or repeat
encoding to work around a failed adapter.

## Sparse band planner contract

Let W be the width, K the palette size and R the current band height, 1..6.
Validate inputs before constructing chunks or indexing: positive dimensions,
checked W*H and 6W, exact pixel length, palette RGB triplets, 1..256 colors,
and every index below K. Respect existing application geometry limits; do
not rely on unchecked casts merely because current callers are valid.

Count occurrences by color, compute exclusive offsets and initialize each
color's cursor. Allocate/reuse at most R*W `u32` events, with checked
capacity and checked `(column << 6) | row_bit` representation. Counts,
offsets and cursors need bounded K-sized storage and checked indexing.

The scatter pass must visit **columns in ascending order outside the row
loop**. Stable placement into each color's region then preserves column
order. A row-major scatter does not provide that property. The count pass
can use row-major traversal.

For each nonempty color, in ascending color order, combine all events with
the same column by OR-ing their row bits. Emit the zero gap from the prior
column and then the combined mask; merge adjacent equal runs before passing
them to the writer. Do not emit trailing zero columns. A partial final band
must never set a bit for an absent row.

This preserves the old plane exactly: each source index contributes its one
row bit to the same color and column; OR is independent of event order;
stable columns reproduce the same leading gaps and mask sequence. Preserved
palette order and writer run semantics then reproduce the same wire bytes.
The tests must check this argument, not merely compare attractive pictures.

Reuse event storage across bands, without one Vec per color or per run.
Include array capacities, any simultaneously live dense map, node data and
writer String in the high-water accounting. A1 must not allocate both full
scratch representations gratuitously. Dense K*W bytes can beat 24W events
for small K; small widths also expose fixed-array overhead. Select the
fallback from measured crossover cases and actual workspace bounds, not
from an entropy sample or an assumed universal palette threshold.

## Cancellation and output limits

Check cancellation before starting work, between bands, and at bounded
intervals within planning and emission. Do not check time or atomics per
pixel. A canceled request must not publish a footer or completed image;
partially produced bytes must be discarded by the owning caller. Test
mid-planning and mid-emission cancellation deterministically through a
test-only checkpoint seam rather than sleeps or scheduler races.

Upstream `advance` is private; it flushes 16,384-byte packets. Public
`putc`/`puts` alone do not call it. Keep run/palette emission on the reviewed
methods and do not manually append large protocol strings expecting an
automatic flush.

Before footer emission, account for `fn_write.len() + buffer.len()` with
checked addition and reserve the unchanged two-byte 7-bit footer. An
unflushed run is not yet represented in that sum: either account for its
bounded encoded size, or check immediately after natural run flushes with a
documented bounded overshoot that is discarded on error. Do not insert
extra flushes solely for checking: that can change exact repeat bytes.
Check at bounded work units, then after the footer verify the final Vec
length. `encode_footer` copies pending bytes but leaves the String present;
adding both lengths afterward double-counts them. Preserve the 8 MiB success
limit and test small injected caps around packet/footer boundaries.

## Executable validation contract

The independent baseline is the frozen release executable:

```
/tmp/lume-next-sixel-baseline/sixel_profile
SHA256 982f95edf47309d94f820f8c57a5cadcd5a8dcaaa06d63bb78d8410dcdffb4a5
revision 1b683efe00693d928205f0bb1350d468f276f1df
```

Its manifest is `/tmp/lume-next-sixel-baseline/manifest.json`. The coordinator
qualified the existing fourteen scenarios, producing twenty-eight outputs,
at `/tmp/lume-next-sixel-baseline/qualification/report.json`. That one-run
qualification establishes a control, not a statistical speed result.

Preserve baseline executable/manifest and source hashes. Build the candidate
with the exact Rust 1.99.1 toolchain and matching release flags. Compare two
executables on identical explicit input files. Candidate `Reference` and
`WuDedup` strategies share its encoder and resize helper; they cannot supply
the old encoder baseline. A new expected image generated by the candidate
resize helper is also not an independent resize control.

For A0/A1, require identical fitted dimensions and source/fitted hashes,
**identical wire bytes**, and identical independently decoded dimensions,
coverage and displayed RGB. Keep the existing independent Python/native
decoders; do not replace both encoder and oracle together. Include all
native and inline strips, palette definitions and frame termination.
Wire compression that deliberately changes bytes is a separate proposal.

The benchmark must explicitly identify the real production strategy. The
current forced `dedup` strategy is valid only while production uses the same
path. Do not introduce an adaptive default and accidentally keep measuring
the previous forced path.

### Proportional corpus

Retain every existing fixture and scenario. Add three independent natural
inputs: coffee, Chelsea and camera from a pinned scikit-image release.
Its official documentation identifies the photographs as CC0 by Rachel
Michetti, Stefan van der Walt and Lav Varshney, respectively. Together with
the existing NASA astronaut, this supplies four independent photographs.
Add a repository-authored dense labeled diagram and a text/screenshot-like
fixture, generated deterministically. These are additional structural
classes, not independent natural photographs.

The [v0.25.2 registry](https://github.com/scikit-image/scikit-image/blob/v0.25.2/skimage/data/_registry.py)
provides these original-file SHA-256 values. Candidate downloads can use
`https://raw.githubusercontent.com/scikit-image/scikit-image/v0.25.2/skimage/data/`
plus the filename, and must verify the expected hash:

```
camera.png  b0793d2adda0fa6ae899c03989482bff9a42d3d5690fc7e3648f2795d730c23a
chelsea.png 596aa1e7cb875eb79f437e310381d26b338a81c2da23439704a73c4651e8c4bb
coffee.png  cc02f8ca188b167c775a7101b5d767d1e71792cf762c33d6fa15a4599b5a8de7
```

Record immutable download URLs/revision, license attribution, original file
SHA-256 and every transformation. Crops or resized variants count as
scenarios of the same input. Keep small licensed files in the fixture area
when practical; a pinned external corpus cache plus manifest is acceptable
for larger follow-up work. Required qualification must fail clearly when an
input is absent or has the wrong hash; do not download at application runtime
or silently narrow the corpus. No new image/Python dependency is necessary
merely to consume PNG fixtures.

Add index-level deterministic differential/property cases against the dense
reference: all 64 masks; run lengths 1/2/3/4/254/255/256/257/510/511 and long
runs; palettes 1/2/3/16/255/256; empty color planes; leading/trailing gaps;
same-column multi-row unions; alternating colors and randomized indices.
Exercise widths 1/2/3/255/256/511 and supported maximum, and all six possible
final-band heights. Use bounded combinations rather than an expensive full
Cartesian product. Include invalid dimensions, indices and palette lengths,
overflow rejection, cap failures, state after flush, both GRI modes, and
consecutive frames with different palettes.

Existing gradients/noise/transparent images remain essential. Cover hidden
RGB under alpha, partial-alpha edges, light/dark matte, RGBA16, cold/warm
source cache, native/inline, resize and zoom. Use viewport widths available
in both executables for paired comparison; use raw encoder tests for maxima
not expressible by the frozen CLI, and Foot for actual pan/reopen behavior.

Exact index-level and wire properties justify this corpus size for an
integer encoder change. Before changing quantization or resize, return to
the proposal's broader visual corpus, including at least twenty independent
licensed natural/documentary inputs and source-relative visual evaluation.
Do not silently carry this narrower exception into a pixel-changing release.

### Performance and release gates

Use paired alternating old/new release runs without concurrent builds or
tests. Initially use three independent processes and nineteen warm renders
per case, matching the historical method. Report spread and process-level
paired ratios; add repetitions only where variation prevents a decision.
Keep correctness runs separate from timing and bound test parallelism.

The current host is a hybrid Intel Core Ultra 7 155H. Record CPU model,
compiler flags and eligible/effective CPU affinity. Optionally pin both
executables to the same permitted CPU using `taskset` or
`os.sched_setaffinity`; apply that choice consistently to the paired runs.
Migration between CPU types is a possible source of variation, not a proven
explanation for previous measurements. Affinity is not physical isolation.
Do not use historical numbers from another day as the candidate's control.

Report encoder and whole-pipeline time, cold/warm behavior, user/sys CPU,
process peak RSS, scratch high-water bytes and wire bytes. Measure A0 alone
before A1 and report default dispatch as shipped. Do not add independent
stage medians to claim total time. A profiler's wall-clock stage time is not
automatically CPU time.

Promotion targets remain at least 20% lower encoder time and 8% lower whole
warm preparation on two complex cases, including natural content. Require
no reproducible whole-pipeline regression above 5% for representative
photos, diagrams and gradients, no wire growth and bounded memory without
a representative peak regression. Report tiny-case absolute microseconds
alongside percentages so measurement noise is not mistaken for regression.
An inconclusive result is not a passed gate.

Use a production App observation to measure delivery separately from the
microbenchmark, comparing the preserved reader benchmark with the real
candidate default through App/worker delivery. Unchanged bytes imply
unchanged terminal transfer/parser work; any claim about actual visible
latency needs a separately described
terminal measurement. Foot qualification must cover open/close/reopen after
scrolling, superseded zoom/pan, overlays, inline clipping, theme changes and
no stray text beneath images. Run root `make check`, required real HTTP
qualification and complete Foot qualification under the pinned toolchain.
Report environment failures explicitly; do not silently skip release gates.

## Implementation order and retreat policy

First add independent comparison/report support and deterministic encoder
tests. Implement A0, validate exactness, and measure. Then implement A1,
validate exactness, and measure dense/sparse crossover without changing the
quantizer. Finish by reviewing module ownership, scratch lifetimes and
public-state assumptions, then run the release checks.

If A1 regresses small palettes, retain a measured dense fallback. If it does
not improve enough to justify its complexity, retain A0 alone if A0 passes
the substantive gates. If the run adapter fails equivalence or maintainable
state ownership, retreat to borrowed per-column writes. If no candidate
passes, keep the current production path and publish the measurements;
do not lower resolution, drop dithering or alter gates to manufacture a win.

## Separate research decisions

Direct Oklab removes dedup overhead on noise but can raise memory elsewhere.
A sampled entropy/cardinality estimate can misclassify structured content;
an exact estimate has its own pass and storage cost. Neither currently
supports universal production promotion.

Pinned Quantette 0.6.0 does expose `IndexedImageCounts::into_parts` and
`IndexedImage::to_image`, so a dedup-Wu/expanded-contiguous-diffusion
experiment is technically feasible. It introduces approximately 12 bytes
per pixel for expanded three-component floats, plus overlapping live buffers
and expansion work. Keep it experimental until independently measured for
natural content, RSS, peak retained allocations and exact/visual fidelity.
Parallel diffusion is a different algorithmic boundary and is not included.

Resize remains material for the photograph. A SIMD resizer may help, but
upstream fast_image_resize explicitly leaves color-space conversion to the
caller. Filter identity, alpha multiplication, matte ordering, crop ordering,
rounding and 16-bit paths require their own review. Likewise, moving fit work
outside a decoder mutex can retain evicted source buffers via Arc ownership;
it needs a total cache-plus-in-flight memory budget before promotion.

## Primary sources

- [Pinned icy_sixel writer](https://github.com/mkrueger/icy_sixel/blob/80038c275a9a2075a31992c61f8b6e1275152b31/src/tosixel.rs)
  and [output state](https://github.com/mkrueger/icy_sixel/blob/80038c275a9a2075a31992c61f8b6e1275152b31/src/output.rs).
  Reviewed from the exact local Cargo source, not inferred from newer APIs.
- [Xterm control sequences](https://invisible-island.net/xterm/ctlseqs/ctlseqs.html):
  six-pixel graphic units, DCS framing and terminal palette/mode context.
- [Pinned Quantette indexed-image API](https://github.com/IanManske/quantette/blob/066855ad3efb4143c3b901c812f80b9b17f8f303/quantette/src/types/indexed_image.rs):
  ownership and expansion APIs, checked against local 0.6.0.
- [scikit-image image documentation](https://scikit-image.org/docs/stable/api/skimage.data.html):
  CC0 authorship for coffee, Chelsea and camera; pin the release files and
  their hashes when importing them. Existing astronaut attribution remains
  in `tests/fixtures/images/README.md`.
- [fast_image_resize upstream](https://github.com/Cykooz/fast_image_resize):
  explicit color-space and alpha-processing requirements, not evidence of
  a Lume speed improvement.

Community discussions in the proposal are useful leads. None is used here
as proof of a performance, fidelity or terminal-compatibility claim.

## Addendum: implementation experiments, 2026-11-02

The instruction above to merge adjacent masks before passing them to the
writer was overly prescriptive. A1 may submit occupied columns individually
through `Output::run(mask, 1)`: icy_sixel coalesces adjacent identical masks
in its existing `save_count` state. Zero gaps remain aggregated. This keeps
one owner of protocol run state and preserves the required exact bytes,
palette boundaries and GRI behavior. A0 still aggregates its borrowed dense
planes before submission; do not claim that A1 reduces calls per occupied
column.

A focused pending-run accumulator prototype was measured against the
existing A1, using three paired processes, nineteen warm renders per process
and CPU 2 affinity. In `/tmp/lume-sixel-coalescing-focused/report.json`, the
median paired encoder-time changes were **+6.93% for gradient, +5.29% for
photo, and +1.62% for entropy**: positive values here mean increased cost.
Antialias was effectively unchanged/slightly worse (+0.66%). The compared
outputs had no wire or independently decoded fidelity differences. The
prototype was rejected; explicit pre-writer aggregation is not a promotion
requirement. These measurements compare two candidate implementations, not
the original release baseline.

The initial full thirty-scenario comparison also found a real antialias
native regression at K=34, approximately +17.8% whole preparation time.
This outweighed the marginal K=32 crossover-probe result. The revised
production candidate therefore requires K>=48 as well as the complete
sparse-workspace <= dense-workspace guard. That threshold remains a measured
candidate policy, not a universal property of sparse encoding; the final
full matrix must validate it.

The peak-RSS gate remains open. The developer fixture factory no longer
embeds the three additional photographs in the benchmark executable; they
remain licensed external inputs. RSS must be compared again against the
original frozen executable. Do not subtract added `.rodata` from earlier RSS
arithmetically or declare the memory gate passed from scratch bounds alone.
Final performance, memory and release qualification gates are still pending
at the time of this addendum.

## Final review status, 2026-11-02

**Qualified promotion approved.** The [final release evaluation](sixel-release-2026-11-02.md)
records the completed CPU/fidelity comparison, matched App controls, local
checks, real HTTP qualification and full Foot qualification. Earlier
statements that measurements or release checks were pending describe the
interim review state and are now historical.

The strict global RSS gate was **not universally met**. The coordinator
explicitly accepts the documented bounded standalone residency tradeoff;
its anonymous-memory cause remains unresolved. Matched App measurements did
not reproduce the larger peak, but do not prove memory equivalence. This
approval does not claim an unconditional memory improvement or relax the
exact-byte and fidelity requirements.
