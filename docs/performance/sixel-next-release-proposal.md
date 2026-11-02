# Sixel proposal for the next release

Research date: 2026-11-02. Status: proposal for independent review; no new
performance result or production implementation is claimed here.

Independent review completed: [review and implementation contract](sixel-next-release-review.md).
It conditionally approves A0/A1 only and refines writer limits, proportional
corpus coverage and performance controls. Its decisions take precedence over
the proposed gates below; no new speed result is claimed by that approval.

The strongest first candidate is an exact encoder optimization. Keep the
current fitted pixels, Wu/Oklab palette, continuous Floyd–Steinberg diffusion,
resolution and palette size. Reduce work between the indexed image and the
existing `icy_sixel` writer. Promote that candidate only after comparison with
a frozen executable and independent decoding. Keep direct Oklab conversion as
a separate experiment until its memory and routing tradeoffs are established.

## What is measured

The evidence is [the previous review](sixel-review-2026-10-31.md) and its
[machine-readable results](sixel-profile-2026-10-31.json), measured at revision
`2505a079fdc8c91582663aec55aea2dc44d982d9`, Rust 1.99.1, release build, on an
Intel Core Ultra 7 155H. These are existing measurements, not a fresh run.

| Warm stage, ms | Photo, 720 × 720 | Noise, 1200 × 600 | Gradient, 1200 × 600 |
| --- | ---: | ---: | ---: |
| Resize and matte | 21.35 | 0.23 | 0.69 |
| Color preparation | 5.89 | 18.48 | 6.50 |
| Wu palette | 0.75 | 2.41 | 0.49 |
| Diffusion and mapping | 8.66 | 21.83 | 11.15 |
| Sixel encoding | 17.62 | 57.06 | 15.59 |
| Whole preparation | 55.28 | 102.28 | 35.40 |

Stage columns are independently summarized and need not sum to the total.
Initial PNG decoding cost 1.43 ms for the photo and 0.63 ms for the noise.
The existing real-application photo-inspector measurement was 62.47 ms median
delivery with a source already decoded for the reader. It excluded terminal
transfer and presentation. It was a one-article offline fixture, not a general
latency estimate for every library.

Direct Oklab conversion reduced noise preparation from 102.28 to 75.46 ms and
whole-process peak RSS from 36.22 to 25.31 MiB. It increased RSS for the photo
and gradient and did not consistently improve photo wall time. All measured
direct outputs decoded to the same pixels, but only 30 of 42 wire outputs
matched the deduplicated path. Floating-point accumulation order remains a
reason to test broader inputs, rather than assert universal equivalence.

Turning diffusion off improved speed and even the gradient's pixel PSNR, but
worsened visible banding and its local-average color error. RGB median-cut
lost fidelity on the measured photo and noise. Neither is a production
replacement on this evidence. Improving speed by reducing resolution,
diffusion or palette size is outside this proposal's first slice.

## Ownership and bottleneck hypotheses

The following statements describe inspected code, not measured speedups:

* `src/media/sixel/encoding.rs::write_cancellable` allocates `colors * width`
  bitplane bytes. Each six-row band fills selected positions, scans planes,
  copies each nonempty prefix into `sixel_node.map`, then clears every plane.
  `paint` emits colors in ascending order and trims trailing zero columns.
* At 256 colors and width 1200, a band has at most 7200 indexed pixels, but
  scanning and clearing visit 307200 plane cells each. Avoiding those visits
  may help rich images; sparse bookkeeping may lose on one-color diagrams.
* `src/media/sixel/palette/quantize.rs::{deduplicated,direct}` use the public
  Quantette 0.6.0 stages. Indexed diffusion adds an index-to-original-palette
  lookup. Contiguous diffusion avoids that lookup but can retain an Oklab
  triple for every pixel. Cache locality is a hypothesis, not a measured
  hardware-counter result.
* `src/media/raster.rs::{prepare,crop_raster,matte}` fits raster images and
  composites transparency before filtering. `src/media/worker.rs::render_with`
  holds the shared decoder mutex through decoding and fitting, then releases
  it before palette preparation and encoding. A long fit can delay the other
  worker, even if its eventual result is superseded.
