# Release qualification and recovery

The initial binary distribution target is Linux x86_64 GNU, built on Ubuntu
24.04 (glibc 2.39 or newer). Other targets remain source builds without a
release support commitment. `lume --version` includes the source revision;
BUILD.json records the revision, toolchain, target, dirty-source status and
binary/lock hashes. An archive checksum verifies bytes, not publisher identity.

Run these gates on the revision being packaged:

```sh
make check-release
make check-foot
make package PACKAGE_ARGS=--require-clean
```

`make package` includes the executable, full dependency inventory and actual
notices, exact MPL source archives, grammar notices, installer, build manifest
and SHA256SUMS. It tests CLI startup and backup/restore outside the checkout
with a private HOME/config/library before packaging. Archive ordering and
metadata are deterministic for the same binary and revision. Compiler output
is not claimed reproducible across arbitrary hosts. No release is uploaded
by this command. Manual CI dispatch retains the qualified package as an
artifact. Tag pushes publish through the separate release job described below.

## Versions and tags

Use Semantic Versioning, starting with version **0.2.0** in `Cargo.toml` and
the annotated Git tag **v0.2.0**. Keep the root package version in `Cargo.lock`
in agreement. Before 1.0, use a patch release for compatible fixes and a minor
release for new capabilities or incompatible changes. Use `1.0.0` once the
public configuration and command contracts have a stability commitment.
Prereleases use suffixes such as `0.3.0-rc.1` and `v0.3.0-rc.1`.

After committing the intended revision and version, publish its tag:

```sh
git tag -a v0.2.0 -m 'Lume 0.2.0'
git push origin v0.2.0
```

The workflow rejects a tag that disagrees with `Cargo.toml` before compiling.
Publication requires local, license, advisory, real HTTP and real Foot checks,
the release build and a clean-source package from that same workflow run.
The publishing job verifies the remote tag revision, archive checksum, build
manifest and executable hash before uploading the archive and checksum to a
draft. It verifies the uploaded asset digests before making the release public.
Prerelease tags produce prereleases; stable tags update the latest release.

Only the publishing job receives repository write permission. A failed check
leaves the tag without a published release. Fix the failure and rerun the
workflow, or issue a new version if source changes are needed. Never move a
published tag. Reruns accept an already published release only when its asset
bytes match exactly; published assets are never replaced. An interrupted draft
can be completed by rerunning the same qualified revision.

License updates require cargo-about 0.9.2 with its CLI feature:

```sh
cargo install cargo-about --version 0.9.2 --locked --features cli
make licenses
make check-licenses
```

Review all intended public screenshots, tracked files and a full-history secret
scan before changing repository visibility. Do not rewrite published history
or print discovered secret values; revoke a real credential before considering
history cleanup. Build, qualification and local data directories are ignored.

## Consistent snapshots

Before an upgrade, close an editing session or save its open notes, then create
an explicit snapshot:

```sh
lume backup /safe/location/lume-before-upgrade.sqlite3
```

The backup CLI opens its source read-only and preserves its schema version,
including when an older library is being protected before an upgrade. It does
not reconcile TOML or migrate the source first. The SQLite snapshot includes
committed WAL state, articles, annotations, tags,
reading state, undo, drafts, layouts and declarative definitions from one
consistent database view. Unsaved editor text requires a draft checkpoint or
saving first. Image cache files are disposable and omitted. External TOML edits
that have not been reconciled are not yet database definitions; reconcile them
before backup or retain their original file separately. Exported Markdown and
incremental notebook exports are useful documents, not full-library snapshots
or a cross-article transaction snapshot.

Restore validates an isolated copy and upgrades only that copy:

```sh
lume restore /safe/location/lume-before-upgrade.sqlite3 /new/location/lume
lume --data-dir /new/location/lume --config /new/location/lume/config.toml
```

The destination must not exist. Both the snapshot and active library remain
unchanged. The new private directory contains library.sqlite3 and config.toml;
configuration bindings are detached from the old absolute file location.
Corrupt, foreign and newer unsupported schemas fail validation. A filesystem
failure during final publication can leave a partial new directory for
inspection; it never replaces an existing destination. Keep the original
snapshot until the restored library has been inspected.

## Upgrade and rollback

`make upgrade` replaces the executable/notices; opening a library performs
forward-only, checksum-verified schema migrations. An older executable refuses
a newer schema. Never lower user_version, edit migration history or copy just
the main live SQLite file without its WAL.

To roll back, keep the previous binary and its pre-upgrade snapshot. Restore
that snapshot into a new location with a compatible binary, verify articles
and notes, then launch it with explicit data/config paths. A newer snapshot
cannot be made compatible simply by using an older binary; retain both
libraries until recovery is verified.

Choose the restoring executable deliberately: `restore` upgrades its staged
copy to that executable's schema. If the previous executable predates this
command, copy the already closed snapshot file into a new location and retain
its matching pre-upgrade configuration. Do not open that rollback copy with
the newer executable first.
