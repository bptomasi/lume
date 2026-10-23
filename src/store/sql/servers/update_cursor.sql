UPDATE server_feeds
SET initialized = 1,
    last_changed = max(last_changed, ?3),
    sync_offset = ?4,
    sync_latest = ?5
WHERE server_id = ?1 AND remote_id = ?2;
