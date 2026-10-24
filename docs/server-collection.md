# Server collection and batch API review

Discovery and entry collection use the same pool of at most four download
threads. Each hostname is assigned as one indivisible group; a free thread takes
the next group, so a slow group cannot strand unrelated groups behind a fixed
lane. Publisher requests retain their hostname spacing. Private API requests
remain serial within the hostname without publisher spacing. Retry-After
carries from discovery through entries and later jobs on that hostname.

The coordinator alone owns SQLite writes. A discovery result commits against
its captured server configuration before its lane receives fresh feed cursors.
Connection changes, removed sources and disabled servers cannot publish stale
subscriptions or entries. The four-slot result channel bounds queued documents;
dropping it also releases discovery reply waiters on failure or cancellation.
A run with no eligible jobs creates no download threads.

Initial job construction loads publisher feeds directly, rather than hydrating
all mirrored subscriptions to discard them. After discovery, only that server's
feeds are loaded. Progress totals include discovery work until the discovered
feed count becomes known; the final count remains collected feeds.

## Global entry endpoint investigation

The [official Miniflux API](https://miniflux.app/docs/api.html#get-entries)
provides account-wide `GET /v1/entries`, including changed-time filters and ID
cursors. It can reduce HTTP request count across many quiet feeds. The current
per-feed endpoint also supports these filters, and remains the implemented
collection path.

A global response is not interchangeable with one history page per feed. For
example, 200 newest entries can all belong to a busy feed, omitting a quiet feed
entirely. Advancing every feed's history or changed-time cursor after that page
would skip entries. Locally disabled subscriptions and independently resumed
history cursors also need distinct treatment. Existing timestamp-tie,
remote-deletion, history-limit and configuration-generation contracts must hold.

This revision therefore implements discovery isolation and retains bounded,
resumable per-feed collection. It does not silently substitute the global
endpoint. A future optional batch mode needs a durable account-wide delta
watermark, atomic fan-out guarded against every affected feed cursor, and a
separate path for initial/backfill history. Qualification must measure actual
request reduction and first-source completion with sparse, busy, disabled and
newly discovered feeds. Fewer nominal requests alone do not establish a speed
improvement or correct coverage.
