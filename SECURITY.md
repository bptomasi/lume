# Security

Lume processes untrusted feed, article and image content. It does not run page
scripts, embedded executables or executable configuration. Parsing, decoding and
network boundaries retain explicit byte/resource budgets and qualified libraries.
Notes and reading state are stored locally; Markdown exports are explicit.

For a suspected vulnerability, use the repository's private vulnerability
reporting channel when the maintainer has enabled it. If no private channel is
available, open a minimal issue asking for a reporting contact without including
exploit details, credentials or private data. Do not submit confidential reports
as ordinary public bug reports.

Reports should identify the affected version, prerequisites, impact and a minimal
reproducer using non-sensitive data. There is no published response-time SLA or
promise to backport fixes to every historical version. Verify the maintainer's
supported releases before relying on an older build.

Dependency selection, scoped license exceptions and qualification requirements
are recorded in [dependency policy](docs/dependency-security.md). No completed
external security audit is claimed.
