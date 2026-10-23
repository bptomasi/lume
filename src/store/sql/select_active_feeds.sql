SELECT
    f.id, f.title, f.url, f.etag, f.modified, f.error,
    s.id, s.url, s.token_env, s.enabled,
    sf.remote_id, sf.initialized, sf.last_changed, sf.present, sf.sync_offset, sf.sync_latest
FROM feeds AS f
LEFT JOIN servers AS s ON s.id = f.server_id
LEFT JOIN server_feeds AS sf ON sf.feed_id = f.id
WHERE f.enabled = 1
ORDER BY f.title COLLATE NOCASE, f.id;
