# Development

Use the exact stable toolchain in `rust-toolchain.toml`; CI must use the same
version. Install the normal C build tools required by bundled SQLite. Local
tool checks require Python 3. Product runtime requires no Python interpreter,
source checkout, containers or external database.

## Quality gates

```sh
make check
make check-release
```

`check` serially composes formatting, Clippy for every target, native behavioral
and benchmark correctness tests, Python tooling tests and syntax checks.
`check-release` additionally runs real loopback HTTP fixtures and builds the
locked release binary. Required HTTP tests are not silently skipped. Ordinary
checks never start containers or bootstrap development tools.

Keep `Cargo.lock` versioned and use `--locked`. Dependency changes update only
the intended packages, record their selection and retain distribution notices.
Never substitute a local toolchain/source tree without identifying it explicitly.
See [dependency policy](dependency-security.md).

## Real terminal qualification

These commands open only their own test windows and temporary libraries:

```sh
make build
make check-foot
xvfb-run -a make check-alacritty
make check-latency
```

Foot qualification requires native Wayland, Foot and ImageMagick's `convert`.
Alacritty qualification requires X11/Xvfb, Alacritty, xdotool, xclip and x11-apps.
The latency harness requires a real Foot session plus an owned loopback HTTP
publisher. It requires at least 100 samples per workload for reported p99.
Use `FOOT_ARGS`, `ALACRITTY_ARGS` or `LATENCY_ARGS` for explicit `--binary` and
`--artifacts` paths. The default binary is `target/release/lume`.

Native buffer checks do not prove compositor presentation, physical screen
pixels or human perception. Report an unavailable desktop gate explicitly;
never describe a buffer test as real-terminal qualification.

## Benchmarks

```sh
make bench
make bench-code
make bench-interaction
make bench-images
make bench-notes
make bench-recommendations
make bench-configuration
```

Workloads check results independently and separate scenario setup from measured
operations. Process memory and I/O include setup where the report states so.
Fresh reports go under ignored `.local/benchmarks/`; retain a small reviewed
baseline under `docs/performance/`. Compare repeated runs before claiming a
speedup. See [performance](performance.md).

To build outside the checkout:

```sh
CARGO_TARGET_DIR=/tmp/lume-build CARGO_INCREMENTAL=0 make check
CARGO_TARGET_DIR=/tmp/lume-build make build
```

The same target directory must be supplied when selecting the resulting binary
for installation or desktop qualification. Build artifacts are disposable;
`make install` never deletes a user library to clean build output.

## Visual review

```sh
cargo run --locked --example preview -- /tmp/lume-preview.json
python3 tools/render_preview.py /tmp/lume-preview.json /tmp/lume-preview.png
cargo run --locked --example welcome -- /tmp/lume-welcome.json 120 40
python3 tools/render_preview.py /tmp/lume-welcome.json /tmp/lume-welcome.png
```

These export real Ratatui cells using original demonstration content. The PNG
renderer illustrates cells and uses its own font; it is not a screenshot of a
physical terminal. Keep public screenshots free of private subscriptions or
annotation text. Regenerate snapshot fixtures only after reviewing the intended
change:

```sh
UPDATE_LUME_VISUALS=1 cargo test --locked --test visual_snapshots
```

## Code and schema

Follow [AGENTS.md](../AGENTS.md): readable English code, cohesive responsibilities,
explicit domain contracts, normal 80-column formatting, no unsafe or production
panic control flow. Fix Clippy findings at their owner. Exceptional coherent
functions may use a local `#[expect]` with a concrete invariant, not a blanket
suppression or inflated threshold.

Operational and test SQL stay under the owning store's `sql/`. Production cannot
include `test_*.sql`. Keep structured literals readable. Never rewrite executed
migration names or bytes. New migration versions are contiguous and must retain
publisher and personal data across actual upgrade tests.

Tests use temporary libraries and explicit configuration paths. They must not
read or modify the developer's personal configuration, subscriptions, clipboard
or terminal preferences. No network fixtures contact arbitrary live publishers.

CI runs the pinned toolchain, local checks, real HTTP, release build and an
isolated Alacritty/Xvfb window. Native Wayland/Foot qualification remains a
separate required validation for changes to its image and input contracts.
