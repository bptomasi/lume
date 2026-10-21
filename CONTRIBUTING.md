# Contributing to Lume

Lume is a general-purpose terminal reader. Features should remain useful across
subjects, operate locally and preserve a fast reading experience. Bundled reading
selections are optional presets; never introduce personal topics into defaults.

For a bug report, include the Lume version, operating system, terminal and its
version, reproduction steps and expected/actual behavior. Include a minimal
redacted configuration or a public feed URL when it helps. Report extraction
failures with the original page and whether Feed or Full content was selected.

Discuss substantial behavior or public configuration changes before implementing
them. Prefer small, cohesive changes with native behavioral/failure tests and
appropriate checked performance evidence. Keep article content, annotations,
favorites and reading progress intact during migrations and publisher refreshes.

Use the pinned toolchain and run `make check`. Database, migration or release
input changes also require `make check-release`; affected terminal behavior needs
its real-terminal gate. Report unavailable checks and their practical limits.
Follow [development](docs/development.md), [architecture](docs/architecture.md),
[configuration](docs/configuration.md) and [dependency policy](docs/dependency-security.md).

Pull requests should explain the concrete problem, resulting behavior, validation
and material limitations. Avoid committing private libraries, subscription
credentials, generated build output or routine benchmark runs. Retain applicable
third-party notices. Contributions are under the repository's MIT license.