* `src/media/sixel.rs::{native,encode}` already quantizes the fitted surface
  before splitting inline output. Keep diffusion continuous across strips.
  Prepared thumbnails and decoded sources already have bounded caches;
  another cache is not automatically less work or less memory.

The noise stream was 3476717 bytes, about 3.32 MiB. Encoder CPU and terminal
transport are distinct costs. An encoder that produces the same stream faster
does not also prove faster terminal parsing or presentation.

## First implementation: preserve pixels and avoid dense encoder work

Use the already qualified `icy_sixel = 0.1.3`; add no dependency. Its public
writer API can eliminate the owned-node copies without replacing framing,
palette formatting, repeat formatting or the terminal parser. Current upstream
has changed APIs, so use the pinned crate source, not examples from `main`.

### A0. Borrow planes and submit runs

Introduce one small adapter beside the encoder. Select a color through
`put_node` with an empty `Vec`, `sx = mx = 0`, and the desired palette index.
Then walk the existing borrowed plane prefix and submit equal-mask runs.
The empty node creates no heap allocation. Keep `$`, `-`, plane order and
leading zeros exactly as the existing encoder does.

Require a positive run length bounded by the plane width. Use the public
`put_pixel(mask)` once to establish a run. Its argument is a
six-bit mask, while `save_pixel` stores the character offset by `?`. Checked
addition of `run_length - 1` to the public `save_count` can extend that run;
`put_flash` remains responsible for repeat syntax and the GRI 255 limit.
Do not assign an unshifted mask to `save_pixel`, duplicate its formatting, or
carry pending state across a palette selection or band boundary. This use of
public mutable writer state needs a short documented contract and tests.
If that contract proves too brittle, retain one `put_pixel` call per column
while still eliminating `to_vec`, and report its measured benefit separately.

This first step isolates copies and per-column writer calls from planning.
It deliberately retains the dense map so the benchmark can attribute gains.

### A1. Build sparse bands without a map per color

Prototype a second planner behind the same run emitter:

1. Count pixels by color within the next six-row band. Prefix-sum the at most
   256 counts to obtain contiguous regions in one reusable event buffer.
2. Visit columns in increasing order, then the band's existing rows. Scatter
   an event `(column << 6) | row_bit` into the region belonging to that pixel's
   color. Each color's region is therefore already sorted by column; no sort
   or per-color allocation is required.
3. Visit nonempty colors in ascending order. Merge adjacent events with the
   same column using bitwise OR. Emit zero gaps and masks through the same
   run emitter. Omit trailing zero columns, as the current encoder does.
4. Reuse the buffer and bounded count/offset arrays for subsequent bands.

There are at most `6 * width` four-byte events, about `24 * width` bytes,
plus small bounded arrays, versus `colors * width` bytes for the dense map.
This bound is a workspace calculation, not a promise about total RSS.
Counting and scattering add work; the dense planner can remain a fallback
for low palette counts if representative benchmarks establish a crossover.
An algorithm choice based on palette count changes representation only and
must produce identical pixels. Do not add a sampled image-entropy policy here.

Keep all dimension, index, multiplication and shift conversions checked at
the owning boundary. A palette index must be in bounds; the last band may
contain only one through five rows. Preserve the 8 MiB encoded-output limit,
including pending library output and the footer, and cancellation before
bands and at bounded planning/emission intervals. Avoid creating a large
partially encoded result after a newer crop has superseded this one.

Implement A0 and A1 as independently benchmarkable steps. Select only a
variant that passes the gates below. If sparse planning regresses realistic
inputs, ship the exact borrowed-plane improvement if worthwhile and retain
the sparse path as a developer experiment or remove it. A failed performance
gate is a result, not a reason to lower the fidelity requirement.

## Quantization: a separate production decision

