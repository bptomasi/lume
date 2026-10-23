UPDATE server_feeds
SET initialized = ?6,
    last_changed = max(last_changed, ?3),
    sync_after_id = ?4,
    sync_latest = ?5,
    history_before_id = ?7,
    history_loaded = ?8,
    history_total = ?9
WHERE server_id = ?1 AND remote_id = ?2
    AND (initialized IS NOT ?6 OR last_changed < ?3
        OR sync_after_id IS NOT ?4 OR sync_latest IS NOT ?5
        OR history_before_id IS NOT ?7 OR history_loaded IS NOT ?8
        OR history_total IS NOT ?9);
