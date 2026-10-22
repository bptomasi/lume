# Lume

A quiet place for curious minds.

![Lume's welcome screen](docs/preview-welcome.png)

A general-purpose RSS, Atom and JSON Feed reader for the terminal, written in
Rust. Read offline, organize articles and keep notes beside the text. Local,
independent and configurable, with no account or server required.

## Install

Requires the pinned Rust toolchain, C build tools and Python 3 for installation.
The installed application needs neither Python nor the source checkout.

```sh
make install
lume
```

Run `make` for available commands. `make upgrade` rebuilds this checkout and
updates an existing installation, preserving configuration and reading data.

The binary is installed in `~/.local/bin`; add it to `PATH` if needed.
Libraries live in `~/.local/lume`, independently of the working directory.
Override this with `LUME_DATA_DIR`, `--data-dir` or `--database`.

**Lume starts empty.** Add a feed with `:feed URL`, discover sources with
`Space d`, or import OPML. Use `lume --demo` to explore an isolated sample
library and `lume --offline` to disable network access.

## Features

- Vim navigation, syntax highlighting, centered reading and Foot inline images.
- Movable panels with keyboard and mouse controls.
- Folders, smart lists, favorites and local recommendations.
- Optional Miniflux mirror for server-owned subscriptions.
- Searchable notes, highlights, backlinks and Markdown export.

Press `?` for help, `:` to discover commands and `Ctrl-w` for layout controls.
See [key bindings](docs/keybindings.md).

## Configuration

Preferences, sources and list definitions use TOML. Changes made through the
interface or CLI update the same file; personal reading data stays in SQLite.

```sh
lume config path
lume config check
lume config export reader.toml
lume --config reader.toml
```

The default path is `~/.config/lume/config.toml`, respecting `XDG_CONFIG_HOME`.
`--config` and `LUME_CONFIG` override it. Existing libraries are preserved.
See [configuration](docs/configuration.md) for isolation, examples and recovery.

`lume preset engineering` installs an optional source selection. See
[presets](docs/reading-sources.md).

## Development

```sh
make check
make check-release
```

See [release and recovery](docs/release.md),
[development and terminal support](docs/development.md),
[architecture](docs/architecture.md) and [performance](docs/performance.md).

MIT licensed. Retain the [third-party notices](docs/dependency-security.md)
in `assets/licenses/` and `assets/syntax/` when distributing applicable assets.
