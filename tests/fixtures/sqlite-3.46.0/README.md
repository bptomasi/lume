# Shipped SQLite compatibility fixtures

These databases were generated on 2026-11-13 **before** updating Lume's SQLite
integration, using the published `rusqlite =0.32.1` with `bundled`,
`libsqlite3-sys 0.30.1` (runtime `sqlite_version()` exactly `3.46.0`) and
`refinery =0.8.16` with only `rusqlite`. Rust 1.99.1 ran the standalone generator.
They contain synthetic public example URLs and synthetic personal state.

The generator read the SQL and dependency lock from source revision
`21b643f2df6bbacd0ab374ec4af0a37f03a51d8d`. That revision's `Cargo.lock`
records the complete old dependency resolution; it can be read with
`git show 21b643f2df6bbacd0ab374ec4af0a37f03a51d8d:Cargo.lock` from a fresh
checkout. Use that historical source and engine for any independent
reproduction, writing to a temporary output directory. Never overwrite these
fixtures: timestamps and SQLite file bytes need not be identical on rerun.

The generator executed the unchanged `src/store/sql/create_schema.sql`, then
`Migration::unapplied(name, sql)` and `Runner::new(...).set_grouped(true).run(...)`
with migrations sorted by numeric version. It executed these existing seed files:

| Database | Applied migrations | Seed under `src/store/sql` |
| --- | --- | --- |
| `v7.sqlite3` | V1 through V7 | `test_setup_read_index_library.sql` |
| `v27.sqlite3` | V1 through V27 | `test_search_upgrade_data.sql` |

The connection was closed without running current Store initialization. The
history JSON files are the original `(version, name, applied_on, checksum)` rows,
ordered by version, read from each closed database. They preserve the old
library's actual timestamps and decimal checksums, rather than recalculating
those expectations with the new library. Fixtures use SQLite's default 4096-byte
pages and DELETE journal mode and have no sibling WAL/SHM files.

SHA-256 identities:

- `v7.sqlite3`:
  `c53696a82110c71b7d728be95ee6f1913a79b6c8ed18d6ecc3333c8fbc237187`
- `v27.sqlite3`:
  `f7136a3ee344e26ca914fc47442f681420dcfea7068ca83b7bb1d79d392e55fe`

Keep these files immutable when updating dependencies. Regeneration with current
libraries would lose the independent compatibility evidence. Migration tests
compare the new library's checksums against every old row, verify that upgrades
append history without rewriting existing rows, and check personal data,
full-text search, isolated restore, no-clobber backup and corrupt-history errors.