The existing direct path has a promising measured case, but an unconditional
switch is not justified. Quantette's own API recommends leaving deduplication
at its default or false with diffusion. That is a useful implementation clue,
not proof about Lume's corpus. The existing exact terminal-color bypass must
remain first, particularly for thin lines, labels and grayscale diagrams.

Evaluate three controlled options after the encoder experiment:

| Option | Benefit to investigate | Cost or risk |
| --- | --- | --- |
| Current deduplicated Wu and indexed diffusion | Strong low-cardinality memory behavior | Index indirection and dedup preparation |
| Existing direct Wu and contiguous diffusion | Proven noise improvement in the prior sample | More memory on other inputs; accumulation order can differ |
| Deduplicated Wu, then contiguous diffusion | Preserve the existing palette construction while testing diffusion locality | Expansion adds a full Oklab buffer and another pass |

The third option is feasible with existing public APIs:
`IndexedImageCounts::into_parts` can release counts, and
`IndexedImage::to_image` expands the original Oklab palette and indices.
Measure expansion, destruction and peak overlap, not only the final diffusion
loop. Drop no-longer-needed buffers before starting another stage. A 12-byte
Oklab triple per pixel is substantial with two workers and a bounded cache.

Exact unique-color count is available after deduplication, but cannot then
save the initial dedup cost. A preliminary full cardinality pass has a cost of
its own; an exact 24-bit RGB bitset alone occupies 2 MiB. A sparse or periodic
sample can misclassify tiled textures, narrow colored labels and gradients.
If routing is proposed later, record its complete cost and evaluate adversarial
spatial patterns, held-out natural images and memory bounds. Never treat sample
density as global cardinality or as evidence of equivalent output quality.

Do not parallelize Floyd–Steinberg by independently processing inline strips.
Quantette's optional parallel implementation initializes separate error
buffers for chunks; it is not evidence of equality to the serial full-image
diffusion state. Enabling thread features also changes dependency/resource
contracts. Prefer exact encoder work before taking that risk.

## Other candidates and their order

1. **Resize and matte:** first profile real downscales, crops and RGBA8/16
   images separately. Check whether an owned RGB conversion is avoidable
   before introducing a new resize engine. `fast_image_resize` exposes safe
   SIMD-enabled resizers and supports these component formats. It is worth
   an isolated experiment if the photo's fit remains dominant. Its upstream
   timings do not predict Lume speed. Preserve crop-before-enlargement and
   matte-before-filter behavior; the same filter name does not guarantee the
   same rounding, edge treatment or pixels. Default color-space conversion
   and premultiplication behavior need explicit review. A dependency change
   requires `docs/dependency-security.md`, exact versions/features, license
   inventory and fallback-platform qualification.
2. **Decoder lock:** separate immutable decoded-source ownership from fitting
   only with an explicit budget for cached and in-flight buffers, source
   lifetime and two-lane cancellation. An `Arc` alone does not bound memory:
   evicted sources can remain pinned by fitting jobs. This is a follow-up
   concurrency change with its own benchmarks, not part of A0/A1.
3. **Preparation reuse:** measure repeated exact-key cache hits and changing
   crop keys first. A fitted-surface cache must include source digest, crop,
   dimensions, matte/theme and algorithm revision. Count buffers shared with
   existing caches once and retained buffers once; do not duplicate the
   existing prepared-result cache or cache every zoom level.
4. **Transport:** measure output bytes and UI write/flush time separately.
   Terminal parser acknowledgment, if available, is not physical presentation
   latency. Keep inline streams self-contained: depending on a global
   terminal palette across reopen, scrolling and other images can corrupt
   output. Header/palette reduction is a separate pixel-equivalent protocol
   experiment, with terminal isolation and occlusion tests.

## Measurement and promotion contract

These are proposed acceptance targets, not achieved results.

