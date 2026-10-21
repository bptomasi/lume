# Optional reading presets

Lume is a general-purpose reader. Fresh libraries contain no subscriptions,
custom smart lists or implicit recommendation interests. Presets are explicit
imports; they are never selected at startup or during installation.

## Engineering

```sh
lume preset engineering
```

This selection includes 98 subscriptions and 46 smart lists across computing,
systems architecture, security research, AI, databases, identity, operations,
platforms, Linux, eBPF and networking. If the recommendation profile has no
interests, the preset adds its explicit engineering vocabulary. Existing personal
reading data and active custom search criteria are retained.

`lume starter` remains a compatibility alias. Installation does not collect
articles immediately; opening Lume starts background collection, or use
`lume sync`. `--offline` retains the library and disables network requests.
Reinstallation re-enables bundled subscriptions that were archived.

The source data is versioned separately from application defaults:

- [Computing OPML](../assets/reading/computing.opml).
- [Security OPML](../assets/reading/security.opml).
- [AI OPML](../assets/reading/ai.opml).
- [Infrastructure OPML](../assets/reading/infrastructure.opml).
- [Saved searches](../assets/reading/searches.json).
- [Source groups](../assets/reading/source-groups.json).
- [Recommendation vocabulary](../assets/reading/interests.json).

Import an individual OPML file to select only part of the bundle. Feed URLs and
publication behavior can change; source health displays failures and retry state.
The network qualification suite uses owned fixtures rather than promising the
availability of every third-party publication.

## Other subjects

Add any supported RSS, Atom or JSON Feed URL, import OPML from another reader,
or define portable sources and lists in TOML. Examples do not execute code and
are not installed automatically. After configuring a selection:

```sh
lume config export my-reading-preset.toml
```

The export contains definitions and explicit preferences. Article content,
annotation text, favorites, memberships, feedback and reading progress are not
included. Review feed URLs before sharing: a private publication may encode
subscription credentials in its URL. See [configuration](configuration.md).
