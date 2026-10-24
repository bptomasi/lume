SELECT
    f.id, f.title, f.url, f.etag, f.modified, f.error,
    s.id, s.url, s.token_env, s.enabled,
    sf.remote_id, sf.initialized, sf.last_changed, sf.present, sf.sync_after_id, sf.sync_latest, s.token, s.sync_settings,
    sf.history_before_id, sf.history_loaded, sf.history_total
FROM feeds AS f
LEFT JOIN servers AS s ON s.id = f.server_id
LEFT JOIN server_feeds AS sf ON sf.feed_id = f.id
WHERE f.enabled = 1 AND f.server_id = ?1
ORDER BY f.title COLLATE NOCASE, f.id;