**Baselines.** The coordinator preserved a release executable from clean HEAD
`1b683efe00693d928205f0bb1350d468f276f1df` before implementation:
`/tmp/lume-next-sixel-baseline/sixel_profile`, SHA-256
`982f95edf47309d94f820f8c57a5cadcd5a8dcaaa06d63bb78d8410dcdffb4a5`, with
`/tmp/lume-next-sixel-baseline/manifest.json`. These temporary artifacts must
be retained for the candidate comparison; they are not repository assets.
Preserve the full revision, executable SHA-256, exact Rust version,
Cargo.lock, build flags and corpus hashes. Compare that executable with the
candidate executable. An embedded `Quantizer::Reference` freezes only palette
preparation; it shares the candidate's encoder and resize code and cannot
serve as an independent baseline for those changes.

Extend `make bench-sixel`, `tools/bench_sixel.py`,
`examples/sixel_profile.rs` and `benches/workloads/sixel_images.rs` only as
needed to measure the new slices. If a new default quantizer policy is ever
introduced, add an explicit real-production/default strategy. Today `dedup`
forces `Quantizer::WuDedup` and could otherwise benchmark the wrong path.
Retain explicit controls alongside the actual default.

**Execution.** Use paired, alternating old/new process runs with the same
corpus and hardware, release compiler and flags, and no concurrent builds.
Use the previous three processes with nineteen warm renders as the initial
comparable sample; report spread and add repetitions only if noise prevents
a decision. Include cold and warm paths, user/sys CPU, whole-process peak
RSS, workspace high-water bytes, wire bytes and allocations where available.
Do not count independent stage medians as an additive whole-pipeline result.
Keep profiling disabled by default and avoid per-pixel timestamps.

**Corpus.** Retain all current fixture scenarios and add at least twenty
licensed, SHA-identified natural inputs covering photos, screenshots, scanned
text, plots and dense diagrams. Use both native and inline modes, dark/light
matte, source-cache cold/warm states, and actual resize/zoom/pan operations.
Include synthetic ramps at several slopes, low-contrast gradients, textured
noise, repeated tiles, one-pixel labels/edges, transparent hidden RGB and
partially transparent antialiasing. Include RGBA16 and boundary dimensions:
widths 1, 2, 3, 255, 256, 511 and the supported maximum; heights around every
six-row boundary; palettes 1, 2, 3, 16, 255 and 256. No natural-image result
should be inferred from noise alone.

**Correctness for A0/A1.** Require zero differences in independently decoded
dimensions, pixel coverage and displayed RGB for every encoder input, plus
byte equality when preserving the existing plane order and RLE decisions.
Test all masks, runs of 1/2/3/4/255/256 and longer repeats, leading gaps,
palette switches, partial bands and consecutive images with different
palettes. Verify public writer state after flush and GRI-limit behavior.
Use the independent decoder already exercised by `tools/bench_sixel.py` and
native encoder tests; do not validate a new encoder only with a matching new
decoder. Pixel equality remains mandatory if a later intentional encoding
change emits fewer bytes; then document and compare the new wire separately.

**Performance for A0/A1.** Aim for at least 20% lower encoder CPU/time on the
large complex cases and at least 8% lower whole warm preparation on two
complex cases, with a gain demonstrated on natural content as well as noise.
Require no reproducible whole-pipeline regression above 5% on representative
diagram/gradient/photo cases, no output-size increase for an exact encoding
change, and no increase in bounded working memory. Treat marginal differences
within measured variation as inconclusive. Report warm-cache UI delivery as
a separate production observation, rather than equating it with presentation.

**Additional gates for quantizer or resize changes.** Prefer exact displayed
pixels. If deliberately allowing differences, evaluate source-relative mean
and p95 Oklab error, local-average gradient error, edge/text crops and visual
comparison at actual presentation scale. Predeclare tolerances and compare
each fixture and image class, not just aggregate PSNR. Require no newly
visible banding, label/line loss, alpha halos or matte changes. A small metric
gain cannot override a visible regression. Count classifier/expansion time
and retained/in-flight allocations. Preserve old behavior as the fallback
until these gates and the memory policy are independently reviewed.

