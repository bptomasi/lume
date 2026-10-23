UPDATE server_feeds
SET initialized = 1,
    last_changed = max(last_changed, ?3),
    sync_after_id = ?4,
    sync_latest = ?5
WHERE server_id = ?1 AND remote_id = ?2
    AND (initialized <> 1 OR last_changed < ?3
        OR sync_after_id IS NOT ?4 OR sync_latest IS NOT ?5);
