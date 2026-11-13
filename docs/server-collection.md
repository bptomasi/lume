# Server collection

## Scheduling and publication

Discovery and entry collection share a pool of at most four download threads.
Each hostname is one indivisible group; available threads take the next group
rather than waiting in fixed lanes. Publisher requests retain hostname spacing.
Private API requests remain serial within a hostname without publisher spacing.
Retry-After carries from discovery through entries and later jobs on that host.

The coordinator owns collection writes to SQLite. Discovery commits against
its captured server configuration before that lane receives fresh feed cursors.
Removed, disabled or reconfigured sources cannot publish stale subscriptions
or entries. A bounded result channel limits queued documents and releases
waiting discovery replies on failure or cancellation. Runs with no eligible
jobs create no download threads.

Initial construction loads publisher feeds directly. After discovery it loads
only the affected server's feeds. Progress includes discovery until the feed
count becomes known; the final count describes collected feeds.

## Per-feed cursors

Collection uses bounded, resumable per-feed history and changed-entry cursors.
Timestamp ties, remote deletions, configured history limits and local disabled
subscriptions must preserve coverage. Cursor advancement and publication are
guarded by the captured configuration and feed identity.

The account-wide Miniflux entries endpoint is not interchangeable with a page
from each feed: a busy feed can occupy the whole response while quiet feeds
are absent. Advancing every feed after that response would skip history. Do
not substitute global batching without defining atomic cursor ownership and
qualifying sparse, busy, disabled and newly discovered feeds. See the
[Miniflux entries API](https://miniflux.app/docs/api.html#get-entries).

For credentials, commands and history settings, see
[server configuration](configuration.md#optional-miniflux-servers).
