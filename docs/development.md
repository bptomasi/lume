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

The Articles list updates after the deletion transaction commits, independently
of sidebar recounts and preparation of the next article. Failed writes retain
the row. Modal editors and selections preserve their captured source until
normal reading resumes; undo remains on the same ordered personal-write lane.

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
It also runs the production architecture and native SQL ownership guards.
`check-release` additionally requires the current RustSec gate, real loopback
HTTP fixtures and the locked release build. Install cargo-audit 0.22.2 explicitly
with `cargo install cargo-audit --version 0.22.2 --locked --no-default-features`. Required HTTP tests are not silently skipped. Ordinary
checks never start containers or bootstrap development tools.

Keep `Cargo.lock` versioned and use `--locked`. Dependency changes update only
the intended packages, record their selection and retain distribution notices.
Never substitute a local toolchain/source tree without identifying it explicitly.
See [dependency policy](dependency-security.md).

## Compiler and capability qualification

`rust-toolchain.toml` is the only release compiler/version/component source.
`tools/ci.py install` reads it, installs that exact toolchain and fingerprints
the actual `rustc -Vv` output for CI caches. `Cargo.toml` independently owns
`package.rust-version`; `tools/ci.py install --compiler minimum` installs that
minimum, never substitutes stable, and uses explicit `cargo +VERSION` calls.
Local release gates continue to use the repository's pinned compiler.

On 2026-11-13, the official rustup Rust 1.96.0 toolchain
(`rustc 1.96.0 (ac68faa20 2026-05-25)`) passed locked offline all-target checks
with full defaults and native correctness with default and minimal features.
The default run passed 1,178 native tests and both checked benchmark targets;
minimal passed 1,093 native tests and the shared capabilities target. Their
34/28 ignored real HTTP cases were not qualified by those runs. This supports
the existing 1.96 minimum without changing the pinned release compiler; it does
not claim a hosted CI or final release qualification.

The reader CI job runs full defaults through local, advisory, HTTP, release and
Foot gates. A bounded matrix additionally tests the actual minimum compiler
with default and minimal features, and the pinned compiler with minimal,
text (`math,syntax,miniflux`), graphics (`graphic-math,sixel`), SVG-only and
Sixel-only features. Each runs all applicable native targets and checked
benchmark scenarios; pinned profiles also require strict Clippy. The full reader
benchmark requires all six default capabilities. The shared capabilities bench
checks conditional output without claiming identical work across profiles.

All matrix jobs and the reader job must succeed before tagged publication.
The publisher still downloads the full-default reader artifact from that run.
Caches separate compiler fingerprints and debug settings. Reduced pinned jobs
restore compatible reader dependencies but never save redundant target trees;
only the reader and minimum/default jobs write compiled caches, on the default
branch. The matrix permits at most two concurrent jobs, each with two build
jobs and two test threads; it does not multiply release or desktop builds.

To reproduce a reduced profile using its source-declared compiler:

```sh
python3 tools/ci.py install --compiler minimum
CARGO_TARGET_DIR=/tmp/lume-minimum python3 tools/ci.py qualify \
  --compiler minimum --profile minimal
python3 tools/ci.py qualify --profile sixel
```

The architecture guard treats each library, binary and build-script crate
separately. Its nodes are declared root modules plus the composition root.
It resolves grouped imports, aliases, reexports, relative and `#[path]` modules,
and explicit `crate`/library paths. Explicit test-only subtrees are excluded;
production reachability wins when the same file has both uses. Unreachable
production files fail instead of escaping inspection. Other cfg predicates
remain production conservatively; there is no cycle exemption list.

The same guard retains private App fields, rejects UI access to App (including
declared aliases and views containing App), forbids App-to-UI imports and catches
`mem::take`/`replace` of a known scalar Copy getter temporary. Real field borrows,
mutable-reference getters and `RefCell` borrows remain valid. This is lexical
inspection of declared architectural units, not Rust name/type checking: macros,
inferred receiver types and unknown return types remain compiler/reviewer work.
Regressions exercise both newly rejected dependencies and valid boundaries.

## Focused changes and review

Keep future commits focused on one owner contract or independently reviewable
behavior. Pair a state extraction with its invariant/failure tests; pair a new
adapter with its capability fallback and relevant checked workload. Separate
mechanical moves from behavioral changes when that makes the resulting diff
easier to verify. Preserve released history rather than rewriting old commits
to improve their size.

Before committing, review the complete diff for ownership, readable functions,
unintended data/config changes and meaningful assertions. Run the affected
native tests plus `make check-architecture check-sql fmt`, then the appropriate
Clippy/profile gates. Final integration still requires `make check-release`
and the real terminal gate after UI changes. Report compiler versions, feature
profiles, actual checks and any blocked gate explicitly. See the
[architecture map](architecture.md#at-a-glance) for where a change belongs.

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
