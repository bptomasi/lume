# Optional build capabilities

The standard build enables every supported capability. A reduced build keeps
ordinary feeds, raster images, browsing, configuration, SQLite and cached
reading available:

```sh
cargo build --locked --no-default-features
cargo build --locked --no-default-features --features math,syntax
```

Use the exact compiler in `rust-toolchain.toml`. These options choose compiled
capabilities; they never change the configuration format or database schema.

| Feature | Compiled capability | Behavior when omitted |
| --- | --- | --- |
| `math` | Bounded math-core TeX to Unicode projection | Original formula source, including delimiters, remains selectable. |
| `graphic-math` | RaTeX outlines; enables `math` and `svg` | Formulas use available Unicode projection or original source. |
| `sixel` | Lume's shared-palette quantization and Sixel preparation | Sixel preferences and terminal hints use portable Blocks. iTerm2 remains available. |
| `svg` | Restricted resvg rasterization and fallback fonts | Vector images show a capability explanation; raster decoding remains available. |
| `syntax` | Syntect grammars, build compilation and runtime highlighting | Code preserves its text, columns, mappings and selection with plain colors. |
| `miniflux` | Miniflux discovery, credentials and entry transport | Cached server feeds remain readable; collection omits server requests. |

`graphic-math` does not enable Sixel implicitly. Direct reader consumers can
prepare formula outlines without a terminal protocol. The interactive reader
uses them only with Sixel; other presentations retain text. Stored formula,
image protocol and server settings round-trip even if their capability is
absent. Reopening the same library with a full build restores those choices.

When Miniflux is absent, collection still commits ordinary feed results. Its
completion explains the unavailable capability once, with completed publisher
feed/failure counts, if an enabled server was omitted. Explicit `lume sync`
therefore reports an error instead of claiming that all configured sources
were synchronized. It does not write synthetic server failures or cooldowns.
Disabled server definitions do not cause this warning.

An unavailable SVG source is remembered by URL identity in a 64 KiB session
cache, within the shared preparation budget, so scrolling, resizing and
protocol changes cannot repeatedly prepare it. An
unsupported vector already in the disk cache is not downloaded again merely
because this build cannot decode it. `:images retry` clears session rejection
state and validates the current bytes once; use it after replacing a cached
resource. A source that is still SVG reports the same capability reason.
Cache eviction can eventually readmit a source; all caches remain bounded.

The aggregate `reader` benchmark requires the six default features because
its correctness checks include real outlines, native Sixel pixels, colored
code and server collection. The Sixel profiler example requires `sixel,svg`;
the dedicated native suites declare their required capabilities. Generic
storage, navigation, reading, raster and configuration tests run in reduced
builds. `tests/build_capabilities.rs` checks the active presentation contract.
The `capabilities` benchmark works in both builds and checks identical mixed
article and raster inputs. Its formula and syntax assertions follow the active
capabilities; timings therefore do not isolate equivalent rendering work.
Run a correctness pass with the following command, adding
`--no-default-features` for the reduced build:

```sh
cargo bench --profile dev --bench capabilities -- --test
```

Optional packages remain in `Cargo.lock`. Measure active normal/build edges
with `cargo tree --locked --edges normal,build --prefix none`, adding
`--no-default-features` for comparison. The ratatui-image dependency still
retains its own upstream Sixel support when Lume's `sixel` feature is absent;
the feature removes Lume's quantizer/preparation path, not every protocol
package from the graph. Package counts alone do not establish binary size,
compile time, runtime speed or memory improvements.

With the current locked versions and Rust 1.99.1, active normal/build
dependencies measured on 2026-11-13 are:

| Graph scope | Default | No default features |
| --- | ---: | ---: |
| Linux `x86_64-unknown-linux-gnu` | 342 | 246 |

These counts exclude Lume itself, deduplicate package name/version pairs and
include build dependencies. The reduced Linux graph omits math-core, RaTeX,
resvg, Syntect and Quantette, including build-time Syntect. It retains
`icy_sixel` through ratatui-image. The cargo-about inventory covers the
supported feature/platform union: 398 licensed packages including Lume, with
four inactive lock entries.
`cargo tree --target all` also enumerates permanently false `cfg(any())`
edges, so its raw count is not an active-package count.
