UPDATE server_feeds SET initialized = 0, history_before_id = 0,
    history_loaded = 0, history_total = 0, sync_after_id = 0
WHERE server_id = ?1 AND present = 1
    AND (initialized <> 0 OR history_before_id <> 0 OR history_loaded <> 0
        OR history_total <> 0 OR sync_after_id <> 0);
