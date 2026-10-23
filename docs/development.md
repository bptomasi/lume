# Development

Use the exact stable toolchain in `rust-toolchain.toml`; CI must use the same
version. Install the normal C build tools required by bundled SQLite. Local
tool checks require Python 3.11 or newer. Product runtime requires no Python interpreter,
source checkout, containers or external database.

The [disk compression review](compression-review.md) records a separate proposal
and its measurement requirements; compression has not been enabled by it.

In the Articles pane, `dd` moves the selected article to Trash; `u` restores it
through undo, and `:trash` / `:restore-article` provide explicit recovery. The
first `d` waits for a second press on the same article. Escape, another key,
pointer navigation or focus loss cancels the prefix. Held-key autorepeat never
completes it. In Library and annotations, `dd` keeps its existing contextual
behavior. Use `:organize` to edit folder membership independently of Trash.

## Build and installation commands

`make` and `make help` list commands without building or modifying data.
`make paths` shows effective build/install locations. `make build` produces
the locked release executable; `make run` opens the local application and
`make demo` opens the isolated demonstration library.

`make install` installs the executable and dependency notices. It preserves an
existing destination library and can import a legacy SQLite snapshot when no
library exists. If several legacy libraries exist, select one explicitly with
`MIGRATE_FROM=/absolute/path`. Relative `XDG_DATA_HOME` values are ignored.

`make upgrade` requires an installed executable, rebuilds the current checkout
and replaces only the executable and notices. It does not pull Git, collect
sources, open/migrate a library or write configuration. Schema upgrades occur
when Lume next opens the library. Upgrade rejects `MIGRATE_FROM`.

Use `PREFIX`, `DATA_DIR`/`LUME_DATA_DIR`, `CARGO_TARGET_DIR`, `BINARY`, `CARGO`
and `PYTHON` for explicit locations/tools. Desktop qualification targets use
the same computed `BINARY`, including builds outside the checkout.
`make clean` removes Cargo build output; it does not remove a library.
`make format` applies Rust formatting; `make fmt` only verifies it.

## Quality gates

```sh
make check
make check-release
```

`check` serially composes formatting, Clippy for every target, native behavioral
and benchmark correctness tests, Python tooling tests and syntax checks.
`check-release` additionally requires the current RustSec gate, real loopback
HTTP fixtures and the locked release build. Install cargo-audit 0.22.2 explicitly
with `cargo install cargo-audit --version 0.22.2 --locked --no-default-features`. Required HTTP tests are not silently skipped. Ordinary
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
make check-latency
```

Foot qualification requires native Wayland, Foot, ImageMagick's `convert` and
Pillow. Install the optional visual tools into a virtualenv:

```sh
python3 -m venv .venv
.venv/bin/python -m pip install -r tools/requirements-visual.txt
make check-foot PYTHON=.venv/bin/python
```
The latency harness requires a real Foot session plus an owned loopback HTTP
publisher. It requires at least 100 samples per workload for reported p99.
Use `FOOT_ARGS` or `LATENCY_ARGS` for explicit `--binary` and
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

CI is configured for the pinned toolchain, local/license/advisory/HTTP checks,
a release build and native Wayland Foot under a private Weston compositor.
Weston's X11 backend runs inside Xvfb with software rendering to provide the
input seat required by Foot. Install `weston`, `wayland-utils`, `xvfb`, `xauth`
and `fonts-dejavu-core` to reproduce this isolated environment:

```sh
xvfb-run -a -s '-screen 0 1920x1440x24' \
  python3 tools/qualify_wayland.py --artifacts /tmp/lume-wayland -- \
  make check-foot PYTHON=.venv/bin/python
```

CI retains terminal output, pixel checks, startup logs and binary hashes.
These wire checks complement physical display inspection; they do not measure
compositor presentation latency. A workflow definition alone is not evidence
that a hosted CI run passed. See [release qualification](release.md).

## Terminal support

| Environment | Text/input | Images | Qualification scope |
| --- | --- | --- | --- |
| Linux x86_64, Foot, native Wayland | Supported | Native Sixel | Required real Foot wire/pixel/input/cleanup gate; physical presentation checked separately. |
| Generic ANSI terminal | Portable baseline | Character previews | Native Rust frame/input tests; terminal-specific differences remain possible. |
| tmux or Zellij | Portable baseline | Character previews by default | Native graphics passthrough is not qualified; force a protocol only deliberately. |
| WezTerm/iTerm2 | Experimental | iTerm2 encoder | Encoder fixtures pass independently; native desktop behavior is not yet qualified. |
| macOS/Windows | Unqualified | Unqualified | No release binaries or native CI commitment for these platforms yet. |

Yank uses OSC52; the terminal controls clipboard permission. It can reject the
request, and Lume cannot verify the system clipboard contents. Qualification
windows disable clipboard access. No publisher content can copy itself.

`make install DATA_DIR=/custom/path` prints the explicit `--data-dir` invocation
and `LUME_DATA_DIR` export needed to select that library at runtime. The binary
retains portable defaults and never embeds an installer's personal paths.
