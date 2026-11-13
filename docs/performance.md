# Performance

## Measuring changes

Use the compiler selected by [rust-toolchain.toml](../rust-toolchain.toml), the
locked dependencies and an optimized build. Correctness checks and timing
runs answer different questions: `make check` runs checked scenarios, while
`make bench` measures representative work. See the
[development gates](development.md#quality-gates).

```sh
make bench
make bench-interaction
make bench-reading
make bench-images
make bench-notes
make bench-configuration
```

Run `make help` for the remaining focused drivers. Generated reports belong in
ignored `.local/benchmarks/`; keep reproducible workloads in `benches/` and
`tools/`. Documentation should explain the method and current contracts rather
than depend on one machine's artifacts or logs.

For an old/new comparison:

1. Preserve the baseline executable before rebuilding. Record source revision,
   dirty files, binary hash, compiler, profile, features and fixture identities.
2. Verify that both timed paths perform equivalent work and retain their
   assertions. Changed fixtures or output capabilities need separate controls.
3. Warm both binaries, then alternate baseline/candidate/candidate/baseline
   across several independent processes. Do not overlap builds or test suites
   with timing runs.
4. Keep every observation, including regressions. State setup, cache, memory
   and I/O boundaries. A median of process means is not a pooled operation
   percentile; report sample counts and harness precision.
5. Validate source text, identities, ordering, pixels and failure behavior
   before accepting an optimization. Reduced allocations or VM-step counts do
   not by themselves establish lower latency.

Compiler, engine, dependency or feature changes can confound an application
comparison. A smaller feature build does different work; shared inputs alone
are not an equivalent-output benchmark. Neither passing tests nor a proposed
algorithm proves a speed improvement.

To limit local resource use:

```sh
CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 \
  CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2 make check
```

## Notebook and navigation workloads

Build and run focused optimized scenarios:

```sh
cargo bench --locked --bench reader -- --notebook-only
cargo bench --locked --bench reader -- --notebook-redundancy-only
cargo bench --locked --bench reader -- --notebook-profile-only
cargo bench --locked --bench reader -- --library-sections-only
cargo bench --locked --bench reader -- --motion-review-only
```

Add `--test` after the workload flag for a short correctness pass. The full
reader benchmark requires all default [capabilities](build-features.md).

The notebook workload checks search, source context, real key/frame work and
exact-byte file export. The redundancy workload compares full-preview versus
ID-only export enumeration and repeated versus clamped page recovery. Its
reference runs before the reduced path inside each process; account for that
fixed order when interpreting results. Enumeration excludes file publication.

The SQL profiler separates preparation, bind/first-row execution, owned-value
materialization, reset and finalization. It checks returned fields against
reference queries. SQLite VM steps describe virtual-machine work, not physical
processor instructions. Timer and generic row-mapping overhead remain in the
stage measurements; use the complete workflow for acceptance.

The populated-library workload verifies that folding retains destinations,
preserves header focus and queues no catalog/article read. Folding projects
cached grouping/counts; it should not trigger a database recount.

## Image and Sixel measurements

Install the pinned optional visual tools as described in
[development](development.md#real-terminal-qualification), then run:

```sh
make bench-sixel PYTHON=.venv/bin/python
cargo bench --locked --bench reader -- --sixel-production-only
```

The first driver builds `examples/sixel_profile.rs` in release mode. It measures
cold decode and warm source-cache reuse for native and cell-height inline
encodings. The production workload drives App admission and worker delivery
through the offline cache and checks the emitted diagnostic stages.

| Stage | Work included |
| --- | --- |
| Decode | Raster decoding/orientation or vector parsing/font preparation. |
| Resize/matte | Crop, alpha compositing, resampling or SVG rasterization. |
| Pixel buffers | RGB ownership and terminal-palette conversion/compaction. |
| Exact colors | Detect images representable without lossy quantization. |
| Color preparation | Deduplication, counts/index mapping and Oklab conversion. |
| Quantize | Palette construction and lookup-map preparation. |
| Dither/map | Continuous error diffusion and indexed-pixel mapping. |
| Encode | Palette definitions, bitplanes, runs and protocol framing. |

Totals also include ownership, hashing, allocations and strip assembly; stage
sums need not equal total time. The combined reference pipeline is not palette
construction alone. `:performance on`, `:performance status` and
`:performance reset` expose bounded local runtime samples.

All compared variants must use the same fitted pixels, orientation, matte,
resolution, palette limit and encoding bounds. Include diagrams, photographs,
gradients, high entropy and transparency. Exact-color diagrams bypass lossy
quantization and therefore do not measure a lossy algorithm's palette cost.

Decode emitted Sixel independently and inspect the gallery at full resolution.
Check dimensions, strip stitching, cold/warm consistency and required reference
equality. Per-pixel PSNR alone can favor an undithered gradient with visible
bands; combine numerical metrics with line/text fidelity and visual inspection.
Sixel's integer RGB percentages also limit color precision. Preserve alpha
compositing order so hidden RGB does not become visible palette noise.

For encoder-only changes, preserve the baseline profiler and compare it with
an independently built candidate:

```sh
make bench-sixel-release \
  SIXEL_ARGS='--baseline .local/baseline/sixel_profile --candidate target/release/examples/sixel_profile --output .local/benchmarks/sixel-release'
```

Experimental quantizers are scoped to the profiler; they do not change another
worker's policy or introduce application configuration. Source fixtures and
provenance live in [the image fixture catalog](../tests/fixtures/images/README.md).

## Interpreting latency and memory

TestBackend timings measure application and buffer work. Foot input/DSR
acknowledgements and independently decoded wire pixels exercise a real terminal,
but do not measure compositor presentation or physical key-to-photon latency.
Use the separate [terminal qualification](development.md#real-terminal-qualification)
when changing drawing, input, clipping or cleanup.

Process peak RSS includes fixture setup, source caches and exported references
where the driver reports them. It is not an isolated decoder or quantizer heap
peak. CPU time, wall time, output bytes and memory can move differently; report
those tradeoffs rather than selecting only the most favorable metric.