**Release behavior.** Exercise cancellation during band planning/emission,
output caps, superseded pan/zoom, inspector close/reopen after scrolling,
overlays, inline clipping and theme changes. Run native behavioral checks,
the representative existing benchmarks, root `make check`, required real
HTTP qualification and complete Foot qualification under Rust 1.99.1.
Report environment failures; do not silently skip a release gate. Retain
the UI's existing ECH/occlusion behavior to avoid stale text below images.

## Research sources and relevance

Technical decisions above rely on primary documentation and inspected pinned
source. Community material supplied leads, not benchmark evidence for Lume.

* [Pinned icy_sixel writer source](https://github.com/mkrueger/icy_sixel/blob/80038c275a9a2075a31992c61f8b6e1275152b31/src/tosixel.rs)
  and [output types](https://github.com/mkrueger/icy_sixel/blob/80038c275a9a2075a31992c61f8b6e1275152b31/src/output.rs):
  matches the local 0.1.3 crate's VCS metadata. These public APIs and writer
  state justify the limited run adapter, not a custom production protocol.
* [Quantette pinned pipeline](https://github.com/IanManske/quantette/blob/066855ad3efb4143c3b901c812f80b9b17f8f303/quantette/src/api/pipeline.rs),
  [diffusion](https://github.com/IanManske/quantette/blob/066855ad3efb4143c3b901c812f80b9b17f8f303/quantette/src/dither.rs)
  and [indexed-image types](https://github.com/IanManske/quantette/blob/066855ad3efb4143c3b901c812f80b9b17f8f303/quantette/src/types/indexed_image.rs):
  matches local 0.6.0 metadata; supports the separate layout experiments and
  explains the parallel error-buffer caveat. No new quantizer is needed.
* [Xterm control-sequence reference](https://invisible-island.net/xterm/ctlseqs/ctlseqs.html):
  primary terminal behavior and palette/mode reference. Preserve standalone
  palette definitions and state isolation rather than relying on another
  image's terminal state.
* [Notcurses Sixel implementation](https://notcurses.com/html/sixel_8c_source.html):
  an established design separates quantization from per-band color sections
  and keeps bounded work units for wipe/restore. It is a pattern to examine,
  not a reason to add a C library or assume its timings apply here.
* [Wu, Graphics Gems II, author institution record](https://experts.mcmaster.ca/scholarly-works/3899043):
  variance-based statistical palette construction explains why a palette
  algorithm and its input representation are separate decisions. The
  historical runtime is not comparable to current hardware.
* [Oklab, original author](https://bottosson.github.io/posts/oklab/):
  perceptual color-space rationale; retain perceptual and local-spatial
  comparisons rather than choosing a path from RGB PSNR alone.
* [fast_image_resize upstream](https://github.com/Cykooz/fast_image_resize):
  candidate safe public API with SIMD paths and explicit component formats.
  It documents color-space and alpha choices that must be matched explicitly.
  Upstream speed charts are not Lume measurements.
* [Rust community discussion of fast_image_resize](https://www.reddit.com/r/rust/comments/qt6ou0/)
  and [Notcurses maintainer notes](https://nick-black.com/dankwiki/index.php/Notcurses):
  discovery leads highlighting end-to-end pipeline and terminal variability.
  No speed or fidelity claim in this proposal relies on those discussions.

## Questions for the independent reviewer

1. Is the small public-state run adapter maintainable enough, or should the
   first slice use borrowed per-column writes until an upstream borrowed-node
   API exists? Check shifted `save_pixel`, flush resets and GRI semantics.
2. Does the sparse scatter preserve all leading zeros, same-column OR masks,
   color order and partial-band semantics without a hidden allocation or
   unchecked boundary? Is its low-color fallback empirically justified?
3. Are the proposed natural corpus and regression gates sufficient to avoid
   optimizing noise at the expense of diagrams, alpha edges or gradients?
4. Does the measurement harness truly compare frozen and candidate binaries,
   and invoke the production default rather than a forced experiment?
5. Should direct conversion remain entirely experimental this release, or is
   a bounded, separately measured contiguous-diffusion experiment warranted
   after the encoder? Identify the missing evidence before recommending it.
