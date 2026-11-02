# Exact Sixel preparation: next-release evaluation

Date: 2026-11-02. Decision: promote the exact encoder with the documented
standalone residency limitation below. Local, real HTTP and Foot checks
passed. CPU and fidelity targets passed. The original strict global RSS gate
was **not universally met**; no unconditional memory improvement is claimed.

The work followed three separate roles: [research/proposal](sixel-next-release-proposal.md),
[independent review](sixel-next-release-review.md), and
[implementation](sixel-encoder-implementation.md). The reviewed scope changed
integer encoding work, with no new dependency or image-quality setting.
Direct Oklab, different resizers and different diffusion remain experiments.

## Production candidate

A0 borrows dense color planes and submits equal-mask runs without allocating
an owned node map per color. A1 uses stable counting scatter into one reused
six-row event buffer, avoiding repeated scans and clears of K*W cells.
Production selects A1 only with at least 48 palette colors and a complete
planner workspace bound no larger than A0. It selects one representation
before allocation. There is no image-content classifier or extra entropy pass.

The narrow adapter preserves pinned icy_sixel 0.1.3 ownership of palette,
ASCII mask conversion, repeat syntax/255 limits and DCS framing. It extends
only the checked pending repeat count after calling `put_pixel`; color switches
and footer require flushed state. See the implementation contract and
[pinned writer source](https://github.com/mkrueger/icy_sixel/blob/80038c275a9a2075a31992c61f8b6e1275152b31/src/tosixel.rs).
Input validation, bounded cancellation checkpoints and accurate pending-buffer/
footer accounting also protect malformed, obsolete and oversized work.

Wu/Oklab, palette order/size, continuous Floyd–Steinberg diffusion, source
pixels, resize, matte, crop, resolution, inline strips and UI occlusion remain
identical. This improves preparation efficiency; it does not claim greater
resolution or faster terminal transfer/parsing of the unchanged payload.

## Final paired CPU and fidelity measurements

[Retained evidence](sixel-encoder-2026-11-02.json) includes raw total/stage
nanoseconds, process resources, paired summaries, input and executable hashes,
independent decoder versions and actual dense/sparse dispatch counters.

Native warm medians, milliseconds, on the same host and within the same
alternating comparison:

| Input | Old encode | New encode | Old whole | New whole | Whole reduction, paired |
| --- | ---: | ---: | ---: | ---: | ---: |
| Diagram | 5.30 | 3.20 | 6.94 | 5.78 | 16.5% |
| Thin antialiased lines | 5.70 | 3.99 | 8.94 | 8.66 | 3.1% |
| Gradient | 26.14 | 9.71 | 58.03 | 41.58 | 28.3% |
| NASA photograph | 29.14 | 14.05 | 88.91 | 71.38 | 19.7% |
| Noise | 91.55 | 51.24 | 157.71 | 118.75 | 24.7% |
| Transparent RGBA8 | 16.27 | 9.47 | 58.42 | 50.74 | 12.4% |
| Camera | 19.25 | 14.06 | 55.95 | 50.12 | 10.4% |
| Chelsea | 40.86 | 20.35 | 102.30 | 85.14 | 16.8% |
| Coffee | 49.85 | 23.81 | 129.97 | 100.07 | 22.9% |
| Text/UI fixture | 9.61 | 7.11 | 21.74 | 19.86 | 8.2% |
| Dense labeled fixture | 9.34 | 7.16 | 22.17 | 20.12 | 9.3% |
| True RGBA16 | 24.63 | 9.66 | 109.00 | 92.90 | 14.1% |

Each time column is a median of three process medians. Percentage is the
median of the three paired reductions; rounding and independent summaries
mean percentages need not equal a ratio of the displayed columns. Never add
independently summarized stage medians to construct whole time.

The final matrix contains 32 scenarios: twelve independent input fixtures in
native/inline modes, light mattes for both transparent inputs, photo zoom,
and a 119x35-cell gradient fitting to 1190x595 pixels. The last scenario
exercises both a partial six-row band and a partial final inline cell. Four
inputs are independent photographs; generated crops/layouts are scenarios,
not additional photographs. Sources and licenses are pinned in
[the fixture manifest](../../tests/fixtures/images/README.md).

There were 3,840 final renders: three independent serial processes per
executable/scenario, each with one cold and nineteen warm renders. Process
order alternated. All cold/warm wires were stable and every candidate source,
fitted image, frame count, payload length and wire hash matched the old
executable. ImageMagick independently decoded 64 complete outputs, including
all inline strips; dimensions, coverage and RGB hashes matched. Existing
native decoder/oracle tests additionally cover indices and boundaries.

The actual default exceeded the reviewed 20% encoder/8% whole improvement
target on multiple complex cases including natural photographs. No final
scenario showed a whole-preparation regression. These are preparation CPU
wall times, excluding file I/O, quality metrics and terminal/compositor
presentation; separately captured process CPU/RSS includes setup and export.

## Controls, rejected experiments and memory investigation

The frozen original is revision `1b683efe00693d928205f0bb1350d468f276f1df`,
SHA-256 `982f95edf47309d94f820f8c57a5cadcd5a8dcaaa06d63bb78d8410dcdffb4a5`.
The final profiling executable is SHA-256
`3149f7a1c4cd27c9d73e2f3c87f25c29e8b222f02eaf6601dc511b4e9ce9d03e`.
Both used Rust 1.99.1, release thin LTO and one codegen unit; no compiler or
dependency source substitution. Final source hashes and exact build manifests
are retained with [the matched production controls](sixel-app-2026-11-02.json).

Measurements used Intel Core Ultra 7 155H, eligible CPUs 0..21, with both
children pinned to CPU 2 and no concurrent builds/tests. Affinity is not
physical isolation. These fresh controls replace cross-day comparisons with
the earlier unpinned review; absolute times should not be compared to it.
Pillow 13.0.0 and ImageMagick 7.1.1-43 Q16 were used outside timed rendering.

[Initial A0/A1 evidence](sixel-initial-encoder-2026-11-02.json) independently
measured A0 before choosing production dispatch. A0 alone was insufficient for
the substantive photo target. The initial K>=32 sparse policy regressed the
34-color thin-line case by about 17.8% whole preparation. The final K>=48
policy routes that case to measured A0. Repeating the unchanged forced A0
path across the entire final corpus was unnecessary; final actual default and
memory were remeasured against the original on every scenario.

An extra scalar pending-run accumulator did not solve that regression and
increased measured encoder cost by about 6.9% on the gradient, 5.3% on the
photo and 1.6% on noise. It was rejected. The existing writer already
coalesces runs correctly. [Results](sixel-coalescing-2026-11-02.json) and
[a reproducible source patch](sixel-coalescing-prototype.patch) retain this
negative experiment; it is not compiled into production.

The new benchmark initially embedded three extra photographs, growing
`.rodata` by 827.8 KiB. They now load from the pinned repository corpus only
when explicitly exported by the developer tool. This removed unnecessary
executable data but **did not remove the observed noise RSS difference**;
subtracting the image size from RSS would have been incorrect.

[Separate memory sampling](sixel-memory-diagnostic-2026-11-02.json) found
additional anonymous residency in the repeated-noise harness. It was present
with two renders and remained bounded with twenty, rather than growing with
each iteration. Both old/new harnesses retain the prior payload during render,
clone the current protocol and concatenate it for hashing. These allocations
are outside the encoder's scratch accounting and differ from App/worker
ownership. This observation does not identify the allocator cause or establish
production memory equivalence. Matched production controls are described below.

## Matched App/worker control and qualified promotion

[App results and build manifests](sixel-app-2026-11-02.json) compare real App
admission, decoded-source reuse, worker preparation, delivery and diagnostics.
The source snapshot for the original was extracted with `git archive` and all
1,168 blobs verified against Git. The sole changed baseline file was the
benchmark fixture parameterization, byte-identical to the current benchmark.
All 1,167 other baseline files were rechecked after building.

The shared bench-only `LUME_BENCH_SIXEL_INPUT` parameter allows an explicit
source without changing application configuration. It reports the consumed
source SHA and the delivered native Sixel SHA/length, borrowed after the
delivery timer without large protocol copies. Default NASA remains borrowed;
a missing external input fails explicitly. The archive/current controls used
the same Rust 1.99.1, release flags and explicit `LUME_BUILD_REVISION` value
for comparable metadata. This override applied only to the benchmark controls;
the user binary was built separately with normal Git metadata. No production
source, compiler or dependency substitution was used to make the baseline pass.

Three alternating processes per executable/input each performed three checked
App inspections (nine per version/input). The exact-color source was generated
from `probe_sixel_crossover.source(1200,256)` and nearest-neighbor fitted to
1400x700, matching the inspector's aspect-constrained size without introducing
colors. Input SHA is `4f5d583055cc2fa1febebbb320dedb75ec3bbc217657b5059a8f28ffea2bd2fd`.
Every delivered source and native wire matched across all processes.

| App source | Old delivery, ms | New delivery, ms | Paired reduction | Old peak RSS, KiB | New peak RSS, KiB |
| --- | ---: | ---: | ---: | --- | --- |
| NASA photograph | 112.53 | 93.96 | 16.5% | 35196–35268 | 35020–35240 |
| Noise | 356.87 | 303.79 | 15.3% | 61848–62480 | 61888–62468 |
| Exact 256 colors | 90.39 | 46.11 | 49.0% | 34996–35368 | 35012–35136 |

Delivery includes admission/scheduling and worker completion; it excludes
terminal transfer/display. The approximately 1 MiB standalone RSS increase
did not recur in these App controls. Noise's paired RSS differences were
+40, -12 and +204 KiB, within overlapping process ranges. Three processes do
not prove memory equivalence, and this is not evidence for a global RSS win.

The independent reviewer approved **qualified promotion**, not an unconditional
RSS pass. The coordinator accepts the scoped standalone residency tradeoff:
CPU gains and exact fidelity are established, planner storage never exceeds
the dense bound, repeated-noise residency was bounded in the measured runs,
and App controls did not reproduce its larger peak. The cause of the
standalone anonymous-residency difference remains unresolved and tracked for
future allocator/lifecycle profiling. Do not attribute it to embedded photos,
subtract `.rodata` from RSS, or silently describe every memory gate as passed.

## Reproduction and final qualification

Use exact Rust 1.99.1 and first preserve an original release executable and
its SHA/build manifest. Build the candidate after code changes, then compare
explicit exported files in serial processes:

```sh
CARGO_BUILD_JOBS=2 cargo build --locked --release --example sixel_profile
make bench-sixel-release PYTHON=/path/to/visual-venv/bin/python \
  SIXEL_ARGS='--baseline /path/to/frozen/sixel_profile --candidate target/release/examples/sixel_profile --output /tmp/sixel-comparison --encoders production --cpu 2'
```

Choose an allowed CPU on the host. The tool checks official natural-source
hashes before export, consumes identical bytes in both executables and fails
on source, fitted, wire, coverage or pixel drift. `report.json` preserves the
full data; `evidence.json` retains raw timing vectors with a compact schema.
`--encoder dense` isolates A0 and `tools/probe_sixel_crossover.py` reproduces
the synthetic palette/width probe without invoking a build.

[Final qualification](sixel-qualification-2026-11-02.json) passed:

- `CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2 make check`: license inventory,
  formatting, all-target Clippy with warnings denied, 1,085 native Rust tests,
  checked reader benchmark sanity, 74 Python tool tests and Python compilation.
- `make check-http`: all 34 real HTTP tests passed, without skipping loopback.
- `make check-foot`: [complete Foot evidence](sixel-foot-2026-11-02.json),
  Foot 1.21.0, user binary SHA-256
  `f55f04409c2035d99ac4ac0509b818764bdbca1b0e5af3fb0d92028031cbced8`.
  This includes completed pan, rapid zoom/pan returning to byte-identical fit,
  theme matte changes/restoration, scroll/reopen, overlays, formula graphics,
  independent strip decoding, inline clipping and restored terminal attributes.

Reproduce the App control by applying the [shared benchmark patch](sixel-app-fixture.patch)
to an isolated original source snapshot, then building both reader benchmarks
with matching flags and metadata as recorded in their manifests. Use
`tools/bench_sixel_app.py --baseline OLD --candidate NEW --output DIRECTORY
--cpu 2`, adding `--input /absolute/image.png` for an external fixture. The tool
requires three completed inspections per process and matching source/native
wire hashes across all runs. No physical screen-presentation speedup is inferred.

Next research candidates remain separately gated: SIMD resize with exact
alpha/matte/filter semantics, direct/expanded Oklab with total live-memory
budgets and a larger natural-image corpus, and upstream writer formatting
allocations. Quantizer choice alone was not the dominant cost here.
