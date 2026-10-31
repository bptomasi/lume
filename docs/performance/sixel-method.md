# Sixel measurement method

Use the exact Rust toolchain in `rust-toolchain.toml`, ImageMagick's Sixel
decoder and the pinned visual Python environment:

```sh
CARGO_BUILD_JOBS=2 make bench-sixel PYTHON=/tmp/lume-visual/bin/python
```

The command builds `examples/sixel_profile.rs` in release mode. The comparison
uses serial child processes and rotates algorithm order across three runs.
Each process measures a cold decode and a second render reusing its source
cache. Native and cell-height inline encodings are separate workloads at
120 columns × 36 rows and 10 × 20 pixels per cell. Neither timings nor quality
metrics imply terminal transfer, compositor presentation or physical latency.
Avoid concurrent builds or heavy workloads when collecting retained timings.

For real application admission and background-worker delivery, run:

```sh
CARGO_BUILD_JOBS=2 cargo bench --locked --bench reader -- --sixel-production-only
```

This checked workload opens the NASA photograph in the inline reader and image
inspector through App commands, using an offline disk-cache entry. It verifies
all production diagnostic stages and prints the actual `:performance` rows plus
admission-to-ready inspector time. It does not measure physical display latency.

The fixtures include the existing SVG diagram, grayscale antialiased vectors,
an RGB gradient, a public-domain NASA photograph, deterministic high entropy,
and RGBA gradients on dark/light mattes. See the fixture provenance in
`tests/fixtures/images/README.md`. All variants use identical oriented/matted
fitted pixels, 256 colors, dimensions and encoding limits. Exact diagrams bypass
lossy quantization for every variant, preserving the reviewed terminal-color
path. They therefore do not measure each lossy algorithm on simple diagrams.

## Separately measured work

| Stage | Included work |
| --- | --- |
| Decode | Raster decode, orientation and opacity inspection; SVG tree lookup/parse/font preparation. Raster cache hits have no decode call. |
| Resize/matte | Crop, alpha compositing and Lanczos resampling; SVG rasterization into the actual matte. |
| Pixel buffers | RGB preparation, conversion of the small output palette and compaction of terminal-equivalent colors. |
| Exact colors | Check whether at most 256 distinct terminal RGB percentage triples suffice. |
| Color preparation | Input deduplication/counts/index mapping and Oklab conversion; direct conversion for the non-deduplicating variant. |
| Quantize | Wu histogram, palette and color lookup map, or RGB median-cut palette. |
| Dither/map | Continuous Floyd–Steinberg and lookup, or lookup alone without diffusion. |
| Encode | Palette definitions, six-row bitplanes, runs and DCS framing. Inline encoding accumulates all cell-height strip calls. |
| Reference pipeline | The original combined Pipeline, whose internal stages are not instrumented. Do not count it as palette construction alone. |

Totals also include cache hashing, ownership, allocations and cell/strip assembly.
Stage sums are not forced to equal total time. The production ImageWork sample
additionally includes disk/HTTP and decoder-lock wait, excluding UI queue wait.
Delivered jobs can include retargeted attempts; entirely discarded jobs do not
contribute a UI sample. Cached thumbnail hits start no new image work.
An enabled capture is thread-local and restored across nested scopes. Experiments
cannot change the policy of another worker or a subsequent ordinary render.

## Comparisons and fidelity

The retained policy is Wu in Oklab with deduplication, a shared 256-color palette
and Floyd–Steinberg diffusion factor 7/8. The staged version removes copies of
3N RGB bytes and N indexed bytes, where N is fitted pixel count. This is an
allocation/ownership optimization, not a lower resolution or palette budget.
The original copying Pipeline remains the reference; native tests and the
benchmark require equal palettes/indices, emitted wire and independently decoded
pixels. Other algorithms may differ and are measured rather than accepted as
equivalent.

Each emitted Sixel surface/strip is decoded independently by ImageMagick and
stitched at original dimensions. Missing pixels, wrong strip dimensions, changed
cold/warm results or default/reference inequality fail the comparison. RGB MAE,
MSE, maximum error and PSNR compare emitted pixels with the fitted RGB reference.
Euclidean Oklab distance × 100 is also measured; it is not CIEDE2000. Separate
8-pixel BOX neighborhood metrics assess local averaging by diffusion. A higher
per-pixel PSNR alone does not establish fewer visible gradient bands or better
line/text rendering. Open the generated HTML gallery and full-resolution images.

Sixel palette definitions use integer RGB percentages, so even the exact path
can differ slightly from original 8-bit samples. Equality with the reviewed
wire/decoded output is stronger than comparing floating palette values alone.
Transparency is composited before resampling; hidden RGB never becomes a palette
color merely because alpha was discarded after fitting.

Process peak RSS and CPU seconds are measured by `/usr/bin/time`, including
fixture creation, source caches and reference export, but excluding independent
decoding and quality calculation. They are not allocator-level quantizer peaks.
RSS can therefore hide a smaller quantizer workspace. Measurements describe one
process/one render at a time; production can have two preparation lanes.

Reports record compiler, platform, source revision/dirty state, fixture hashes,
executable hash, raw cold/warm samples, stages, RSS and fidelity. The generated
gallery and raw Sixel/PNG files remain under `.local/benchmarks/sixel` by default.
An existing local input can also be profiled without network access:

```sh
target/release/examples/sixel_profile --input /path/image.png \
  --strategy dedup --output /tmp/image-profile
```

Available experiments are `reference`, `dedup`, `direct`, `undithered` and
`median-cut`. They add no dependencies or dependency features. The latter two
are quality/cost controls, not automatic production fallbacks.
